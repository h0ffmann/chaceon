# How WW3's REAL and DOUBLE PRECISION are represented in Rust

2026-10-30. The rule every translated line follows: a Rust value has the kind the Fortran value has,
at every step of the expression, and a value crosses between `f32` and `f64` exactly where gfortran
converts it. [ADR-0004](ADRs/ADR-0004-float-representation.md) is the decision. WW3 facts are read
at metocean/WW3 `92e2cf66`.

## The mapping

| Fortran | Rust | Bytes |
|---|---|---|
| default `REAL`, a literal `1.5`, `1.E-3` | `Real` (`f32`) | 4 |
| `DOUBLE PRECISION`, `REAL*8`, `REAL(KIND=8)`, a literal `1.5D0` | `Dbl` (`f64`) | 8 |
| `DBLE(x)` | `x as f64` | |
| `REAL(d)`, `SNGL(d)` | `d as f32`, round to nearest | |
| `REAL(i)` | `i as f32` | |
| default `INTEGER` | `i32` | 4 |
| `INTEGER*8`, `INTEGER(KIND=8)` | `i64` | 8 |
| `LOGICAL` | `bool` in a kernel; `u32` in a device buffer, where `bool` has no agreed layout | |

`Real` and `Dbl` live in [`real.rs`](../rust/crates/ww3-kernels/src/real.rs). A kernel names them,
never `f32` or `f64` directly, so a line says which Fortran kind it holds. Kernels are not generic
over a float type. WW3's gfortran build has no precision option: the flags are `-g
-fno-second-underscore -ffree-line-length-none`, plus `-O3` in Release
(`model/src/CMakeLists.txt:70-71`) (v), so `REAL` is 4 bytes in every build, and a build of the
whole model in `f64` would be a different model.

## Where f64 is in WW3

Most of the model is `f32`, and the double-precision code sits in known places. `just f64-census`
prints every file: 817 declaration lines and 545 lines with a `D` literal, in 43 of the 135 `.F90`
files (v, 2026-10-30). Its pattern finds `DOUBLE PRECISION`, `REAL*8` and `REAL(8)` forms. `w3gsrumd.F90` and a few
other files also use a `_8` suffix or `REAL(x,8)`, which it does not count ⚠: a translator reads the
routine, not the census.

| Code | Declarations | Literals | Batch | Why it matters |
|---|---|---|---|---|
| `W3SRC4`, `W3SNL1`, `W3PRO3`, `W3UQCK`, `W3WAVE`, `W3SBT1`, `W3SBT4`, `W3SLN1`, `W3DISP` | 0 | 0 | 1 to 3 | The heaviest kernels are pure `f32` |
| `W3SDB1` | 2 | 12 | 2 | **On the device:** `REAL*8` at `w3sdb1md.F90:177-178`, with double `EXP`, `TANH` and `DSQRT` |
| `W3SRCE` | 2 | 0 | 2 | **On the device:** `w3srcemd.F90:707`, `:793`, summed at `:1635-1636` |
| `constants.F90` | 4 | 40 | 1 | `KZEONE` (`:243`), used by the `TABU_FW` and `KERKEI` code, in `f64` |
| `w3timemd.F90`, `w3servmd.F90`, `w3gdatmd.F90` | 42 | 44 | 1 | Host side: dates, grid metadata |
| `w3parall.F90` | 6 | 4 | 3 | To read before it is placed ⚠ |
| `w3gsrumd.F90` | 331 | 44 | 5 | Host side: grid search utility, the largest user |
| `w3profsmd.F90`, `w3profsmd_pdlib.F90`, `w3triamd.F90` | 211 | 214 | 8 | Unstructured solvers, device side, the largest f64 load on a GPU |
| `w3snl4md.F90`, `w3sic3md.F90`, `w3tidemd.F90` | 81 | 140 | 7 | Other physics switches |

**Measured, 2026-10-31 (v):** dropping one `W3SDB1` value to `f32` (`DSQRT` of `8.d0*DBLE(EMEAN)`)
changes about 170 000 values of the integrated spectrum on the storm and filipot cases. The `f64` in
`W3SDB1` is not cosmetic.

## What the device must do

| Backend | f32 | f64 | What follows |
|---|---|---|---|
| Rust-CUDA on the H100 or RTX 4090 | yes | PTX has `f64`; not run here ⚠ | `W3SDB1` and `W3SRCE` translate as they stand |
| rust-gpu on Vulkan | yes | needs the `Float64` capability and `shaderFloat64` (v, Vulkan specification, read 2026-10-30) | The kernel checks the feature at start and stops without it |
| rust-gpu on Apple silicon through MoltenVK | yes | Metal has no double type ⚠ | The target switch cannot run its `DB1` path on the owner's Mac |

**f64 is never demoted to f32 to fit a device.** That changes `W3SDB1`'s arithmetic: "Translate,
don't improve". Without `f64` the run stops with a message naming the kernel. Moving such a kernel
to `f32` is a separate change, asked for first, and it needs its own evidence.

Mathematical functions need both widths. `W3SDB1` calls `exp`, `tanh` and `sqrt` in `f64`, so
`ww3-libm` (Batch 0) carries the double versions too.

## Four ways the obvious Rust is wrong

Each row was run on 2026-10-30 (v): gfortran 14.3.0 `-O3` in `gcc:14-bookworm` against rustc 1.99.1
([log](log/2026-10-30-scaffold.md)). Repeated on arm64 with GCC 12.5.0: same bits (v). `a = 0.1` and `b = 3.3` are `REAL`; `d` is `REAL*8`.

| Fortran | gfortran's bits | Correct Rust | Wrong Rust, and its bits |
|---|---|---|---|
| `d = 0.1` | `3FB99999A0000000` | `0.1_f32 as f64` | `let d: f64 = 0.1`, `3FB999999999999A` |
| `THR = DBLE(1.E-15)` (`w3sdb1md.F90:193`) | `3CD203AFA0000000` | `1.0e-15_f32 as f64` | `1.0e-15_f64` |
| `d = a*b` | `3FD51EB860000000` | `(a * b) as f64` | `(a as f64) * (b as f64)`, `3FD51EB852147AE0` |
| `4.E7` | | `4.0E7` | `4.E7` does not parse in Rust |

The rule in one line: **Fortran widens after the operation it is told to do in `REAL`.** A literal
without `D` is `f32` even when it lands in an `f64`; a mixed operation widens its `f32` operand just
before that operation, not earlier. Rust has no implicit conversion, so the type checker catches
mixed operands, and only the hand-written `as` can sit in the wrong place. Put it where gfortran
converts, one `as` per Fortran conversion.

## Other representation traps

- **Constants.** gfortran folds `GRAV**2` and `TPI**3` with one rounding each, and a Rust `const`
  folds a chain of products. All nine derived constants match gfortran's bits (v, the log above),
  but that is a result for these values, never a rule. Each new `**` in a `PARAMETER` gets the same
  probe.
- **`x**n`.** Never `powi` or `powf`. `f32::powi`'s result is not specified, and gfortran's `-O3`
  uses its own multiplication chain for a literal exponent. Write the chain from the `-O3` GIMPLE
  (ww3-gpu `docs/log/2026-10-14-snl1-bitwise-af11.md`, measured at NOAA's pin; to repeat at this one ⚠).
- **`MAX`, `MIN` and NaN.** `f32::max` returns the number when one operand is NaN. gfortran's `MAX`
  is not defined to, and MetOcean's `ISNAN` guards (`w3src4md.F90:780`, `:835`) mean NaN reaches
  this code. Each site keeps gfortran's optimised behaviour, read from the `-O3` output ⚠.
- **`INT` and `NINT`.** `x as i32` truncates like `INT` but saturates out of range and gives 0 for
  NaN, where Fortran's result is undefined. Note it at each site that can see those values.

## Device float behaviour

| | CPU reference | Rust-CUDA | rust-gpu on Vulkan |
|---|---|---|---|
| Contraction into FMA | **off by build flag**: arm64 gfortran fuses by default, so the reference adds `-ffp-contract=off` (v, ADR-0005) | **on by default**: `fma_contraction: true` (`cuda_builder/src/lib.rs:211` at `3a82c35`). Set `false`, which passes `-fma=0` (`:743-744`) (v) | **Allowed** without `NoContraction` (v, Vulkan specification). `rustc_codegen_spirv` at `60d1e25` emits no such decoration (v, `grep -rn NoContraction crates`); `rust/tools/spirv_exact` adds it to every float operation |
| `/` | IEEE | correctly rounded when `fast_div` is off, its default (v, option; rounding not run ⚠) | **2.5 ULP** allowed (v, Vulkan specification). `spirv_exact` divides `f32` in `f64`, itself allowed 2.5 ULP, then rounds |
| `+`, `*` | IEEE | IEEE | correctly rounded (v) |
| `exp`, `log`, `pow` | glibc | **libdevice by default**: `override_libm: true`, "likely to not be deterministic" (`cuda_builder/src/lib.rs`, doc of `override_libm`) (v). Set `false` and call `ww3-libm` | `GLSL.std.450`, a ULP bound, not glibc's bits (v, `libm_intrinsics.rs:84-85`, `:146-147`). Call `ww3-libm` |
| Subnormals | kept | `ftz: false` is the default (v); set it explicitly | "may" be flushed unless the execution mode says otherwise ⚠. lavapipe does not offer `DenormPreserve` (v, `lvp_device.c`, Mesa 22.3.6), and kept them on the subnormal inputs of `just spirv-libm` on arm64 (v); a device is unread ⚠ |

**Measured on `W3SNL1`'s PTX, 2026-10-30 (v, `just ptx`):** no `fma` or float `mad`, no libdevice
call, and every float operation `.rn`. `ptxas` fuses a plain `mul`, `add` pair by default but leaves
`mul.rn`, `add.rn` alone with or without `--fmad=false`. The 27 FMA-class instructions in the binary
are the expansion of the three `div.rn.f32`: one `div.rn.f32` alone gives 17. None has run on a device ⚠.

**Measured on lavapipe, 2026-10-31 (v, `just spirv-replay`, `just spirv-libm`):** `W3SNL1`, `W3SPR4`,
`W3SIN4`, `W3SDS4` and `W3SRCE` are bit-identical, 66 captures, 0 of 108 316 135 values. They were not while
`rust-gpu`'s `spirv-opt` ran before anything had decorated the module: `spirv-opt` folds floats unless
`NoContraction` is on the operation, so `(z-pi_lo)-pi` in `atan2f` became `z-(pi_lo+pi)`, 1 ULP wrong on 29% of
its inputs. Mesa folds an undecorated module too. `scripts/spirv.sh` now decorates first and optimises after.
[The log](log/2026-10-31-spirv-bit-parity.md) has the table. On PTX all float operations are `.rn` and nothing fuses.

So Rust-CUDA can reach the CPU's bits with the right builder settings. rust-gpu on lavapipe does reach them. A
Vulkan device is a separate question ⚠: `NoContraction` binds it, but `f64` division may be off by 2.5 ULP there, which
`W3SDB1`, `exp`, `expm1` and `tanh` would see, and an `f32` quotient survives only by the margin to a rounding tie
that `rust/tools/spirv_exact` relies on (`2^-49`, against a 2.5 ULP error in `f64`). Subnormals and the rounding of `OpFConvert` are unread there too. ADR-0005 judges `spirv` by tolerance until a device has run.

## Memory and files

- **Device buffers** are `#[repr(C)]` structs of `Real`, `Dbl`, `i32` and `u32`, with no `bool`, no
  padding the compiler chooses, and an `f64` on an 8-byte boundary. The same layout is read by the
  host, by PTX and by SPIR-V, so it is defined once, in `ww3-kernels`.
- **The spectrum** `VA` stays `f32`: `NSPEC * NSEA * 4` bytes. An H100's 80 GB is spent at that
  rate, and the f64 work arrays of `W3SRCE` and `W3SDB1` are per-point scratch, not residents.
- **WW3's binary files** hold `REAL` as 4 bytes and record lengths per Fortran's unformatted
  sequential format, with `ENDIANNESS` set by the build (`model/src/CMakeLists.txt:42`), which is
big-endian here (v). chaceon
  reads and writes those bytes exactly, so `cmp` and `test.comp` can judge its output.
