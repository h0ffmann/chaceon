# ADR-0005: The host, cuda and spirv verdicts

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

ww3-gpu's rule (the same executable, a run-time flag) cannot hold for a pure Rust port. The
reference is gfortran's executable, chaceon is rustc's. On a device, Vulkan allows 2.5 ULP in
division and allows contraction without `NoContraction`, which rust-gpu does not emit (v,
2026-10-30). Rust-CUDA defaults to contraction and libdevice, both of which can be turned off (v).

## Decision

- **`host`:** `ww3-kernels` built by rustc for `aarch64-unknown-linux-gnu`, run inside the
  reference's linux/arm64 image so both call the same glibc, compared by `cmp` and `test.comp`. Byte-identical or a defect. A file the reference does not reproduce against itself
  is excused, and only that file.
- **`cuda`:** the same inputs on Rust-CUDA, with the settings of `AGENTS.md`. Byte-identical is the
  target. Where the audit of Batch 0 shows it cannot be, the run is judged by `rust/tools/ww3cmp`'s
  tolerances, with every difference recorded.
- **`spirv`:** tolerance only, as above.
- **Reference machine:** the owner's Apple Silicon Mac, native linux/arm64 in Docker. The owner,
  2026-10-30: "x84_64, ignore here you're here using apple sillicon". The reference adds
  `-ffp-contract=off`, passed through `CMAKE_Fortran_FLAGS` with no change to `WW3/`: arm64 gfortran
  fuses `a*b+c` by default, and Rust does not. The same `a*b+c` gave `28800000` fused and `00000000`
  with the flag (v, gfortran 12.5.0, `-O3`, 2026-10-30).

A tolerance is fixed before the first output it judges and is never widened. A device run needs the
owner.

## Consequences

`host` is not a product, and exists to separate a translation error from a device one. The reference
is contraction-off arm64, not the stock arm64 build and not LabECO's x86_64 one: a baseline x86_64
build has no FMA either, but its libm may differ ⚠, so `host` bits are this reference's. `cuda` still
needs an NVIDIA machine, which a Mac is not.
