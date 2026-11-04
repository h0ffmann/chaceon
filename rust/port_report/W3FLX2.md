# W3FLX2

Stress of Tolman and Chalikov (1996) with the drag depending on the peak frequency, the `FLX2` switch (used with `ST2`).
Translated from `model/src/w3flx2md.F90` at metocean/WW3 `92e2cf66`: `W3FLX2` (`:90-231`; the reduced `WAVNU1` is `:190-206`,
the iteration on `UST` `:212-222`). Kernel: `rust/crates/ww3-kernels/src/flx2.rs`. No new `ww3-libm` function: it uses `powf`,
`logf`, `expf` and `sqrtf`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host and in compiled SPIR-V on lavapipe
(ADR-0007). It is not called from `W3SRCE` yet, and it has run on no GPU ⚠.

## Arguments

| Fortran | Kind | Rust |
|---|---|---|
| `ZWIND`, `DEPTH`, `FP`, `U`, `UDIR` (in), `UST` (inout) | `REAL` | `Flx2In` |
| `UST`, `USTD`, `Z0`, `CD` (out) | `REAL` | `Flx2Out` |
| `NITTIN`, `CINXSI` (`W3GDATMD`, set `w3gridmd.F90:1628-1629` to 3 and 0.20) | `INTEGER`, `REAL` | `Flx2Params` |
| `DSIE`, `N1MAX`, `EWN1(0:121)` (`W3DISPMD`, built by `DISTAB`) | `REAL`, `INTEGER`, `REAL` | `Flx2Params` and a table buffer |

No `f64` anywhere (v, `w3flx2md.F90`). The table is input to the kernel; `flx2_cmp` rebuilds it with `ww3-host`'s `Dispersion`
and compares it with what the reference held: equal in every capture (v).

## Comparison with the reference

The reference is `switch_chaceon_st2` (`ST2`, `FLX2`), built by the coordinator. gdb records the calls of `ww3_shel` on the
`ww3_ts1` case (`regtests/capture/flx2.py`, stride 1; `CAPTURE_POKE` writes other inputs at the routine's entry). `ww3_ts1` has
`DMIN = 5.75`, so 6 m is the shallowest depth it runs.

| Capture | Calls | Wind, depth | Table / deep | Floors reached | Host differ | SPIR-V differ |
|---|---|---|---|---|---|---|
| `st2flx2` | 5540 | 12 m/s, 2500 m | 0 / 5540 | | 0 | 0 |
| `st2flx2` | 7255 | 12 m/s, 10 m | 6851 / 404 | | 0 | 0 |
| `st2flx2calm` | 1500 | 0 m/s, 2500 m | 0 / 1500 | `U` 1500 | 0 | 0 |
| `st2flx2light` | 1500 | 6 m/s, 50 m | 0 / 1500 | | 0 | 0 |
| `st2flx2storm` | 1500 | 30 m/s, 2500 m | 0 / 1500 | `RDCH` 76 | 0 | 0 |
| `st2flx2storm` | 1500 | 30 m/s, 6 m | 1455 / 45 | `RDCH` 78 | 0 | 0 |
| `st2flx2gale` | 1500 | 60 m/s, 2500 m | 0 / 1500 | `RDCH` 1500 | 0 | 0 |
| `st2flx2gale` | 1500 | 60 m/s, 6 m | 1492 / 8 | `RDCH` 1500 | 0 | 0 |
| `st2flx2poke` | 2000 | poke, 2500 m | 811 / 1189 | `FP` 315, `U` 189, `UST` 253, `RDCH` 499 | 0 | 0 |
| `st2flx2poke` | 2000 | poke, 12 m | 811 / 1189 | same | 0 | 0 |
| `st2flx2pokecalm` | 1500 | poke, 0 m/s, 8 m | 517 / 983 | `FP` 235, `U` 1359, `UST` 189, `RDCH` 375 | 0 | 0 |

27 295 calls, 109 180 values (`UST`, `USTD`, `Z0`, `CD`), 0 differ, and the batch layout (all points in one array, the table
once) 0 differ. The non-poked captures are the first 1500 calls except the two full runs (5540 and 7255 calls), because gdb was slow
under load ⚠ (a run's later calls are not sampled for the capped variants). The poke writes, on 30 of every 32 calls in turn,
`FP` in {0, 1e-5, -0.3, NaN, inf, 0.0005}, `U` in {0, 0.005, -3, 80, 1000}, `UST` in {0, -1, NaN, 1e-5, 2}, `DEPTH` in
{0.001, 0.5, 5, 40, 1e5, 0, -10, NaN} (two with `FP`), and `ZWIND` in {1, 10, 100, 0} (`U = 25` once). Both poke runs count the
same branches because the poke list is the same and the real calls at the start of the run have `FP` either at the 0.001 floor
or high enough to be in deep water at either depth (v, 252 table and 1002 deep non-poked calls in both).

```bash
just capture flx2 st2flx2 2500           # CAPTURE_SWITCH defaults to switch_chaceon_st2 for flx2 and st2* variants
just capture flx2 st2flx2 10
just capture flx2 st2flx2poke 12
just flx2-cmp /work/capture/flx2_st2flx2_2500m.bin /work/capture/flx2_st2flx2_10m.bin   # and the others
just spirv && just spirv-replay flx2 st2flx2 2500
```

## Fault runs

Over the 11 captures; each fault is an edit of `flx2.rs`, undone afterwards, except the five input faults (`FLX2_FAULT`).

| Fault | Result |
|---|---|
| `TPI` one ULP up | caught: 13 220 values |
| `FP` floor 0.0011 | caught: 945 |
| `FP` floor removed | caught: panic, a negative table index (`FP <= 0`) |
| `FP` floor as `if m > 0.001 {m} else {0.001}` | **equivalent**: it equals `fmaxnm` on every input including NaN, so not a fault |
| `1-R1` replaced by `R1` | caught: 28 152 |
| `EWN1(I1+1)` read as `EWN1(I1)` | caught: 29 706 |
| `I1 .LE. N1MAX` as `.LT.` | caught: 30 (the single interval `I1 = 120`) |
| deep `WNP` with `GRAV` 9.8061 | caught: 38 973 |
| deep `WNP` as a product with the reciprocal | caught: 369 |
| `U` floor 0.011 | caught: 9243 |
| `UST` floor 0.00011 and removed | caught: 2091 each |
| 0.57, 1.5, 1.23, 1.85, 10.4, 0.021, 0.4 one ULP off | caught: 14 111, 17 795, 44 422, 15 835, 45 363, 41 273, 27 125 |
| 0.001 one ULP up | caught: 7255 `CD` in the 10 m capture (a first try with a literal that rounds to the same `f32` changed nothing) |
| `RDCH` floor removed | caught: 13 417 |
| `UNZ**2` as `UNZ` | caught: 78 267 |
| `CINXSI` one ULP up | caught: 19 834 |
| `UST` not updated in the loop | caught: 49 250 |
| one iteration fewer | caught: 41 660 |
| `USTD = 0` | caught: 24 277 |
| `ZWIND*GRAV` with 9.807 | caught: 68 304 |
| `Z0` without `ZWIND` | caught: 27 124 |
| `MAX` returning the other operand | caught: 81 715 |
| `MAX` propagating NaN | caught: 1038 |
| `MAX(+0,-0)` as `-0` (AND of the sign bits replaced by OR) | **undetected**: no `MAX` site sees a zero against a zero. The other operands are 0.001, 0.0001, 0.01 and `LOG`, and `LOG` is never `-0` ⚠ |
| the `black_box` and the integer sign flip of `-0.4/SQRT(CD)` | **cannot be reached**: `CD >= 0.001*0.021` is positive for every `RDCH >= 0`, so the quotient is never a NaN and its sign is never seen. Kept because it costs nothing ⚠ |
| `ZWIND`, `DEPTH`, `FP`, `U`, `UST` one ULP up (`FLX2_FAULT`) | all five caught |

## GPU code

- SPIR-V: entry point `flx2` in `rust/gpu-spirv/shader`, 4 storage buffers (params, `EWN1`, points, results); `spirv-val` passes in `just spirv`.
- PTX: kernel `flx2` in `rust/gpu-cuda/kernels`. Assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body has 9 `div.rn.f32` and no float `fma` or `mad`. Not run ⚠.

## Translation notes

- Read from `__w3flx2md_MOD_w3flx2` in the reference's `ww3_shel` (gdb `disassemble`): `fmaxnm` for all four `MAX` sites (`FP`, `U`, `UST`, `LOG`),
  `fmul` by `TPI` (`0x40c90fdb`), `fcvtzs` for `INT`, `cmp N1MAX` then `b.lt` to the deep-water branch, the interpolation as
  `fsub` `1-R1`, two `fmul`, one `fadd`, `fdiv DEPTH`, `bl powf` for both `**1.5` and `**1.23`, `bl logf`, `bl expf`. No
  `fmadd`. `UNZ**2` is one `fmul`, hoisted out of the loop, and `ZWIND*GRAV` is hoisted too. `Z0` is computed once after the loop,
  from the last `SQRT(CD)`; the kernel computes it every iteration and the last one wins, the same bits.
- `x**1.5` is `powf`, not `x*sqrt(x)`: that rewrite needs `-funsafe-math-optimizations`, and the disassembly has the call.
- `-0.4/SQRT(CD)` is `fdiv` then `fneg` (`-(0.4/SQRT(CD))`), as in `W3FLX1`.
- `IF (I1.LE.N1MAX)` has no lower bound. `AFP` is at least `TPI*0.001` and `SQRTH` is not negative, so `I1` is never negative: a NaN `DEPTH`
  gives `I1 = 0` (`fcvtzs`), as Rust's `as i32` does. A negative `DEPTH` makes `SQRTH` NaN, `WNP` NaN, `CP` NaN, `ALPHA` NaN, `RDCH` floored to 0, and the result
  is finite (v, the poke of -10 and NaN).
- For `NITTIN < 1` the Fortran leaves `CD` and `Z0` unassigned; the kernel returns 0 for both. `NITTIN` is 3 (`w3gridmd.F90:1628`) ⚠ never reached.

## Not translated, not reached

- `W3SRCE`'s state around the two calls (`:1139`, `:1838`): `U10ABS`, `U10DIR`, `FP` from `W3SPR2`, and `USTAR`, `Z0`, `CD` kept per point.
- A grid with `NITTIN != 3` or `CINXSI != 0.20`: both are inputs, but only 3 and 0.20 were captured ⚠.
- `W3FLX2` of `ww3_multi` and of a model with several grids: the table is per process.

## Timing

None. No timing without a device run (`AGENTS.md`).
