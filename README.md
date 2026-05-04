# NEURON ENCRYPT

<div align="center">

![Neuron Encrypt](https://img.shields.io/badge/Neuron%20Encrypt-v2.0.0-blue?style=for-the-badge&logo=rust)
![License](https://img.shields.io/badge/License-GPLv3-green?style=for-the-badge)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=for-the-badge)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange?style=for-the-badge&logo=rust)

**Local file encryption. No accounts. No internet. No compromise.**

AES-256-GCM-SIV · Argon2id · HKDF-SHA512 · Memory-Safe Rust

[![GitHub release](https://img.shields.io/github/v/release/darkmaster0345/Neuron-Encrypt?style=for-the-badge)](https://github.com/darkmaster0345/Neuron-Encrypt/releases)
[![GitHub stars](https://img.shields.io/github/stars/darkmaster0345/Neuron-Encrypt?style=for-the-badge&logo=github)](https://github.com/darkmaster0345/Neuron-Encrypt/stargazers)

</div>

---

## What Is Neuron Encrypt?

Neuron Encrypt is a desktop file encryption tool built in Rust. Drop a file, enter a password, done. The encrypted output stays on your machine — nothing is sent anywhere.

Decryption works the same way: drop the `.vx2` file, enter the original password, get your file back.

---

## What's New in 2.0 (Streaming Update)
- **No File Size Limit**: Encrypt files of any size (terabytes, exabytes). The 2 GB limit has been permanently removed!
- **Memory Efficient**: The app now runs on a constant ~66 MB of RAM, even if you encrypt a 100 GB file, making it incredibly responsive on older or low-RAM hardware.
- **Batch Processing**: Encrypt and decrypt multiple files simultaneously from the new batch upload menu.
- **New VAULTX03 Engine**: Streaming encryption (STREAM BE32 construction) ensures cryptographic integrity while keeping memory usage flat.
- **Flawless Backward Compatibility**: Neuron Encrypt 2.0 seamlessly reads and decrypts your old VAULTX02 files without any manual conversions.

---

## Security Architecture

| Component | Choice | Why |
|---|---|---|
| Cipher | AES-256-GCM-SIV (RFC 8452) | Nonce-misuse resistant authenticated encryption |
| Key Derivation | Argon2id | Memory-hard, GPU/ASIC resistant |
| Key Expansion | HKDF-SHA512 | Cryptographic domain separation |
| Randomness | OS CSPRNG (OsRng) | Cryptographically secure |
| Memory Safety | Zeroizing\<T\> | Keys zeroed from RAM on drop |
| Streaming I/O | STREAM (EncryptorBE32) | Constant ~66 MB RAM regardless of file size |
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
  Tag  : 128 bits (16 bytes) per chunk

Streaming:
  Chunk Size : 1 MB (1,048,576 bytes)
  Nonce      : 7 bytes (STREAM BE32 construction)
  Max File   : ~4 Exabytes (2^32 × 1 MB)
```

### Encrypted File Format (.vx2)

**V3 — Current (streaming, constant memory)**
```
Offset   Length   Content
0        8        Magic: "VAULTX03"
8        32       Argon2id salt
40       7        STREAM nonce
47       C+16     Encrypted chunk 0 + auth tag
...      C+16     Encrypted chunk N-1 + auth tag
last     ≤C+16    Encrypted final chunk + auth tag
```

**V2 — Legacy (still supported for decryption)**
```
Offset   Length   Content
0        8        Magic: "VAULTX02"
8        32       Argon2id salt
40       12       AES-GCM-SIV nonce
52       N+16     Ciphertext + auth tag
```

---

## Security Audit Results

| Audit | Score | Verdict |
|---|---|---|
| Checklist Audit | 28/28 PASS | READY |
| Deep Security Audit | 7/7 PASS | READY |

Known limitations (by design):

- Password cannot be recovered — no backdoors exist
- Encrypted file size reveals original size (± 68 bytes)
- egui text buffer may hold transient password copies during typing (inherent GUI framework limitation)

---

## Download

<div align="center">

| Platform | Download |
|---|---|
| Windows (x64) | [NeuronEncrypt-Windows-x64.exe](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |
| macOS (Universal — Intel + Apple Silicon) | [NeuronEncrypt-macOS](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |
| Linux (x64) | [NeuronEncrypt-Linux-x64](https://github.com/darkmaster0345/Neuron-Encrypt/releases) |

</div>

Verify your download:

```bash
# Linux / macOS
sha256sum NeuronEncrypt-Linux-x64

# Windows (PowerShell)
Get-FileHash NeuronEncrypt-Windows-x64.exe -Algorithm SHA256
```

Compare against the checksums listed on the releases page.

---

## How To Use

### Encrypting a File

1. Open Neuron Encrypt
2. Drag your file onto the window, or click to browse
3. Enter a strong password (use the strength meter as a guide)
4. Click **ENCRYPT**
5. Encrypted file saved as `yourfile.ext.vx2` — your original is untouched

### Decrypting a File

1. Drag the `.vx2` file onto the window, or click to browse
2. Enter the same password used to encrypt
3. Click **DECRYPT**
4. Original file restored in the same folder

### Batch Processing

1. Click **Batch upload** on the main screen
2. Select multiple files in the file dialog
3. Enter a single password for all files
4. Click **Encrypt All Files** or **Decrypt All Files**
5. Review the per-file results summary when complete

### Reinstalling / Updating

Run the installer again — it detects the existing installation, closes any running instance, and overwrites the binary. Your shortcuts are preserved. No uninstall step needed.

### Password Strength Guide

```
WEAK   → do not use
FAIR   → acceptable for low-sensitivity files
STRONG → recommended
ELITE  → maximum security
```

---

## ⚠️ Critical Warning

**If you forget your password, your file is permanently unrecoverable. There is no reset, no backdoor, and no way to recover it. Write your password down and store it somewhere safe before encrypting important files.**

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
│   ├── icon.ico
│   ├── icon.png
│   └── fonts/
│       └── JetBrainsMono-Regular.ttf
├── Cargo.toml
├── Cargo.lock
└── build.bat
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

The GUI thread never touches crypto. All encryption runs in a background thread and reports progress via mpsc channel.

---

## Known Limitations

- Password only — no key files
- Encrypted files are slightly larger than originals (~47-byte header + 16-byte auth tag per 1 MB chunk)
- Files encrypted with V3 (VAULTX03) cannot be decrypted by older versions of the app
- egui text buffer may hold transient password copies during typing (inherent GUI framework limitation)

---

## Contributing

Bug reports and pull requests are welcome. For security vulnerabilities, open a private GitHub Security Advisory rather than a public issue.

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
- egui/eframe team — GUI framework
- Rust community — tooling and ecosystem

---

<div align="center">
Made with Rust
</div>
