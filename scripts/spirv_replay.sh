#!/usr/bin/env bash
# Runs the compiled SPIR-V on a capture, on lavapipe. Usage: spirv_replay.sh <capture under work/>.
set -euo pipefail
cd "$(dirname "$0")/.."
docker build --platform linux/arm64 -q -f docker/Dockerfile.lavapipe -t chaceon-lavapipe docker >/dev/null
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/rust/target/linux -w /rust rust:1.90-bookworm cargo build --release -q --bin spirv_replay
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work:ro" \
  --entrypoint /rust/target/linux/release/spirv_replay chaceon-lavapipe /rust/gpu-spirv/out/chaceon.spv "/work/$1"
