---
name: ww3-port
description: "The traps of translating a WAVEWATCH III Fortran routine to no_std Rust for Rust-CUDA and rust-gpu. Use while translating or reviewing a routine, when a Rust result differs from the Fortran reference by a few ULP, or when choosing a float kind, a math call or a device buffer layout."
---

# ww3-port: translating WW3's Fortran to Rust without changing a bit

Readers are scientists: every WW3 fact cites `model/src/<file>.F90:<line>` at `92e2cf66`, and every
claim carries `(v)` or `⚠`. There are no Rust tests. `AGENTS.md` wins where this file disagrees, and
`docs/FLOAT_REPRESENTATION_202610.md` is the authority on kinds. ww3-gpu's `ww3-port` skill has the
longer history; its facts are at NOAA's pin.

## Arithmetic

- The target is gfortran `-O3`'s executable. On a ULP difference, build the module with
  `-fdump-tree-optimized` and read it before suspecting the physics.
- A literal power such as `x**11` is a multiplication chain at `-O3`. A run-time `XFR**IFRP` is
  `__powisf2`. Write each out; never `powi` or `powf` for an integer power.
- `REAL` is `f32`. A literal without `D` is `f32` even inside an `f64` expression.
- `d = a*b` is `(a * b) as f64`. Put each `as` where gfortran converts.
- `f32::max` hides NaN. Read what gfortran's `MAX` compiled to at that site.
- Rust's `a * b + c` is not contracted on the host. On PTX that needs `fma_contraction(false)`.
- Keep the operation order, reductions included. Asking comes before any change.

## Memory

- Fortran bounds survive translation: `SIG(0:NK+1)`, `CG(0:NK+1,0:NSEA)`. Index through a named
  helper that shows the base. Fortran is column-major: `A(ITH,IK)` is `a[(ith-1) + (ik-1)*nth]`.
- WW3 state is per grid (`W3SETG`, `W3SETA`). A file-static table breaks `ww3_multi`.
- A condition around an initialisation is part of the translation: `INSNL1` runs only for
  `IQTPE > 0`, per ww3-gpu; check at this pin ⚠.
- Device buffers are `#[repr(C)]`, without `bool`, defined once in `ww3-kernels`.

## Device

- No allocation, `std`, formatting or panic that depends on data inside a kernel. Keep slices
  bounds-checked in `host` builds and read the cost before removing a check.
- Math goes through `ww3-libm`. Never `cuda_std`'s `GpuFloat`, never `spirv-std`'s `GLSL.std.450`.
- A kernel that needs `f64` declares it, and the host checks the device before starting.
- Never run on a device without the owner.

## Before claiming a verdict

`translated` means the code exists. `host` means `cmp` and `test.comp` passed on the regtest the
batch names, on a native x86_64 machine, against the same commit of the reference. A zero difference
from a run that never reached the routine proves nothing: check it ran.
