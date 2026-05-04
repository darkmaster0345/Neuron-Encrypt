// error.rs - Custom error types via thiserror
// No egui imports. No crypto logic.

use thiserror::Error;

/// All errors that can occur during encryption/decryption operations.
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("File too small to be a valid VAULTX02 encrypted file")]
    FileTooSmall,

    #[error("Not a valid Neuron Encrypt file (bad magic bytes)")]
    InvalidMagic,

    #[error("Unsupported file format version: {0}")]
    UnsupportedVersion(String),

    #[error("Wrong password or corrupted file")]
    DecryptionFailed,

    #[error("Argon2id key derivation failed: {0}")]
    Argon2Failed(String),

    #[error("HKDF key expansion failed: {0}")]
    HkdfFailed(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("File too large ({size_gb:.1} GB). Maximum supported size is ~{max_gb:.1} GB.")]
    FileTooLarge { size_gb: f64, max_gb: f64 },

    #[error("Destination file already exists: {0}")]
    FileAlreadyExists(std::path::PathBuf),

    #[error("Invalid destination path: {0}")]
    InvalidDestination(std::path::PathBuf),

    #[error("Source and destination must be different files: {0}")]
    SourceAndDestinationSame(std::path::PathBuf),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not a regular file: {0}")]
    NotAFile(std::path::PathBuf),

    #[error("Invalid salt length: expected {expected} bytes, got {actual}")]
    InvalidSaltLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected} bytes, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    #[error("Passphrase too short (minimum {0} characters required)")]
    PassphraseTooShort(usize),
}

/// Result type alias for crypto operations.
pub type CryptoResult<T> = Result<T, CryptoError>;
