# W3SRCE, W3SLN1, W3SBT1, W3SDB1

The source-term integrator and its three small routines, in `rust/crates/ww3-kernels/src/srce.rs`.
Translated from `model/src/w3srcemd.F90`, `w3sln1md.F90`, `w3sbt1md.F90` and `w3sdb1md.F90` at metocean/WW3
`92e2cf66`, **as `switch_chaceon_shrd` builds them**: the text of `gfortran -E -cpp -P` with the
switches defined, about 400 lines of the 2692.

**Status: `spirv`** in `rust/ports.tsv`: byte-identical whole runs with this kernel chained in the time loop on
lavapipe, with `VA` resident (`scripts/spirv_pair.sh`, `rust/port_report/W3PRO3.md`), and bit-identical on every
capture. It has run on no GPU ⚠.

## Comparison with the reference

One call of `W3SRCE` runs all of `W3SPR4`, `W3SIN4`, `W3SLN1`, `W3SNL1`, `W3SDS4`, `W3SDB1` and `W3SBT1`,
several times over. Its locals are reset on entry, so a captured call can replay alone.

| Tag | Calls | `W3SDB1` active | `W3SBT1` active | Host differ |
|---|---|---|---|---|
| `default_2500m`, `light_2500m`, `circle_2500m` | 721 each | 0 | 0 | 0 |
| `default_8m`, `default_20m`, `default_6m`, `strain_20m`, `t700_8m` | 721 each | 0 | 720 | 0 |
| `filipot_20m` | 721 | 700 | 720 | 0 |
| `storm_6m`, 30 m/s | 721 | 713 | 720 | 0 |
| `calm_2500m`, 0 m/s | 721 | 0 | 0 | 0 |

7 931 calls, 7 082 383 values, 0 differ: `SPEC`, 25 scalars per call, `WHITECAP`. Every third call of the
run, all `srce_direct`, one with `IT=0`.

```bash
just capture srce storm 6 3    # and snl1, sds4, sin4, spr4 for the tables, at any stride
just srce-cmp storm_6m
LIBM_F64_ONLY=1 just libm-bits # exp, expm1, tanh, sqrt: 8e9 samples each, 0 differ
```

## With `BT4`

`SrceParams::btopt` selects the bottom friction the build's switch selects: `W3SBT1`, or `W3SBT4`
(`w3srcemd.F90:1336-1342`), which also overwrites `TAUBBL` on every pass and `BEDFORM`, three values per point
in `SrceWork::bedform`. `W3SRCE` divides the last pass's `TAUBBL` by `DTG` and takes it from `TAUOX`, `TAUOY`
(`:2052-2058`). `ERFTABLE` is a new table; `D50` and `PSIC` join `SrcePoint`. With `BT1` none of it is read.

The capture's header now names the build's switch ("SRCB"; older "SRCE" files are `BT1`), and every capture of
a `bt4*` variant comes from `switches/switch_chaceon_bt4`. `srce_cmp` takes `SBTCX` from the case's `sbt4`
capture and builds `ERFTABLE` with `TABU_ERF`, as a run would.

| Tag, `switch_chaceon_bt4` | Calls | `W3SDB1` active | `W3SBT4` active | Host differ |
|---|---|---|---|---|
| `bt4_8m`, `bt4_20m` | 2161 each | 0 | 2160 | 0 |
| `bt4storm_8m`, 30 m/s | 2161 | 2129 | 2160 | 0 |
| `bt4ripple_8m`, `bt4ripple_20m` (ripple branch, per `W3SBT4.md`) | 2161 each | 0 | 2160 | 0 |
| `bt4coarse_8m`, `bt4flat_8m` | 2161 each | 0 | 2160 | 0 |

`just spirv-replay srce <variant> <depth>` on the same seven, one dispatch each: 0 differ, `BEDFORM` included (v,
lavapipe). `BEDFORM` written at offset 0, not `3*point`, passes the host replay, which runs one call at a time, and the SPIR-V
replay catches it: 5 695 values.
`spirv_shel` binds the new buffers but reads `mod_def.ww3` for `BT1` only, so no chained run has used `BT4` ⚠.

15 127 calls, every call of each run: 13 069 728 `SPEC`, 378 175 scalar, 60 508 `WHITECAP` and 45 381 `BEDFORM`
values, 0 differ (v, `just srce-cmp <tag>`). `BEDFORM` changed on every call. The 11 `BT1` captures above still
give 0 (v).

| Fault, `bt4_8m` unless named | Result |
|---|---|
| `TAUBBL(1)` left at 0 | caught: 6 414 scalars |
| `D50` and `PSIC` swapped | caught: 1 101 945 `SPEC`, 6 399 `BEDFORM` |
| `W3SBT1` called in a `BT4` build | caught: 1 129 467 `SPEC` |
| `TAUBBL(2)` one ULP up, `bt4storm_8m` | caught: 2 167 scalars |
| `D_INP` for `DEPTH` in `W3SBT4` | **no effect**: `DEPTH=MAX(DMIN,D_INP)` and every case is deeper than `DMIN` |

```bash
just capture srce bt4ripple 8           # bt4* variants use switch_chaceon_bt4
for r in snl1 sds4 sin4 spr4 sbt4; do just capture $r bt4ripple 8 1000; done
just srce-cmp bt4ripple_8m
```

## Fault runs

| Fault | Result |
|---|---|
| `W3SDB1` two fixed-point passes, not three | caught on filipot and storm |
| `DSQRT` of `W3SDB1` in `f32` | caught, 170 000+ `SPEC` values |
| `W3SBT1` `0.5` to `0.51` | caught where it is active |
| `W3SLN1` `RFR**(-4)` reassociated | caught |
| MetOcean's `USTAR` floor in `W3SRCE`, 0.001 to 0.002 | caught on calm |
| `SHAVE` limiter removed | caught |
| `HM` limiter, `/16.` to `/17.` | caught on filipot |
| `USTAR=0.001` at `IT=0` to 0.002 | **no effect**: the next `W3SPR4` overwrites it |
| `DBLE(1.E-15)` as `1e-15_f64` | **undetected**: no case lands near that threshold |

## SPIR-V on lavapipe

Bit-identical (v, `just spirv-replay`): 11 captures, 0 of 7 082 383 values, `SPEC`, the scalars and `WHITECAP`,
`W3SDB1`'s `f64` included. Before, `SPEC` differed up to 45 ULP and the scalars up to 2 534 235 ULP. Cause: float
folds on a module without `NoContraction`, in [the log](../../docs/log/2026-10-31-spirv-bit-parity.md). A dispatch
took about 50 s for 721 points: lavapipe on a laptop, with soft math. It is not a timing.

This replays each call alone. `W3SRCE` has not run chained as SPIR-V: `spirv_shel` dispatches only `W3XYP3` and
`W3KTP3` ⚠.

## GPU code

SPIR-V 3.0 MB, passes `spirv-val`, needs `Float64`, `Int64` and 52 storage buffers. Since `BT4`, the `srce`
entry point binds 56: `active` (53), `ERFTABLE` (54) and `BEDFORM` (55); the whole module is 4.3 MB. PTX 3.0 MB, five
kernels, 29 812 float operations, all `.rn`; no `fma`, no libdevice, no `ftz`. Neither has run on a GPU ⚠.

## Translation notes

- `SrceTables` and `SrceWork` are structs of slices: rust-gpu accepts them, so the 52 buffers do not reach
  the kernel's signature. The closures of `W3SNL1`'s first build failed for a different reason.
- `IT=0` keeps `SPEC`, and `USTAR=0.001` there changes nothing.
- `TWS` is set once, before the loop.
- `MAPWN(IS)` is computed, as the wavenumber index of `IS`.
- `W3SDB1` returns with `S` and `D` unchanged on a near-empty spectrum, so `VSDB` keeps its old value.
- `W3SDB1` reads `Q0` unset when `B > 1` in no path, so 0 stands in.

## Not covered ⚠

- `srce_imp_pre` and `srce_imp_post`, `SHRD`'s implicit paths: no capture ran them. `INFLAGS2(4)` and the ice
  scaling: no case has ice.
- `W3SDB1`'s `FDONLY` branch and its threshold cases.
- `switch_chaceon_shrd`, and its `BT4` twin through captures: every other `#ifdef` branch of `W3SRCE`.
- Other spectral grids, more than one sea point at a time inside WW3, threads, x86_64, any GPU.

## Timing

None.
