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
use std::io::Read;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::os::windows::fs::FileTimesExt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Instant};

use aes_gcm_siv::{
    aead::{Aead, KeyInit, Payload},
    Aes256GcmSiv, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha512;
use zeroize::Zeroizing;

use crate::error::{CryptoError, CryptoResult};

// ── Constants ──────────────────────────────────────────────────
pub const SALT_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const HEADER_LEN: usize = 8 + SALT_LEN + NONCE_LEN; // MAGIC + SALT + NONCE
pub const MAGIC: &[u8; 8] = b"VAULTX02";
pub const TAG_LEN: usize = 16;
pub const EXTENSION: &str = ".vx2";

/// Hard cap on file size to avoid OOM crashes.
/// AES-256-GCM-SIV requires the entire plaintext in memory at once,
/// so we cap at 2 GB which is safe on modern 64-bit systems.
pub const MAX_FILE_SIZE: u64 = 2_000_000_000;

/// Minimum passphrase length for security.
pub const MIN_PASSWORD_LEN: usize = 8;

// ── Progress callback trait ────────────────────────────────────
// Platform-agnostic: the caller decides how to handle progress.
// On Windows GUI: the impl sends messages via mpsc.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: f32, message: &str);
}

/// A reporter that only sends messages if they are different from the last
/// OR if a certain time has passed (e.g. 100ms). Prevents saturating mpsc channels.
/// FIX BUG-001: Redesigned to be Sync using atomics and Mutex.
pub struct ThrottledReporter<'a> {
    inner: &'a dyn ProgressReporter,
    last_progress: AtomicU32,
    last_time: Mutex<Option<Instant>>,
    last_message: Mutex<String>,
}

impl<'a> ThrottledReporter<'a> {
    pub fn new(inner: &'a dyn ProgressReporter) -> Self {
        Self {
            inner,
            last_progress: AtomicU32::new(f32::to_bits(-1.0)),
            last_time: Mutex::new(None),
            last_message: Mutex::new(String::new()),
        }
    }
}

impl<'a> ProgressReporter for ThrottledReporter<'a> {
    fn report(&self, progress: f32, message: &str) {
        let now = Instant::now();
        let last_prog_bits = self.last_progress.load(Ordering::Relaxed);
        let last_prog = f32::from_bits(last_prog_bits);

        let (last_msg, last_time_opt) = {
            let msg_guard = self.last_message.lock().unwrap();
            let time_guard = self.last_time.lock().unwrap();
            (msg_guard.clone(), *time_guard)
        };

        let should_report = match last_time_opt {
            Some(t) => {
                let time_delta = now.duration_since(t).as_millis();
                (progress - last_prog).abs() > 0.01 || time_delta > 100 || last_msg != message
            }
            None => true,
        };

        if should_report {
            self.inner.report(progress, message);
            *self.last_message.lock().unwrap() = message.to_owned();
            *self.last_time.lock().unwrap() = Some(now);
            self.last_progress.store(progress.to_bits(), Ordering::Relaxed);
        }
    }
}

// ── Internal Helpers ───────────────────────────────────────────

fn derive_key(password: &[u8], salt: &[u8]) -> CryptoResult<Zeroizing<Vec<u8>>> {
    let mut final_key = Zeroizing::new(vec![0u8; 32]);

    // Stage 1: Argon2id (Slow, Memory-hard)
    let mut intermediate = Zeroizing::new(vec![0u8; 64]);
    let params = Params::new(65536, 3, 4, Some(64)).map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2.hash_password_into(password, salt, &mut intermediate)
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;

    // Stage 2: HKDF-SHA512 (Key expansion)
    let hk = Hkdf::<Sha512>::new(None, &intermediate);
    hk.expand(b"VAULTX02_AES_256_GCM_SIV", &mut final_key)
        .map_err(|e| CryptoError::HkdfFailed(e.to_string()))?;

    Ok(final_key)
}

fn build_aad(salt: &[u8], nonce: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(SALT_LEN + NONCE_LEN);
    aad.extend_from_slice(salt);
    aad.extend_from_slice(nonce);
    aad
}

fn tmp_path(dest: &Path) -> PathBuf {
    let mut tmp = dest.to_path_buf();
    let name = dest.file_name().unwrap_or_default().to_string_lossy();
    tmp.set_file_name(format!("{}.tmp", name));
    tmp
}

// ── Encrypt raw bytes (pure, no filesystem) ────────────────────
/// FIX BUG-019: Return Zeroizing<Vec<u8>> for ciphertext.
#[allow(clippy::type_complexity)]
pub fn encrypt_bytes(
    plaintext: &[u8],
    password: &[u8],
) -> CryptoResult<(Zeroizing<Vec<u8>>, [u8; SALT_LEN], [u8; NONCE_LEN])> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(CryptoError::PassphraseTooShort(MIN_PASSWORD_LEN));
    }

    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(password, &salt)?;

    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    let aad = build_aad(&salt, &nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, Payload { msg: plaintext, aad: &aad })
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    Ok((Zeroizing::new(ciphertext), salt, nonce_bytes))
}

// ── Decrypt raw bytes (pure, no filesystem) ────────────────────
pub fn decrypt_bytes(
    ciphertext: &[u8],
    password: &[u8],
    salt: &[u8],
    nonce_bytes: &[u8],
) -> CryptoResult<Zeroizing<Vec<u8>>> {
    let key = derive_key(password, salt)?;

    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let aad = build_aad(salt, nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, Payload { msg: ciphertext, aad: &aad })
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
    if password.len() < MIN_PASSWORD_LEN {
        return Err(CryptoError::PassphraseTooShort(MIN_PASSWORD_LEN));
    }

    // FIX BUG-040: Validate source is a file.
    let file = fs::File::open(src)?;
    let source_metadata = file.metadata()?;
    if !source_metadata.is_file() {
        return Err(CryptoError::NotAFile(src.to_path_buf()));
    }

    let source_len = source_metadata.len();
    if source_len > MAX_FILE_SIZE {
        return Err(CryptoError::FileTooLarge {
            size_gb: source_len as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    reporter.report(0.10, "Reading source file…");
    let mut buffer = Vec::with_capacity(source_len as usize);
    if file.take(MAX_FILE_SIZE + 1).read_to_end(&mut buffer)? > MAX_FILE_SIZE as usize {
        return Err(CryptoError::FileTooLarge {
            size_gb: (MAX_FILE_SIZE + 1) as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }
    let plaintext = Zeroizing::new(buffer);

    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    reporter.report(0.20, "Deriving encryption key (Argon2id)…");
    let key = derive_key(password, &salt)?;

    reporter.report(0.50, "Encrypting data (AES-256-GCM-SIV)…");
    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    let aad = build_aad(&salt, &nonce_bytes);
    let ciphertext = Zeroizing::new(
        cipher
            .encrypt(nonce, Payload { msg: plaintext.as_ref(), aad: &aad })
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?
    );

    let tmp = tmp_path(dest);
    reporter.report(0.75, &format!(
        "Writing encrypted file: {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    let write_result = (|| -> CryptoResult<()> {
        // FIX BUG-004: O_EXCL to prevent truncation and race.
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;

        f.write_all(MAGIC)?;
        f.write_all(&salt)?;
        f.write_all(&nonce_bytes)?;
        f.write_all(&ciphertext)?;
        f.flush()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    fs::rename(&tmp, dest)?;
    reporter.report(1.00, "Encryption complete.");
    Ok(dest.to_path_buf())
}

// ── File-level decrypt (convenience, uses filesystem) ──────────
pub fn decrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    let source_metadata = fs::metadata(src)?;
    let source_len = source_metadata.len();
    if source_len > MAX_FILE_SIZE + HEADER_LEN as u64 {
        return Err(CryptoError::FileTooLarge {
            size_gb: source_len as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    reporter.report(0.10, "Reading encrypted file…");
    let raw = fs::read(src)?;
    let (salt, nonce, ct) = parse_header(&raw)?;

    reporter.report(0.20, "Deriving decryption key (Argon2id)…");
    let key = derive_key(password, salt)?;

    reporter.report(0.50, "Decrypting data (AES-256-GCM-SIV)…");
    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let aad = build_aad(salt, nonce);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad: &aad })
            .map_err(|_| CryptoError::DecryptionFailed)?
    );

    let tmp = tmp_path(dest);
    reporter.report(0.75, &format!(
        "Writing decrypted file: {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    let write_result = (|| -> CryptoResult<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(&plaintext)?;
        f.flush()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    fs::rename(&tmp, dest)?;

    // FIX BUG-025: Timestomp the decrypted file to match the original.
    #[cfg(target_os = "windows")]
    {
        if let Ok(f) = fs::File::open(dest) {
            let times = fs::FileTimes::new()
                .set_accessed(source_metadata.accessed().unwrap_or(SystemTime::now()))
                .set_modified(source_metadata.modified().unwrap_or(SystemTime::now()));
            let _ = f.set_times(times);
        }
    }

    reporter.report(1.00, "Decryption complete.");
    Ok(dest.to_path_buf())
}

// ── Secure Wipe (Forensic Hardening) ───────────────────────────
pub fn secure_wipe(path: &Path) -> CryptoResult<()> {
    if !path.exists() { return Ok(()); }
    let metadata = fs::metadata(path)?;
    let len = metadata.len();

    let mut file = fs::OpenOptions::new().write(true).open(path)?;

    // 3-pass overwrite
    for _ in 0..3 {
        let mut buffer = vec![0u8; 64 * 1024];
        file.seek(std::io::SeekFrom::Start(0))?;
        let mut written = 0;
        while written < len {
            let to_write = std::cmp::min(buffer.len() as u64, len - written) as usize;
            OsRng.fill_bytes(&mut buffer[..to_write]);
            file.write_all(&buffer[..to_write])?;
            written += to_write as u64;
        }
        file.sync_all()?;
    }

    // Timestomp to epoch
    #[cfg(target_os = "windows")]
    {
        let epoch = SystemTime::UNIX_EPOCH;
        let times = fs::FileTimes::new().set_accessed(epoch).set_modified(epoch);
        let _ = file.set_times(times);
    }

    drop(file);

    // Rename to random string before delete
    let mut random_name = [0u8; 16];
    OsRng.fill_bytes(&mut random_name);
    let new_path = path.with_file_name(hex::encode(random_name));
    fs::rename(path, &new_path)?;
    fs::remove_file(new_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestReporter;
    impl ProgressReporter for TestReporter {
        fn report(&self, _progress: f32, _message: &str) {}
    }

    #[test]
    fn test_encrypt_decrypt_bytes() {
        let plaintext = b"Hello, security audit!";
        let password = b"supersecretpassword";

        let (ciphertext, salt, nonce) = encrypt_bytes(plaintext, password).unwrap();
        let decrypted = decrypt_bytes(&ciphertext, password, &salt, &nonce).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_file() {
        let tmp_dir = std::env::temp_dir().join(format!("neuron_test_{}", rand_core::RngCore::next_u64(&mut rand_core::OsRng)));
        fs::create_dir_all(&tmp_dir).unwrap();
        let src_path = tmp_dir.join("src.txt");
        let dest_path = tmp_dir.join("dest.vx2");
        let final_path = tmp_dir.join("final.txt");
        let password = b"password123";
        let reporter = TestReporter;

        fs::write(&src_path, b"File content for testing").unwrap();

        encrypt_file(&src_path, &dest_path, password, &reporter).unwrap();
        assert!(dest_path.exists());

        decrypt_file(&dest_path, &final_path, password, &reporter).unwrap();
        assert!(final_path.exists());

        let final_content = fs::read(final_path).unwrap();
        assert_eq!(final_content, b"File content for testing");

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_password_too_short() {
        let plaintext = b"some data";
        let password = b"short";
        let res = encrypt_bytes(plaintext, password);
        assert!(matches!(res, Err(CryptoError::PassphraseTooShort(_))));
    }

    #[test]
    fn test_invalid_magic() {
        let mut raw = vec![0u8; 100];
        raw[0..8].copy_from_slice(b"NOTMAGIC");
        let res = parse_header(&raw);
        assert!(matches!(res, Err(CryptoError::InvalidMagic)));
    }
}
