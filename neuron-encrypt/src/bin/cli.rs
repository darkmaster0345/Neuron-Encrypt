use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use neuron_encrypt_core::crypto::{self, ProgressReporter, ThrottledReporter};
use neuron_encrypt_core::error::CryptoError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(
    name = "neuron-encrypt-cli",
    version = concat!(env!("CARGO_PKG_VERSION"), " (git: ", env!("GIT_HASH"), ")"),
    about = "Command-line file encryption using AES-256-GCM-SIV + Argon2id",
    long_about = "Encrypt or decrypt files from the terminal. \
Supports piping, progress bars, JSON output, and silent mode for scripts.",
    after_help = r#"EXAMPLES:
  # Encrypt a file (prompts for passphrase)
  neuron-encrypt-cli encrypt -i secret.pdf

  # Encrypt with custom output
  neuron-encrypt-cli encrypt -i secret.pdf -o vault/secret.vx2

  # Decrypt (prompts for passphrase)
  neuron-encrypt-cli decrypt -i secret.vx2

  # Decrypt to custom path
  neuron-encrypt-cli decrypt -i secret.vx2 -o recovered.pdf

  # Quiet mode (scripts)
  neuron-encrypt-cli encrypt -i secret.pdf -q
  neuron-encrypt-cli encrypt -i secret.pdf --password-file /run/secrets/key

  # Pipe passphrase via env var
  NEURON_PASSWORD="mypass" neuron-encrypt-cli encrypt -i secret.pdf

  # Pipe data through CLI
  cat backup.tar.gz | neuron-encrypt-cli decrypt -i - -o backup.tar.gz

  # JSON output for automation
  neuron-encrypt-cli encrypt -i secret.pdf --json

  # Generate shell completions
  neuron-encrypt-cli completions bash > ~/.local/share/bash-completion/neuron-encrypt-cli
"#
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Suppress all non-error output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Disable progress bar (implies --quiet if not --json)
    #[arg(long, global = true)]
    no_progress: bool,

    /// Emit structured JSON output for automation
    #[arg(long, global = true)]
    json: bool,

    /// Read passphrase from file (useful for CI/CD)
    #[arg(long, global = true)]
    password_file: Option<PathBuf>,

    /// Overwrite existing output files
    #[arg(short = 'F', long, global = true)]
    force: bool,

    /// Generate shell completions (bash, zsh, fish, powershell, elvish)
    #[arg(long)]
    completions: Option<Shell>,
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
        /// Skip passphrase confirmation prompt (for non-interactive/script use)
        #[arg(long, default_value_t = false)]
        no_confirm: bool,
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

fn read_password(password_file: &Option<PathBuf>) -> Zeroizing<String> {
    if let Some(path) = password_file {
        let pw = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error: Cannot read password file: {e}");
            std::process::exit(ExitCode::BadInput as i32);
        });
        return Zeroizing::new(pw.trim_end_matches(&['\r', '\n'][..]).to_owned());
    }

    if let Ok(pw) = std::env::var("NEURON_PASSWORD") {
        eprintln!(
            "[WARN] NEURON_PASSWORD is set: password is visible in /proc/<pid>/environ \n       and may appear in shell history or process monitors."
        );
        eprintln!(
            "[WARN] Prefer --password-file with a 0600-permission file for sensitive use."
        );
        return Zeroizing::new(pw);
    }

    loop {
        match rpassword::prompt_password("Enter passphrase: ") {
            Ok(pw) => {
                if pw.is_empty() {
                    eprintln!("Error: Passphrase cannot be empty.");
                    continue;
                }
                return Zeroizing::new(pw);
            }
            Err(e) => {
                eprintln!("Error reading passphrase: {e}");
                std::process::exit(ExitCode::BadInput as i32);
            }
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let len_a = a.len();
    let len_b = b.len();
    let max_len = len_a.max(len_b);

    let mut byte_comparison_result = 0;

    for i in 0..max_len {
        let byte_a = a.get(i).unwrap_or(&0);
        let byte_b = b.get(i).unwrap_or(&0);
        byte_comparison_result |= byte_a ^ byte_b;
    }

    let len_diff = len_a ^ len_b;
    // len_mismatch_flag will be 0 if lengths are equal, 1 if lengths are different
    let len_mismatch_flag = (((len_diff | len_diff.wrapping_neg()) >> (usize::BITS - 1)) & 1) as u8;

    // Combine byte comparison result with length mismatch flag
    // If either bytes don't match OR lengths don't match, final_result will be non-zero
    let final_result = byte_comparison_result | len_mismatch_flag;

    final_result == 0
}

fn read_password_confirmed(password_file: &Option<PathBuf>) -> Result<Zeroizing<String>, String> {
    let pass1 = read_password(password_file);
    let pass2 = loop {
        match rpassword::prompt_password("Confirm passphrase: ") {
            Ok(pw) => {
                if pw.is_empty() {
                    eprintln!("Error: Passphrase cannot be empty.");
                    continue;
                }
                break Zeroizing::new(pw);
            }
            Err(e) => return Err(format!("Error reading passphrase: {e}")),
        }
    };
    if !constant_time_eq(pass1.as_bytes(), pass2.as_bytes()) {
        return Err("Passphrases do not match".to_owned());
    }
    Ok(pass1)
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
                "[{bar:10.cyan/blue}] {percent}% | {msg} | {bytes:>7}/{total_bytes:7} | {bytes_per_sec} | ETA {eta}",
            )
            .expect("Invalid progress bar template")
            .with_key("bytes_per_sec", |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{}/s", HumanBytes(state.pos())).expect("Failed to format bytes per second");
            })
            .progress_chars("##-")
        } else {
            ProgressStyle::with_template("{spinner} | {msg} | {bytes} | {bytes_per_sec}")
                .expect("Invalid progress bar spinner template")
                .with_key(
                    "bytes_per_sec",
                    |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                        write!(w, "{}/s", HumanBytes(state.pos())).expect("Failed to format bytes per second");
                    },
                )
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
            write!(f, "{b} B")
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

#[derive(Serialize)]
struct JsonResult {
    status: String,
    output_path: Option<String>,
    bytes_processed: Option<u64>,
    duration_ms: u128,
    sha256: Option<String>,
    error: Option<String>,
}

#[repr(i32)]
enum ExitCode {
    Success = 0,
    RuntimeError = 1,
    BadInput = 2,
    WrongPassword = 3,
}

fn resolve_output(input_path: &Path, mode: &str, output_arg: &Option<String>) -> PathBuf {
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
            input_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(format!("{name}.vx2"))
        }
        "decrypt" => {
            let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
            let stem = stem.strip_suffix(".vx2").unwrap_or(&stem);
            input_path.parent().unwrap_or(Path::new(".")).join(stem)
        }
        _ => input_path.to_path_buf(),
    }
}

fn check_output_exists(path: &Path, force: bool, json: bool) -> Result<(), ExitCode> {
    if path.exists() && !force {
        let msg = format!(
            "Output file already exists: {}. Use --force to overwrite.",
            path.display()
        );
        if json {
            emit_json(&JsonResult {
                status: "error".into(),
                output_path: Some(path.display().to_string()),
                bytes_processed: None,
                duration_ms: 0,
                sha256: None,
                error: Some(msg.clone()),
            });
        }
        eprintln!("Error: {msg}");
        return Err(ExitCode::BadInput);
    }
    Ok(())
}

fn is_vx2_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("vx2"))
        .unwrap_or(false)
}

fn emit_json(result: &JsonResult) {
    match serde_json::to_string(result) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Error serializing JSON result: {e}"),
    }
}

fn compute_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let result = hasher.finalize();
    Ok(format!("{result:x}"))
}

fn run() -> Result<(), ExitCode> {
    let cli = Cli::parse();

    if let Some(shell) = &cli.completions {
        let mut cmd = Cli::command();
        generate(*shell, &mut cmd, "neuron-encrypt-cli", &mut io::stdout());
        return Ok(());
    }

    let start = Instant::now();
    let (input_str, output_str, mode) = match &cli.command {
        Command::Encrypt { input, output, .. } => (input.clone(), output.clone(), "encrypt"),
        Command::Decrypt { input, output } => (input.clone(), output.clone(), "decrypt"),
    };
    let password = match &cli.command {
        Command::Encrypt { no_confirm, .. }
            if std::env::var("NEURON_PASSWORD").is_err() && !no_confirm =>
        {
            match read_password_confirmed(&cli.password_file) {
                Ok(password) => password,
                Err(msg) => {
                    eprintln!("Error: {msg}");
                    return Err(ExitCode::BadInput);
                }
            }
        }
        _ => read_password(&cli.password_file),
    };

    if mode == "encrypt" && password.len() < crypto::MIN_PASSWORD_LEN {
        let msg = format!(
            "Passphrase too short (minimum {} characters).",
            crypto::MIN_PASSWORD_LEN
        );
        if cli.json {
            emit_json(&JsonResult {
                status: "error".into(),
                output_path: None,
                bytes_processed: None,
                duration_ms: start.elapsed().as_millis(),
                sha256: None,
                error: Some(msg.clone()),
            });
        }
        eprintln!("Error: {msg}");
        return Err(ExitCode::BadInput);
    }

    let is_pipe_in = input_str == "-";
    let is_pipe_out = output_str.as_deref() == Some("-");
    let show_progress = !cli.quiet && !cli.no_progress && !cli.json && !is_pipe_out;

    if !is_pipe_in && !is_pipe_out {
        let input_path = PathBuf::from(&input_str);
        if !input_path.exists() {
            let msg = format!("Input file not found: {}", input_path.display());
            if cli.json {
                emit_json(&JsonResult {
                    status: "error".into(),
                    output_path: None,
                    bytes_processed: None,
                    duration_ms: start.elapsed().as_millis(),
                    sha256: None,
                    error: Some(msg.clone()),
                });
            }
            eprintln!("Error: {msg}");
            return Err(ExitCode::BadInput);
        }

        if mode == "encrypt" && is_vx2_file(&input_path) && !cli.force {
            let msg = "Input file appears to be already encrypted (.vx2). Encrypting again will produce .vx2.vx2. Use --force to proceed.".to_owned();
            if cli.json {
                emit_json(&JsonResult {
                    status: "error".into(),
                    output_path: None,
                    bytes_processed: None,
                    duration_ms: start.elapsed().as_millis(),
                    sha256: None,
                    error: Some(msg.clone()),
                });
            }
            eprintln!("Warning: {msg}");
            return Err(ExitCode::BadInput);
        }

        let source_size = fs::metadata(&input_path).ok().map(|m| m.len());
        let output_path = resolve_output(&input_path, mode, &output_str);

        check_output_exists(&output_path, cli.force, cli.json)?;

        if !cli.quiet && !cli.json {
            let action = if mode == "encrypt" {
                "Encrypting"
            } else {
                "Decrypting"
            };
            let total = source_size.unwrap_or(0);
            eprintln!(
                "{} {}{} → {}",
                action,
                input_path.display(),
                if total > 0 {
                    format!(" ({})", HumanBytes(total))
                } else {
                    String::new()
                },
                output_path.display()
            );
        }

        let reporter: Box<dyn ProgressReporter> = if show_progress {
            Box::new(IndProgress::new(source_size, "Deriving key..."))
        } else {
            Box::new(QuietReporter)
        };
        let throttled = ThrottledReporter::new(reporter.as_ref());

        let result = if mode == "encrypt" {
            crypto::encrypt_file(
                &input_path,
                &output_path,
                cli.force,
                password.as_bytes(),
                &throttled,
            )
        } else {
            crypto::decrypt_file(
                &input_path,
                &output_path,
                cli.force,
                password.as_bytes(),
                &throttled,
            )
        };

        match result {
            Ok(dest) => {
                let elapsed = start.elapsed().as_millis();
                if mode == "encrypt" {
                    let hash = compute_sha256(&input_path).ok();
                    if cli.json {
                        emit_json(&JsonResult {
                            status: "success".into(),
                            output_path: Some(dest.display().to_string()),
                            bytes_processed: source_size,
                            duration_ms: elapsed,
                            sha256: hash.clone(),
                            error: None,
                        });
                    } else if !cli.quiet {
                        eprintln!(
                            "✓ Written to {} ({:.2}s)",
                            dest.display(),
                            elapsed as f64 / 1000.0
                        );
                        if let Some(h) = &hash {
                            eprintln!("SHA-256 (original): {h}");
                        }
                    }
                } else {
                    let hash = compute_sha256(&dest).ok();
                    if cli.json {
                        emit_json(&JsonResult {
                            status: "success".into(),
                            output_path: Some(dest.display().to_string()),
                            bytes_processed: source_size,
                            duration_ms: elapsed,
                            sha256: hash.clone(),
                            error: None,
                        });
                    } else if !cli.quiet {
                        eprintln!(
                            "✓ Written to {} ({:.2}s)",
                            dest.display(),
                            elapsed as f64 / 1000.0
                        );
                        if let Some(h) = &hash {
                            eprintln!("SHA-256 (decrypted): {h}");
                        }
                    }
                }
                Ok(())
            }
            Err(CryptoError::DecryptionFailed) => {
                let msg = "Wrong passphrase or corrupted file.";
                if cli.json {
                    emit_json(&JsonResult {
                        status: "error".into(),
                        output_path: Some(output_path.display().to_string()),
                        bytes_processed: None,
                        duration_ms: start.elapsed().as_millis(),
                        sha256: None,
                        error: Some(msg.into()),
                    });
                } else {
                    eprintln!("✗ {msg}");
                }
                Err(ExitCode::WrongPassword)
            }
            Err(e) => {
                let msg = e.to_string();
                if cli.json {
                    emit_json(&JsonResult {
                        status: "error".into(),
                        output_path: Some(output_path.display().to_string()),
                        bytes_processed: None,
                        duration_ms: start.elapsed().as_millis(),
                        sha256: None,
                        error: Some(msg.clone()),
                    });
                } else {
                    eprintln!("✗ {msg}");
                }
                Err(ExitCode::RuntimeError)
            }
        }
    } else {
        let source_size = if !is_pipe_in {
            let p = PathBuf::from(&input_str);
            fs::metadata(&p).ok().map(|m| m.len())
        } else {
            None
        };

        if !cli.quiet && !cli.json {
            let action = if mode == "encrypt" {
                "Encrypting"
            } else {
                "Decrypting"
            };
            eprintln!(
                "{} stdin → stdout{}",
                action,
                if let Some(s) = source_size {
                    format!(" ({})", HumanBytes(s))
                } else {
                    String::new()
                }
            );
        }

        let reporter: Box<dyn ProgressReporter> = if show_progress {
            Box::new(IndProgress::new(source_size, "Deriving key..."))
        } else {
            Box::new(QuietReporter)
        };
        let throttled = ThrottledReporter::new(reporter.as_ref());

        let result = if mode == "encrypt" {
            if is_pipe_in {
                let msg = "Encryption from stdin is not supported (requires seeking). Use a file input instead.";
                if cli.json {
                    emit_json(&JsonResult {
                        status: "error".into(),
                        output_path: None,
                        bytes_processed: None,
                        duration_ms: start.elapsed().as_millis(),
                        sha256: None,
                        error: Some(msg.into()),
                    });
                }
                eprintln!("Error: {msg}");
                return Err(ExitCode::BadInput);
            } else {
                let mut f = match fs::File::open(&input_str) {
                    Ok(f) => f,
                    Err(e) => {
                        let msg = format!("Cannot open input: {e}");
                        if cli.json {
                            emit_json(&JsonResult {
                                status: "error".into(),
                                output_path: None,
                                bytes_processed: None,
                                duration_ms: start.elapsed().as_millis(),
                                sha256: None,
                                error: Some(msg.clone()),
                            });
                        }
                        eprintln!("Error: {msg}");
                        return Err(ExitCode::BadInput);
                    }
                };
                let stdout = io::stdout();
                let mut stdout_lock = stdout.lock();
                crypto::encrypt_stream(
                    &mut f,
                    &mut stdout_lock,
                    password.as_bytes(),
                    source_size,
                    &throttled,
                )
            }
        } else {
            let mut reader: Box<dyn Read> = if is_pipe_in {
                let mut buffer = Vec::new();
                io::stdin()
                    .lock()
                    .read_to_end(&mut buffer)
                    .map_err(|_| ExitCode::RuntimeError)?;
                Box::new(io::Cursor::new(buffer))
            } else {
                match fs::File::open(&input_str) {
                    Ok(f) => Box::new(f),
                    Err(e) => {
                        let msg = format!("Cannot open input: {e}");
                        if cli.json {
                            emit_json(&JsonResult {
                                status: "error".into(),
                                output_path: None,
                                bytes_processed: None,
                                duration_ms: start.elapsed().as_millis(),
                                sha256: None,
                                error: Some(msg.clone()),
                            });
                        }
                        eprintln!("Error: {msg}");
                        return Err(ExitCode::BadInput);
                    }
                }
            };
            if is_pipe_out {
                let stdout = io::stdout();
                let mut stdout_lock = stdout.lock();
                crypto::decrypt_stream(
                    &mut reader,
                    &mut stdout_lock,
                    password.as_bytes(),
                    source_size,
                    &throttled,
                )
            } else if let Some(path) = output_str.as_deref() {
                let mut out = match fs::File::create(path) {
                    Ok(f) => f,
                    Err(e) => {
                        let msg = format!("Cannot open output: {e}");
                        if cli.json {
                            emit_json(&JsonResult {
                                status: "error".into(),
                                output_path: Some(path.to_owned()),
                                bytes_processed: None,
                                duration_ms: start.elapsed().as_millis(),
                                sha256: None,
                                error: Some(msg.clone()),
                            });
                        }
                        eprintln!("Error: {msg}");
                        return Err(ExitCode::BadInput);
                    }
                };
                crypto::decrypt_stream(
                    &mut reader,
                    &mut out,
                    password.as_bytes(),
                    source_size,
                    &throttled,
                )
            } else {
                let msg = "Output path is required when decrypting piped input unless -o - is set.";
                if cli.json {
                    emit_json(&JsonResult {
                        status: "error".into(),
                        output_path: None,
                        bytes_processed: None,
                        duration_ms: start.elapsed().as_millis(),
                        sha256: None,
                        error: Some(msg.into()),
                    });
                }
                eprintln!("Error: {msg}");
                return Err(ExitCode::BadInput);
            }
        };

        match result {
            Ok(()) => {
                let elapsed = start.elapsed().as_millis();
                if cli.json {
                    emit_json(&JsonResult {
                        status: "success".into(),
                        output_path: None,
                        bytes_processed: source_size,
                        duration_ms: elapsed,
                        sha256: None,
                        error: None,
                    });
                } else if !cli.quiet {
                    eprintln!("✓ Complete ({:.2}s)", elapsed as f64 / 1000.0);
                }
                Ok(())
            }
            Err(CryptoError::DecryptionFailed) => {
                let msg = "Wrong passphrase or corrupted file.";
                if cli.json {
                    emit_json(&JsonResult {
                        status: "error".into(),
                        output_path: None,
                        bytes_processed: None,
                        duration_ms: start.elapsed().as_millis(),
                        sha256: None,
                        error: Some(msg.into()),
                    });
                }
                eprintln!("✗ {msg}");
                Err(ExitCode::WrongPassword)
            }
            Err(e) => {
                let msg = e.to_string();
                if cli.json {
                    emit_json(&JsonResult {
                        status: "error".into(),
                        output_path: None,
                        bytes_processed: None,
                        duration_ms: start.elapsed().as_millis(),
                        sha256: None,
                        error: Some(msg.clone()),
                    });
                }
                eprintln!("✗ {msg}");
                Err(ExitCode::RuntimeError)
            }
        }
    }
}

fn main() {
    match run() {
        Ok(()) => std::process::exit(ExitCode::Success as i32),
        Err(code) => std::process::exit(code as i32),
    }
}
