# ww3_bound

`rust/crates/ww3-host/src/bound.rs` translates `ww3_bound` (`model/src/ww3_bound.F90:22-751` at `92e2cf66`) and
`W3NMLBOUND` (`w3nmlboundmd.F90:44-332`): `nest.ww3` from point spectra in WW3's text format, and the `READ` mode that
prints such a file. Host only, no kernel.

**Status: `host`.** Nothing ran on a GPU ⚠.

## Result

`just bound-all`: `nest.ww3`, the log, the status and `ww3_bound.nml.log` are byte-identical on `ww3_tr1`'s own
`input_bndin` and on the inputs of `scripts/bound_gen.py` (v). These cover `ww3_bound.nml` and `ww3_bound.inp`, `INTERP`
1 and 2, `VERBOSE` 0 and 1, `MODE = 'READ'`, spectra starting under `FREQ1` (`IFMIN`), over it (`IFMIN2`), with fewer
frequencies than the grid, and the stops: status 66 to 69 and the bare `STOP` on files of different size. Status 70, a
spectrum file that is missing, and an `.inp` with no `'STOPSTRING'` were not run ⚠.

`ww3_tr1` needs `RTD`, which this switch has not, so the grid runs unrotated and its three stations are far outside it.
Every boundary point then takes weights 1, 0 from one station. Stations on the boundary, from `bound_gen.py`, give the
two-station weights.

## Traps

- `MOD(2.5*PI-THETA(1),TPI)` is `fmodf`, Rust's `%`. The header's `NK` is `NK1`, but each spectrum holds `NK*NTH` values, so
  the records are longer than the header says when `NK1 < NK`. `W3IOBC` reads only the first `NKI*NTHI` of each.
- `ABPIN` is allocated and not cleared. Bins past `NK1*NTH` are `malloc`'s bytes in the Fortran and zero here; the one
  input that reaches them, `fewf`, was identical, which proves that heap only ⚠.
- With spectra from a higher first frequency and fewer bins, the `IFMAX` search reads `FREQ(J)` past `NK1`. The value there
  is heap padding; Rust takes zero, which loses every comparison as that padding did on `highf` ⚠.
- `XFR**(J-1)` with a variable exponent is `__powisf2`, translated in `powi`.
- `NDSE` is 6 in this program, so its error messages are in the log, and an `OPEN` with no `ERR=` stops in the runtime
  with status 2.

## What stops

`ww3_bound` on an unstructured grid stops in `ModDef::read`. `RTD` is not in this switch.

## Timing

None.
