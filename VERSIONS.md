# Versions

Every pinned version, read on 2026-10-30 (v) unless marked.

| Component | Version | Pinned in | Read with |
|---|---|---|---|
| WW3 reference | metocean/WW3 tag `7.14.mos-92e2cf6`, commit `92e2cf66` (2026-09-01): NOAA-EMC/WW3 `3eb8161` plus MetOcean's ten commits | the `WW3/` submodule, `docker/Dockerfile.reference` | `git submodule status WW3` |
| Host Rust | 1.99.1 | not yet pinned ⚠ | `rustup toolchain list` |
| Rust-CUDA | `3a82c35` (2026-09-30), `nightly-2026-04-02`, LLVM 7 backend (LLVM 21 reaches sm_100 and later only) | `rust/gpu-cuda/Cargo.toml`, its `rust-toolchain.toml` | `just ptx` |
| rust-gpu | `60d1e25` (2026-10-27), `nightly-2026-08-15` | `rust/gpu-spirv/Cargo.toml`, its `rust-toolchain.toml` | `just spirv` |
| SPIRV-Tools | Homebrew `spirv-tools` 1.4.363.0, `SPIRV-Tools v2026.4`: runs `spirv-opt` after `spirv_exact`, and `spirv-val` | not pinned ⚠: `just spirv` uses what is on the PATH | `spirv-opt --version` |
| CUDA | 13.3.1 (`nvcr.io/nvidia/cuda:13.3.1-cudnn-devel-ubuntu24.04`, arm64), compile only; driver 580 or later for CUDA 13 ⚠ | `docker/Dockerfile.ptx` | `just ptx` |
| Linux Rust for replays | `rust:1.90-bookworm`, same Debian as the reference | `scripts/rust_in_ref.sh` | `just snl1-cmp` |
| Runtime MPI, build | `rust:1.90-bookworm` plus `libopenmpi-dev` 4.1.4-3, `libclang-dev` and `clang` 14.0-55.7 (bindgen), rsmpi `mpi` 0.8.0 and `mpi-sys` 0.2.4 (`rust/Cargo.lock`) | `docker/Dockerfile.mpi` (target `builder`), `rust/crates/ww3-host/Cargo.toml` | `scripts/rust_mpi.sh --image` |
| Runtime MPI, run | Open MPI 4.1.4 (`libopenmpi3` 4.1.4-3+b1) on the reference image, glibc 2.36 | `docker/Dockerfile.mpi` (target `run`) | `docker run --rm --entrypoint mpirun chaceon-ref-mpi:switch_chaceon_shrd --version` |
| CPU Vulkan | Mesa lavapipe 22.3.6 from source, SHA-256 pinned, `LP_MAX_TGSI_LOOP_ITERATIONS` and the shader-buffer limits raised, LLVM 15 | `docker/Dockerfile.lavapipe` | `just spirv-replay` |
| Reference compiler | gfortran 12.2.0, glibc 2.36 (Debian bookworm, linux/arm64), `-ffp-contract=off` | `docker/Dockerfile.reference` | `just ref-build`, which prints `BUILD_INFO` |
| Lean 4 and mathlib | Lean `v4.35.0`, mathlib `v4.35.0` = commit `56a00585`; Ubuntu 24.04 arm64 image, elan, never on the host. `lake exe cache get` fetches the built mathlib | `lean/lean-toolchain`, `lean/lakefile.toml`, `lean/lake-manifest.json`, `docker/Dockerfile.lean` | `just lean` |
| WW3 regtest data | not fetched yet ⚠ | Batch 0 | `WW3/model/bin/ww3_from_ftp.sh` |

ww3-gpu's own WW3 is NOAA's `03adda91`, a different model: its timings and verdicts do not carry
over.
