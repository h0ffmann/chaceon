# chaceon

WAVEWATCH III® in pure Rust, on the GPU, through [Rust-CUDA](https://github.com/Rust-GPU/rust-cuda)
and [rust-gpu](https://github.com/Rust-GPU/rust-gpu). It is compared with
[metocean/WW3](https://github.com/metocean/WW3) 7.14 (`7.14.mos-92e2cf6`), and it follows the
discipline of [ww3-gpu](https://github.com/h0ffmann/ww3-gpu) without Kokkos.

**State, 2026-10-31:** WW3's constants, `W3SNL1`, all of ST4 with its setup tables and the `W3SRCE`
integrator are translated and match the reference bit for bit on every captured call on the host.
As compiled SPIR-V on CPU Vulkan, `W3SNL1` is bit-identical, and ST4 and `W3SRCE` differ slightly. The PTX builds. No model runs yet, and
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
