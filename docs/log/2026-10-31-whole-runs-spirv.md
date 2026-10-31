# Whole runs, and SPIR-V byte-identical on lavapipe

2026-10-31, agent session `cd86e8f5` for the owner, who asked to continue "with bigger PRs, with same
discipline"; no issue. Branch `goal/cd86e8f5-7206-grid-output`. Results: `rust/port_report/W3PRO3.md`,
`rust/port_report/W3OUTG.md`.

## Question

Can every output file of a case match, on the target's physics and grid? And can the propagation kernels run
as compiled shaders inside the time loop without changing a bit?

## Result

Yes to both on the cases run: 50 files, 83 293 901 bytes on the CPU, and 44 files, 78 889 351 bytes with
propagation in SPIR-V on lavapipe, 0 differ (v, `just tp-all`, `scripts/spirv_pair.sh`).

`ww3_tp2.15`'s `wind.nc` is not in the repository, so that case runs under homogeneous wind.

## What went wrong on the way

- **`W3IOGO`'s loop order** was reversed in the first draft, and reading the listing against the code caught it.
- **`UST` starts at `1.E-5`.** Runs with source terms hide it; `ww3_tp2.2`'s restart differed in 69 524 bytes
  while its grid output matched.
- **`W3FLDH`'s fallback tests `TFN`**, so a first input after the start time leaves `TF0` at -1, and the
  translation keeps that.
- **`NoContraction` alone changed nothing useful**, and that looked like a dead end. Division alone did not
  help either, and the two together reached 0. That was a workaround: the cause was the order of
  `spirv-opt` and the decoration (`2026-10-31-spirv-bit-parity.md`), and the kernel's `div()` is gone.
- **An attribution run was invalid:** a path came back empty, `cp` failed, and the run kept the decorated
  module, so the table first said undecorated was identical. It was redone.

## What it changes

Lavapipe is one Vulkan implementation, and `W3SRCE` stayed on the CPU in these runs. The next questions are a
real device, with the owner, and the source terms in the loop on the device.
