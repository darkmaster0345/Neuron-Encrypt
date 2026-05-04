use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use clap::{Parser, Subcommand};
use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};
use neuron_encrypt_core::error::CryptoError;
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(name = "neuron-encrypt-cli", version, about = "Command-line file encryption using AES-256-GCM-SIV + Argon2id")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Encrypt a file
    Encrypt {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path (defaults to input.vx2)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Passphrase (min 8 characters)
        #[arg(short, long)]
        password: String,
    },
    /// Decrypt a file
    Decrypt {
        /// Input .vx2 file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path (defaults to original name)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Passphrase used during encryption
        #[arg(short, long)]
        password: String,
    },
}

struct CliReporter {
    last_progress: AtomicU32,
}

impl CliReporter {
    fn new() -> Self {
        Self {
            last_progress: AtomicU32::new(0),
        }
    }
}

impl ProgressReporter for CliReporter {
    fn report(&self, progress: f32, message: &str) {
        let pct = (progress * 100.0) as u32;
        let prev = self.last_progress.load(Ordering::Relaxed);
        if pct != prev {
            self.last_progress.store(pct, Ordering::Relaxed);
            eprint!("\r\033[K[{:>3}%] {}", pct, message);
        }
    }
}

impl Drop for CliReporter {
    fn drop(&mut self) {
        eprintln!(); // newline after final progress
    }
}

fn default_dest(input: &PathBuf, mode: &str) -> PathBuf {
    match mode {
        "encrypt" => {
            let mut dest = input.clone();
            let ext = dest
                .extension()
                .map(|e| format!(".{}.vx2", e.to_string_lossy()))
                .unwrap_or_else(|| ".vx2".to_owned());
            dest.set_extension("");
            dest.set_file_name(format!("{}{}", dest.file_name().unwrap_or_default().to_string_lossy(), ext));
            dest
        }
        "decrypt" => {
            let name = input.file_stem().unwrap_or_default().to_string_lossy();
            let parent = input.parent().unwrap_or_else(|| std::path::Path::new("."));
            // Remove trailing .vx2 if present, then reconstruct original name
            let name = name
                .strip_suffix(".vx2")
                .unwrap_or(&name);
            parent.join(name)
        }
        _ => input.clone(),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let password: Zeroizing<String> = match &cli.command {
        Command::Encrypt { password, .. } => Zeroizing::new(password.clone()),
        Command::Decrypt { password, .. } => Zeroizing::new(password.clone()),
    };

    let (input, output, mode) = match &cli.command {
        Command::Encrypt { input, output, .. } => {
            let out = output
                .clone()
                .unwrap_or_else(|| default_dest(input, "encrypt"));
            (input.clone(), out, "encrypt")
        }
        Command::Decrypt { input, output, .. } => {
            let out = output
                .clone()
                .unwrap_or_else(|| default_dest(input, "decrypt"));
            (input.clone(), out, "decrypt")
        }
    };

    if !input.exists() {
        return Err(format!("Input file not found: {}", input.display()).into());
    }

    if password.len() < crypto::MIN_PASSWORD_LEN {
        return Err(format!(
            "Passphrase too short (minimum {} characters required)",
            crypto::MIN_PASSWORD_LEN
        )
        .into());
    }

    let reporter = CliReporter::new();
    let throttled = ThrottledReporter::new(&reporter);

    let result = match mode {
        "encrypt" => {
            eprintln!("Encrypting: {}", input.display());
            eprintln!("Output:     {}", output.display());
            crypto::encrypt_file(&input, &output, password.as_bytes(), &throttled)
        }
        "decrypt" => {
            eprintln!("Decrypting: {}", input.display());
            eprintln!("Output:     {}", output.display());
            crypto::decrypt_file(&input, &output, password.as_bytes(), &throttled)
        }
        _ => unreachable!(),
    };

    match result {
        Ok(dest) => {
            eprintln!("Complete: {}", dest.display());
            Ok(())
        }
        Err(CryptoError::DecryptionFailed) => {
            eprintln!("Error: Wrong passphrase or corrupted file.");
            Err("Decryption failed".into())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            Err(e.to_string().into())
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
