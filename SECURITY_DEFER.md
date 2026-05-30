[ARGON2ID_PARAMS] - Insufficient Argon2id memory cost
Severity: Critical
Location: neuron-encrypt/src/crypto.rs:114
Description: The current Argon2id memory cost is set to 64MB (m_cost=65536). Security guidelines recommend a minimum of 256MB (m_cost=262144) for hardened file encryption to resist specialized hardware-based brute-force attacks.
Why not fixed: no-touch zone
Suggested fix: Update Argon2id Params in derive_key and derive_key_v3 to use m_cost=262144.
First detected: 2026-05-09
Last attempted: 2026-05-30
Attempt count: 3
---
[UNEXPECTED_PANICS] - Unhandled unwrap() calls in crypto-adjacent paths
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:116
Description: Multiple uses of .unwrap() or .unwrap_or_else() on locks/IO results within src/crypto.rs could lead to panics in production if environment conditions change (e.g., mutex poisoning or unexpected IO failure during file processing).
Why not fixed: no-touch zone
Suggested fix: Replace .unwrap() with proper error handling using CryptoError or .expect() with descriptive panic messages.
First detected: 2026-05-09
Last attempted: 2026-05-30
Attempt count: 3
---
[RUSTSEC-2024-0436] - Unmaintained dependency: paste
Severity: Medium
Location: Cargo.lock
Description: The `paste` crate is no longer maintained. While it's a macro-utility crate, unmaintained dependencies increase the long-term risk of unpatched vulnerabilities or incompatibility.
Why not fixed: no-touch zone (Cargo.lock modification restricted)
Suggested fix: Replace `paste` with `pastey` or `with_builtin_macros`.
First detected: 2026-05-09
Last attempted: 2026-05-30
Attempt count: 3
---
[TMP_PATH_FALLBACK] - Unsafe fallback in tmp_path for paths without filenames
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:274
Description: In `tmp_path`, if the destination path lacks a filename (e.g., root or directory path), `unwrap_or_default()` returns an empty string. This results in a temporary filename like `.suffix.tmp`, which might be hidden on some systems or cause unexpected behavior.
Why not fixed: no-touch zone
Suggested fix: Provide a safe fallback filename like "output" or "file" when `file_name()` is None.
First detected: 2026-05-23
Last attempted: 2026-05-30
Attempt count: 2
---
[CLIPPY_MANUAL_IS_MULTIPLE_OF] - Manual implementation of .is_multiple_of()
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:489,898
Description: Clippy identifies manual implementations of `.is_multiple_of()` using the modulo operator. While not a security vulnerability itself, it violates the project's quality standard (Clippy deny-level warnings).
Why not fixed: no-touch zone
Suggested fix: Replace `chunk_counter % INTERVAL == 0` with `chunk_counter.is_multiple_of(INTERVAL)`.
First detected: 2026-05-30
Last attempted: 2026-05-30
Attempt count: 1
---
