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
use std::io::Write;
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

// BUG-16 note: AES-GCM-SIV has no padding, so ciphertext length == plaintext
// length. An attacker can compute original_size = encrypted_size - 68 bytes.
// To mitigate, consider adding optional random padding (0-4096 bytes) appended
// before encryption, with the padding length stored in the last 2 bytes of
// plaintext. This would require a file format version bump (VAULTX03) and is
// deferred to avoid breaking backward compatibility with existing .vx2 files.

// ── Progress callback trait ────────────────────────────────────
// Platform-agnostic: the caller decides how to handle progress.
// On Windows GUI: the impl sends messages via mpsc.
// On Android/iOS: the impl calls back into Java/Swift via FFI.
// On CLI: the impl prints to stdout.
pub trait ProgressReporter: Send {
    fn report(&self, progress: f32, message: &str);
}

/// A no-op reporter for callers that don't need progress.
pub struct NoopReporter;
impl ProgressReporter for NoopReporter {
    fn report(&self, _progress: f32, _message: &str) {}
}

/// A throttled wrapper that only forwards when progress changes by ≥1%.
/// Ensures at most ~100 updates per operation.
///
/// BUG-13 fix: Uses AtomicU32 instead of Cell<f32> so the type is Sync-safe
/// for potential multi-threaded FFI usage (Android NDK, iOS Swift).
pub struct ThrottledReporter<R: ProgressReporter> {
    inner: R,
    last_reported: std::sync::atomic::AtomicU32,
}

impl<R: ProgressReporter> ThrottledReporter<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            // Store f32 bits; -1.0f32 as bits signals "no report yet"
            last_reported: std::sync::atomic::AtomicU32::new((-1.0f32).to_bits()),
        }
    }
}

impl<R: ProgressReporter> ProgressReporter for ThrottledReporter<R> {
    fn report(&self, progress: f32, message: &str) {
        let last = f32::from_bits(
            self.last_reported.load(std::sync::atomic::Ordering::Relaxed),
        );
        // Always forward the very first and very last updates,
        // plus any update that moves ≥0.01 (1%) from the last.
        if last < 0.0 || progress >= 1.0 || (progress - last) >= 0.01 {
            self.last_reported
                .store(progress.to_bits(), std::sync::atomic::Ordering::Relaxed);
            self.inner.report(progress, message);
        }
    }
}

// ── Key derivation (pure function, no I/O) ─────────────────────
/// Derive a 32-byte key from password + salt using Argon2id → HKDF-SHA512.
/// All intermediates are wrapped in Zeroizing for guaranteed zeroing.
///
/// This is a standalone function — no filesystem, no GUI, no platform code.
pub fn derive_key(password: &[u8], salt: &[u8]) -> CryptoResult<Zeroizing<[u8; 32]>> {
    // ─ Argon2id ─
    let params = Params::new(65_536, 3, 4, Some(32))
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut argon2_out = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password, salt, argon2_out.as_mut())
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;

    // ─ HKDF-SHA512 ─
    let hk = Hkdf::<Sha512>::new(Some(salt), &*argon2_out);
    let mut final_key = Zeroizing::new([0u8; 32]);
    hk.expand(b"vaultx-aesgcmsiv", final_key.as_mut())
        .map_err(|e| CryptoError::HkdfFailed(e.to_string()))?;

    Ok(final_key)
}

// ── Encrypt raw bytes (pure, no filesystem) ────────────────────
/// Encrypt a plaintext buffer. Returns (salt, nonce, ciphertext_with_tag).
/// This is the core encryption primitive — no file I/O.
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
/// Decrypt a VAULTX02 payload (everything after parsing the header).
/// Returns the plaintext wrapped in Zeroizing.
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
/// Parse and validate a VAULTX02 file header from raw bytes.
/// Returns (salt, nonce, ciphertext_slice).
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

/// Helper: delete a file, ignoring errors (best-effort cleanup).
fn cleanup_file(path: &Path) {
    let _ = fs::remove_file(path);
}

/// Helper: compute the .tmp path for atomic writes.
fn tmp_path(dest: &Path) -> PathBuf {
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".tmp");
    PathBuf::from(tmp)
}

// ── File-level encrypt (convenience, uses filesystem) ──────────
/// Encrypt a file on disk. Uses ProgressReporter for status updates.
/// The reporter is platform-agnostic — it could be mpsc, JNI callback, etc.
///
/// Uses atomic write: data goes to `dest.tmp` first, then renamed on success.
/// On any failure the temporary file is deleted — never leaves partial output.
pub fn encrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    // BUG-04 fix: Read the file first, then validate in-memory size.
    // This eliminates the TOCTOU race between metadata check and fs::read.
    reporter.report(0.10, "Reading source file…");
    let plaintext = Zeroizing::new(fs::read(src)?);
    let source_len = plaintext.len() as u64;

    // ── Validate file size (OOM protection) ──
    // BUG-07 fix: Use dedicated FileTooLarge error variant.
    if source_len > MAX_FILE_SIZE {
        return Err(CryptoError::FileTooLarge {
            size_gb: source_len as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    // BUG-12 fix: Check if destination exists before proceeding.
    if dest.exists() {
        return Err(CryptoError::FileAlreadyExists(dest.to_path_buf()));
    }

    // Generate random salt and nonce via OsRng
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    // Derive key
    // BUG-15 note: Key material is Zeroizing but not mlock'd. To prevent
    // the OS from swapping key pages to disk, integrate memsec or seckey
    // crate for mlock after allocation in a future hardening pass.
    reporter.report(0.20, "Deriving encryption key (Argon2id)…");
    let key = derive_key(password, &salt)?;

    // Encrypt
    reporter.report(0.50, "Encrypting data (AES-256-GCM-SIV)…");
    let cipher = Aes256GcmSiv::new((&*key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    // ── Atomic write: write to .tmp, rename on success ──
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
        f.write_all(&ciphertext)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        cleanup_file(&tmp);
        return Err(e);
    }

    // Validate output size
    let expected = HEADER_LEN as u64 + source_len + TAG_LEN as u64;
    let actual = fs::metadata(&tmp)?.len();
    if actual != expected {
        cleanup_file(&tmp);
        return Err(CryptoError::SizeMismatch { expected, actual });
    }

    // Atomic rename: .tmp → final destination
    if let Err(e) = fs::rename(&tmp, dest) {
        cleanup_file(&tmp);
        return Err(CryptoError::Io(e));
    }

    // BUG-17 fix: Strip timestamps on all platforms (not just Windows)
    // to reduce metadata leakage about when encryption occurred.
    {
        if let Ok(f) = std::fs::OpenOptions::new()
            .write(true)
            .open(dest)
        {
            let epoch = std::fs::FileTimes::new()
                .set_modified(std::time::SystemTime::UNIX_EPOCH)
                .set_accessed(std::time::SystemTime::UNIX_EPOCH);
            let _ = f.set_times(epoch);
        }
    }

    reporter.report(1.0, &format!(
        "SUCCESS — Encrypted to {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    Ok(dest.to_path_buf())
}

// ── File-level decrypt (convenience, uses filesystem) ──────────
/// Decrypt a .vx2 file on disk. Uses ProgressReporter for status updates.
///
/// Uses atomic write: data goes to `dest.tmp` first, then renamed on success.
/// On any failure the temporary file is deleted — never leaves partial output.
pub fn decrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    // BUG-12 fix: Check if destination exists before proceeding.
    if dest.exists() {
        return Err(CryptoError::FileAlreadyExists(dest.to_path_buf()));
    }

    reporter.report(0.05, "Reading encrypted file…");
    let raw = fs::read(src)?;

    // BUG-07 fix: Use dedicated FileTooLarge error variant for decrypt path.
    let max_encrypted = MAX_FILE_SIZE + HEADER_LEN as u64 + TAG_LEN as u64;
    if raw.len() as u64 > max_encrypted {
        return Err(CryptoError::FileTooLarge {
            size_gb: raw.len() as f64 / 1_000_000_000.0,
            max_gb: MAX_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    // Parse header at exact offsets
    let (salt, nonce_bytes, ct) = parse_header(&raw)?;

    // Derive key
    reporter.report(0.20, "Deriving decryption key (Argon2id)…");
    let key = derive_key(password, salt)?;

    // Decrypt
    reporter.report(0.50, "Decrypting data (AES-256-GCM-SIV)…");
    let cipher = Aes256GcmSiv::new((&*key).into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(nonce, ct)
            .map_err(|_| CryptoError::DecryptionFailed)?,
    );

    // ── Atomic write: write to .tmp, rename on success ──
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

    // Validate written size
    let expected = plaintext.len() as u64;
    let actual = fs::metadata(&tmp)?.len();
    if actual != expected {
        cleanup_file(&tmp);
        return Err(CryptoError::SizeMismatch { expected, actual });
    }

    // Atomic rename: .tmp → final destination
    if let Err(e) = fs::rename(&tmp, dest) {
        cleanup_file(&tmp);
        return Err(CryptoError::Io(e));
    }

    reporter.report(1.0, &format!(
        "SUCCESS — Decrypted to {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    Ok(dest.to_path_buf())
}
