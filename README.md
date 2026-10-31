# chaceon

WAVEWATCH III® in pure Rust, on the GPU, through [Rust-CUDA](https://github.com/Rust-GPU/rust-cuda)
and [rust-gpu](https://github.com/Rust-GPU/rust-gpu). It is compared with
[metocean/WW3](https://github.com/metocean/WW3) 7.14 (`7.14.mos-92e2cf6`), and it follows the
discipline of [ww3-gpu](https://github.com/h0ffmann/ww3-gpu) without Kokkos.

**State, 2026-10-31:** from the Fortran's `mod_def.ww3` and `restart.ww3`, Rust runs 15 regtests and
`ww3_ts1` on the CPU with `out_grd.ww3`, `out_pnt.ww3` and a last-step restart byte-identical to the
reference. Propagation in compiled SPIR-V is byte-identical too, on CPU Vulkan: nothing has run on a real
GPU, and the source terms stay on the CPU in those runs. As compiled SPIR-V on CPU Vulkan, `W3SNL1` is
bit-identical, and ST4 and `W3SRCE` differ slightly. The PTX builds. No model runs yet, and
nothing has run on a GPU ⚠.

| Read | For |
|---|---|
| [`AGENTS.md`](AGENTS.md) | The rules |
| [`docs/RUST_GPU_PLAN_202610.md`](docs/RUST_GPU_PLAN_202610.md) | The plan, in batches |
| [`docs/FLOAT_REPRESENTATION_202610.md`](docs/FLOAT_REPRESENTATION_202610.md) | `f32` and `f64` against Fortran's kinds |
| [`docs/ADRs/`](docs/ADRs/README.md) | The decisions |
| [`VERSIONS.md`](VERSIONS.md) | What is pinned |

## Licence

MPL-2.0, translated files included ([ADR-0006](docs/ADRs/ADR-0006-licence.md)). WAVEWATCH III is a
trademark of the NWS.
