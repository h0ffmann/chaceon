# 2026-10-31: `ww3_grid` and the `mod_def.ww3` writer in Rust

2026-10-31, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-5582-ww3-grid`.
Results: `rust/port_report/WW3_GRID.md`.

## Question

`mod_def.ww3` from the Fortran `ww3_grid` was the last Fortran input a normal run depended on. Can Rust write it, byte
for byte, for the regtests this repository runs?

## Result

Yes for `RECT` and `CURV` grids read from `ww3_grid.nml`: `mod_def.ww3` is byte-identical on 28 cases, 982 946 392 bytes
(v, `just grid-all`; the cases are in the report): `ww3_ts1` with its namelist variants and two depths, `ww3_tp1.1` to
`ww3_tp1.10`, `ww3_tp2.1` to `ww3_tp2.4` with the curvilinear variant, and `ww3_tp2.15`. The writer alone round-trips
all 28 reference files (v, `just iogr-roundtrip`). Five faults, one per stage, were each caught and then restored (v).

## What it took

- The writer came first. `ModDef::read` skipped dozens of fields (names, `NBO`, the ice constants, `GRIDSHIFT`,
  `MAPTH`, the output boundary arrays, the partitioning constants, `SSDSBRF1`, and more), so each became a field.
  `iogr.rs` writes them in `W3IOGR`'s order. Reading each of the 28 reference files and writing it again gives the
  same bytes before any grid code existed (v, `just iogr-roundtrip`).
- The first whole-file run of the grid code was 330 bytes short, the eleven 30-character names of the first record;
  after them it differed in four bytes, `ICLOSE`: `ICLOSE_NONE` is -1 in `w3gsrumd.F90:314`, not 0. Nothing else
  differed on `ww3_ts1`.
- Depth files use repeat counts (`1080*1`); the first sweep stopped on them.
- Output boundary points (`ww3_tp2.2`, 200 points in two files) needed a part of the grid search utility: the bucket
  search, the cell test, the Newton step for the cell position and the bilinear weights, all in `REAL(8)`.
- `W3NMLGRID` reads the excluded-body namelist with the excluded-point reader, so a body in the namelist file never
  reaches `W3GRID` at this pin. Rust keeps that.

## What did not get done

- `UNST` and `SMCG` grids, the old `ww3_grid.inp`, `IDFM` 2 and 3 and tripole closure stop with a message.
- Excluded points and bodies, odd `TRFLAG`, `PTM` 2 to 5 and most namelist keys were not run by any case ⚠; the report
  lists them.
- `ST4TABUHF2.bin` and `ww3_grid.nml.log` are not written.
- `GRAV**4` and `TPI**4` as four products give the same `f32` as the exact power, so the cases do not show whether
  gfortran folds with one rounding ⚠.
- The sweep took about 40 minutes with another job sharing Docker; no timing was taken.

## What it changes

A Rust run no longer needs the Fortran `ww3_grid` for `RECT` and `CURV` grids of the 28 cases. `ww3_prep` is still
Fortran where a case has a current file, and `ww3_strt` start type 2 is still not translated.
