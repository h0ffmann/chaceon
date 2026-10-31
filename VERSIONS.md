# Versions

Every pinned version, read on 2026-10-30 (v) unless marked.

| Component | Version | Pinned in | Read with |
|---|---|---|---|
| WW3 reference | metocean/WW3 tag `7.14.mos-92e2cf6`, commit `92e2cf66` (2026-09-01): NOAA-EMC/WW3 `3eb8161` plus MetOcean's ten commits | the `WW3/` submodule, `docker/Dockerfile.reference` | `git submodule status WW3` |
| Host Rust | 1.99.1 | not yet pinned ⚠ | `rustup toolchain list` |
| Rust-CUDA | `3a82c35` (2026-09-30), `nightly-2026-04-02`, LLVM 7 by default, LLVM 21 by feature | to pin in Batch 0 | `rust-toolchain.toml` of that commit |
| rust-gpu | `60d1e25` (2026-10-27), `nightly-2026-08-15` | to pin in Batch 0 | `rust-toolchain.toml` of that commit |
| CUDA, driver | CUDA 12 or 13; driver 580 or later for 13 | to pin in Batch 0 | `guide/src/guide/getting_started.md` of Rust-CUDA |
| Reference compiler | Debian bookworm's gfortran; read from `BUILD_INFO` | `docker/Dockerfile.reference` | `just ref-build` |
| WW3 regtest data | not fetched yet ⚠ | Batch 0 | `WW3/model/bin/ww3_from_ftp.sh` |

ww3-gpu's own WW3 is NOAA's `03adda91`, a different model: its timings and verdicts do not carry
over.
