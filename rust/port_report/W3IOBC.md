# W3IOBC: boundary data files

`rust/crates/ww3-host/src/iobc.rs` translates `W3IOBC` with `INXOUT = 'WRITE'` and `'READ'`, and `W3UBPT`
(`model/src/w3updtmd.F90:1313-1502`), at `92e2cf66`. `cspc.rs` is `W3CSPC`, `gridsru.rs` has `W3GFPT` with the `FNCL` path of
`W3GFCL`, and `pro3.rs` has step 3.c of `W3XYP3`. `WRITE` gives the files `nest01.ww3`, `nest02.ww3` ... that `ww3_shel` writes at the times of
`DATE%BOUNDARY` for a grid with `OUTBND` points. `shel.rs` has a fourth output stream for it (`J` = 5,
`FLOUT(5) = FLBPO`, `w3initmd.F90:1099`).

**Status: `host`** for `WRITE` and `READ`. `DUMP` belongs to `ww3_multi` and is not translated. Nothing ran on a GPU ⚠:
`ww3_shel` refuses a `nest.ww3` on a device, as it refuses water levels and ice.

## Boundary input in a run

`just bound-all`: `out_grd.ww3`, `out_pnt.ww3` and the last restart are byte-identical on `ww3_tr1`'s `nest.ww3` and on the
variants of `scripts/bound_gen.py`, from a calm start, so every wave in these runs came in through the boundary (v):

| Case | What it reaches |
|---|---|
| `tr1`, `edge`, `two` | One station, and two with weights from `ww3_bound`; `NTLOC` above 1, so the sub-step time of step 3.c matters |
| `nonest` | No file: `IOTST = 1`, `FLBPI` cleared |
| `early`, `late` | Start before the first spectra; start after two records, so the read loop runs more than once in a step |
| `eof` | The file ends mid-run, `FLBPI` clears and the boundary stays as it was |
| `small` | `DTXY` of 600 s |
| `level` | A water level due at a time spectra are read: `W3UBPT` reads `CG` before `W3ULEV` |
| `rot`, `spec`, `coarse` | `SPCONV`, `W3CSPC`: another first direction, finer and coarser grids, the tail by `FACHFE` |
| `parent`, `child` | `nest01.ww3` written by one run and read by another grid, with bilinear `RDBPI` |
| `near` | Points 0.05 of a cell outside the grid: `W3GFCL`'s closest-cell search and `DCIN` |
| `threads` | `--threads 4` |
| `fault_id`, `fault_ver`, `fault_nodata`, `fault_far`, `fault_inner` | `W3IOBC`'s stops, status 10, 11, 43, 12 and 20 |

`just bound-faults` puts 16 faults in this code one at a time and each case must differ; see the log of 2026-11-02.

The hook is the trait `Xyp3Boundary`, not an argument of `Xyp3Tables`, so `NoBoundary` compiles away in the device entry
points and their bindings stay the same: `just spirv-replay` stayed bit-identical. Boundary input on a device needs three
more buffers ⚠.

## Result

`ww3_tp2.3` with two `OUTBND` lines (5 points and 4 points, appended to the regtest's `ww3_grid.nml`; the regtest has
none) and `DATE%BOUNDARY = '19680606 000000' '43200' '19680611 000000'`: `nest01.ww3` (80 142 bytes) and
`nest02.ww3` are byte-identical to the reference's, with `out_grd.ww3` and `restart001.ww3` still identical (v,
`just bnd-all`, `scripts/bnd_pair.sh`). The reference's `nest01.ww3` has SHA-256 `0440b0c9b9db9165...` on two runs (v).

| Fault | `nest01.ww3` |
|---|---|
| a factor 1.0000001 on `ABPOS` | differs |
| `VA*SIG2/CG` to `VA*(SIG2/CG)` | identical, not caught ⚠ (the garden-sprinkler spectra have few non-zero bins) |

## What stops

- A `nest.ww3` on a device.
- Pole and near-pole cells in `W3GFPT` and `W3GFCL` were not reached ⚠.
- Boundary input under `mpirun` was not run ⚠: every rank reads `nest.ww3` and `W3XYP3` runs on the whole field, as with
  `--threads`, which was. `just mpi-pair` ran only cases with no `nest.ww3`.
- A record with more spectra than the first: the Fortran writes past `ABPIN`, Rust stops.
- `DUMP`, and `W3IOBC` under `PDLIB`, `SMC`, `RTD` and `DIST`: not in this switch.

## Timing

None.
