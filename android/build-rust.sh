#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to build the Rust JNI library" >&2
  exit 1
fi

if ! cargo ndk --version >/dev/null 2>&1; then
  echo "cargo-ndk is required. Install it with: cargo install cargo-ndk --locked" >&2
  exit 1
fi

if [[ -z "${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}" ]]; then
  echo "ANDROID_NDK_HOME or ANDROID_NDK_ROOT must point to an installed Android NDK" >&2
  exit 1
fi

cd "$SCRIPT_DIR/neuron-encrypt-jni"
cargo ndk \
  --platform 26 \
  -t arm64-v8a \
  -t armeabi-v7a \
  -t x86_64 \
  -t x86 \
  -o "$SCRIPT_DIR/app/src/main/jniLibs" \
  build --release
