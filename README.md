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

## Project Structure

```
├── neuron-encrypt/
│   ├── src/
│   │   ├── main.rs      — entry point, window + theme setup
│   │   ├── gui.rs       — full UI: screens, widgets, state machine
│   │   ├── crypto.rs    — encryption, decryption, streaming, tests
│   │   ├── error.rs     — CryptoError enum via thiserror
│   │   └── lib.rs       — library crate root
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
│   └── release.yml         — CI/CD pipeline
├── LICENSE              — GPLv3
└── README.md            — this file
```

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
