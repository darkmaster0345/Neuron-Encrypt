// crypto.rs - standalone cryptographic library
// Zero egui imports. Zero GUI logic. Zero platform-specific behavior in the API.

use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;
#[cfg(target_os = "windows")]
use std::time::SystemTime;

use aead::stream::{DecryptorBE32, EncryptorBE32};
use aes_gcm_siv::{
    aead::{Aead, KeyInit, Payload},
    Aes256GcmSiv, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha512;
use zeroize::{Zeroize, Zeroizing};

use crate::error::{CryptoError, CryptoResult};

// ── V2 (legacy) constants ──
pub const SALT_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const HEADER_LEN: usize = 8 + SALT_LEN + NONCE_LEN;
pub const MAGIC: &[u8; 8] = b"VAULTX02";
pub const TAG_LEN: usize = 16;
pub const EXTENSION: &str = ".vx2";

// ── V3 (streaming) constants ──
pub const MAGIC_V3: &[u8; 8] = b"VAULTX03";
/// EncryptorBE32 uses 5 bytes of the 12-byte nonce (4-byte counter + 1-byte flag).
pub const STREAM_NONCE_LEN: usize = 7;
pub const HEADER_V3_LEN: usize = 8 + SALT_LEN + STREAM_NONCE_LEN; // 47
/// 1 MB chunks — balances memory usage vs per-chunk overhead.
pub const CHUNK_SIZE: usize = 1_048_576;

// ── V2 legacy limits (kept for backward-compat decryption) ──
pub const MAX_FILE_SIZE: u64 = 8_000_000_000;
pub const MAX_ENCRYPTED_FILE_SIZE: u64 = MAX_FILE_SIZE + HEADER_LEN as u64 + TAG_LEN as u64;

/// Minimum passphrase length for security.
pub const MIN_PASSWORD_LEN: usize = 8;

pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: f32, message: &str);
}

/// A reporter that only forwards materially different updates.
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
        let last_progress = f32::from_bits(self.last_progress.load(Ordering::Relaxed));

        let (last_message, last_time) = {
            let message_guard = self.last_message.lock().expect("Mutex was poisoned");
            let time_guard = self.last_time.lock().expect("Mutex was poisoned");
            (message_guard.clone(), *time_guard)
        };

        let should_report = match last_time {
            Some(last_time) => {
                let elapsed_ms = now.duration_since(last_time).as_millis();
                (progress - last_progress).abs() > 0.01
                    || elapsed_ms > 100
                    || last_message != message
            }
            None => true,
        };

        if should_report {
            self.inner.report(progress, message);
            *self
                .last_message
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = message.to_owned();
            *self
                .last_time
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = Some(now);
            self.last_progress
                .store(progress.to_bits(), Ordering::Relaxed);
        }
    }
}

fn derive_key(password: &[u8], salt: &[u8]) -> CryptoResult<Zeroizing<Vec<u8>>> {
    let mut final_key = Zeroizing::new(vec![0u8; 32]);

    let mut intermediate = Zeroizing::new(vec![0u8; 64]);
    let params = Params::new(65_536, 3, 4, Some(64))
        .map_err(|error| CryptoError::Argon2Failed(error.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2
        .hash_password_into(password, salt, &mut intermediate)
        .map_err(|error| CryptoError::Argon2Failed(error.to_string()))?;

    let hkdf = Hkdf::<Sha512>::new(None, &intermediate);
    hkdf.expand(b"VAULTX02_AES_256_GCM_SIV", &mut final_key)
        .map_err(|error| CryptoError::HkdfFailed(error.to_string()))?;
    intermediate.zeroize();

    Ok(final_key)
}

/// V3 key derivation — same Argon2id + HKDF pipeline, different HKDF info
/// for cryptographic domain separation from V2.
fn derive_key_v3(password: &[u8], salt: &[u8]) -> CryptoResult<Zeroizing<Vec<u8>>> {
    let mut final_key = Zeroizing::new(vec![0u8; 32]);
    let mut intermediate = Zeroizing::new(vec![0u8; 64]);
    let params = Params::new(65_536, 3, 4, Some(64))
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2
        .hash_password_into(password, salt, &mut intermediate)
        .map_err(|e| CryptoError::Argon2Failed(e.to_string()))?;
    let hkdf = Hkdf::<Sha512>::new(None, &intermediate);
    hkdf.expand(b"VAULTX03_AES_256_GCM_SIV_STREAM", &mut final_key)
        .map_err(|e| CryptoError::HkdfFailed(e.to_string()))?;
    intermediate.zeroize();
    Ok(final_key)
}

fn build_aad(salt: &[u8], nonce: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(SALT_LEN + NONCE_LEN);
    aad.extend_from_slice(salt);
    aad.extend_from_slice(nonce);
    aad
}

fn open_regular_file(path: &Path) -> CryptoResult<(fs::File, fs::Metadata)> {
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(CryptoError::NotAFile(path.to_path_buf()));
    }

    Ok((file, metadata))
}

fn normalize_destination_path(path: &Path) -> CryptoResult<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| CryptoError::InvalidDestination(path.to_path_buf()))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let canonical_parent = fs::canonicalize(parent)?;

    if !canonical_parent.is_dir() {
        return Err(CryptoError::InvalidDestination(path.to_path_buf()));
    }

    Ok(canonical_parent.join(file_name))
}

#[cfg(target_os = "windows")]
fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

#[cfg(not(target_os = "windows"))]
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
}

fn validate_destination_path(src: &Path, dest: &Path) -> CryptoResult<PathBuf> {
    let canonical_src = fs::canonicalize(src)?;
    let canonical_dest = normalize_destination_path(dest)?;

    if same_path(&canonical_src, &canonical_dest) {
        return Err(CryptoError::SourceAndDestinationSame(canonical_dest));
    }

    match fs::symlink_metadata(&canonical_dest) {
        Ok(_) => return Err(CryptoError::FileAlreadyExists(canonical_dest)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    Ok(canonical_dest)
}

fn read_limited_file(file: fs::File, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut reader = file.take(limit + 1);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn persist_temp_file(tmp: &Path, dest: &Path) -> CryptoResult<()> {
    #[cfg(unix)]
    {
        match fs::hard_link(tmp, dest) {
            Ok(()) => {
                let _ = fs::remove_file(tmp);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(tmp);
                Err(CryptoError::FileAlreadyExists(dest.to_path_buf()))
            }
            Err(error) => {
                let _ = fs::remove_file(tmp);
                Err(error.into())
            }
        }
    }

    #[cfg(not(unix))]
    {
        match fs::rename(tmp, dest) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(tmp);
                Err(CryptoError::FileAlreadyExists(dest.to_path_buf()))
            }
            Err(error) => {
                let _ = fs::remove_file(tmp);
                Err(error.into())
            }
        }
    }
}

fn tmp_path(dest: &Path) -> PathBuf {
    let mut rng_bytes = [0u8; 8];
    OsRng.fill_bytes(&mut rng_bytes);
    let suffix = hex::encode(rng_bytes);
    let mut tmp = dest.to_path_buf();
    let name = dest.file_name().unwrap_or_default().to_string_lossy();
    tmp.set_file_name(format!("{}.{}.tmp", name, suffix));
    tmp
}

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
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|error| CryptoError::EncryptionFailed(error.to_string()))?;

    Ok((Zeroizing::new(ciphertext), salt, nonce_bytes))
}

pub fn decrypt_bytes(
    ciphertext: &[u8],
    password: &[u8],
    salt: &[u8],
    nonce_bytes: &[u8],
) -> CryptoResult<Zeroizing<Vec<u8>>> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(CryptoError::PassphraseTooShort(MIN_PASSWORD_LEN));
    }
    if salt.len() != SALT_LEN {
        return Err(CryptoError::InvalidSaltLength {
            expected: SALT_LEN,
            actual: salt.len(),
        });
    }
    if nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength {
            expected: NONCE_LEN,
            actual: nonce_bytes.len(),
        });
    }

    let key = derive_key(password, salt)?;
    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let aad = build_aad(salt, nonce_bytes);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::DecryptionFailed)?;

    Ok(Zeroizing::new(plaintext))
}

pub fn parse_header(raw: &[u8]) -> CryptoResult<(&[u8], &[u8], &[u8])> {
    if raw.len() < HEADER_LEN + TAG_LEN {
        return Err(CryptoError::FileTooSmall);
    }
    if &raw[..8] != MAGIC {
        return Err(CryptoError::InvalidMagic);
    }

    let salt = &raw[8..40];
    let nonce = &raw[40..52];
    let ciphertext = &raw[52..];
    Ok((salt, nonce, ciphertext))
}

pub fn encrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(CryptoError::PassphraseTooShort(MIN_PASSWORD_LEN));
    }

    let (mut file, source_metadata) = open_regular_file(src)?;
    let dest = validate_destination_path(src, dest)?;
    let source_len = source_metadata.len();

    let mut salt = [0u8; SALT_LEN];
    let mut stream_nonce = [0u8; STREAM_NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut stream_nonce);

    reporter.report(0.05, "Deriving encryption key (Argon2id)...");
    let key = derive_key_v3(password, &salt)?;

    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let mut encryptor = EncryptorBE32::from_aead(cipher, (&stream_nonce).into());

    let tmp = tmp_path(&dest);
    let write_result = (|| -> CryptoResult<()> {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;

        // Write V3 header
        out.write_all(MAGIC_V3)?;
        out.write_all(&salt)?;
        out.write_all(&stream_nonce)?;

        // Stream encryption in 1 MB chunks
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut bytes_done: u64 = 0;

        loop {
            let n = read_exact_or_eof(&mut file, &mut buf)?;
            if n == 0 {
                // Empty file or exact chunk boundary — finalize with empty last
                let ct = encryptor
                    .encrypt_last(&[][..])
                    .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;
                out.write_all(&ct)?;
                break;
            }

            bytes_done += n as u64;
            let frac = if source_len > 0 {
                0.10 + 0.85 * (bytes_done as f32 / source_len as f32)
            } else {
                0.95
            };

            // Check if there is more data after this chunk
            let mut peek = [0u8; 1];
            let peeked = file.read(&mut peek)?;

            if peeked == 0 {
                // This is the last chunk
                reporter.report(frac, "Encrypting final chunk...");
                let ct = encryptor
                    .encrypt_last(&buf[..n])
                    .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;
                out.write_all(&ct)?;
                break;
            } else {
                // More data follows — encrypt as intermediate chunk
                reporter.report(frac, "Encrypting...");
                let ct = encryptor
                    .encrypt_next(&buf[..n])
                    .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;
                out.write_all(&ct)?;
                // Put the peeked byte back by seeking back 1
                file.seek(std::io::SeekFrom::Current(-1))?;
            }
        }

        out.sync_all()?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    persist_temp_file(&tmp, &dest)?;
    reporter.report(1.00, "Encryption complete.");
    Ok(dest)
}

/// Read up to `buf.len()` bytes, returning how many were read (may be less at EOF).
fn read_exact_or_eof(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(total)
}

pub fn decrypt_file(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(CryptoError::PassphraseTooShort(MIN_PASSWORD_LEN));
    }

    // Read magic bytes to determine format version
    let mut magic_buf = [0u8; 8];
    {
        let mut f = fs::File::open(src)?;
        f.read_exact(&mut magic_buf)
            .map_err(|_| CryptoError::FileTooSmall)?;
    }

    if &magic_buf == MAGIC {
        decrypt_file_legacy(src, dest, password, reporter)
    } else if &magic_buf == MAGIC_V3 {
        decrypt_file_streaming(src, dest, password, reporter)
    } else {
        Err(CryptoError::InvalidMagic)
    }
}

/// Legacy V2 decryption — loads entire file into RAM.
fn decrypt_file_legacy(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    let (file, source_metadata) = open_regular_file(src)?;
    let dest = validate_destination_path(src, dest)?;
    let source_len = source_metadata.len();
    #[cfg(target_os = "windows")]
    let (src_accessed, src_modified) = (
        source_metadata.accessed().ok(),
        source_metadata.modified().ok(),
    );

    if source_len > MAX_ENCRYPTED_FILE_SIZE {
        return Err(CryptoError::FileTooLarge {
            size_gb: source_len as f64 / 1_000_000_000.0,
            max_gb: MAX_ENCRYPTED_FILE_SIZE as f64 / 1_000_000_000.0,
        });
    }

    reporter.report(0.10, "Reading encrypted file (legacy V2)...");
    let raw = read_limited_file(file, MAX_ENCRYPTED_FILE_SIZE)?;

    let (salt, nonce, ciphertext) = parse_header(&raw)?;

    reporter.report(0.20, "Deriving decryption key (Argon2id)...");
    let key = derive_key(password, salt)?;

    reporter.report(0.50, "Decrypting data (AES-256-GCM-SIV)...");
    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let aad = build_aad(salt, nonce);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::DecryptionFailed)?,
    );

    let tmp = tmp_path(&dest);
    reporter.report(0.75, "Writing decrypted file...");
    let write_result = (|| -> CryptoResult<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(&plaintext)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    persist_temp_file(&tmp, &dest)?;

    #[cfg(target_os = "windows")]
    {
        if let Ok(file) = fs::File::open(&dest) {
            let times = fs::FileTimes::new()
                .set_accessed(src_accessed.unwrap_or(SystemTime::now()))
                .set_modified(src_modified.unwrap_or(SystemTime::now()));
            let _ = file.set_times(times);
        }
    }

    reporter.report(1.00, "Decryption complete.");
    Ok(dest)
}

/// V3 streaming decryption — constant memory usage.
fn decrypt_file_streaming(
    src: &Path,
    dest: &Path,
    password: &[u8],
    reporter: &dyn ProgressReporter,
) -> CryptoResult<PathBuf> {
    let (mut file, source_metadata) = open_regular_file(src)?;
    let dest = validate_destination_path(src, dest)?;
    let file_len = source_metadata.len();
    #[cfg(target_os = "windows")]
    let (src_accessed, src_modified) = (
        source_metadata.accessed().ok(),
        source_metadata.modified().ok(),
    );

    if file_len < HEADER_V3_LEN as u64 + TAG_LEN as u64 {
        return Err(CryptoError::FileTooSmall);
    }

    // Skip magic (already read by dispatcher), read salt + nonce
    file.seek(std::io::SeekFrom::Start(8))?;
    let mut salt = [0u8; SALT_LEN];
    let mut stream_nonce = [0u8; STREAM_NONCE_LEN];
    file.read_exact(&mut salt)?;
    file.read_exact(&mut stream_nonce)?;

    reporter.report(0.05, "Deriving decryption key (Argon2id)...");
    let key = derive_key_v3(password, &salt)?;

    let cipher = Aes256GcmSiv::new(key.as_slice().into());
    let mut decryptor = DecryptorBE32::from_aead(cipher, (&stream_nonce).into());

    let encrypted_body = file_len - HEADER_V3_LEN as u64;
    let enc_chunk_size = CHUNK_SIZE + TAG_LEN;

    let tmp = tmp_path(&dest);
    let write_result = (|| -> CryptoResult<()> {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;

        let mut buf = vec![0u8; enc_chunk_size];
        let mut bytes_done: u64 = 0;
        let mut last_chunk: Option<Vec<u8>> = None;

        // Read all chunks, processing intermediate ones immediately
        loop {
            let n = read_exact_or_eof(&mut file, &mut buf)?;
            if n == 0 {
                break;
            }

            // If we had a pending chunk, it wasn't the last — decrypt it now
            if let Some(prev) = last_chunk.take() {
                let frac = 0.10 + 0.85 * (bytes_done as f32 / encrypted_body as f32);
                reporter.report(frac, "Decrypting...");
                let pt = decryptor
                    .decrypt_next(prev.as_slice())
                    .map_err(|_| CryptoError::DecryptionFailed)?;
                out.write_all(&pt)?;
            }

            bytes_done += n as u64;
            last_chunk = Some(buf[..n].to_vec());
        }

        // Process the final chunk
        if let Some(final_data) = last_chunk {
            reporter.report(0.95, "Decrypting final chunk...");
            let pt = decryptor
                .decrypt_last(final_data.as_slice())
                .map_err(|_| CryptoError::DecryptionFailed)?;
            out.write_all(&pt)?;
        }

        out.sync_all()?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    persist_temp_file(&tmp, &dest)?;

    #[cfg(target_os = "windows")]
    {
        if let Ok(file) = fs::File::open(&dest) {
            let times = fs::FileTimes::new()
                .set_accessed(src_accessed.unwrap_or(SystemTime::now()))
                .set_modified(src_modified.unwrap_or(SystemTime::now()));
            let _ = file.set_times(times);
        }
    }

    reporter.report(1.00, "Decryption complete.");
    Ok(dest)
}

/// Attempts a 3-pass random overwrite, rename, and deletion of `path`.
///
/// WARNING: This is not cryptographically guaranteed on SSDs, APFS, Btrfs, ZFS,
/// or NTFS with shadow copies enabled.
pub fn secure_wipe(path: &Path) -> CryptoResult<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() {
        return Err(CryptoError::NotAFile(path.to_path_buf()));
    }

    let len = metadata.len();
    let mut file = fs::OpenOptions::new().write(true).open(path)?;

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

    #[cfg(target_os = "windows")]
    {
        let epoch = SystemTime::UNIX_EPOCH;
        let times = fs::FileTimes::new().set_accessed(epoch).set_modified(epoch);
        let _ = file.set_times(times);
    }

    drop(file);

    for _ in 0..8 {
        let mut random_name = [0u8; 16];
        OsRng.fill_bytes(&mut random_name);
        let renamed_path = path.with_file_name(hex::encode(random_name));

        match fs::rename(path, &renamed_path) {
            Ok(()) => {
                fs::remove_file(renamed_path)?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to generate a unique wipe filename",
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestReporter;

    impl ProgressReporter for TestReporter {
        fn report(&self, _progress: f32, _message: &str) {}
    }

    fn unique_test_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "neuron_test_{}",
            rand_core::RngCore::next_u64(&mut rand_core::OsRng)
        ))
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
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let src_path = tmp_dir.join("src.txt");
        let encrypted_path = tmp_dir.join("dest.vx2");
        let decrypted_path = tmp_dir.join("final.txt");
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        fs::write(&src_path, b"File content for testing").unwrap();

        encrypt_file(&src_path, &encrypted_path, password.as_bytes(), &reporter).unwrap();
        assert!(encrypted_path.exists());

        decrypt_file(
            &encrypted_path,
            &decrypted_path,
            password.as_bytes(),
            &reporter,
        )
        .unwrap();
        assert!(decrypted_path.exists());

        let final_content = fs::read(decrypted_path).unwrap();
        assert_eq!(final_content, b"File content for testing");

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_password_too_short() {
        let plaintext = b"some data";
        let password = b"short";
        let result = encrypt_bytes(plaintext, password);
        assert!(matches!(result, Err(CryptoError::PassphraseTooShort(_))));
    }

    #[test]
    fn test_invalid_magic() {
        let mut raw = vec![0u8; 100];
        raw[0..8].copy_from_slice(b"NOTMAGIC");
        let result = parse_header(&raw);
        assert!(matches!(result, Err(CryptoError::InvalidMagic)));
    }

    #[test]
    fn test_decrypt_bytes_rejects_invalid_nonce_length() {
        let result = decrypt_bytes(
            b"ciphertext",
            b"supersecretpassword",
            &[1u8; SALT_LEN],
            &[0u8; 8],
        );
        assert!(matches!(
            result,
            Err(CryptoError::InvalidNonceLength {
                expected: NONCE_LEN,
                actual: 8
            })
        ));
    }

    #[test]
    fn test_decrypt_bytes_rejects_short_password() {
        let result = decrypt_bytes(b"ciphertext", b"short", &[1u8; SALT_LEN], &[0u8; NONCE_LEN]);
        assert!(matches!(result, Err(CryptoError::PassphraseTooShort(_))));
    }

    #[test]
    fn test_encrypt_file_rejects_existing_destination() {
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let src_path = tmp_dir.join("src.txt");
        let dest_path = tmp_dir.join("dest.vx2");
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        fs::write(&src_path, b"secret").unwrap();
        fs::write(&dest_path, b"already here").unwrap();

        let result = encrypt_file(&src_path, &dest_path, password.as_bytes(), &reporter);
        assert!(matches!(result, Err(CryptoError::FileAlreadyExists(_))));

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_encrypt_file_rejects_same_source_and_destination() {
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let src_path = tmp_dir.join("src.txt");
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        fs::write(&src_path, b"secret").unwrap();

        let result = encrypt_file(&src_path, &src_path, password.as_bytes(), &reporter);
        assert!(matches!(
            result,
            Err(CryptoError::SourceAndDestinationSame(_))
        ));

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_v3_stream_roundtrip_multi_chunk() {
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let src_path = tmp_dir.join("big.bin");
        let encrypted_path = tmp_dir.join("big.vx2");
        let decrypted_path = tmp_dir.join("big_dec.bin");
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        // Create a file larger than CHUNK_SIZE (2.5 MB)
        let data: Vec<u8> = (0u8..=255)
            .cycle()
            .take(CHUNK_SIZE * 2 + CHUNK_SIZE / 2)
            .collect();
        fs::write(&src_path, &data).unwrap();

        encrypt_file(&src_path, &encrypted_path, password.as_bytes(), &reporter).unwrap();

        // Verify the file starts with VAULTX03
        let header = fs::read(&encrypted_path).unwrap();
        assert_eq!(&header[..8], MAGIC_V3);

        decrypt_file(
            &encrypted_path,
            &decrypted_path,
            password.as_bytes(),
            &reporter,
        )
        .unwrap();
        let result = fs::read(&decrypted_path).unwrap();
        assert_eq!(result, data);

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_v3_stream_roundtrip_exact_chunk() {
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let src_path = tmp_dir.join("exact.bin");
        let encrypted_path = tmp_dir.join("exact.vx2");
        let decrypted_path = tmp_dir.join("exact_dec.bin");
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        // Exactly 1 chunk
        let data = vec![42u8; CHUNK_SIZE];
        fs::write(&src_path, &data).unwrap();

        encrypt_file(&src_path, &encrypted_path, password.as_bytes(), &reporter).unwrap();
        decrypt_file(
            &encrypted_path,
            &decrypted_path,
            password.as_bytes(),
            &reporter,
        )
        .unwrap();
        assert_eq!(fs::read(&decrypted_path).unwrap(), data);

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_v3_wrong_password() {
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let src_path = tmp_dir.join("secret.txt");
        let encrypted_path = tmp_dir.join("secret.vx2");
        let decrypted_path = tmp_dir.join("secret_dec.txt");
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let wrong = "y".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        fs::write(&src_path, b"top secret data").unwrap();
        encrypt_file(&src_path, &encrypted_path, password.as_bytes(), &reporter).unwrap();

        let result = decrypt_file(
            &encrypted_path,
            &decrypted_path,
            wrong.as_bytes(),
            &reporter,
        );
        assert!(result.is_err());

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_v2_backward_compat() {
        // Manually create a V2-format encrypted file and verify decrypt_file can handle it
        let tmp_dir = unique_test_dir();
        fs::create_dir_all(&tmp_dir).unwrap();

        let plaintext = b"Legacy V2 content";
        let password = "x".repeat(MIN_PASSWORD_LEN + 4);
        let reporter = TestReporter;

        let (ciphertext, salt, nonce) = encrypt_bytes(plaintext, password.as_bytes()).unwrap();

        let v2_path = tmp_dir.join("legacy.vx2");
        {
            let mut f = fs::File::create(&v2_path).unwrap();
            f.write_all(MAGIC).unwrap();
            f.write_all(&salt).unwrap();
            f.write_all(&nonce).unwrap();
            f.write_all(&ciphertext).unwrap();
            f.sync_all().unwrap();
        }

        let dec_path = tmp_dir.join("legacy_dec.txt");
        decrypt_file(&v2_path, &dec_path, password.as_bytes(), &reporter).unwrap();
        assert_eq!(fs::read(&dec_path).unwrap(), plaintext);

        let _ = fs::remove_dir_all(tmp_dir);
    }
}
