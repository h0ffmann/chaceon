#!/usr/bin/env bash
# Compiles the kernels to SPIR-V with rust-gpu and copies them to rust/gpu-spirv/out/chaceon.spv.
set -euo pipefail
cd "$(dirname "$0")/../rust/gpu-spirv"
unset RUSTC
PATH="$HOME/.cargo/bin:$PATH"
module=$(rustup run nightly-2026-08-15 cargo run -q --release -p chaceon-spirv-builder | sed -E 's/.*"(.*)".*/\1/')
mkdir -p out
cp "$module" out/chaceon.spv
ls -l out/chaceon.spv
