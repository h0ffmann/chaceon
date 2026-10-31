# W3SRCE, the integrator, with W3SDB1's f64

2026-10-31, agent session `cd86e8f5` for the owner; no issue. Branch
`goal/cd86e8f5-8427-srce-parity`. Results: `rust/port_report/W3SRCE.md`.

## Question

Do the ported kernels hold together in the integrator, and does `f64` reach a compiled kernel?

## Result

- `W3SRCE` matches the reference on 7 931 calls across 11 captures (v).
- `exp`, `expm1`, `tanh` and `sqrt` in `f64` equal glibc on 8 * 10^9 samples each (v).
- rust-gpu accepts structs of slices, so the kernel takes `SrceTables` and `SrceWork` (v).
- The integrator runs as SPIR-V on lavapipe, and stays within 45 ULP on `SPEC` (v).

## What went wrong on the way

- **Deep and moderate cases said little.** `W3SBT1` is idle in deep water and `W3SDB1` is idle at 12 m/s
  even at 6 m. A pass there proves nothing about them, which is why `srce_cmp` now counts them, and why
  `storm` exists.
- **A replay that printed nothing.** lavapipe allows 32 storage buffers and `W3SRCE` binds 52, so it died
  during setup. `docker/Dockerfile.lavapipe` now raises that limit too.
- **A wrong buffer index** in `spirv_replay` compared `WHITECAP` with something else. A relative error
  of 1e299 gave it away.

## What it changes

`W3SRCE` needs only a sea-point loop and state to run `ww3_ts1`: Batch 1's readers are next.
