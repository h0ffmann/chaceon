# W3SPR6

ST6's mean wave parameters: mean energy `EMEAN`, mean frequency `FMEAN`, mean wavenumber `WNMEAN`, maximum action density
`AMAX` of one spectrum, with the spectral tail added from `FTE`, `FTF` and `FTWN`, and the peak frequency `FP` from
Young's weighted integral (1999, p239), sought only when `4*SQRT(EMEAN) > HSMIN`. `ST6` is not in the port's target
`switch_chaceon_shrd`; the reference for it is `switches/switch_chaceon_st6` (`ST6`, `FLX4`, `STAB0`). Translated from
`model/src/w3src6md.F90:121-257` at metocean/WW3 `92e2cf66`. Kernel: `rust/crates/ww3-kernels/src/spr6.rs`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts and in
compiled SPIR-V on lavapipe (ADR-0007). `W3SRCE` does not call it (it is wired for `ST4` only) ⚠. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NTH,NK)` | `REAL`, in | `a` at `spec_off`, `A(ITH,IK)` is `a[spec_off + ITH-1 + (IK-1)*NTH]` |
| `CG(NK)`, `WN(NK)` | `REAL`, in | `cg`, `wn` at `cg_off` |
| `EMEAN`, `FMEAN`, `WNMEAN`, `AMAX`, `FP` | `REAL`, out | `Spr6Out` |
| `EB(NK)` | automatic `REAL` | `eb` at `w_off`, reused for section 5 as the Fortran does |
| `NK`, `NTH`, `DTH`, `FTE`, `FTF`, `FTWN` | module | `Spr6Params` |
| `SIG(0:NK+1)`, `DDEN(NK)`, `DSII(NK)` | module `REAL` | `sig`, `dden`, `dsii` |

No `f64` anywhere (v, `w3src6md.F90:121-257`). `HSMIN = 0.05` is a `REAL` parameter (`:211`).

## Comparison with the reference

Reference: `chaceon-ref-capture:switch_chaceon_st6`. gdb records the arguments and the five results of `W3SPR6` on
`ww3_ts1` (`regtests/capture/spr6.py`). `spr6_cmp` counts the branches from the inputs.

| Capture (variant, depth, stride) | Calls | Gate open / shut | FP floor | Host differ | SPIR-V differ |
|---|---|---|---|---|---|
| `st6spr6`, 2500 m, 3, whole run | 2357 | 2352 / 5 | 40 | 0 | 0 |
| `st6spr6`, 20 m, 6 | 1200 | 1197 / 3 | 20 | 0 | 0 |
| `st6spr6storm`, 8 m, 6 | 1200 | 1199 / 1 | 5 | 0 | 0 |
| `st6spr6storm` (30 m/s), 2500 m, 6 | 1200 | 1199 / 1 | 5 | 0 | 0 |
| `st6spr6gale` (45 m/s), 2500 m, 6 | 1200 | 1199 / 1 | 3 | 0 | 0 |
| `st6spr6light` (6 m/s), 2500 m, 6 | 738 | 731 / 7 | 57 | 0 | 0 |
| `st6spr6calm` (0 m/s), 2500 m, 6 | 721 | 0 / 721 | 0 (every spectrum is zero) | 0 | 0 |
| `st6spr6dir` (wind from 45 deg), 2500 m, 6 | 1143 | 1140 / 3 | 20 | 0 | 0 |
| `st6spr6grid` (NK 40, XFR 1.07), 2500 m, 6 | 746 | 744 / 2 | 12 | 0 | 0 |
| `st6spr6grid2` (XFR 1.05), 2500 m, 6 | 724 | 719 / 5 | 66 | 0 | 0 |
| `st6spr6poke`, 2500 m, 1040 calls | 1040 | 556 / 484 | 236 | 0 | 0 |

12 269 calls, 61 345 results, 0 differ, in call and batch layouts (hence 122 690 comparisons on the host); SPIR-V on
lavapipe, 61 345 values, 0 differ, max 0 ULP. "FP floor" counts calls with the gate open and `SUM(EB**4*DSII)` below
`1.E-10`, recomputed from the inputs; the `1.E-7` floors of section 4 are counted from the reference's own outputs.

`ww3_ts1` reaches the `1.E-10` floor by itself (20 to 66 calls a capture) and the closed gate only at the start (3 to 7
calls), never the `1.E-7` floors. The poke capture (`CAPTURE_POKE`, 1040 calls) rewrites `A` at the entry of the real
`W3SPR6` and restores it at the exit: the whole spectrum scaled onto 20 chosen significant wave heights (0.05 straddled
by plus or minus 1e-7 and 3e-7 relative, 0.0499, 0.0501, 0.06 up to 1e10 and down to 1e-20), an empty spectrum, one bin at
the middle, at the lowest and at the highest frequency, a negated spectrum, and one NaN bin placed in four positions
(40 calls). It reaches the `HSMIN` gate on both sides with 198 calls within 1e-5 of `HSMIN`, both `1.E-7` floors
(200 calls each), the `1.E-10` floor (236), negative `EMEAN` (40) and NaN `EMEAN` (40).

```bash
just capture spr6 st6spr6 2500 3        # CAPTURE_SWITCH defaults to switch_chaceon_st6 for spr6
CAPTURE_LIMIT=1040 just capture spr6 st6spr6poke 2500 1
just spr6-cmp /work/capture/spr6_st6spr6_2500m.bin /work/capture/spr6_st6spr6poke_2500m.bin
just spirv && just spirv-replay spr6 st6spr6poke 2500
```

## Fault runs

Kernel faults: the kernel edited, `spr6_cmp` run (natively, `cargo run --release -p spr6_cmp`) over all 11 captures.
Input faults (`SPR6_FAULT`): one input one ULP up, same captures. Counts are results that differ.

| Fault | Result |
|---|---|
| `A` one ULP up (every nonzero) | caught: 8419 `EMEAN`, 7258 `FMEAN`, 7084 `WNMEAN`, 11 457 `AMAX`, 6048 `FP` |
| `CG` | caught: 8613 `EMEAN`, 7367 `FMEAN`, 7145 `WNMEAN`, 6376 `FP`; `AMAX` does not read it |
| `WN` | caught: 4962 `WNMEAN` only |
| `SIG` | caught: 7757 `FMEAN`, 7751 `FP` |
| `DDEN` | caught: 8450 `EMEAN`, 7299 `FMEAN`, 7110 `WNMEAN`, 9 `FP` |
| `DSII` | caught: 5662 `FP` only |
| `DTH` | caught: 6531 `FP` only |
| `FTE` | caught: 369 `EMEAN`, 306 `FMEAN`, 336 `WNMEAN`, 3 `FP` |
| `FTF`, `FTWN` | caught: 164 `FMEAN`; 201 `WNMEAN` |
| `FMEAN` floor `1.E-6` for `1.E-7`, `WNMEAN` floor `1.E-6` | caught, by the poke capture only |
| `1.E-10` floor at `1.E-9`, at `1.E-11` | caught (many `FP` in every `ww3_ts1` capture) |
| `HSMIN` 0.0501; gate `>=`; gate on `2*SQRT` | caught; `>=` and 0.0501 by the poke capture only (83 and 27 `FP` on the first, 780-call poke capture), `2*SQRT` by `ww3_ts1` captures too |
| `SUM(A,1)*(DDEN/CG)` for `*DDEN/CG` | caught |
| `E*(1/SIG)`, `E*(1/SQRT(WN))`, `TPIINV*(EMEAN/MAX)` | caught |
| no square in `WNMEAN`; tail from `EB(NK-1)`; `FTF` for `FTWN` | caught |
| peak `EB` as `S*(SIG/CG)*DTH`, without `DTH`, with `DTH` first | caught |
| `EB**4` as `((E*E)*E)*E`, in the numerator or the denominator | caught |
| `SIG*(E4*DSII)`; no `SIG` in the numerator; `SIG` in the denominator | caught |
| `DSII(IK+1)` | stops with a bounds panic (exit -6), so a kernel that reads out of range cannot pass |
| no `TPIINV` on `FP`; `FP/6.2831855` for `FP*TPIINV` | caught |
| `AMAX` starting at 0 | caught by the negated spectrum (40 `AMAX`, `-0` against `0` first) |
| `AMAX` taking a NaN bin | caught only once the NaN sat in four positions (10 `AMAX`); with one fixed position the peak lay after it and nothing changed |
| `ITH` reversed in `EB` or in the peak sum; `IK` reversed in `EMEAN` | caught |
| `FP` initialised to 1 | caught (closed gate) |
| `SQRT(WN)` as `WN` | caught |
| `AMAX` with `<=` for `<` | missed: equivalent, a tie replaces equal bits (zeros are `+0`, the start is `-Inf`) |
| `MAX(1.E-7, x)` without the NaN branch | missed: equivalent here. A NaN reaches `FMEAN` and `WNMEAN` only through `EMEAN`, which is then NaN too, so `NaN/MAX` is NaN either way ⚠ |

## GPU code

- SPIR-V: entry point `spr6` in `rust/gpu-spirv/shader`, 9 storage buffers. `just spirv` passes.
- PTX: kernel `spr6` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body has no
  float `fma` or `mad` (the four `mad` are `mad.lo.s32`, addresses) (v, `grep` of the entry in `chaceon.ptx`). Not run ⚠.

## Translation notes

- Array statements: `SUM(A,1)` is a sum over `ITH` in order from `+0`, `SUM(EB)`, `SUM(EB/SIG(1:NK))` and the rest in `IK`
  order. gfortran does not reassociate a `REAL` reduction without `-ffast-math`. `SUM(A,1)*DDEN/CG` associates to the left,
  `EB**4` is `(EB*EB)*(EB*EB)`, `**2` is a product. It matched the reference on the first run; the optimised GIMPLE was
  not read ⚠, because nothing differed.
- `MAXVAL(A)` starts at `-Inf` and takes a larger bin (`if amax < av`), which skips a NaN bin as the reference does
  (40 NaN calls) and returns the true maximum of a negated spectrum. `f32::max` is not used (`fmaxnm` against SPIR-V `FMax`,
  `rust/port_report/W3SPR1.md`).
- `MAX(1.E-7, x)` and `MAX(1.E-10, x)` are `if x > c { x } else { c }`, which is `fmaxnm` for a positive constant `c`.

## Not translated, not reached

- `W3EXPO` and `GXEXPO`, the point post-processors that also call it.
- `4*SQRT(EMEAN)` exactly equal to `HSMIN` in `f32`: the poke straddles 0.05 by 1e-7 relative, but no call is known to land
  on the tie ⚠. An infinite `EMEAN` (the 1e10 target may overflow `EB**4` to `Inf`) is not counted ⚠.
- The `W3_T` and `W3_S` test output and tracing, which the switch does not enable.

## Timing

None. No timing without a device run (`AGENTS.md`).
