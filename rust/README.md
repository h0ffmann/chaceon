# rust/

| Path | What |
|---|---|
| `crates/ww3-kernels/` | WW3's numerics, `no_std`, built for host, PTX and SPIR-V |
| `port_files.tsv` | Every WW3 `.F90` file and the batch that ports it |
| `ports.tsv` | Every translated routine and its verdicts |
| `port_report/` | One report per routine |

`crates/ww3-libm`, `crates/ww3-io`, `gpu-cuda/`, `gpu-spirv/` and `tools/ww3cmp` arrive in Batches 0
and 1 ([plan](../docs/RUST_GPU_PLAN_202610.md)). Use a rustup toolchain: Homebrew's `rustc` 1.78
shadows it on the owner's Mac, and it cannot read edition 2024.
