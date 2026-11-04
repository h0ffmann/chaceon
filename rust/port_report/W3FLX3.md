# W3FLX3

Wind stress after Tolman and Chalikov (1996) with a cap on the drag coefficient, the `FLX3` switch (used with `ST2`).
Translated from `model/src/w3flx3md.F90` at metocean/WW3 `92e2cf66`: `W3FLX3` (`:95-249`; the arithmetic is `:201-240`).
Kernel: `rust/crates/ww3-kernels/src/flx3.rs`. No new `ww3-libm` function: it uses `expf`, `logf`, `powf`, `sqrtf` and `tanhf`.
`W3SRCE` calls it at `model/src/w3srcemd.F90:1143` and `:1842`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical on the host. It is not called from `W3SRCE`, and it has
run on no GPU ⚠. Compiled SPIR-V on lavapipe (`just spirv-replay flx3`) is bit-identical on the same 14 captures: 0 of 27 588 values differ.

## Arguments

| Fortran | Kind | Rust |
|---|---|---|
| `ZWIND`, `DEPTH`, `FP`, `U`, `UDIR`, `UST` (inout) | `REAL`, in | `Flx3In` |
| `UST`, `USTD`, `Z0`, `CD` | `REAL`, out | `Flx3Out` |
| `NITTIN`, `CAP_ID` (`INTEGER`), `CINXSI`, `CD_MAX` (`REAL`), from `W3GDATMD` | per grid | `Flx3Params` |
| `DSIE`, `N1MAX`, `EWN1(0:121)` of `W3DISPMD` (inlined `WAVNU1`) | per run | `Flx3Params` and a table buffer |

No `f64` anywhere (v, `w3flx3md.F90`). `NITTIN = 3`, `CINXSI = 0.20` are set in `w3gridmd.F90:1628-1633` and no namelist changes them (v).
`CDMAX` (default `2.5E-3`) and `CTYPE` (0 a minimum, 1 a hyperbolic tangent; anything else becomes 0) are the `&FLX3` namelist
(`w3gridmd.F90:962`, `:1629-1647`). `UST` is `INTENT(INOUT)`: its value at entry is the first iteration's `MAX(UST,0.0001)`.

## Comparison with the reference

The reference is `switch_chaceon_st2flx3` (`ST2`, `FLX3`), built by the coordinator. gdb records the module tables once and
every sampled call of `ww3_ts1` (`regtests/capture/flx3.py`). A sampled call is one in `stride`: `W3SRCE` calls `W3FLX3` 5540 times at
2500 m and about 10 000 at 8 m, and each capture stops at a breakpoint twice per call, so only the 2500 m default and the poke runs are
fuller (stride 4 and 1). The sample is a limit of the evidence ⚠.

| Capture | Calls | What it reaches | Host differ |
|---|---|---|---|
| `flx3deep` 2500 m, 12 m/s (stride 4) | 1385 | deep branch of `WAVNU1`, minimum cap acts once | 0 |
| `flx3deep` 8 m (stride 40) | 248 | table branch 240, deep 8 | 0 |
| `flx3storm` 30 m/s, 8 m (40) | 271 | table 268, cap acts on all | 0 |
| `flx3storm` 2500 m (10) | 792 | cap acts on all | 0 |
| `flx3light` 6 m/s, 20 m (40) | 112 | | 0 |
| `flx3calm` 0 m/s, 8 m (40) | 109 | `U` floor 0.01 on all (`ww3_ts1` raises 0 to 0.001) | 0 |
| `flx3cap` `CDMAX = 1.E-3`, 30 m/s, 8 m (40) | 271 | the minimum at a cap that always acts, table branch | 0 |
| `flx3cap` 2500 m (10) | 980 | | 0 |
| `flx3tanh` `CTYPE = 1`, `CDMAX = 1.E-3`, 8 m (40) | 271 | the hyperbolic tangent | 0 |
| `flx3tanhdef` `CTYPE = 1`, default cap, 2500 m (10) | 814 | the hyperbolic tangent | 0 |
| `flx3zero` `CDMAX = 0`, 2500 m (10) | 433 | `CD = 0`, `UST = 0`, `Z0 = 0` | 0 |
| `flx3z` `&SIN2 ZWND = 15`, 8 m (40) | 211 | | 0 |
| `flx3poke` 8 m (1) | 500 | inputs written at the entry, below | 0 |
| `flx3poke` 2500 m (1) | 500 | | 0 |

6897 calls, 27 588 values, 0 differ, in the call layout (the table alone) and in a batch layout (all inputs in one buffer, all
results in one buffer, the table inside a larger buffer at an offset): `flx3_cmp` prints 0 differences between the two layouts on every capture.

The poke writes, on most of every 28 calls, one of: `FP` to 0, -1, NaN, 5 and 0.05; `U` to 0, -3, NaN, 100 and 0.0005; `UST` to 0, -1,
NaN and 5; `DEPTH` to 0.01, 1, 200, 10^5, 0 and -1; `ZWIND` to 0, 0.001, 100 and -1. The poked input is put back at exit and
the results are replaced by the last unpoked call's, so the run stays finite. Branch counts printed by `flx3_cmp` on the poke: `FP`
floor 54 of 500, `U` floor 72, `UST` floor 55, table branch 268 (8 m) and 124 (2500 m), the other branch 232 and 376.

```bash
just capture flx3 flx3deep 2500        # scripts/capture.sh picks switch_chaceon_st2flx3 for flx3 and flx3* variants
just capture flx3 flx3poke 8 1
just flx3-cmp /work/capture/flx3_flx3deep_2500m.bin /work/capture/flx3_flx3poke_8m.bin
just spirv && just spirv-replay flx3 flx3deep 2500
```

## Fault runs

Over the 14 captures, host, by editing `flx3.rs` (undone afterwards). 34 runs; the table is in
`docs/log/2026-11-04-flx3-parity.md`. Caught: every constant one ULP up (`0.57`, both exponents, `1.85`, `10.4`, `0.021`,
`0.001` at two ULP), the floors of `UST`, `U` and `FP`, no floor on `RDCH`, one iteration fewer, `CAP_ID` test inverted, the cap and
the tangent scale, the tangent argument, `<` for `<=` on the table (9 values), swapped interpolation weights, no division by `DEPTH`,
`GRAV` in the deep wavenumber, `CP` as a product, no `GRAV` or no `CINXSI` in the logarithm, `UST` from `U` rather than `UNZ`, `USTD = 0`,
the entry `UST` ignored, `Z0` without `ZWIND`, `DSIE` 0.1 percent up, and a NaN that wins a `MAX`.

Undetected, and why:

| Fault | Reason |
|---|---|
| `0.001` factor one ULP up (`0.0010000001`) | a weak fault: the literal rounds to the same `f32`. The two ULP version is caught |
| `DSIE` one ULP up in the index only | a weak fault: `SIX/DSIE` is rarely within a ULP of an integer; 0.1 percent is caught |
| a NaN that wins `MIN (CD_MAX, CD)` | unreachable ⚠: `CD` is never NaN (`RDCH` is at least 0, so `CD` is finite and positive) and `CDMAX = MAX (0., CDMAX)` is never NaN |
| `-0.4/SQRT(CD)` as `(-0.4)/SQRT(CD)` (the NaN sign of `flx1.rs`) | unreachable ⚠: it differs only for a negative or NaN `CD`, and none exists. The kernel keeps the `black_box` and the integer sign flip anyway, so the arithmetic is the Fortran's if a later change makes one |
| `MAX` returning `-0` for `+0` against `-0` | unreachable ⚠: no `MAX` here sees a `-0` (`LOG` gives `+0` at 1) |

## GPU code

- SPIR-V: entry point `flx3` in `rust/gpu-spirv/shader`, 4 storage buffers (params, `EWN1`, inputs, outputs).
- PTX: kernel `flx3` in `rust/gpu-cuda/kernels`. Assembled for sm_89 and sm_90 with `ptxas --fmad=false`; the whole PTX has no float `fma` or `mad` and no libdevice call. Not run ⚠.

## Translation notes

- Read from the optimised GIMPLE of `w3flx3` (gfortran 12.2, `-O3 -ffp-contract=off -fdump-tree-optimized`, the module built alone in
  `gcc:12-bookworm` with the `USE`d variables as stand-ins): `AFP = MAX_EXPR<FP, 0.001> * TPI`; the index `(int)(SIX/DSIE)` and
  `R1 = SIX/DSIE - (real)I1` share one quotient; `WNP = ((1-R1)*EWN1(I1) + EWN1(I1+1)*R1) / DEPTH`;
  `**1.5` and `**1.23` are `powf` calls (not `x*sqrt(x)`, which is a fast-math rewrite); `UNZ**2` is `UNZ*UNZ` and `ZWIND*GRAV` is
  hoisted out of the loop; `LOG` and `EXP` are `logf` and `expf`; `SQRT(CD)` is one `sqrtf` after the loop; `MIN (CD_MAX, CD)` is
  `MIN_EXPR`, and `CD_MAX * TANH (CD / CD_MAX)` is a division, `tanhf` and a product. The loop's `Z0` is dead: only the one after the cap is stored.
- `MAX` and `MIN` are `fmaxnm` and `fminnm`; the kernel writes the comparison out (`max_ww3`, `min_ww3`), which also gives `+0` for `+0` against `-0`.
- `NITTIN <= 0` would leave `CD` as the caller's value (`INTENT(OUT)` and never set); the kernel starts at 0. `NITTIN` is 3 in every build ⚠.
- `DEPTH < 0` gives `I1 <= 0` or a NaN-to-0 index and the Fortran may read `EWN1` below its start for a negative index. The poke
  writes `DEPTH = -1` and `0`: `INT(NaN)` is 0 on arm64 (`fcvtzs`), as in Rust, so `EWN1(0)` is read and no negative index arises. A
  quotient below -1 would index outside the table in both languages ⚠, and `W3SRCE` never calls with a dry point.

## Not reached

- `NITTIN` other than 3, `CAP_ID` other than 0 and 1 (`W3GRID` maps every other value to 0), `CINXSI` other than 0.20: not settable by a namelist.
- A NaN or negative `CD`, and a `-0` in a `MAX`: see the table above.
- `W3SRCE`'s state around the call, and the sampling stride above.

## Timing

None. No timing without a device run (`AGENTS.md`).
