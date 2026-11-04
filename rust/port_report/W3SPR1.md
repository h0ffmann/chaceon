# W3SPR1

The mean parameters of WAM cycle 3 (`ST1`): mean energy `EMEAN`, mean frequency `FMEAN`, mean wavenumber `WNMEAN` and the
maximum action density `AMAX` of one spectrum, with the spectral tail added from `FTE`, `FTF` and `FTWN`. `ST1` is not in
the port's target `switch_chaceon_shrd`; the reference for it is `switches/switch_chaceon_st1` (`ST1`, `FLX1`, `STAB0`
in place of `ST4`, `FLX4`). Translated from `model/src/w3src1md.F90:87-240` at metocean/WW3 `92e2cf66`. Kernel:
`rust/crates/ww3-kernels/src/spr1.rs`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts and in
compiled SPIR-V on lavapipe (ADR-0007). `W3SRCE` does not call it yet (`W3SRCE` is wired for `ST4` only) ⚠. It has run
on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NTH,NK)` | `REAL`, in | `a` at `spec_off`, `A(ITH,IK)` is `a[spec_off + ITH-1 + (IK-1)*NTH]` |
| `CG(NK)`, `WN(NK)` | `REAL`, in | `cg`, `wn` at `cg_off` |
| `EMEAN`, `FMEAN`, `WNMEAN`, `AMAX` | `REAL`, out | `Spr1Out` |
| `EB(NK)` | automatic `REAL` | `eb` at `w_off` |
| `NK`, `NTH`, `FTE`, `FTF`, `FTWN` | module | `Spr1Params` |
| `SIG(0:NK+1)`, `DDEN(NK)` | module `REAL` | `sig`, `dden` |

No `f64` anywhere (v, `w3src1md.F90:87-240`). `TPIINV` is `CONSTANTS`' `REAL` 1/(2π), the same value `W3SPR4` uses.

## Comparison with the reference

Reference: `chaceon-ref-capture:switch_chaceon_st1`. gdb records the arguments and the four results of `W3SPR1` on
`ww3_ts1` (`regtests/capture/spr1.py`); `poke` rewrites `A` at the entry, so the real Fortran runs the floors.

| Capture | Calls | Nonzero `EMEAN` | Floors acting (`FMEAN`, `WNMEAN`) | Host differ | SPIR-V differ |
|---|---|---|---|---|---|
| default, 2500 m, stride 3, whole run | 1597 | 1596 | 0, 0 | 0 | 0 |
| default, 20 m, first 2399 calls, stride 2 | 1200 | 1198 | 0, 0 | 0 | 0 |
| calm (0 m/s), 2500 m, same | 1200 | 0 | 0, 0 (every spectrum is zero) | 0 | 0 |
| light (6 m/s), 2500 m, same | 1200 | 1198 | 0, 0 | 0 | 0 |
| storm (30 m/s), 2500 m, same | 1200 | 1198 | 0, 0 | 0 | 0 |
| storm, 20 m, stride 3, whole run | 1896 | 1895 | 0, 0 | 0 | 0 |
| poke, 2500 m, 500 calls | 500 | 499 | 374, 374 | 0 | 0 |

8793 calls, 35 172 results, 0 differ, in call and batch layouts. `ww3_ts1` never reaches either floor by itself: the poke
capture does (spectra scaled by 1e-12, negated, or a single 3e-3 bin; the real arguments are restored at the exit).
"Floors acting" is counted from the reference's own outputs (`FMEAN` bit-equal to `TPIINV*EMEAN/1.E-7`).

```bash
just capture spr1 default 2500 3        # CAPTURE_SWITCH defaults to switch_chaceon_st1 for spr1
CAPTURE_LIMIT=500 just capture spr1 poke 2500 1
just spr1-cmp /work/capture/spr1_default_2500m.bin /work/capture/spr1_poke_2500m.bin
just spirv && just spirv-replay spr1 poke 2500
```

## Fault runs

Over the captures above (5 for the kernel faults, all 7 for the input faults). Counts are results that differ.

| Fault | Result |
|---|---|
| every nonzero `A` one ULP up (`SPR1_FAULT=a`) | caught: 5497 `EMEAN`, 4889 `FMEAN`, 4624 `WNMEAN`, 7459 `AMAX` |
| every `CG` one ULP up (`cg`) | caught: 5654 `EMEAN`, 4975 `FMEAN`, 4600 `WNMEAN`; `AMAX` does not read `CG` |
| every `WN` one ULP up (`wn`) | caught: 3094 `WNMEAN` only |
| `FTE` one ULP up (`fte`) | caught: 67 `EMEAN`, 51 `FMEAN`, 55 `WNMEAN` |
| `FTF` one ULP up (`ftf`) | caught: 13 `FMEAN` |
| `FTWN` one ULP up (`ftwn`) | caught: 16 `WNMEAN` |
| `FMEAN` floor `1.E-6` for `1.E-7` | caught by the poke capture: 377 `FMEAN` |
| `WNMEAN` floor `1.E-6` for `1.E-7` | caught by the poke capture: 375 `WNMEAN` |
| `EB*(DDEN/CG)` for `EB*DDEN/CG` | caught: 1685 `EMEAN`, 2218 `FMEAN`, 2161 `WNMEAN` |
| `E*(1/SIG)` for `E/SIG` | caught: 883 `FMEAN` |
| `E*(1/SQRT(WN))` for `E/SQRT(WN)` | caught: 1070 `WNMEAN` |
| `TPIINV*(EMEAN/MAX)` for `TPIINV*EMEAN/MAX` | caught: 2486 `FMEAN` |
| `AMAX` starting at `A(1,1)`, not 0 | caught by the poke capture (negated spectrum): 125 `AMAX` |
| no square in `WNMEAN` | caught: 6386 `WNMEAN` |
| tail from `EB(NK-1)` | caught: 6256 `EMEAN` |
| `ITH` summed in reverse order | caught: 2926 `EMEAN`, 3604 `FMEAN`, 3369 `WNMEAN` |

## GPU code

- SPIR-V: entry point `spr1` in `rust/gpu-spirv/shader`, 8 storage buffers.
- PTX: kernel `spr1` in `rust/gpu-cuda/kernels`. Assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body has `div.rn.f32` (6) and no float `fma` or `mad` (the three `mad` are `mad.lo.s32`, addresses). Not run ⚠.

## Translation notes

- The routine is short enough to read as it is: sums run in `IK`, `ITH` order, `EB(IK)*DDEN(IK)/CG(IK)` associates to the
  left, `**2` is a product, and `SQRT` is `sqrtf` (correctly rounded). It matched the reference on the first run; the
  optimised GIMPLE was not read ⚠, because nothing differed.
- `MAX(1.E-7, ...)` is `f32::max`. `MAX(AMAX, A(ITH,IK))` is `if amax < av { amax = av }`, not `f32::max`: gfortran's
  `fmaxnm` returns +0 for `MAX(+0, -0)` and skips a NaN, which `f32::max` does on the host, but SPIR-V's `FMax` on lavapipe
  returned -0 there. The poke capture (negated spectrum, so `-0.` bins) found it: `spirv-replay spr1 poke` had 125 of
  2000 values differ, `-0` against `0`, with `f32::max`. `amax` starts at +0 and is replaced only by a larger value, so
  it is never -0, and the comparison is `fmaxnm` for every reachable input. A real spectrum is never negative, so
  this does not touch a model run.

## Not translated, not reached

- `W3EXPO` and `GXEXPO`, the point post-processors that also call it.
- A NaN or infinite spectrum, and `EMEAN` with a nonzero `AMAX` of the opposite sign: no capture reaches them ⚠.
- The `W3_T` and `W3_S` test output and tracing, which the switch does not enable.

## Timing

None. No timing without a device run (`AGENTS.md`).
