# W3SIN1

WAM cycle 3 wind input (Snyder et al. 1981, Komen et al. 1984): the diagonal of the input source term,
`D = SINC1 * SIG2 * MAX(0, U* cos(theta - theta_u) k / sigma - 0.035714)`, and `S = D * A`. `ST1` is in
`switches/switch_chaceon_st1`, not in the port's target `switch_chaceon_shrd`. Translated from
`model/src/w3src1md.F90:255-417` at metocean/WW3 `92e2cf66`. Kernel: `rust/crates/ww3-kernels/src/sin1.rs`.
Its siblings `W3SPR1` and `W3SDS1` are separate ports.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts
and in compiled SPIR-V on lavapipe (ADR-0007). Not called from `W3SRCE` yet. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `K(NSPEC)` | `REAL`, in | `a`, `k` at offset `spec_off` |
| `USTAR`, `USDIR` | `REAL`, in | `Sin1Point` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `spec_off` |
| `NK`, `NTH`, `NSPEC`, `SINC1` | module `INTEGER`, `REAL` | `Sin1Params` |
| `SIG2(NSPEC)`, `ECOS(NSPEC+NTH)`, `ESIN(NSPEC+NTH)` | module `REAL` | `sig2`, `ecos`, `esin` |

No `f64` anywhere (v, `w3src1md.F90:255-417`). `NK` and `NTH` are carried for the layout and not read.

## Comparison with the reference

The reference is `switches/switch_chaceon_st1`: `switch_chaceon_shrd` with `ST1` and `FLX1` for `ST4` and `FLX0`
(`chaceon-ref-capture:switch_chaceon_st1`). gdb records
every call of `ww3_ts1` (`regtests/capture/sin1.py`), stride 1. Outputs compared: `S` and `D`.

| Capture | Calls | Nonzero `D` / zero `D` | `USTAR` | Host differ | SPIR-V differ |
|---|---|---|---|---|---|
| default (12 m/s), 2500 m | 2629 | 617 390 / 1 654 066 | 0.47699 | 0 | 0 |
| default, 8 m | 3808 | 1 196 401 / 2 093 711 | 0.47699 | 0 | 0 |
| storm (30 m/s), 2500 m | 2938 | 1 082 919 / 1 455 513 | 1.57321 | 0 | 0 |
| light (6 m/s), 2500 m | 2300 | 316 863 / 1 670 337 | 0.20698 | 0 | 0 |
| `winddir` (first wind direction 45), 2500 m | 2711 | 636 542 / 1 705 762 | 0.47699 | 0 | 0 |
| calm (0 m/s), 2500 m | 2161 | 0 / 1 867 104; **proves nothing about the arithmetic** | 2.8e-5 | 0 | 0 |

16 547 calls, 14 296 608 values of `S` and as many of `D`, 0 differ, in call layout and in the one-buffer layout of a
device, on the host and in SPIR-V. `sin1_cmp` prints the clamp: both sides of `MAX(0., ...)` are reached in every
capture but the calm one. `USDIR` ranges over [0, 3.926991] rad in the captures (the case's wind changes direction in
time), `winddir` over [1.5707965, 3.926991], so the cosine and the sine of `USDIR` and `ECOS`, `ESIN` take both signs
(v, `sin1_cmp` prints the range).

```bash
just capture sin1 default 2500           # CAPTURE_SWITCH defaults to switch_chaceon_st1 for sin1
just capture sin1 winddir 2500
just sin1-cmp /work/capture/sin1_default_2500m.bin
just spirv && just spirv-replay sin1 default 2500
```

## Fault runs

On `default, 2500 m`; "caught" is the count of `D` values that differ.

| Fault | Result |
|---|---|
| input `USTAR`, `USDIR`, `K`, `SINC1`, `SIG2`, `ECOS` or `ESIN` one ULP up (`sin1_cmp --fault <name>`) | all caught: 369 513, 408 931, 504 586, 539 862, 455 106, 292 321, 213 719 |
| input `A` one ULP up | caught in `S` (553 319), `D` unchanged as it must be |
| literal `0.035715` for `0.035714` | caught: 617 390 |
| `SINC1*(SIG2*MAX(...))` | caught: 210 379 |
| `K/SIG2` regrouped as `U*(...)*(K/SIG2)` | caught: 210 847 |
| `ECOS*SINU + ESIN*COSU` | caught: 826 629 |
| `MAX(0.001, ...)` for `MAX(0., ...)` | caught: 1 662 389 |
| `MAX` removed | caught: 1 654 066 (every clamped band) |
| `ECOS*COSU + ESIN*SINU` with one rounding (through `f64`, as a fused op would round) | caught: 124 123 |
| `(U*(...)*K)/SIG2 - 0.035714` with one rounding | caught: 216 490 |
| `S = D * A(IS+1)` | caught: 616 918 `S`, `D` unchanged |
| `ECOS(MOD(IS-1,NTH)+1)` for `ECOS(IS)` | **no effect**: `ECOS(NTH+1:)` repeats `ECOS(1:NTH)` (`w3gridmd.F90:1294`). Weak, not a blind spot |

`f32::mul_add` does not exist in `core`, so a fused multiply-add was emulated through `f64`. A NaN `USTAR`, `K` or
`A` was not run: `W3SRCE` never passes one ⚠.

## GPU code

- SPIR-V: entry point `sin1` in `rust/gpu-spirv/shader`, 9 storage buffers, passes `spirv-val` (`just spirv`).
- PTX: kernel `sin1` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`).
  The module has no float `fma` or `mad.rn` (v, `grep`); the body has `div.rn.f32` and `max.f32`. Not run ⚠.

## Translation notes

- Read from `__w3src1md_MOD_w3sin1` in the reference's `ww3_shel` (gdb `disassemble`, `objdump` is not in the image):
  one `sincosf` call for `COS(USDIR)` and `SIN(USDIR)`; per band `fmul, fmul, fadd` for `ECOS*COSU + ESIN*SINU`, then
  `fmul` by `USTAR`, `fmul` by `K`, `fdiv` by `SIG2`, `fsub` of `0.035714`, `fmaxnm` with 0, and a final `fmul` by
  `SINC1*SIG2`, which is formed first. No `fmadd`, `fmsub` or `fmla` in the routine (`-ffp-contract=off`). gfortran
  vectorises the loop four wide and keeps a scalar tail; the vector body adds `-0.035714` where the tail subtracts
  `0.035714`, which is the same value bit for bit.
- `0.035714` is a `REAL` literal.
- `MAX(0., X)` is `fmaxnm`; Rust's `f32::max` is the same operation, NaN included.
- `S = D * A` is a whole-array statement (`:389`), written as a second loop. No `W3_T*` or `W3_S` test output is in the
  build.
- `sincosf` is the one in `ww3-libm`, proven against glibc by `just libm-bits` (not rerun here: no new libm function).

## Not translated, not reached

- The call in `W3SRCE` (`w3srcemd.F90`, `ST1` branch): the per-point `USTAR` and `USDIR` come from `W3FLX1`, a
  separate port. `W3SIN1` is not wired into `srce.rs`.
- `W3EXPO` and `GXEXPO`, the point post-processors that also call it.
- A varying `USTAR` inside one run: `FLX1` of a constant wind gives a constant `USTAR` per capture, so the six captures
  hold four values (0.47699, 1.57321, 0.20698, 2.8e-5) ⚠. Other speeds change `USTAR` only through the same product.
- A NaN or infinite input ⚠.

## Timing

None. No timing without a device run (`AGENTS.md`).
