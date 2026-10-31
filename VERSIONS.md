# Versions

Every pinned version, read on 2026-10-30 (v) unless marked.

| Component | Version | Pinned in | Read with |
|---|---|---|---|
| WW3 reference | metocean/WW3 tag `7.14.mos-92e2cf6`, commit `92e2cf66` (2026-09-01): NOAA-EMC/WW3 `3eb8161` plus MetOcean's ten commits | the `WW3/` submodule, `docker/Dockerfile.reference` | `git submodule status WW3` |
| Host Rust | 1.99.1 | not yet pinned ⚠ | `rustup toolchain list` |
| Rust-CUDA | `3a82c35` (2026-09-30), `nightly-2026-04-02`, LLVM 7 backend (LLVM 21 reaches sm_100 and later only) | `rust/gpu-cuda/Cargo.toml`, its `rust-toolchain.toml` | `just ptx` |
| rust-gpu | `60d1e25` (2026-10-27), `nightly-2026-08-15` | `rust/gpu-spirv/Cargo.toml`, its `rust-toolchain.toml` | `just spirv` |
| CUDA | 13.3.1 (`nvcr.io/nvidia/cuda:13.3.1-cudnn-devel-ubuntu24.04`, arm64), compile only; driver 580 or later for CUDA 13 ⚠ | `docker/Dockerfile.ptx` | `just ptx` |
| Linux Rust for replays | `rust:1.90-bookworm`, same Debian as the reference | `scripts/rust_in_ref.sh` | `just snl1-cmp` |
| CPU Vulkan | Mesa lavapipe, Debian bookworm (LLVM 15.0.6) | `docker/Dockerfile.lavapipe` | `just spirv-replay` |
| Reference compiler | gfortran 12.2.0, glibc 2.36 (Debian bookworm, linux/arm64), `-ffp-contract=off` | `docker/Dockerfile.reference` | `just ref-build`, which prints `BUILD_INFO` |
| WW3 regtest data | not fetched yet ⚠ | Batch 0 | `WW3/model/bin/ww3_from_ftp.sh` |

ww3-gpu's own WW3 is NOAA's `03adda91`, a different model: its timings and verdicts do not carry
over.
