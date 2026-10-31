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
- `-O3` folds with one rounding what it can see, even inside a loop: it peels `DO IKD=1,NKD` and folds
  `1.01**(1-1000)`, while every later pass calls `__powisf2`. Only `IKD=1` differs, by 2^-15. A
  difference in one row or the first index suggests this.
- `x**2.` is `x*x` at any optimisation level; other real exponents stay `powf`.
- `REAL` is `f32`. A literal without `D` is `f32` even inside an `f64` expression.
- `d = a*b` is `(a * b) as f64`. Put each `as` where gfortran converts.
- `f32::max` hides NaN. Read what gfortran's `MAX` compiled to at that site.
- Rust's `a * b + c` is not contracted on the host. The reference needs `-ffp-contract=off` to match
  (arm64 gfortran fuses by default), and PTX needs `fma_contraction(false)`.
- Keep the operation order, reductions included. Asking comes before any change.

## Files

- WW3's CMake sets `ENDIANNESS='big_endian'`, so `.ww3` files are big-endian with big-endian record
  markers, even on arm64.
- On `ww3_ts1`, a chain of `W3SRCE` calls needs only `SPEC`, `UST` and `USTDIR` carried over.
- Prove a first whole-file match: delete the output, regenerate it, compare hashes, then break it on
  purpose. A file left over from the reference passes `cmp` too.

- `CLATS`, `CLATIS`, `CTHG0S` are `(0:NSEA)`, unlike `ZB`. A case on the equator hides a shift by one,
  since every `CLATS` is 1: `ww3_tp1.1` passed with it and `ww3_tp1.2` did not.
- `MAPSF(:,3)` is not in `mod_def.ww3`: `W3IOGR` fills it on read.
- A fault that changes nothing may be a weak fault, not a blind spot: `CFAC=1` discards the clamped
  `QCN`, and all transparencies are 1 without obstructions. Say which, and find one that reaches.

## Memory

- Fortran bounds survive translation: `SIG(0:NK+1)`, `CG(0:NK+1,0:NSEA)`. Index through a named
  helper that shows the base. Fortran is column-major: `A(ITH,IK)` is `a[(ith-1) + (ik-1)*nth]`.
- WW3 state is per grid (`W3SETG`, `W3SETA`). A file-static table breaks `ww3_multi`.
- A condition around an initialisation is part of the translation: `INSNL1` runs only for
  `IQTPE > 0`, per ww3-gpu; check at this pin ⚠.
- Device buffers are `#[repr(C)]`, without `bool`, defined once in `ww3-kernels`.

## Device

- No closures in a kernel: one that captures a local by reference is a pointer to a pointer, and
  SPIR-V validation refuses it. Use a plain function or a local macro.
- No sub-slices either: pass the whole buffer and an offset.
- No allocation, `std`, formatting or panic that depends on data inside a kernel. Keep slices
  bounds-checked in `host` builds and read the cost before removing a check.
- Math goes through `ww3-libm`. Never `cuda_std`'s `GpuFloat`, never `spirv-std`'s `GLSL.std.450`.
- A kernel that needs `f64` declares it, and the host checks the device before starting.
- Never run on a device without the owner.

## Reading a routine

- Preprocess with the target's switches before reading: `gfortran -E -cpp -P` with `-DW3_<SWITCH>` for
  each switch cut `W3SRCE` from 2692 lines to about 400, and leaves no chance of translating code
  this build does not have.

## ww3-libm

- A fused site is read from the reference's `libm.so.6` with `objdump`, never predicted. GCC fuses a
  product with one use, but `atanf` also fuses `s1+s2`, and it shares `x*(s1+s2)` between branches.
- Take a constant from fdlibm's decimal, not its hex comment: `aT[0]` says `0x3eaaaaaa`, and the
  decimal, like the machine code, is `0x3eaaaaab`.
- Sweep every `f32`: `sin` differed on 12 inputs of 2^32, and `powf` on 1 pair in 10^9.

## Parity by capture

- `regtests/capture/` records a routine's real arguments from the running reference with gdb.
  Nothing is linked into WW3 and no Fortran is written (ADR-0007).
- Run a fault through the comparison before believing its zeros, and make the fault large enough to
  reach `f32`: the last bit of an `f64` coefficient reached nothing and proved nothing.
- A host replay with zero offsets cannot see an offset bug: also replay in one buffer, as a device does.
- If every loop of a shader stops after one pass from some point on, suspect llvmpipe's limit, which
  `docker/Dockerfile.lavapipe` raises.
- A table of zeros passes everything: `DCKI` and `QBI` matched on every default capture only because
  those settings leave them empty. Print the count of nonzero values and read it.
- A guard can be out of reach of any run: MetOcean's `W3SRCE` floors `USTAR` at 0.001, so `W3SIN4`'s
  0.0001 floor never acts. Show that with a fault run, then poke the input at the routine's entry
  (`regtests/capture/sin4.py`), so the real Fortran runs that line.
- Count what ran. `srce_cmp` prints how many calls had `W3SDB1` and `W3SBT1` active: `W3SDB1` is idle at
  12 m/s even in 6 m of water, so its faults mean nothing there.
- A fault that nothing catches may be a line with no effect: `USTAR=0.001` at `IT=0` is overwritten by
  the next `W3SPR4`. Say which it is.
- A case can hide a branch. `ww3_ts1` is 2500 m deep, where `EXP(X2)` in `W3SNL1` vanishes: a wrong
  `expf` still matched there, and only a shallow run caught it. Ask what each input never reaches.

## Before claiming a verdict

`translated` means the code exists. `routine` means every captured call matched bit for bit, which
does not cover the configurations the capture did not run. `host` means `cmp` and `test.comp` passed on the regtest the
batch names, in the reference's linux/arm64 image on Apple Silicon, against the same commit of the
reference. A zero difference
from a run that never reached the routine proves nothing: check it ran.
