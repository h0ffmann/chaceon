# 2026-11-01: `ww3_outp` and `ww3_outf` byte-identical on 40 regtest inputs

2026-11-01, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-1483-ww3-outf-outp`.
Results: `rust/port_report/WW3_OUTP.md`, `rust/port_report/WW3_OUTF.md`.

## Question

The two non-NetCDF postprocessors still ran in Fortran. Can Rust write the same bytes, for the log and for every
table and transfer file, from the `out_pnt.ww3` and `out_grd.ww3` that Rust already writes identically?

## Result

Yes for the options the regtests use. `just outp-all`: 21 inputs of WW3's regtests, 3 446 253 bytes, and 22 inputs
written here, 67 265 bytes, identical. `just outf-all`: 19 regtest inputs, 4 146 511 bytes (1 949 files, 1 807 of them
from `ww3_tp1.7`), and 27 inputs written here, 101 730 bytes, identical (v, 2026-11-01). The comparison covers the
standard output log and the exit status as well as the files. Six faults, six caught after two were moved to a case
that can see them (v), then restored.

Refused with a message and a `REFUSED` line, never skipped: `ww3_outp` `ITYPE` 1 `OTYPE` 2, `ITYPE` 2 `OTYPE` 3 and 4,
`ITYPE` 3, `ITYPE` 4; both programs on a `GTYPE` 3 grid (`ww3_tp2.6`); `ww3_outf` group 10.

## What it took

- A formatter module. Rust's `{:.N}` and `{:.Ne}` on the `f64` of a `REAL(4)` equal the C library's `snprintf`
  on 29 729 124 pairs (v, a throwaway check); the work is the field rules around the digits.
- A reader for `out_grd.ww3` (new) and for `out_pnt.ww3` (new), and for `ww3_outp.inp` and `ww3_outf.inp` a model of
  `NEXTLN` and the list-directed `READ` that continues over records.
- Faults that the first case could not see: wind direction and the order of two reals did nothing on `ww3_tp1.4`
  because that case has no wind. The comparison was right and the case was wrong.
- One real bug found by a variant, not by a regtest: the unformatted transfer-file header holds `XGRD` limits as eight
  bytes because `XGRD` is `DOUBLE PRECISION`.

## What did not get done

- The refused options above. `ITYPE` 3 needs `W3SIN4`, `W3SDS4` and the other source terms called on one point's
  spectrum from the host; the Rust kernels take `W3SRCE`'s arrays ⚠.
- Inputs of cases with no `out_pnt.reference` or `out_grd.reference` here (the `mww3_test_*`, `ww3_tbt*`, `ww3_tic*`,
  `ww3_ts2`, `ww3_tr1`, `ww3_tpt1.1` and the higher `ww3_tp2.*`).
- `cargo clippy --workspace` has warnings in older files and none is an error; the new files add style warnings of the
  same kinds.
