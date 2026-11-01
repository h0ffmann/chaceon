# 2026-11-01: `ww3_ounp` byte-identical on 36 regtest runs

2026-11-01, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-3301-ounp`.
Results: `rust/port_report/WW3_OUNP.md`.

## Question

`ww3_ounp` turns `out_pnt.ww3` into the NetCDF files of the point output, one per station or one for all. It is the
second program of this port that needs `libnetcdf`, and 5580 lines of Fortran, 2500 of them the definition of variables
and attributes. Can Rust write the same bytes, and how much of the program does that take?

## Result

Yes, for the part the regtests that run on `switch_chaceon_shrd` use. Rust's `ww3_ounp` is byte-identical to the
reference's on 36 runs of 33 cases (the regtests of `work/pair` and `ww3_ts2`), 50 NetCDF files, with
the namelist log, the standard output and the exit status (v, `just ounp-all`, 2026-11-01); on 297 namelist-variant runs, 1079
NetCDF files (v, `just ounp-variants`); and the reference writes the same bytes twice (v, `just ounp-selfcmp`: 70 NetCDF files from
14 inputs, 0 differences), so `cmp` of the whole file is the verdict and nothing is excluded. Ten faults, one in each stage (reading, a scale factor, an attribute, rounding, storage, a global attribute, a formula, the log, the status, the namelist log), were each
caught and restored (v, `rust/port_report/WW3_OUNP.md`).

## What it took

- The definition of the variables (`W3CRNC`) is a flat sequence of NetCDF calls, so it is generated from the Fortran
  (`scripts/ounp_crnc_gen.py`) rather than typed: 790 lines of Rust for the five (`TYPE`, output) pairs reached, and
  `--check` fails when the Fortran or the generator changes under it.
- The first sweep matched at once. What the work was, then, is reading the program: `FNMPRE` is `./`, `product_name` drops
  those two characters, the file name in format 2940 is 128 columns wide, `SINGLEFILE` is a stack slot that the namelist
  path never sets (read as true, as in `ww3_ounf`), and `CHECK_ERR` writes list-directed output with an 11-column integer.
  The last two showed only in the reference's own output, after a prefix in a missing directory (status 20) and a file with
  no station (status 22).
- `DIMORDER = F` is a trap of the Fortran: `TYPE` 2 writes with the same `start` whatever the order, and the library
  refuses the writes. Rust makes the same calls with the same `start`, `count` and shape, so the files agree; the
  `NF90_PUT_VAR` rule for a missing `count` (the array's shape in the first dimensions) is the part that mattered.
- A fault is only a test if it can show. The first one tried (two fields swapped in the reader) changed nothing on
  `ww3_tp1.1`, where the wind is zero, and was caught on `default_2500m`.
- The reference aborts with "malloc(): invalid size" when the point list has more words than the file has points
  (`STRSPLIT` writes past `POINTLIST`). That is refused here, not reproduced.
- A zero `POINT%TIMESTRIDE` with a start before the first output time never ends in the Fortran, and does not in Rust.
  The pair script stops both at 60 s and reports it.

## What did not get done

- `TYPE` 1 with `SPECTRA%OUTPUT` 2 and 4, `TYPE` 2 with `PARAM%OUTPUT` 3 and 4, and `TYPE` 3 stop with status 64: they call
  `W3SPR4`, `W3SIN4`, `W3SDS4` and `W3PART`. No regtest that runs on this switch uses them (`ww3_tic1.4` has `TYPE` 3 and
  needs `IS2`).
- `ww3_ounp.inp`, `NC_globatt.inp`, a calendar other than `standard` and unstructured grids stop with status 64.
- `SINGLEFILE` is the content of a stack slot, so the splitting of files by `TIMESPLIT` is not exercised ⚠.
- The variant sweep took about 25 minutes with other jobs sharing Docker; no timing of the program was taken.

## What it changes

A Rust run can now go from `out_pnt.ww3` to the point NetCDF files without the Fortran `ww3_ounp` for the spectra file and
the four parameter tables. With `ww3_ounf`, both NetCDF postprocessors have a Rust program for the options named; the source-term tables,
partitions and the old `.inp` input remain in Fortran.
