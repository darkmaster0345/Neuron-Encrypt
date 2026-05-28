# NEURON ENCRYPT

<div align="center">

![Neuron Encrypt](https://img.shields.io/badge/Neuron%20Encrypt-v2.0.0-blue?style=for-the-badge&logo=rust)
![License](https://img.shields.io/badge/License-GPLv3-green?style=for-the-badge)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=for-the-badge)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange?style=for-the-badge&logo=rust)

**Local file encryption. No accounts. No internet. No compromise.**

AES-256-GCM-SIV · Argon2id · HKDF-SHA512 · 100% Safe Rust

[![GitHub release](https://img.shields.io/github/v/release/darkmaster0345/Neuron-Encrypt?style=for-the-badge)](https://github.com/darkmaster0345/Neuron-Encrypt/releases)
[![GitHub stars](https://img.shields.io/github/stars/darkmaster0345/Neuron-Encrypt?style=for-the-badge&logo=github)](https://github.com/darkmaster0345/Neuron-Encrypt/stargazers)

</div>

---

## What Is Neuron Encrypt?

Neuron Encrypt is a lightweight desktop file encryption tool built in Rust with a minimal dark-themed GUI. Drag a file in, enter a passphrase, and get an encrypted `.vx2` output. Your original file is never modified. Nothing leaves your machine.

Decrypting works the same way: drop the `.vx2` file, enter the original passphrase, and get your file back. Neuron Encrypt 2.0 automatically detects whether a file uses the legacy VAULTX02 format or the current VAULTX03 streaming format — no manual switching required.

---

## What's New in 2.0

- **Zero File Size Limit**: Encrypt files of any size. The streaming engine removed the old 2 GB cap — terabyte-scale files work fine.
- **Constant ~66 MB RAM**: Argon2id key derivation uses 64 MB (fixed), and 1 MB chunk streaming adds ~2 MB. Peak stays at ~66 MB regardless of file size.
- **High-Efficiency Batch Queue Processing**: Select multiple files, enter one passphrase, and the app processes them sequentially. If any file fails (disk full, permission denied), its temporary data is cleaned up and the queue moves to the next file — the app never crashes.
- **VAULTX03 Streaming Engine**: AES-256-GCM-SIV with STREAM BE32 construction. Files are encrypted in 1 MB chunks with per-chunk authentication tags, keeping memory flat and guaranteeing integrity.
- **I/O Governor for Large Files**: Prevents OS dirty page cache saturation during multi-gigabyte encryptions. Every ~100 MB the engine forces a disk flush and yields the thread, eliminating system-wide freezes.
- **Automatic Legacy Detection**: Magic bytes at offset 0 determine format version. VAULTX02 files are routed to the legacy decryptor transparently.
- **Windows Installer with .vx2 File Association**: NSIS-based setup installs to `Program Files`, creates Start Menu and Desktop shortcuts, and optionally associates `.vx2` files so double-clicking launches the app.
- **Fast**: ~500 MB/s encryption speed on modern hardware (tested on core i7 10thgen )
---

## Features

- **Single-file encrypt/decrypt** — drag-and-drop or file browser
- **Batch processing** — queue multiple files with a single passphrase
- **Password strength meter** — visual feedback (Weak / Fair / Strong / Elite)
- **Real-time progress bar** — per-file progress in batch mode with throttled UI updates
- **Batch cancellation** — cancel button aborts remaining files gracefully
- **I/O Governor** — periodic disk flush every 100 MB with thread yield. Prevents OS page cache saturation and system lag on multi-GB files.
- **Atomic file writes** — writes to a `.tmp` file first, then renames on success. No partial files on crash.
- **Secure memory handling** — keys wrapped in `Zeroizing<T>`, wiped from RAM on drop
- **Original files untouched** — output is always written to a new file beside the source
- **Cross-platform** — Windows (x64 MSVC), macOS (Universal Intel + Apple Silicon), Linux (x64)
- **No dependencies on runtime** — Windows builds use static CRT (`crt-static`). Just the binary.

---

## Technical Specifications

### Security Stack

| Component | Implementation | Detail |
|---|---|---|
| Language | **100% Safe Rust** | Zero `unsafe` blocks in the codebase — memory safety guaranteed by the borrow checker |
| Cipher | **AES-256-GCM-SIV** (RFC 8452) | Nonce-misuse resistant authenticated encryption |
| Key Derivation | **Argon2id** | Memory-hard KDF: 64 MiB, 3 iterations, 4 lanes, 32-byte output — GPU/ASIC resistant |
| Key Expansion | **HKDF-SHA512** | Cryptographic domain separation between V2 and V3 pipelines |
| Randomness | **OsRng** (OS CSPRNG) | Fresh 16-byte salt + 7-byte stream nonce generated per file |
| Memory Hygiene | **Zeroizing\<T\>** | Key material zeroed from RAM on scope exit |
| Streaming I/O | **EncryptorBE32 / DecryptorBE32** | Constant ~66 MB RAM regardless of file size |
| File Writes | **Atomic .tmp → rename** | No partial or corrupted files on crash |
| Batch Error Handling | **Per-file isolation** | Failed files cleaned up; remaining queue continues |
| I/O Governor | **sync_data + thread yield** | Flushes disk cache every ~100 MB; prevents OS dirty page freezes on large files |

### Cryptographic Parameters

```
Argon2id:
  Memory     : 64 MiB (65,536 KiB)
  Iterations : 3
  Lanes      : 4
  Output     : 32 bytes

AES-256-GCM-SIV:
  Key  : 256 bits
  Tag  : 128 bits (16 bytes) per chunk

Streaming:
  Chunk Size : 1 MB (1,048,576 bytes)
  Nonce      : 7 bytes (STREAM BE32 construction)
  Max File   : ~4 Exabytes (2^32 × 1 MB)
```

### VAULTX03 File Format

```
┌─────────────────────────────────────────────────────────────────┐
│                        VAULTX03 HEADER (31 bytes)               │
├──────────┬──────────┬──────────┬─────────────────────────────────┤
│ Magic    │ Salt     │ Nonce    │ Encrypted Data (streaming)      │
│ 8 bytes  │ 16 bytes │ 7 bytes  │ N × (1 MB chunk + 16B tag)      │
└──────────┴──────────┴──────────┴─────────────────────────────────┘
```

- **Magic**: Literal `b"VAULTX03"` — identifies the streaming format version
- **Salt**: 128-bit fresh `OsRng`-generated per file
- **Nonce**: 56-bit fresh `OsRng`-generated per file; `EncryptorBE32` appends an internal 5-byte counter/flag (12 bytes total internally)
- **Body**: Sequence of AES-256-GCM-SIV encrypted chunks. Intermediate chunks via `encrypt_next`, final chunk via `encrypt_last` to prevent truncation attacks.

### Legacy VAULTX02 Format

```
┌─────────────────────────────────────────────────────────────────┐
│                        VAULTX02 HEADER (52 bytes)               │
├──────────┬──────────┬──────────┬─────────────────────────────────┤
│ Magic    │ Salt     │ Nonce    │ Ciphertext + Auth Tag           │
│ 8 bytes  │ 32 bytes │ 12 bytes │ Entire file loaded into RAM     │
└──────────┴──────────┴──────────┴─────────────────────────────────┘
```

VAULTX02 files are automatically detected by their magic bytes and routed to the legacy decryptor. Supported for decryption only.

### Memory Profile

```
Key derivation (Argon2id) : ~64 MB  (fixed, 65,536 KiB blocks)
Stream buffer (1 MB chunk): ~2 MB   (read + ciphertext buffer)
───────────────────────────────────────────────────────────────
Peak RAM                  : ~66 MB  (constant, independent of file size)
```

---

## Security Audit Results

| Audit | Score | Verdict |
|---|---|---|
| Checklist Audit | 28/28 PASS | READY |
| Deep Security Audit | 7/7 PASS | READY |

Known limitations (by design):

- Passphrase cannot be recovered — no backdoors exist
- Encrypted file size reveals original file size (± 31-byte header + 16-byte tag per 1 MB chunk)
- egui text buffer may hold transient password copies during typing (inherent GUI framework limitation)

---

## How To Use

### Encrypting a File

1. Open Neuron Encrypt
2. Drag your file onto the window, or click **Browse file**
3. Enter a passphrase (use the strength meter as a guide)
4. Confirm the passphrase
5. Click **ENCRYPT**
6. Output saved as `yourfile.ext.vx2` beside the original — the source is untouched

### Decrypting a File

1. Drag a `.vx2` file onto the window, or click **Browse file**
2. Enter the passphrase used during encryption
3. Click **DECRYPT**
4. Original file restored beside the encrypted copy

### Batch Processing

1. Click **Batch upload** on the main screen
2. Select multiple files in the dialog
3. The app auto-detects mode: all `.vx2` → Decrypt, all others → Encrypt
4. Enter one passphrase for all files
5. Click **Encrypt All Files** or **Decrypt All Files**
6. Review the per-file results summary (success/failure) when complete
7. Failed files do not crash the batch — the queue continues

### Password Strength Guide

```
WEAK   → do not use
FAIR   → acceptable for low-sensitivity files
STRONG → recommended
ELITE  → maximum security
```

### CLI Usage

Neuron Encrypt also ships with a full-featured command-line interface — `neuron-encrypt-cli` — for terminal users, shell scripts, CI/CD pipelines, and automation.

#### Build the CLI

```bash
git clone https://github.com/darkmaster0345/Neuron-Encrypt.git
cd Neuron-Encrypt/neuron-encrypt

# Build CLI binary only (skip GUI dependencies)
cargo build --release --bin neuron-encrypt-cli
```

Binary locations:
```
Linux   : target/release/neuron-encrypt-cli
macOS   : target/release/neuron-encrypt-cli
Windows : target/release/neuron-encrypt-cli.exe
```

Install globally (optional):
```bash
cp target/release/neuron-encrypt-cli /usr/local/bin/
# or
cargo install --path . --bin neuron-encrypt-cli
```

#### Encrypt a File

```bash
# Interactive (prompts for passphrase)
neuron-encrypt-cli encrypt -i secret.pdf

# With environment variable
NEURON_PASSWORD='MyStr0ngP@ssw0rd!' neuron-encrypt-cli encrypt -i secret.pdf

# Custom output path
neuron-encrypt-cli encrypt -i secret.pdf -o vault/encrypted.vx2

# Passphrase from file (best for scripts/CI)
neuron-encrypt-cli encrypt -i secret.pdf --password-file /run/secrets/key
```

Output: `secret.pdf.vx2` — the original file is **never modified**.

#### Decrypt a File

```bash
# Interactive
neuron-encrypt-cli decrypt -i secret.pdf.vx2

# With environment variable
NEURON_PASSWORD='MyStr0ngP@ssw0rd!' neuron-encrypt-cli decrypt -i secret.pdf.vx2

# Custom output path
neuron-encrypt-cli decrypt -i secret.pdf.vx2 -o recovered.pdf
```

The `.vx2` extension is automatically stripped — output becomes `secret.pdf`.

The CLI auto-detects VAULTX03 (streaming) and VAULTX02 (legacy) formats. No manual switching required.

#### Global Options

| Flag | Description |
|------|-------------|
| `-q, --quiet` | Suppress all non-error output |
| `--no-progress` | Disable progress bar |
| `--json` | Emit structured JSON output for automation |
| `--password-file <PATH>` | Read passphrase from a file |
| `-F, --force` | Overwrite existing output files |
| `--completions <SHELL>` | Generate shell completions (bash, zsh, fish, powershell, elvish) |
| `-V, --version` | Print version |
| `-h, --help` | Print help with examples |

#### Passphrase Methods

The CLI supports three ways to provide a passphrase (minimum 8 characters):

| Method | Example | Best For |
|--------|---------|----------|
| **Interactive prompt** | `neuron-encrypt-cli encrypt -i file.txt` | Everyday terminal use |
| **Environment variable** | `NEURON_PASSWORD='pass' neuron-encrypt-cli encrypt -i file.txt` | Quick scripting |
| **Password file** | `neuron-encrypt-cli encrypt -i file.txt --password-file key.txt` | CI/CD, secrets managers |

Priority order: `--password-file` → `NEURON_PASSWORD` → interactive prompt.

#### JSON Output

The `--json` flag produces machine-readable output for scripting and CI/CD:

```bash
neuron-encrypt-cli encrypt -i secret.pdf --json
```

Success:
```json
{"status":"success","output_path":"secret.pdf.vx2","bytes_processed":24576,"duration_ms":152,"sha256":"a1b2c3d4...","error":null}
```

Error:
```json
{"status":"error","output_path":null,"bytes_processed":null,"duration_ms":98,"sha256":null,"error":"Wrong passphrase or corrupted file."}
```

Fields:

| Field | Description |
|-------|-------------|
| `status` | `"success"` or `"error"` |
| `output_path` | Absolute path to the output file |
| `bytes_processed` | Size of the source file in bytes |
| `duration_ms` | Wall-clock time in milliseconds |
| `sha256` | SHA-256 hash of the original (encrypt) or decrypted (decrypt) file |
| `error` | Error message, or `null` on success |

#### Piping & Streams

```bash
# Encrypt to stdout → pipe anywhere
neuron-encrypt-cli encrypt -i secret.pdf -o - --no-progress > encrypted.vx2

# Encrypt → pipe over SSH
neuron-encrypt-cli encrypt -i secret.pdf -o - | ssh user@host 'cat > secret.pdf.vx2'

# Decrypt from stdin
cat encrypted.vx2 | neuron-encrypt-cli decrypt -i - -o recovered.pdf

# Decrypt from stdin → pipe to another tool
cat backup.tar.gz.vx2 | neuron-encrypt-cli decrypt -i - -o - | tar -xz
```

> ⚠️ **Encryption from stdin** (`-i -`) is **not supported** — the streaming engine requires a seekable file to handle chunk boundaries. File input is required for encryption. **Decryption from stdin** works fine.

#### Exit Codes

| Code | Meaning | When |
|------|---------|------|
| `0` | **Success** | Operation completed successfully |
| `1` | **Runtime Error** | I/O error, disk full, permission denied |
| `2` | **Bad Input** | File not found, passphrase too short, output already exists |
| `3` | **Wrong Password** | Incorrect passphrase or corrupted file |

Use in scripts:
```bash
neuron-encrypt-cli decrypt -i backup.vx2 -o backup.tar.gz
case $? in
  0) echo "Decryption successful" ;;
  3) echo "Wrong password!"; exit 1 ;;
  *) echo "Unexpected error"; exit 1 ;;
esac
```

#### Shell Completions

```bash
# Bash
neuron-encrypt-cli --completions bash > /etc/bash_completion.d/neuron-encrypt-cli

# Zsh
neuron-encrypt-cli --completions zsh > ~/.zfunc/_neuron-encrypt-cli

# Fish
neuron-encrypt-cli --completions fish > ~/.config/fish/completions/neuron-encrypt-cli.fish

# PowerShell
neuron-encrypt-cli --completions powershell > neuron-encrypt-cli.ps1
```

#### CLI Examples

```bash
# Encrypt a file (interactive passphrase)
neuron-encrypt-cli encrypt -i secret.pdf

# Encrypt with custom output and JSON result
neuron-encrypt-cli encrypt -i secret.pdf -o vault/secret.vx2 --json

# Decrypt to a specific path
neuron-encrypt-cli decrypt -i secret.vx2 -o recovered.pdf

# Batch encrypt all .docx files
export NEURON_PASSWORD='SecureBatchP@ss!'
for f in *.docx; do
  neuron-encrypt-cli encrypt -i "$f" --force
done

# CI/CD: encrypt with secrets manager
neuron-encrypt-cli encrypt -i artifact.zip --json \
  --password-file /secrets/encrypt-key | tee result.json

# Verify integrity: compare SHA-256 from encrypt and decrypt
neuron-encrypt-cli encrypt -i important.pdf    # note the SHA-256
neuron-encrypt-cli decrypt -i important.pdf.vx2 # SHA-256 should match

# Quiet mode (scripts)
neuron-encrypt-cli encrypt -i secret.pdf -q

# Pipe encrypted backup over SSH
neuron-encrypt-cli encrypt -i backup.tar.gz -o - | ssh user@host 'cat > backup.vx2'
```

---

## Windows Installer

Neuron Encrypt ships with an NSIS-based installer for Windows. Download `NeuronEncrypt-Windows-x64-Setup.exe` from the releases page.

**What the installer does:**

- Installs to `C:\Program Files\NeuronEncrypt`
- Creates Start Menu shortcuts (app + uninstaller)
- Optional Desktop shortcut
- Optional `.vx2` file association (double-click encrypted files to launch the app)
- Registers itself in Windows Add/Remove Programs
- On reinstall: detects existing install, kills any running instance, and overwrites the binary
- Clean uninstall removes all installer-added files, shortcuts, and registry keys — your `.vx2` files are never deleted

**Optional installer components** (selectable during setup):

- **Core Application** — always installed (required)
- **Desktop Shortcut** — optional
- **Associate .vx2 files** — optional

---

## Build From Source

**Requirements:** Rust 1.70+ from [rustup.rs](https://rustup.rs) and Git.

```bash
git clone https://github.com/darkmaster0345/Neuron-Encrypt.git
cd Neuron-Encrypt/neuron-encrypt
cargo build --release
```

Binary locations:
```
Windows : target\release\neuron-encrypt.exe
macOS   : target/release/neuron-encrypt
Linux   : target/release/neuron-encrypt
```

CLI binary locations:
```
Windows : target\release\neuron-encrypt-cli.exe
macOS   : target/release/neuron-encrypt-cli
Linux   : target/release/neuron-encrypt-cli
```

Windows users can also double-click `build.bat` instead.

### Building the Windows Installer

```powershell
# 1. Build the release binary
cd neuron-encrypt
cargo build --release --target x86_64-pc-windows-msvc

# 2. Install NSIS (if not already installed)
choco install nsis --yes

# 3. Build the installer from repo root
cd ..
makensis installer/neuron-encrypt.nsi
```

Output: `installer/NeuronEncrypt-Windows-x64-Setup.exe`

### Building macOS Universal Binary

```bash
cd neuron-encrypt
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create \
  target/x86_64-apple-darwin/release/neuron-encrypt \
  target/aarch64-apple-darwin/release/neuron-encrypt \
  -output NeuronEncrypt-macOS
```

---

## CI/CD

The repository uses GitHub Actions (`release.yml`) for automated builds on every push to `main`:

1. **Security audit** — `cargo audit` with medium severity threshold
2. **Linting** — `cargo fmt --check` + `cargo clippy -- -D warnings`
3. **Tests** — `cargo test --all` with `RUST_BACKTRACE=1`
4. **Builds** — parallel builds for Linux (x64), Windows (x64 MSVC), and macOS (Universal)
5. **Windows installer** — NSIS packaging after the Windows build completes

Artifacts are uploaded with 30-day retention.

---

## Architecture

```
┌─────────────────────────────────────┐
│         GUI Layer (egui/eframe)     │
│  Dark theme · JetBrains Mono font   │
│  Drag-and-drop · Progress bars      │
├─────────────────────────────────────┤
│       Application Controller        │
│  State machine: drop → configure →  │
│  processing → success/failure       │
├─────────────────────────────────────┤
│     Background Crypto Thread        │
│   std::thread + crossbeam channel   │
├─────────────────────────────────────┤
│        Core Crypto Library          │
│  AES-256-GCM-SIV · Argon2id · HKDF  │
│  Zeroizing · OsRng · Atomic I/O     │
│  VAULTX03 BE32 streaming engine     │
└─────────────────────────────────────┘
```

The GUI thread never handles crypto. All encryption/decryption runs in a background thread and reports progress through a `crossbeam_channel`. The `ThrottledReporter` suppresses redundant UI updates, firing only on meaningful progress deltas or elapsed time thresholds.

---

## Android Version

Neuron Encrypt is also available as a native Android application. It packages the core Rust cryptographic engine via JNI to guarantee the same speed and safety profile on mobile.

- **Offline-First Security**: Zero network permissions requested.
- **Modern SAF API**: Works natively via Android Storage Access Framework without asking for system-wide files storage permission.
- **Material3 Custom Dark Theme**: Near-black design with active passphrase feedback, matching the desktop experience.

For full setup, JNI architecture details, and building instructions, see the [Android README](android/README.md).

---

## Project Structure

```
├── android/             — Native Android app & JNI shared library
│   ├── app/             — Kotlin/Compose Android app
│   ├── neuron-encrypt-jni/ — Rust JNI wrapper crate
│   ├── build-rust.sh    — Cross-compiles JNI targets
│   └── README.md        — Android build & architecture details
├── neuron-encrypt/
│   ├── src/
│   │   ├── main.rs      — entry point, window + theme setup
│   │   ├── gui.rs       — full UI: screens, widgets, state machine
│   │   ├── crypto.rs    — encryption, decryption, streaming, tests
│   │   ├── error.rs     — CryptoError enum via thiserror
│   │   ├── lib.rs       — library crate root
│   │   └── bin/
│   │       └── cli.rs   — CLI binary: encrypt/decrypt from the terminal
│   ├── assets/
│   │   ├── icon.ico
│   │   └── fonts/
│   │       └── JetBrainsMono-Regular.ttf
│   ├── .cargo/
│   │   └── config.toml  — static CRT for Windows MSVC/GNU
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── build.bat        — one-click Windows release build
├── installer/
│   ├── neuron-encrypt.nsi  — NSIS installer script
│   └── README.md           — installer build instructions
├── .github/workflows/
│   ├── release.yml         — CI/CD pipeline
│   └── android.yml         — Android CI pipeline
├── LICENSE              — GPLv3
└── README.md            — this file
```

---

## Security

### Cipher Suite

| Layer | Algorithm | Parameters |
|-------|-----------|------------|
| Encryption (V3) | XChaCha20-Poly1305 (streaming AEAD) | 192-bit nonce, 128-bit tag, 1 MB chunks |
| Encryption (V2 legacy) | AES-256-GCM-SIV | 96-bit nonce, 128-bit tag |
| Key Derivation | Argon2id | m=65536 (64 MB), t=3 iterations, p=1 lane |
| Key Expansion | HKDF-SHA512 | Two separate 256-bit subkeys: enc + nonce |
| Integrity | Poly1305 / GHASH | Per-chunk authentication; any bit-flip fails |
| RNG | OS CSPRNG (`OsRng`) | `getrandom` crate; no `rand` global state |

### Key Derivation Details

```
passphrase + 32-byte OsRng salt
       │
    Argon2id (64 MB, 3 passes)
       │
    64-byte master key
       ├─ HKDF-Extract+Expand → 32-byte encryption key
       └─ HKDF-Extract+Expand → 32-byte nonce seed
```

The salt is stored in plaintext in the file header. The master key and all intermediate key material are stored in `Zeroizing<Vec<u8>>` which zeroes memory on drop.

### Threat Model

**Protected against:**
- Offline brute-force: Argon2id with 64 MB memory cost makes GPU attacks expensive.
- Ciphertext tampering: Every 1 MB chunk has an independent Poly1305 authentication tag. Corruption of any byte fails decryption of that chunk.
- Nonce reuse: Each encryption generates a fresh 24-byte random nonce via `OsRng`. With 192-bit nonces, collision probability is negligible even at billions of files.
- Sensitive data in memory: Plaintext buffers and keys are wrapped in `Zeroizing<>`, zeroed on drop. Passwords are cleared from UI state immediately after the crypto thread starts.

**Not protected against:**
- Adversaries with access to RAM (cold-boot, DMA attacks) — no in-memory encryption.
- Keyloggers or malicious screen readers capturing the passphrase during input.
- Weak passphrases: The strength meter is advisory only. A passphrase scoring "Weak" can still be used.
- SSD wear-levelling: The source wipe function cannot guarantee physical erasure on flash storage, CoW filesystems (Btrfs, APFS, ZFS), or NTFS volumes with shadow copies enabled.
- Metadata: File names, sizes, and access timestamps of the original file are not encrypted.

### Passphrase Requirements

- Minimum 8 characters enforced at the API level (`MIN_PASSWORD_LEN`).
- The UI strength meter scores: length ≥ 8/12/16, uppercase, digit, symbol presence.
- No maximum length — passphrases are hashed through Argon2id regardless of length.

### Reporting Vulnerabilities

Do **not** open a public GitHub issue for security vulnerabilities. Use the [GitHub Security Advisory](https://github.com/darkmaster0345/Neuron-Encrypt/security/advisories) to report privately.

---

## Known Limitations

- Passphrase-only authentication — no key file support
- Encrypted files are slightly larger than originals (~31-byte header + 16-byte auth tag per 1 MB chunk)
- Files encrypted with V3 (VAULTX03) cannot be decrypted by older versions of the app
- Secure wipe uses 3-pass random overwrite + rename, which is not cryptographically guaranteed on SSDs, APFS, Btrfs, ZFS, or NTFS with shadow copies
- egui text buffer may hold transient password copies during typing (inherent GUI framework limitation)

---

## Contributing

Bug reports and pull requests are welcome. For security vulnerabilities, open a private [GitHub Security Advisory](https://github.com/darkmaster0345/Neuron-Encrypt/security/advisories) rather than a public issue.

```bash
rustup component add rustfmt clippy
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

---

## License

GPLv3 — Copyright (c) 2024–2026 Ubaid ur Rehman. See LICENSE for full text.

---

## Acknowledgments

- RustCrypto team — aes-gcm-siv, argon2, hkdf crates
- egui/eframe team — immediate-mode GUI framework
- JetBrains — JetBrains Mono font
- NSIS community — Nullsoft Scriptable Install System
- Rust community — tooling and ecosystem

---

<div align="center">
Made with Rust
</div>
