# First whole-file verdict: out_pnt.ww3 on ww3_ts1

2026-10-31, agent session `cd86e8f5` for the owner; no issue. Branch
`goal/cd86e8f5-3381-ts1-whole-run`. ADR-0007 accepted by the owner: "accept ADR-0007, continue porting."
Results: `rust/port_report/WW3_SHEL_TS1.md`.

## Question

Does the port hold when each step feeds the next, with no input from the Fortran but `mod_def.ww3`?

## Result

`out_pnt.ww3` is byte-identical on 18 cases, 137 739 924 bytes (v, `just ts1-pair`). The first run
matched, so provenance and two faults were checked before believing it (v).

## What it took

- `gfortran -E` cut `W3IOGR` and `W3WAVE` to what this switch builds.
- `ww3_ts1` sets only `RUN%FLSOU`, so most of `W3WAVE` sits behind flags it never sets.
- Between calls of `W3SRCE` the driver carries only `SPEC`, `UST` and `USTDIR`
  (`rust/crates/ww3-host/src/shel.rs:348-350`), so ADR-0007's routine parity carried over to a chain.

## What went wrong on the way

- **Little-endian was assumed.** `mod_def.ww3` looked like one record of a negative length: WW3's CMake
  builds big-endian.
- **`ICESCALES` was read three fields early.** `moddef_cmp` caught it, and the read is now anchored at
  the end of the record.
- `RLGTYPE` is 1, not 3, and `TFN` is compared with `DSEC21(TIME0,TTT)`: both caught by reading.

## What it changes

Next: more than one sea point, `W3PRO3` and `W3QCK` propagation, restart, `out_grd.ww3`, then
`ww3_tp2.15`.
