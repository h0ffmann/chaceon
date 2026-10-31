# ADR-0005: The host, cuda and spirv verdicts

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

ww3-gpu's rule (the same executable, a run-time flag) cannot hold for a pure Rust port. The
reference is gfortran's executable, chaceon is rustc's. On a device, Vulkan allows 2.5 ULP in
division and allows contraction without `NoContraction`, which rust-gpu does not emit (v,
2026-10-30). Rust-CUDA defaults to contraction and libdevice, both of which can be turned off (v).

## Decision

- **`host`:** `ww3-kernels` built by rustc, on the reference's x86_64 machine, compared by `cmp` and
  `test.comp`. Byte-identical or a defect. A file the reference does not reproduce against itself
  is excused, and only that file.
- **`cuda`:** the same inputs on Rust-CUDA, with the settings of `AGENTS.md`. Byte-identical is the
  target. Where the audit of Batch 0 shows it cannot be, the run is judged by `rust/tools/ww3cmp`'s
  tolerances, with every difference recorded.
- **`spirv`:** tolerance only, as above.
- **Reference machine:** one named native x86_64 machine, recorded with each verdict. Never arm64,
  and never emulation: its Fortran may contract, and glibc selects functions by CPU ⚠.

A tolerance is fixed before the first output it judges and is never widened. A device run needs the
owner.

## Consequences

`host` is not a product, and exists to separate a translation error from a device one. The owner's
Mac cannot give a bit verdict. A native x86_64 machine is needed from Batch 2 ⚠.
