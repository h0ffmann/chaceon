# 2026-10-31: `ww3_strt` byte-identical on 16 regtests

2026-10-31, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-3318-ww3-strt`.
Results: `rust/port_report/WW3_STRT.md`.

## Question

A Rust run still read the `restart.ww3` that the Fortran `ww3_strt` wrote. Can Rust write it, bit for bit, for
every start type the regtests use?

## Result

Yes for `ITYPE` 1, 3, 4 and 5: `restart.ww3` is byte-identical on 16 cases, 49 091 988 bytes (v,
`just strt-pair`; the cases are in the report). The first full sweep matched at once, so a fault went through
each type before the zeros were believed: six faults, six caught, then restored (v).

## What it took

- For this switch the program is the `W3_SHRD` path of `ww3_strt.F90`: `W3_DIST`, `W3_MPI`, `W3_WRST` and `W3_EXPORTWWM` are off, and `O4`, `O5` and `O6` only print.
- `COLD` writes no time record, and the header (1078 bytes) is longer than a record on small grids, so
  the writer follows WW3's order and positions.
- `HPFAC` and `HQFAC` already existed in `grid::Metrics`, and `FR1` was dropped by the `mod_def.ww3` reader, so one
  field was added.

## What did not get done

- `ITYPE` 2 stops with a message. Three regtests use it ⚠.
- `cargo clippy --workspace` has one error that is not in this change: `reversed_empty_ranges` at
  `rust/crates/ww3-host/src/shel.rs:1017`, which another change is editing. With
  `-- -A clippy::reversed_empty_ranges` the new code has no diagnostic.

## What it changes

A Rust run no longer needs the Fortran `ww3_strt` to start a case from `mod_def.ww3` and `ww3_strt.inp`. `ww3_grid` is still
Fortran, and `ww3_prep` where a case uses it.
