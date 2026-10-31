#!/usr/bin/env bash
# Compiles the kernels to SPIR-V with rust-gpu, makes the arithmetic exact, optimises and validates, and
# writes rust/gpu-spirv/out/chaceon.spv. The order is the point: spirv-opt folds floats unless NoContraction
# is already there. spirv-tools' version is in VERSIONS.md.
set -euo pipefail
cd "$(dirname "$0")/../rust/gpu-spirv"
unset RUSTC
PATH="$HOME/.cargo/bin:$PATH"
module=$(rustup run nightly-2026-08-15 cargo run -q --release -p chaceon-spirv-builder | sed -E 's/.*"(.*)".*/\1/')
mkdir -p out
# rust-gpu emits no NoContraction: spirv_exact adds it, so nothing may fuse or reassociate, and it widens f32
# division, which Vulkan does not require to be correctly rounded.
cd ..
tc=$(rustup toolchain list | grep -m1 '^1\.' | cut -d' ' -f1)
rustup run "$tc" cargo run -q --release -p spirv_exact -- "$module" gpu-spirv/out/exact.spv
# The module has logical pointers until spirv-opt removes them, so only its output can be validated.
spirv-opt -O --skip-validation --target-env=vulkan1.2 gpu-spirv/out/exact.spv -o gpu-spirv/out/chaceon.spv
spirv-val --target-env vulkan1.2 gpu-spirv/out/chaceon.spv
rm gpu-spirv/out/exact.spv
ls -l gpu-spirv/out/chaceon.spv
