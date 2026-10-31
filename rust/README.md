# rust/

| Path | What |
|---|---|
| `crates/ww3-kernels/` | WW3's numerics, `no_std`, built for host, PTX and SPIR-V |
| `crates/ww3-libm/` | glibc's math in plain Rust, the same on every target |
| `crates/ww3-host/` | Host side: setup tables, capture reader |
| `gpu-cuda/`, `gpu-spirv/` | GPU entry points, each its own workspace and nightly |
| `tools/` | `snl1_cmp`, `sds4_cmp`, `spirv_replay`, `libm_bits`: comparisons with the reference |
| `port_files.tsv` | Every WW3 `.F90` file and the batch that ports it |
| `ports.tsv` | Every translated routine and its verdicts |
| `port_report/` | One report per routine |

`crates/ww3-io` and `tools/ww3cmp` are next ([plan](../docs/RUST_GPU_PLAN_202610.md)). Use a rustup
toolchain: Homebrew's `rustc` 1.78 shadows it on the owner's Mac and cannot read edition 2024, so
the `justfile` puts rustup first.
