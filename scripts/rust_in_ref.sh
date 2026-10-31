#!/usr/bin/env bash
# Builds rust/ for Linux in a Debian bookworm toolchain and runs a binary with the reference's glibc.
# Usage: rust_in_ref.sh <binary> [args]. Paths under work/ are /work/ inside.
set -euo pipefail
cd "$(dirname "$0")/.."
bin="$1"; shift
image=rust:1.90-bookworm
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/rust/target/linux -w /rust "$image" cargo build --release -q --bin "$bin"
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
  --entrypoint "/rust/target/linux/release/$bin" chaceon-ref:switch_chaceon_shrd "$@"
