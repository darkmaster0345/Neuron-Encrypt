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

    #[error("Write incomplete. Disk full? Expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for crypto operations.
pub type CryptoResult<T> = Result<T, CryptoError>;
