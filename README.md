# NEURON ENCRYPT - SECURE FILE VAULT

AES-256-GCM-SIV · ARGON2ID · HKDF-SHA512  
Version 1.0 - RUST NATIVE EDITION

## ⚠️ IMPORTANT SECURITY NOTICE

**This is OPEN SOURCE software for maximum security and transparency.**

✅ **ALWAYS verify the source code before use**  
✅ **REVIEW the cryptographic implementation**  
✅ **USE strong passwords (12+ characters recommended)**

**Why Open Source?**  
Closed-source encryption tools can hide backdoors and vulnerabilities. With Neuron Encrypt, you can verify every line of Rust code and build it yourself.

## Overview

Neuron Encrypt is a transparent, open-source file encryption application featuring military-grade cryptographic security and a modern cyberpunk-themed GUI. It has been completely rewritten from Python into memory-safe Rust to eliminate OOM crashes, memory scrubbing leaks, and Python decompilation attacks.

## Core Advancements (Version 1.0 Rust Native)

- **Memory Safety**: Powered by the Rust compiler. Zeroizing memory allocations securely erase passwords and decrypted plaintext immediately after use.
- **Performance**: Up to 100x faster than previous conceptual implementations. Uses highly optimized AES-NI hardware instructions.
- **Cross-Platform**: Compiles to standalone native binaries for Windows, macOS (Universal), and Linux.
- **Atomic Operations**: Employs atomic write-and-rename patterns to protect against sudden power-loss and TOCTOU vulnerabilities.
- **Anti-Forensics**: Timestamps (creation, modified, accessed) of output payload files are stripped down to the OS epoch to prevent metadata timeline side channels.

## Security Features

- **Encryption**: AES-256-GCM-SIV (RFC 8452) - Misuse-resistant authenticated encryption
- **Key Derivation**: Argon2id (memory-hard) → HKDF-SHA512 for final key derivation
- **Security Parameters**: 
  - Argon2id: iterations=3, memory_cost=65536 (64MB), lanes=1 (sequential for predictable execution)
  - Fresh random salt (32 bytes) and nonce (12 bytes) per encryption derived from OS entropy ring
- **File Format**: Custom .vx2 format with structured header and authentication tag
- **Validation**: Strict size verification and constant-time execution paths inside AEAD verification loops

## Installation & Usage

### ⚙️ Automated GitHub CI/CD Releases (EASIEST)
Download the latest pre-compiled binaries from the **Releases** tab on this GitHub repository. Our GitHub Actions CI strictly builds the binaries directly from the open source payload. Available for Windows (`.exe`), Linux, and macOS.

### 🛠 Building From Source (RECOMMENDED)

You will need the [Rust Toolchain](https://rustup.rs/) installed.

```bash
# 1. Clone the repository
git clone https://github.com/darkmaster0345/Neuron-Encrypt.git
cd Neuron-Encrypt/neuron-encrypt

# 2. Compile for release (optimized for speed and stripped)
cargo build --release

# 3. Your executable is ready
# Windows: target/release/neuron-encrypt.exe
# MacOS/Linux: target/release/neuron-encrypt
```

### System Requirements
- **OS**: Windows 10/11 (x64), macOS 10.15+, or modern Linux
- **RAM**: Minimal overhead (Argon2id uses 64MB of working memory per operation)

## How to Use

### Encrypt a file
1. Open the application.
2. Ensure you are in **ENCRYPT** mode.
3. Drag & Drop a file onto the GUI or click **BROWSE...** to open the OS file picker.
4. Input your master password.
5. Click **EXECUTE**. A `.vx2` payload will be placed in the exact directory of the source file. 

### Decrypt a file
1. Open the application.
2. Toggle the switch to **DECRYPT** mode.
3. Drag & Drop your `.vx2` file onto the GUI or click **BROWSE...**.
4. Input the same complex master password.
5. Click **EXECUTE**. The `.vx2` extension is stripped and the deciphered original file drops securely to your drive.

## File Format Specification

```text
Offset   Length   Content
──────   ──────   ──────────────────────────────────────────
0        8        Magic bytes: b'VAULTX02'
8        32       Argon2id salt (random, stored in plaintext)
40       12       AES-GCM-SIV nonce (random, stored in plaintext)
52       N+16     Ciphertext + GCM-SIV 16-byte authentication tag
```

Output file size formula: `source_size + 68 bytes` (8+32+12+16)

## Security Limitations
- **Password Loss**: There is NO backdoor and NO password recovery mechanism.
- **Information Leaks**: Encrypted sizes leak original sizes minus 68 bytes. Output files retain the original extension string (`document.txt.vx2`). 
- **Memory Forensics**: Due to hardware operating system constraints involving graphics memory buffer rendering vectors, the egui text frame does not definitively zero out intermediate GUI allocations while the user is physically typing.

## License

MIT License

Copyright (c) 2024–2026 Ubaid ur Rehman

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
