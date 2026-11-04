# 2026-11-04: `INSIN3`, `TABU_STRESS` and `TABU_TAUHF` at bit parity

ST3's table builders (`model/src/w3src3md.F90:726-1116`), translated into `rust/crates/ww3-host/src/insin3.rs`. Status
`routine`, three ledger rows (`rust/port_report/INSIN3.md`). Host side, so no SPIR-V, no PTX, nothing on a GPU, no
timings. Not called from a model run yet.

## Result

| Command | Result |
|---|---|
| `just capture insin3 <variant> 2500 1` for `st3`, `st3zmax`, `st3tune`, `st3grid`, `st3grid2` | one `INSIN3` call each, reference `switch_chaceon_st3` |
| `just insin3-cmp` on the five | `TAUT`, `TAUHFT`, `DELTAUW`, `DELU`, `DELUST`, `DELALP`: 0 of 303 530 values differ |
| faults 1 to 7 | caught, each by the captures that can reach it |
| fault 8, `SIG(NK)/TPI` for `SIG(NK)*TPIINV` | undetected, the two roundings agree on these grids |
| `just check`, `just lint-sh` | pass |

## What the first four captures could not see

`INSIN3` passes `SIG(NK)*TPIINV` and `TABU_TAUHF` multiplies it by `TPI`: in `f32` the pair returns `SIG(NK)` on the
default grid and on the `XFR=1.07`, `NK=40` grid, so a port that used `SIG(NK)` directly matched both. The same fault
failed on 11 241 `TAUHFT` values on a grid where the pair rounds (`XFR=1.05`, `SIG(NK)=1.6809176`). The grid was found
by a model of `f32` rounding in Python before any capture, then confirmed by the capture's `SIG(NK)`. `ZZ0MAX` is 0 in
ST3's defaults, so only `st3zmax` reaches its two `MIN` lines.

## Method

The capture stops gdb at the routine's return and ends the run, so each variant costs one `ww3_shel` start, not a
model run. `scripts/capture.sh` gained a `grid` hook (a `sed` on `ww3_grid.nml`) and the variants above; `rust_in_ref.sh`
passes `INSIN3_FAULT`.

## Not done

`INSIN3` is not called by Rust code that reads `mod_def.ww3`; `ZZ0MAX != 0` was run for one value; the optimised GIMPLE
of the module was not read.
