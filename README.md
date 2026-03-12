# NEURON ENCRYPT

<div align="center">

![Neuron Encrypt](https://img.shields.io/badge/Neuron%20Encrypt-v1.0.0-blue?style=for-the-badge&logo=rust)
![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=for-the-badge)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange?style=for-the-badge&logo=rust)

**Military-Grade File Encryption**
AES-256-GCM-SIV · Argon2id · HKDF-SHA512
*Memory-Safe Rust Implementation*

[![GitHub release](https://img.shields.io/github/v/release/darkmaster0345/Neuron-Encrypt?style=for-the-badge)](https://github.com/darkmaster0345/Neuron-Encrypt/releases)
[![GitHub stars](https://img.shields.io/github/stars/darkmaster0345/Neuron-Encrypt?style=for-the-badge&logo=github)](https://github.com/darkmaster0345/Neuron-Encrypt/stargazers)

</div>

---

## What Is Neuron Encrypt?

Neuron Encrypt is a desktop file encryption application
built entirely in Rust. It encrypts any file using
AES-256-GCM-SIV — the same cipher approved by the NSA
for TOP SECRET information — with a password you choose.

No accounts. No servers. No internet connection.
Everything happens locally on your machine.

---

## Security Architecture

| Component | Choice | Why |
|---|---|---|
| Cipher | AES-256-GCM-SIV (RFC 8452) | Nonce-misuse resistant authenticated encryption |
| Key Derivation | Argon2id | Memory-hard, GPU/ASIC resistant |
| Key Expansion | HKDF-SHA512 | Cryptographic domain separation |
| Randomness | OS CSPRNG (OsRng) | Cryptographically secure |
| Memory Safety | Zeroizing\<T\> | Keys zeroed from RAM on drop |
| File Writes | Atomic .tmp → rename | No partial files on crash |

### Security Parameters

```
Argon2id:
  Memory     : 64 MiB (65,536 KiB)
  Iterations : 3
  Lanes      : 4
  Output     : 32 bytes

AES-256-GCM-SIV:
  Key  : 256 bits
  Nonce: 96 bits (12 bytes) — fresh per encryption
  Tag  : 128 bits (16 bytes)
```

### Encrypted File Format (.vx2)

```
Offset   Length   Content
0        8        Magic: "VAULTX02"
8        32       Argon2id salt
40       12       AES-GCM-SIV nonce
52       N+16     Ciphertext + auth tag
```

---

## Security Audit Results

The codebase passed two independent audits:

| Audit | Score | Verdict |
|---|---|---|
| Checklist Audit | 28/28 PASS | READY |
| Deep Security Audit | 7/7 PASS | READY |

Known limitations (by design):
- Password cannot be recovered — no backdoors exist
- Encrypted file size reveals original size (± 68 bytes)
- egui text buffer may hold transient password copies
  during typing (inherent GUI framework limitation)

---

## Download

<div align="center">

| Platform | Download |
|---|---|
| Windows (x64) | [NeuronEncrypt-Windows-x64.exe](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |
| macOS (Intel) | [NeuronEncrypt-macOS-Intel](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |
| macOS (Apple Silicon) | [NeuronEncrypt-macOS-ARM](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |
| Linux (x64) | [NeuronEncrypt-Linux-x64](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |

</div>

Always verify your download:
```bash
# Linux / macOS
sha256sum NeuronEncrypt-Linux-x64

# Windows (PowerShell)
Get-FileHash NeuronEncrypt-Windows-x64.exe -Algorithm SHA256
```
Compare the hash against the checksums listed on the
releases page.

---

## Build From Source

### Requirements
- Rust 1.70+ — install from https://rustup.rs
- Git

### Steps

```bash
# Clone
git clone https://github.com/darkmaster0345/Neuron-Encrypt.git
cd Neuron-Encrypt/neuron-encrypt

# Build release binary
cargo build --release

# Binary location:
# Windows : target\release\neuron-encrypt.exe
# macOS   : target/release/neuron-encrypt
# Linux   : target/release/neuron-encrypt
```

### Windows Shortcut

Double-click `build.bat` — it installs dependencies
and builds the .exe automatically.

---

## How To Use

### Encrypting a File

1. Open Neuron Encrypt
2. Select **ENCRYPT MODE** (left pill button)
3. Click **BROWSE** and select your file
4. Enter a strong passphrase
5. Click **ENCRYPT →**
6. Encrypted file saved as `yourfile.ext.vx2`
7. Your original file is untouched

### Decrypting a File

1. Select **DECRYPT MODE** (right pill button)
2. Click **BROWSE** and select the `.vx2` file
3. Enter the same passphrase used to encrypt
4. Click **DECRYPT →**
5. Original file restored without `.vx2` extension

### Password Tips

```
Minimum recommended : 12 characters
Strong example      : correct-horse-battery-staple-7
Use the strength meter in the app as a guide:
  WEAK   → do not use
  FAIR   → acceptable for low-sensitivity files
  STRONG → recommended
  ELITE  → maximum security
```

---

## ⚠️ Critical Warning

**If you forget your password your file is permanently
unrecoverable. There is no reset, no backdoor, and no
way to help you. Write your password down and store it
somewhere safe before encrypting important files.**

---

## Project Structure

```
neuron-encrypt/
├── src/
│   ├── main.rs      — entry point, window setup
│   ├── gui.rs       — all UI code
│   ├── crypto.rs    — all encryption logic
│   └── error.rs     — error types
├── assets/
│   └── fonts/
│       └── JetBrainsMono-Regular.ttf
├── Cargo.toml
├── Cargo.lock
└── build.bat        — Windows build script
```

---

## Architecture

```
┌─────────────────────────────────────┐
│         GUI Layer (egui)            │
├─────────────────────────────────────┤
│       Application Controller        │
├─────────────────────────────────────┤
│     Background Crypto Thread        │
│   std::thread + mpsc channel        │
├─────────────────────────────────────┤
│        Core Crypto Library          │
│  AES-GCM-SIV · Argon2id · HKDF     │
│  Zeroizing · OsRng · Atomic I/O     │
└─────────────────────────────────────┘
```

The GUI thread never touches crypto operations.
Crypto runs in a background thread and reports
progress via mpsc channel.

---

## Known Limitations

- **No batch encryption** — one file at a time
- **No CLI** — GUI only in this version
- **2 GB file limit** — by design to prevent OOM
- **No key files** — password only
- **No compression** — encrypted files are
  slightly larger than originals (+ 68 bytes)

---

## Contributing

Bug reports and pull requests are welcome.

For security vulnerabilities, open a private
GitHub Security Advisory rather than a
public issue.

```bash
# Development setup
rustup component add rustfmt clippy
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

---

## License

MIT License — Copyright (c) 2024–2026 Ubaid ur Rehman

See LICENSE file for full text.

---

## Acknowledgments

- RustCrypto team — for aes-gcm-siv, argon2, hkdf crates
- egui/eframe team — for the GUI framework
- Rust community — for the tooling and ecosystem

---

<div align="center">

Made with ❤️ and Rust

**[⬆ Back to Top](#neuron-encrypt)**

</div>
```

---

## What I Changed From Yours

```
✅ Removed CLI section  — it does not exist
✅ Removed Docker       — it does not exist
✅ Removed fake benchmarks
✅ Removed broken screenshot links
✅ Removed nonexistent email address
✅ Added real audit results
✅ Added honest known limitations
✅ Kept everything that was accurate
✅ Made the warning more prominent
```
