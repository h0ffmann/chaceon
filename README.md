# chaceon

WAVEWATCH III® in pure Rust, on the GPU, through [Rust-CUDA](https://github.com/Rust-GPU/rust-cuda)
and [rust-gpu](https://github.com/Rust-GPU/rust-gpu). It is compared with
[metocean/WW3](https://github.com/metocean/WW3) 7.14 (`7.14.mos-92e2cf6`), and it follows the
discipline of [ww3-gpu](https://github.com/h0ffmann/ww3-gpu) without Kokkos.

**State, 2026-10-30:** a scaffold. WW3's constants are translated; nothing runs yet ⚠.

| Read | For |
|---|---|
| [`AGENTS.md`](AGENTS.md) | The rules |
| [`docs/RUST_GPU_PLAN_202610.md`](docs/RUST_GPU_PLAN_202610.md) | The plan, in batches |
| [`docs/FLOAT_REPRESENTATION_202610.md`](docs/FLOAT_REPRESENTATION_202610.md) | `f32` and `f64` against Fortran's kinds |
| [`docs/ADRs/`](docs/ADRs/README.md) | The decisions |
| [`VERSIONS.md`](VERSIONS.md) | What is pinned |

## Licence

See [ADR-0006](docs/ADRs/ADR-0006-licence.md): this repository is MPL-2.0, and files translated from
WW3 are marked LGPL-3.0-only. WAVEWATCH III is a trademark of the NWS.
