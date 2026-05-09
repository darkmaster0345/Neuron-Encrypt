[ARGON2ID_PARAMS] - Insufficient Argon2id memory cost
Severity: Critical
Location: neuron-encrypt/src/crypto.rs:114
Description: The current Argon2id memory cost is set to 64MB (m_cost=65536). Security guidelines recommend a minimum of 256MB (m_cost=262144) for hardened file encryption to resist specialized hardware-based brute-force attacks.
Why not fixed: no-touch zone
Suggested fix: Update Argon2id Params in derive_key and derive_key_v3 to use m_cost=262144.
First detected: 2026-05-09
Last attempted: 2026-05-09
Attempt count: 1
---
[UNEXPECTED_PANICS] - Unhandled unwrap() calls in crypto-adjacent paths
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:116
Description: Multiple uses of .unwrap() or .unwrap_or_else() on locks/IO results within src/crypto.rs could lead to panics in production if environment conditions change (e.g., mutex poisoning or unexpected IO failure during file processing).
Why not fixed: no-touch zone
Suggested fix: Replace .unwrap() with proper error handling using CryptoError or .expect() with descriptive panic messages.
First detected: 2026-05-09
Last attempted: 2026-05-09
Attempt count: 1
---
[RUSTSEC-2024-0436] - Unmaintained dependency: paste
Severity: Medium
Location: Cargo.lock
Description: The `paste` crate is no longer maintained. While it's a macro-utility crate, unmaintained dependencies increase the long-term risk of unpatched vulnerabilities or incompatibility.
Why not fixed: no-touch zone (Cargo.lock modification restricted)
Suggested fix: Replace `paste` with `pastey` or `with_builtin_macros`.
First detected: 2026-05-09
Last attempted: 2026-05-09
Attempt count: 1
---
