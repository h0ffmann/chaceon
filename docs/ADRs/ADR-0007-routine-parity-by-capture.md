# ADR-0007: Routine parity by capture, and how GPU code is built

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

ADR-0005's verdicts judge whole output files, which need a running model (Batch 3). A kernel needs
evidence before that. Writing a Fortran driver would break ADR-0001, and a Rust test would break
ADR-0002. The owner, 2026-10-30: "port next routines from pure fortran to rust on GPU keeping parity".

## Decision

- **Capture.** gdb records a routine's real arguments and results from the running reference
  (`regtests/capture/`). Nothing is linked into WW3, and no Fortran is written.
- **Replay.** Rust repeats each call and compares bits: on the host in the reference's image, and as
  compiled SPIR-V on lavapipe, a CPU Vulkan implementation, which is not a device.
- **Status.** All calls equal gives `routine`, which is not `host`, `cuda` or `spirv`.
- **Fault run.** A comparison is believed once a fault big enough to reach `f32` has failed it.
- **Math.** A kernel calls `ww3-libm`, proved against glibc over every `f32` input.
- **PTX.** Rust-CUDA's LLVM 7 backend, the settings in `AGENTS.md`, then `ptxas --fmad=false`. A host
  loads that binary, not the PTX, so the driver's JIT cannot fuse anything.

## Consequences

Captured files stay in `work/`, out of git, and `just capture` remakes them. Replay covers only what
the run reached, so each routine's report says which configurations it saw. lavapipe shows the
compiled module's arithmetic, not a GPU driver's: Vulkan still allows a device to contract and to
round division differently (ADR-0005).
