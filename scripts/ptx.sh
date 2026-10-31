#!/usr/bin/env bash
# Compiles the kernels to rust/gpu-cuda/out/chaceon.ptx in a container, and assembles them with
# ptxas with contraction off, so nothing fuses a mul and an add at any later stage.
set -euo pipefail
cd "$(dirname "$0")/.."
image=chaceon-ptx
docker build --platform linux/arm64 -q -f docker/Dockerfile.ptx -t "$image" docker >/dev/null
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-ptx-cargo:/root/.cargo/registry \
  -e CARGO_TARGET_DIR=/rust/gpu-cuda/target/linux -w /rust/gpu-cuda "$image" sh -c '
    set -e
    mkdir -p out
    # cuda_std needs the glam of the Rust-CUDA lockfile: newer ones gate UVec2 behind a feature.
    cargo update -q -p glam --precise 0.30.9
    cargo run --release -p chaceon-cuda-builder
    ptxas --fmad=false -arch sm_89 -m64 -o out/chaceon_sm89.cubin out/chaceon.ptx
    ptxas --fmad=false -arch sm_90 -m64 -o out/chaceon_sm90.cubin out/chaceon.ptx
    ls -l out'
