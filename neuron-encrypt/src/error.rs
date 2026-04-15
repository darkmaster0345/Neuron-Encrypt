// error.rs — Custom error types via thiserror
// No egui imports. No crypto logic.

use thiserror::Error;

/// All errors that can occur during encryption/decryption operations.
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("File too small to be a valid VAULTX02 encrypted file")]
    FileTooSmall,

    #[error("Not a VAULTX02 encrypted file (bad magic bytes)")]
    InvalidMagic,

    #[error("Wrong password or corrupted file")]
    DecryptionFailed,

    #[error("Argon2id key derivation failed: {0}")]
    Argon2Failed(String),

    #[error("HKDF key expansion failed: {0}")]
    HkdfFailed(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    /// BUG-07: Dedicated variant for file-too-large errors in both encrypt and decrypt paths.
    #[error("File too large ({size_gb:.1} GB). Maximum supported size is ~{max_gb:.1} GB.")]
    FileTooLarge { size_gb: f64, max_gb: f64 },

    /// BUG-12: Destination file already exists — GUI should prompt for confirmation.
    #[error("Destination file already exists: {0}")]
    FileAlreadyExists(std::path::PathBuf),

    #[error("Write incomplete. Disk full? Expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Secure wipe failed for {0}: {1}")]
    SecureWipeFailed(std::path::PathBuf, String),
}

/// Result type alias for crypto operations.
pub type CryptoResult<T> = Result<T, CryptoError>;
