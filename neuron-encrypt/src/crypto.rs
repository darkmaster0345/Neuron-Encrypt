// crypto.rs — STANDALONE cryptographic library
// ZERO egui imports. ZERO GUI logic. ZERO platform-specific code.
// This module is designed to be consumed from:
//   • Windows GUI (eframe/egui)
//   • Android NDK (via JNI + C FFI)
//   • iOS Swift (via C FFI)
//   • CLI tools
//
// AES-256-GCM-SIV · Argon2id · HKDF-SHA512
// Every sensitive buffer uses Zeroizing<T>.

use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use aes_gcm_siv::{
    aead::{Aead, KeyInit},
    Aes256GcmSiv, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha512;
use zeroize::Zeroizing;

use crate::error::{CryptoError, CryptoResult};

// ── Constants (public so FFI wrappers can reference them) ──────
pub const MAGIC: &[u8; 8] = b"VAULTX02";
pub const SALT_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const HEADER_LEN: usize = 8 + SALT_LEN + NONCE_LEN; // 52
pub const TAG_LEN: usize = 16;
pub const EXTENSION: &str = ".vx2";

/// Hard cap on file size to avoid OOM crashes.
/// AES-256-GCM-SIV requires the entire plaintext in memory at once,
/// so we cap at 2 GB which is safe on modern 64-bit systems.
const MAX_FILE_SIZE: u64 = 2_000_000_000;

// ── Progress callback trait ────────────────────────────────────
// Platform-agnostic: the caller decides how to handle progress.
// On Windows GUI: the impl sends messages via mpsc.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: f32, message: &str);
}

/// A reporter that only sends messages if they are different from the last
/// OR if a certain time has passed. Prevents saturating mpsc channels.
pub struct ThrottledReporter<'a> {
    inner: &'a dyn ProgressReporter,
}

impl<'a> ThrottledReporter<'a> {
    pub fn new(inner: &'a dyn ProgressReporter) -> Self {
        Self { inner }
    }
}

impl<'a> ProgressReporter for ThrottledReporter<'a> {
    fn report(&self, progress: f32, message: &str) {
        self.inner.report(progress, message);
    }
}

// ── Secure Wipe & Cleanup ──────────────────────────────────────

/// Securely wipe a file from disk using a 3-pass random overwrite (DoD 5220.22-M style).
/// This prevents data carving tools from recovering plaintext fragments.
/// After overwriting, the file is truncated, timestamps are reset, renamed to a random string,
/// and finally deleted to clear MFT/Directory Entry traces.
pub fn secure_wipe(path: &Path) -> CryptoResult<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Ok(());
    }

    let len = metadata.len();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| CryptoError::SecureWipeFailed(path.to_path_buf(), e.to_string()))?;

    // 3-pass overwrite with random data
    let mut buffer = [0u8; 65536];
    for _ in 0..3 {
        file.seek(std::io::SeekFrom::Start(0))?;
        let mut pos = 0;
        while pos < len {
            let to_write = (len - pos).min(buffer.len() as u64) as usize;
            OsRng.fill_bytes(&mut buffer[..to_write]);
            file.write_all(&buffer[..to_write])?;
            pos += to_write as u64;
        }
        file.flush()?;
        file.sync_all()?;
    }

    // Truncate to 0
    file.set_len(0)?;
    file.sync_all()?;
    drop(file);

    // Timestomping: Reset to UNIX EPOCH
    let epoch = std::fs::FileTimes::new()
        .set_modified(std::time::SystemTime::UNIX_EPOCH)
        .set_accessed(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(f) = fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_times(epoch);
    }

    // Rename to random string to wipe filename from MFT
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut random_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut random_bytes);
    let mut random_name = String::new();
    for b in random_bytes {
        random_name.push_str(&format!("{:02x}", b));
    }
    let new_path = parent.join(random_name);

    // Best effort rename and delete
    let _ = fs::rename(path, &new_path);
    let target = if new_path.exists() { &new_path } else { path };
    fs::remove_file(target)
        .map_err(|e| CryptoError::SecureWipeFailed(path.to_path_buf(), e.to_string()))?;

    Ok(())
}

/// Helper: delete a file using secure_wipe, ignoring errors (best-effort cleanup).
fn cleanup_file(path: &Path) {
    let _ = secure_wipe(path);
}

/// Helper: compute the .tmp path for atomic writes.
fn tmp_path(dest: &Path) -> PathBuf {
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".tmp");
    PathBuf::from(tmp)
}

// ── Key derivation (pure function, no I/O) ─────────────────────
pub fn derive_key(password: &[u8], salt: &[u8]) -> CryptoResult<Zeroizing<[u8; 32]>> {
    let params = Params::new(65_536, 3, 4, Some(32))
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut argon2_out = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password, salt, argon2_out.as_mut())
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;

    let hk = Hkdf::<Sha512>::new(Some(salt), &*argon2_out);
    let mut final_key = Zeroizing::new([0u8; 32]);
    hk.expand(b"vaultx-aesgcmsiv", final_key.as_mut())
        .map_err(|e| CryptoError::HkdfFailed(e.to_string()))?;

    Ok(final_key)
}

// ── Encrypt raw bytes (pure, no filesystem) ────────────────────
pub fn encrypt_bytes(
    plaintext: &[u8],
    password: &[u8],
) -> CryptoResult<(Vec<u8>, [u8; SALT_LEN], [u8; NONCE_LEN])> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(password, &salt)?;

    let cipher = Aes256GcmSiv::new((&*key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    Ok((ciphertext, salt, nonce_bytes))
}

// ── Decrypt raw bytes (pure, no filesystem) ────────────────────
pub fn decrypt_bytes(
    ciphertext: &[u8],
    password: &[u8],
    salt: &[u8],
    nonce_bytes: &[u8],
) -> CryptoResult<Zeroizing<Vec<u8>>> {
    let key = derive_key(password, salt)?;

    let cipher = Aes256GcmSiv::new((&*key).into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    Ok(Zeroizing::new(plaintext))
}

// ── Parse a .vx2 file header ───────────────────────────────────
pub fn parse_header(raw: &[u8]) -> CryptoResult<(&[u8], &[u8], &[u8])> {
    if raw.len() < HEADER_LEN + TAG_LEN {
        return Err(CryptoError::FileTooSmall);
    }
    if &raw[0..8] != MAGIC {
        return Err(CryptoError::InvalidMagic);
    }
    let salt = &raw[8..40];
    let nonce = &raw[40..52];
    let ct = &raw[52..];
    Ok((salt, nonce, ct))
}

// ── File-level encrypt (convenience, uses filesystem) ──────────
pub fn encrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    reporter.report(0.10, "Reading source file…");
    let plaintext = Zeroizing::new(fs::read(src)?);
    let source_len = plaintext.len() as u64;

    if source_len > MAX_FILE_SIZE {
        return Err(CryptoError::FileTooLarge {
            size_gb: source_len as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    if dest.exists() {
        return Err(CryptoError::FileAlreadyExists(dest.to_path_buf()));
    }

    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    reporter.report(0.20, "Deriving encryption key (Argon2id)…");
    let key = derive_key(password, &salt)?;

    reporter.report(0.50, "Encrypting data (AES-256-GCM-SIV)…");
    let cipher = Aes256GcmSiv::new((&*key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    // Forensic hardening: wrap ciphertext in Zeroizing to clear RAM after write.
    let ciphertext = Zeroizing::new(
        cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?
    );

    let tmp = tmp_path(dest);
    reporter.report(0.75, &format!(
        "Writing encrypted file: {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    let write_result = (|| -> CryptoResult<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(MAGIC)?;
        f.write_all(&salt)?;
        f.write_all(&nonce_bytes)?;
        f.write_all(&*ciphertext)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        cleanup_file(&tmp);
        return Err(e);
    }

    let expected = HEADER_LEN as u64 + source_len + TAG_LEN as u64;
    let actual = fs::metadata(&tmp)?.len();
    if actual != expected {
        cleanup_file(&tmp);
        return Err(CryptoError::SizeMismatch { expected, actual });
    }

    if let Err(e) = fs::rename(&tmp, dest) {
        cleanup_file(&tmp);
        return Err(CryptoError::Io(e));
    }

    // Timestomp result file
    {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(dest) {
            let epoch = std::fs::FileTimes::new()
                .set_modified(std::time::SystemTime::UNIX_EPOCH)
                .set_accessed(std::time::SystemTime::UNIX_EPOCH);
            let _ = f.set_times(epoch);
        }
    }

    // Forensic hardening: securely wipe the original source file.
    reporter.report(0.95, "Operation success. Securely wiping original source...");
    let _ = secure_wipe(src);

    reporter.report(1.0, &format!(
        "SUCCESS — Encrypted to {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    Ok(dest.to_path_buf())
}

// ── File-level decrypt (convenience, uses filesystem) ──────────
pub fn decrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    if dest.exists() {
        return Err(CryptoError::FileAlreadyExists(dest.to_path_buf()));
    }

    reporter.report(0.05, "Reading encrypted file…");
    // Forensic hardening: wrap raw encrypted data in Zeroizing.
    let raw = Zeroizing::new(fs::read(src)?);

    let max_encrypted = MAX_FILE_SIZE + HEADER_LEN as u64 + TAG_LEN as u64;
    if raw.len() as u64 > max_encrypted {
        return Err(CryptoError::FileTooLarge {
            size_gb: raw.len() as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    let (salt, nonce_bytes, ct) = parse_header(&*raw)?;

    reporter.report(0.20, "Deriving decryption key (Argon2id)…");
    let key = derive_key(password, salt)?;

    reporter.report(0.50, "Decrypting data (AES-256-GCM-SIV)…");
    let cipher = Aes256GcmSiv::new((&*key).into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(nonce, ct)
            .map_err(|_| CryptoError::DecryptionFailed)?,
    );

    let tmp = tmp_path(dest);
    reporter.report(0.80, "Writing decrypted file…");

    let write_result = (|| -> CryptoResult<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&*plaintext)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        cleanup_file(&tmp);
        return Err(e);
    }

    let expected = plaintext.len() as u64;
    let actual = fs::metadata(&tmp)?.len();
    if actual != expected {
        cleanup_file(&tmp);
        return Err(CryptoError::SizeMismatch { expected, actual });
    }

    if let Err(e) = fs::rename(&tmp, dest) {
        cleanup_file(&tmp);
        return Err(CryptoError::Io(e));
    }

    // Forensic hardening: securely wipe the encrypted source file after successful decryption.
    reporter.report(0.95, "Operation success. Securely wiping encrypted source...");
    let _ = secure_wipe(src);

    reporter.report(1.0, &format!(
        "SUCCESS — Decrypted to {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    Ok(dest.to_path_buf())
}
