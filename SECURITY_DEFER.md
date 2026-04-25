[RUSTSEC-2024-0388] - `derivative` is unmaintained
Severity: Medium
Location: Cargo.lock
Description: The `derivative` crate is no longer maintained. It is a transitive dependency of `eframe`.
Why not fixed: Cargo.lock is a no-touch zone and the dependency is transitive.
Suggested fix: Wait for `eframe` to update its dependencies or use a patched version.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[RUSTSEC-2024-0384] - `instant` is unmaintained
Severity: Medium
Location: Cargo.lock
Description: The `instant` crate is no longer maintained. Transitive dependency.
Why not fixed: Cargo.lock is a no-touch zone.
Suggested fix: Consider alternatives like `web-time`.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[RUSTSEC-2024-0436] - `paste` is unmaintained
Severity: Medium
Location: Cargo.lock
Description: The `paste` crate is no longer maintained. Transitive dependency.
Why not fixed: Cargo.lock is a no-touch zone.
Suggested fix: Use `pastey` or `with_builtin_macros`.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[RUSTSEC-2026-0097] - `rand` is unsound
Severity: High
Location: Cargo.lock
Description: Rand is unsound with a custom logger using `rand::rng()`. Transitive dependency.
Why not fixed: Cargo.lock is a no-touch zone.
Suggested fix: Update `rand` to a patched version (>=0.8.6).
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[YANKED-UDS-WINDOWS] - `uds_windows` is yanked
Severity: Medium
Location: Cargo.lock
Description: The `uds_windows` v1.2.0 crate is yanked. Transitive dependency.
Why not fixed: Cargo.lock is a no-touch zone.
Suggested fix: Update to a non-yanked version.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[UNWRAP-CRYPTO-85] - Unwrap in ProgressReporter impl
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:85
Description: `self.last_message.lock().unwrap()` can panic if the mutex is poisoned.
Why not fixed: neuron-encrypt/src/crypto.rs is a no-touch zone.
Suggested fix: Replace `unwrap()` with `expect("mutex poisoned")` or handle the error gracefully.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[UNWRAP-CRYPTO-86] - Unwrap in ProgressReporter impl
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:86
Description: `self.last_time.lock().unwrap()` can panic if the mutex is poisoned.
Why not fixed: neuron-encrypt/src/crypto.rs is a no-touch zone.
Suggested fix: Replace `unwrap()` with `expect("mutex poisoned")` or handle the error gracefully.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[UNWRAP-CRYPTO-100] - Unwrap in ProgressReporter impl
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:100
Description: `self.last_message.lock().unwrap()` can panic if the mutex is poisoned.
Why not fixed: neuron-encrypt/src/crypto.rs is a no-touch zone.
Suggested fix: Replace `unwrap()` with `expect("mutex poisoned")` or handle the error gracefully.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
---
[UNWRAP-CRYPTO-101] - Unwrap in ProgressReporter impl
Severity: Medium
Location: neuron-encrypt/src/crypto.rs:101
Description: `self.last_time.lock().unwrap()` can panic if the mutex is poisoned.
Why not fixed: neuron-encrypt/src/crypto.rs is a no-touch zone.
Suggested fix: Replace `unwrap()` with `expect("mutex poisoned")` or handle the error gracefully.
First detected: 2026-04-25
Last attempted: 2026-04-25
Attempt count: 1
