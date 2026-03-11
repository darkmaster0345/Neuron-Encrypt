# NEURON ENCRYPT - SECURE FILE VAULT

AES-256-GCM-SIV · ARGON2ID · HKDF-SHA512  
Version 1.3 - OPEN SOURCE EDITION

## ⚠️ IMPORTANT SECURITY NOTICE

**This is OPEN SOURCE software for maximum security and transparency.**

✅ **ALWAYS verify the source code before use**  
✅ **NEVER trust compiled executables for crypto applications**  
✅ **REVIEW the cryptographic implementation**  
✅ **USE strong passwords (12+ characters recommended)**

**Why Open Source?**  
Closed-source encryption tools can hide backdoors and vulnerabilities. With Neuron Encrypt, you can verify every line of code and build it yourself.

## Overview

Neuron Encrypt is a transparent, open-source file encryption application featuring military-grade cryptographic security and a cyberpunk-themed user interface.

## New Features (Version 1.3)

### Phase 1: Core Usability Improvements
- **Drag & Drop Support** - Simply drag files onto the window to select them
- **Keyboard Shortcuts** - Work faster with comprehensive hotkeys
- **Recent Files History** - Quick access to your last 10 encrypted/decrypted files

## Security Features

- **Encryption**: AES-256-GCM-SIV (RFC 8452) - Misuse-resistant authenticated encryption
- **Key Derivation**: Argon2id (memory-hard) → HKDF-SHA512 for final key
- **Security Parameters**: 
  - Argon2id: iterations=3, memory_cost=65536 (64MB), lanes=4
  - Fresh random salt (32 bytes) and nonce (12 bytes) per encryption
- **File Format**: Custom .vx2 format with structured header and authentication
- **Validation**: Output file size verification with automatic cleanup of partial files
- **TPM Binding (Optional)**: When a TPM 2.0 chip is present, Neuron Encrypt can seal the derived AES key via `trustcore-tpm` and emit a `.vx2.tpm` blob so the ciphertext only decrypts on that same hardware state.

## Installation & Usage

### Open Source Installation (RECOMMENDED)

#### Method 1: Direct Python Execution
```bash
# 1. Install Python 3.8+ from python.org
# 2. Install dependencies
pip install -r requirements.txt

# 3. Run Neuron Encrypt
python vaultx.py
```

#### Method 2: Verification Script
```bash
# 1. Verify source integrity
python verify_installation.py

# 2. Install if verification passes
python install_neuron_encrypt.py

# 3. Run Neuron Encrypt
python vaultx.py
```

### ⚠️ WARNING: Executable Distribution

**We do NOT recommend distributing compiled executables for cryptographic applications.**

If you must build an executable:
```bash
# Build executable (NOT RECOMMENDED for distribution)
python -m PyInstaller --onefile --windowed --name VaultX vaultx.py
```

**Risks of executable distribution:**
- Source code can be extracted in 30 seconds
- Backdoors can be injected maliciously
- Users cannot verify authenticity
- Supply chain attacks possible

### System Requirements
- **Python 3.8+**
- **Windows 10/11** (x64)
- **Dependencies**: cryptography>=42.0.0, tkinterdnd2>=0.4.3

## User Interface

### Enhanced Features
- **Mode Selection**: Switch between ENCRYPT and DECRYPT modes
- **File Selection**: Browse, drag & drop, or select from recent files
- **Password Input**: Real-time password strength meter
- **System Log**: All operations and errors displayed here (no popups)
- **Progress Tracking**: Visual progress bar during operations
- **Animated Background**: Cyberpunk hex grid animation

## How to Use

### Method 1: Drag & Drop (Recommended)
1. Drag any file onto the Neuron Encrypt window
2. Enter a strong password
3. Press Enter or click ENCRYPT/DECRYPT button

### Method 2: Browse
1. Click BROWSE button or press Ctrl+O
2. Select your file
3. Enter password and execute

### Method 3: Recent Files
1. Use the dropdown to select from recent files
2. Neuron Encrypt automatically switches to the correct mode
3. Enter password and execute

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| **Ctrl+E** | Switch to ENCRYPT mode |
| **Ctrl+D** | Switch to DECRYPT mode |
| **Ctrl+O** | Browse for file |
| **Enter** | Execute operation |
| **Escape** | Clear selection |
| **Ctrl+Q** | Quit application |
| **F1** | Show help |

## File Operations

### Encryption
- Select any file for encryption
- Enter a strong password (ELITE strength recommended)
- Encrypted file saved as `original.ext.vx2`
- Original file remains unchanged
- File automatically added to recent history

### Decryption
- Select `.vx2` files only
- Enter the exact password used for encryption
- Original file restored without extension
- Wrong passwords show clear error messages
- File automatically added to recent history

## Recent Files Management

- **Automatic**: All processed files are added to recent history
- **Smart**: Automatically switches mode based on file type
- **Persistent**: History saved between sessions
- **Limited**: Keeps only last 10 files for privacy
- **Clear**: Use CLEAR RECENT button to wipe history

## Important Security Notes

### ⚠️ CRITICAL WARNINGS

**LOSS OF PASSWORD = PERMANENT DATA LOSS**  
There is no password recovery mechanism. Store passwords securely.

**OPEN SOURCE SECURITY**  
- Always verify source code before use
- Never trust compiled executables for crypto
- Check for suspicious network calls
- Build from source yourself when possible

**PASSWORD SECURITY**  
- Use minimum 12 characters (recommended 20+)
- Include uppercase, lowercase, digits, special characters
- Avoid common passwords and patterns
- Use a password manager for storage

**FILE SECURITY**  
- Output files are saved alongside source files
- TPM-bound files also emit a `.vx2.tpm` sidecar; keep it with the encrypted payload and the original TPM, because losing either will make decryption impossible.
- All cryptographic operations run in background threads
- UI remains responsive during encryption/decryption
- Partial/failed encryptions are automatically cleaned up
- Recent files stored locally for convenience

### Security Verification Checklist

Before using Neuron Encrypt, verify:

✅ **Source Code Review**  
- Check `neuron_encrypt_secure.py` for suspicious code  
- Verify no network connections are made  
- Confirm cryptographic implementation is correct  
- Review key derivation parameters  

✅ **Installation Verification**  
- Run `python verify_installation.py`  
- Check file integrity hashes  
- Verify dependencies are legitimate  

✅ **Runtime Security**  
- Ensure no unexpected network traffic  
- Monitor for suspicious file access  
- Verify password strength requirements  

✅ **Cryptographic Verification**  
- Test with known vectors: `python run_tests.py`  
- Verify AES-256-GCM-SIV implementation  
- Check Argon2id parameters are correct

## Technical Specifications

- **Dependencies**: cryptography>=42.0.0, pyinstaller>=6.0.0, tkinterdnd2>=0.4.3, trustcore-tpm>=1.0.1
- **Platform**: Windows (x64)
- **Format**: Single executable, no installer required
- **UI Framework**: tkinter with custom styling
- **Threading**: Background daemon threads for crypto operations

## Validation Checklist

AESGCMSIV from cryptography.hazmat (not AESGCM)  
Argon2id memory_cost = 65536  
HKDF-SHA512 key derivation  
Exact header offsets: magic[0:8], salt[8:40], nonce[40:52], ciphertext[52:]  
Background threading implementation  
File size validation and cleanup  
Single .exe output (--onefile --windowed)  
Courier New font throughout  
Hex grid animation (≤200 cells)  
No console window  
Drag & drop support  
Keyboard shortcuts  
Recent files history  

## File Format Specification

```
Offset   Length   Content
──────   ──────   ──────────────────────────────────────────
0        8        Magic bytes: b'VAULTX02'
8        32       Argon2id salt (random, stored in plaintext)
40       12       AES-GCM-SIV nonce (random, stored in plaintext)
52       N+16     Ciphertext + GCM-SIV 16-byte authentication tag
```

Output file size formula: `source_size + 68 bytes` (8+32+12+16)

## License

See the [LICENSE](LICENSE) file for full license text.

MIT License

Copyright (c) 2024 Urooj

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
