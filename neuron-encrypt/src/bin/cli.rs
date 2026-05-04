use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};
use neuron_encrypt_core::error::CryptoError;
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(
    name = "neuron-encrypt-cli",
    version,
    about = "Command-line file encryption using AES-256-GCM-SIV + Argon2id",
    long_about = "Encrypt or decrypt files from the terminal. Supports piping, progress bars, and silent mode for scripts."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Suppress all output (for scripts/automation)
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Encrypt a file
    Encrypt {
        /// Input file path (use "-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Output file path (use "-" for stdout, defaults to input.vx2)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Decrypt a file
    Decrypt {
        /// Input .vx2 file path (use "-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Output file path (use "-" for stdout, defaults to original name)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn prompt_password() -> Zeroizing<String> {
    use std::io::BufRead;

    eprint!("Enter passphrase: ");
    io::stderr().flush().unwrap();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();
    Zeroizing::new(line.trim_end_matches(&['\r', '\n'][..]).to_owned())
}

struct IndProgress {
    pb: ProgressBar,
    total_bytes: u64,
}

impl IndProgress {
    fn new(total: Option<u64>, action: &str) -> Self {
        let pb = ProgressBar::new(total.unwrap_or(0));

        let style = if total.is_some() {
            ProgressStyle::with_template(
                "[{bar:10.cyan/blue}] {percent}% | {msg} | {bytes:>7}/{total_bytes:7} | {bytes_per_sec}",
            )
            .unwrap()
            .with_key("bytes_per_sec", |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{}/s", HumanBytes(state.pos())).unwrap();
            })
            .progress_chars("##-")
        } else {
            ProgressStyle::with_template(
                "{spinner} | {msg} | {bytes} | {bytes_per_sec}",
            )
            .unwrap()
            .with_key("bytes_per_sec", |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{}/s", HumanBytes(state.pos())).unwrap();
            })
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
        };

        pb.set_style(style);
        pb.set_message(action.to_owned());

        Self {
            pb,
            total_bytes: total.unwrap_or(0),
        }
    }
}

impl ProgressReporter for IndProgress {
    fn report(&self, progress: f32, message: &str) {
        let bytes = if self.total_bytes > 0 {
            ((progress - 0.10).max(0.0) / 0.85 * self.total_bytes as f32) as u64
        } else {
            (progress * self.total_bytes as f32) as u64
        };

        if self.total_bytes > 0 {
            self.pb.set_position(bytes);
        }
        self.pb.set_message(message.to_owned());
    }
}

impl Drop for IndProgress {
    fn drop(&mut self) {
        self.pb.finish_with_message("Complete");
    }
}

struct HumanBytes(u64);

impl std::fmt::Display for HumanBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = self.0;
        if b < 1024 {
            write!(f, "{} B", b)
        } else if b < 1024 * 1024 {
            write!(f, "{:.1} KB", b as f64 / 1024.0)
        } else if b < 1024 * 1024 * 1024 {
            write!(f, "{:.1} MB", b as f64 / (1024.0 * 1024.0))
        } else {
            write!(f, "{:.2} GB", b as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

struct QuietReporter;
impl ProgressReporter for QuietReporter {
    fn report(&self, _progress: f32, _message: &str) {}
}

fn resolve_output(input_path: &PathBuf, mode: &str, output_arg: &Option<String>) -> PathBuf {
    if let Some(out) = output_arg {
        if out == "-" {
            return PathBuf::from("-");
        }
        return PathBuf::from(out);
    }

    match mode {
        "encrypt" => {
            let name = input_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "output".to_owned());
            input_path.parent().unwrap_or(Path::new(".")).join(format!("{}.vx2", name))
        }
        "decrypt" => {
            let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
            let stem = stem.strip_suffix(".vx2").unwrap_or(&stem);
            input_path.parent().unwrap_or(Path::new(".")).join(stem)
        }
        _ => input_path.clone(),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let password: Zeroizing<String> = match &cli.command {
        Command::Encrypt { .. } => {
            let pw = prompt_password();
            if pw.len() < crypto::MIN_PASSWORD_LEN {
                eprintln!("Error: Passphrase must be at least {} characters.", crypto::MIN_PASSWORD_LEN);
                std::process::exit(1);
            }
            pw
        }
        Command::Decrypt { .. } => {
            let pw = prompt_password();
            if pw.is_empty() {
                eprintln!("Error: Passphrase required.");
                std::process::exit(1);
            }
            pw
        }
    };

    let (input_str, output_str, mode) = match &cli.command {
        Command::Encrypt { input, output } => (input.clone(), output.clone(), "encrypt"),
        Command::Decrypt { input, output } => (input.clone(), output.clone(), "decrypt"),
    };

    let is_pipe_in = input_str == "-";
    let is_pipe_out = output_str.as_deref() == Some("-");

    // File-based operation
    if !is_pipe_in && !is_pipe_out {
        let input_path = PathBuf::from(&input_str);
        if !input_path.exists() {
            eprintln!("Error: Input file not found: {}", input_path.display());
            std::process::exit(1);
        }

        let source_size = if !is_pipe_in {
            fs::metadata(&input_path).ok().map(|m| m.len())
        } else {
            None
        };

        let output_path = resolve_output(&input_path, mode, &output_str);

        if !cli.quiet {
            let action = if mode == "encrypt" { "Encrypting" } else { "Decrypting" };
            let total = source_size.unwrap_or(0);
            eprintln!("{} {}{} → {}", action, input_path.display(),
                if total > 0 { format!(" ({})", HumanBytes(total)) } else { String::new() },
                output_path.display());
        }

        let reporter: Box<dyn ProgressReporter> = if cli.quiet {
            Box::new(QuietReporter)
        } else {
            Box::new(IndProgress::new(source_size, if mode == "encrypt" { "Deriving key..." } else { "Deriving key..." }))
        };
        let throttled = ThrottledReporter::new(reporter.as_ref());

        let result = if mode == "encrypt" {
            crypto::encrypt_file(&input_path, &output_path, password.as_bytes(), &throttled)
        } else {
            crypto::decrypt_file(&input_path, &output_path, password.as_bytes(), &throttled)
        };

        match result {
            Ok(dest) => {
                if !cli.quiet {
                    eprintln!("✓ Written to {}", dest.display());
                }
                Ok(())
            }
            Err(CryptoError::DecryptionFailed) => {
                eprintln!("✗ Wrong passphrase or corrupted file.");
                Err("Decryption failed".into())
            }
            Err(e) => {
                eprintln!("✗ {}", e);
                Err(e.to_string().into())
            }
        }
    } else {
        // Piped operation: stdin → stdout
        if is_pipe_out && cli.quiet {
            eprintln!("Error: Cannot use --quiet with stdout piping (progress must go somewhere).");
            std::process::exit(1);
        }

        let source_size = if mode == "decrypt" && is_pipe_in {
            None // stdin, unknown size
        } else if !is_pipe_in {
            let p = PathBuf::from(&input_str);
            fs::metadata(&p).ok().map(|m| m.len())
        } else {
            None
        };

        let action = if mode == "encrypt" { "Encrypting" } else { "Decrypting" };
        if !cli.quiet {
            eprintln!("{} stdin → stdout{}", action,
                if let Some(s) = source_size { format!(" ({})", HumanBytes(s)) } else { String::new() });
        }

        let reporter: Box<dyn ProgressReporter> = if cli.quiet {
            Box::new(QuietReporter)
        } else {
            Box::new(IndProgress::new(source_size, "Deriving key..."))
        };
        let throttled = ThrottledReporter::new(reporter.as_ref());

        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdin_lock = stdin.lock();
        let mut stdout_lock = stdout.lock();

        let result = if mode == "encrypt" {
            // stdin needs Seek for the peek pattern — buffer to temp file if piped
            if is_pipe_in {
                eprintln!("Error: Encryption from stdin requires buffering. Use a file input instead.");
                std::process::exit(1);
            } else {
                let mut f = fs::File::open(&input_str)?;
                crypto::encrypt_stream(&mut f, &mut stdout_lock, password.as_bytes(), source_size, &throttled)
            }
        } else {
            if is_pipe_in {
                // For decryption from stdin, we need to read header first, then stream
                // Since stdin isn't Seek, we use a buffered approach
                let mut buffer = Vec::new();
                stdin_lock.read_to_end(&mut buffer)?;
                let mut cursor = io::Cursor::new(buffer);
                crypto::decrypt_stream(&mut cursor, &mut stdout_lock, password.as_bytes(), source_size, &throttled)
            } else {
                let mut f = fs::File::open(&input_str)?;
                crypto::decrypt_stream(&mut f, &mut stdout_lock, password.as_bytes(), source_size, &throttled)
            }
        };

        match result {
            Ok(()) => {
                if !cli.quiet {
                    eprintln!("✓ Complete");
                }
                Ok(())
            }
            Err(CryptoError::DecryptionFailed) => {
                eprintln!("✗ Wrong passphrase or corrupted file.");
                Err("Decryption failed".into())
            }
            Err(e) => {
                eprintln!("✗ {}", e);
                Err(e.to_string().into())
            }
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
