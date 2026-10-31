#!/usr/bin/env bash
# Compiles the kernels to SPIR-V with rust-gpu, decorates them and writes rust/gpu-spirv/out/chaceon.spv.
set -euo pipefail
cd "$(dirname "$0")/../rust/gpu-spirv"
unset RUSTC
PATH="$HOME/.cargo/bin:$PATH"
module=$(rustup run nightly-2026-08-15 cargo run -q --release -p chaceon-spirv-builder | sed -E 's/.*"(.*)".*/\1/')
mkdir -p out
# rust-gpu emits no NoContraction: spirv_exact adds it, so a driver may not fuse a mul and an add.
cd ..
tc=$(rustup toolchain list | grep -m1 '^1\.' | cut -d' ' -f1)
rustup run "$tc" cargo run -q --release -p spirv_exact -- "$module" gpu-spirv/out/chaceon.spv
ls -l gpu-spirv/out/chaceon.spv
