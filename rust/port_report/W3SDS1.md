# W3SDS1

WAM cycle 3 whitecapping (switch `ST1`): one common factor from the mean parameters, then `D = FACTOR * K` and
`S = D * A` over the spectrum. Translated from `model/src/w3src1md.F90:432-586` at metocean/WW3 `92e2cf66`; the
executable lines are `:549-556`. Kernel: `rust/crates/ww3-kernels/src/sds1.rs`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts
and in compiled SPIR-V on lavapipe (ADR-0007). It is not called from `W3SRCE` yet, and no model run uses it ⚠. It has
run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `K(NSPEC)` | `REAL`, in; `K` is `WN2` of `W3SRCE` (`w3srcemd.F90:1288`) | `a`, `k` at offsets `o`, `k_off` |
| `EMEAN`, `FMEAN`, `WNMEAN` | `REAL`, in; from `W3SPR1` | `Sds1Point` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `o` |
| `SDSC1` | module `REAL`, `TPI * CDIS / APM**2` (`w3gridmd.F90:2140`) | `Sds1Params::sdsc1` |
| `NK`, `NTH` | module `INTEGER` | `Sds1Params` |

No `f64` anywhere (v, `w3src1md.F90`, read with `grep -in "double\|d0"`: no hit in the routine).

## The compiled order

`__w3src1md_MOD_w3sds1` in the reference's `ww3_shel` (`gdb disassemble`, gfortran 12.2 `-O3
-ffp-contract=off`, aarch64) computes, in `f32`:

```
t = WNMEAN*WNMEAN; t = t*WNMEAN          (WNMEAN**3 as a product chain, left to right)
f = SDSC1*FMEAN; f = f*t                 (SDSC1 * FMEAN * WNMEAN**3, left to right)
f = f*(EMEAN*EMEAN)                      (EMEAN**2 as a product, then the last multiply)
D(i) = f*K(i)   (vectorised, fmul v.4s)  then a second loop S(i) = D(i)*A(i)
```

`FACTOR = SDSC1 * FMEAN * WNMEAN**3 * EMEAN**2` is therefore `(((SDSC1*FMEAN)*((WNMEAN*WNMEAN)*WNMEAN))*(EMEAN*EMEAN))`,
which is what the kernel writes. Nothing fuses (`-ffp-contract=off`); the two loops are element-wise, so vectorising
them changes no bit. `S` is formed from the stored `D`, equal to `FACTOR*K` held in a register.

## Comparison with the reference

The reference is `switch_chaceon_st1` (`ST1`, `FLX1`; `just ref-build switch_chaceon_st1`). gdb records the
arguments and results of `W3SDS1` in `ww3_ts1` (`regtests/capture/sds1.py`). The case has `NK 36`, `NTH 24`,
`SDSC1 = -16.258408`. Outputs compared: `S` and `D`.

| Capture | Calls | `S` nonzero | Host differ | SPIR-V differ |
|---|---|---|---|---|
| default, 2500 m | 2629 | 2 152 277 | 0 | 0 |
| default, 8 m | 1904 | 1 466 822 | 0 | 0 |
| storm (30 m/s), 8 m | 4321 | 3 713 911 | 0 | 0 |
| storm, 2500 m | 1469 | 1 263 305 | 0 | 0 |
| light (6 m/s), 20 m | 1150 | 780 733 | 0 | 0 |
| calm (0 m/s), 8 m | 1081 | 0: every `S` and `D` is zero (`EMEAN` is 0), proves nothing | 0 | 0 |

12 554 calls, 21 693 312 values (`S` and `D`), 0 differ, in call layout and in the one-buffer layout of a device on the
host, and in compiled SPIR-V on lavapipe. Stride 1 for the 2500 m default, 2 for the rest. The first call of every
capture has `EMEAN = FMEAN = WNMEAN = 0` (the start state), which gives `FACTOR = 0`.

```bash
just capture sds1 default 2500          # CAPTURE_SWITCH defaults to switch_chaceon_st1 for sds1
just sds1-cmp /work/capture/sds1_default_2500m.bin
just spirv && just spirv-replay sds1 default 2500
```

## Faults

On `default, 8 m`, through `SDS1_FAULT` in `sds1_cmp` (`rust/tools/sds1_cmp/src/main.rs`); values of `S` and `D`
that differ, of 1 645 056 each.

| Fault | Result |
|---|---|
| `SDSC1` one ULP up (`sdsc1`) | caught: 1 208 034 `S`, 1 458 672 `D` |
| `FACTOR` as `SDSC1*(FMEAN*(WNMEAN**3*EMEAN**2))` (`order`) | caught: 593 496 `S`, 730 200 `D` |
| `FACTOR` in `f64`, narrowed once (`kind`) | caught: 430 387 `S`, 531 048 `D` |
| `WNMEAN**2` for `WNMEAN**3` (`cube`) | caught: 1 468 684 `S`, 1 644 192 `D` |
| `K` of the next bin (`kshift`) | caught: 61 259 `S`, 68 508 `D` (adjacent bins are often equal) |
| `S = FACTOR*(K*A)` for `S = D*A` (`sfirst`) | caught: 502 999 `S`; `D` untouched, 0 |

6 of 6 caught, in both layouts. The routine has no guard, no branch and no clamp (v, `w3src1md.F90:549-556`), so
there is nothing for a poked input to reach. A fault in the SPIR-V shader itself was not run: the replay compares the
shader with the same reference values and the host kernel is the same source ⚠.

## Not reached ⚠

- `SDSC1` has one value in every capture: the `CDIS` and `APM` namelist (`&SDS1`) was not varied, and
  `SDSC1 * FMEAN * ...` has no branch that depends on it.
- No `NaN`, infinity or negative `EMEAN` input; the routine has no guard, so the kernel has none.
- `W3EXPO` and `GXEXPO`, the point post-processors that also call `W3SDS1`.
- The state around the call in `W3SRCE` (`w3srcemd.F90:1288`) is not translated here: `WN2`, and the mean
  parameters of `W3SPR1`, which another port covers.

## GPU code

- SPIR-V: entry point `sds1` in `rust/gpu-spirv/shader`, 6 storage buffers, passes `spirv-val` (`just spirv`).
- PTX: kernel `sds1` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`).
  Its body has `mul.rn.f32` and no float `fma` or `mad` (the `mul.lo` and `mad` are integer addresses). Not run ⚠.

## Timing

None. No speed figure without a passed `host` verdict on this commit (`AGENTS.md`).
