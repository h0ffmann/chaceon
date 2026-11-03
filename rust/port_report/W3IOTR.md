# W3IOTR, ww3_trck, ww3_trnc: spectra along tracks

`rust/crates/ww3-host/src/iotr.rs` translates `W3IOTR` (`model/src/w3iotrmd.F90:104-910` at `92e2cf66`), `trck.rs` translates
`ww3_trck`, and `rust/tools/ww3_trnc` translates `ww3_trnc` with `W3NMLTRNC`. `crnc.rs` there is generated from `W3CRNC` by
`scripts/trnc_crnc_gen.py`. `gridsru.rs` has `W3GFCL_R4`. Host only; no kernel changed.

**Status: `host`.** Nothing ran on a GPU ⚠.

## Result

`just track-all` compares `track_o.ww3` with the run's other files, then `ww3_trck` and `ww3_trnc` on the reference's
`track_o.ww3` (v):

| Case | What it reaches |
|---|---|
| `ww3_tp2.2` | The regtest's five points; `tp_all.sh` no longer removes the track |
| `many` | Points too early, too late, out of the grid, at one time, with no name, with a name over 32 characters, near cell edges |
| `nocmpr` | `TRCKCMPR` false, grid `ww3_grid_nocmpr.nml` |
| `bin` | `TYPE%TRACK%FORMAT` false, binary `track_i.ww3` |
| `win` | An output window that ends before `DTOUT` does, so `TOLAST` cuts `TIMEE` |
| `short` | The input ends before the run, `STOP` |
| `wrongid`, `empty` | Labels 803 and 802: no `track_o.ww3`, and the run goes on |
| `cart` | Cartesian, `ww3_tp2.3`, with the `E3` format in `track.ww3` |
| `curv` | Curvilinear, `ww3_tp2.15`, with water levels and ice, and a track through the northern rows: `SEA`, `LND`, `XCL`, `DRY`, `ICE` and `DIS` all reached, a 2.2 MB `track_o.ww3` (v) |

`ww3_trnc`: 17 inputs, every file, log and status identical, NetCDF 3 and 4 (the HDF5 files are the same bytes), `TIMESPLIT`
0 to 11, `TIMECOUNT`, `ww3_trnc.nml` and `.inp`, status 1 and 10.

`just track-faults`: 15 faults, 15 caught.

## Traps

- `MASK2` of one output is `MASK1` of the next, and `TRCKID` persists: a point in the second half of a window is written by
  the next pass.
- `BACKSPACE` on a point too late for this window is a held entry here, so the next call sees it first.
- `W3GFCL` is the `REAL(4)` form, `INTENT(INOUT)` on `XT`, and `RD` uses `XT` as the search left it. It runs with no `FNCL`.
- The swap of corners 3 and 4 comes before the compression, which reads `XX(1)`, not swapped.
- `ww3_trnc` never reads `mod_def.ww3`, so `CALTYPE` stays eight NUL bytes: `time` has no `long_name`, `calendar` holds the
  NULs, and `TSUB` counts no leap days. The value of `time` is therefore off from a `standard` calendar, as in the reference.
- A wrong or empty `track_i.ww3` is not fatal: `W3IOTR` sets `TOLAST` to `TIME` and the run goes on.

## What stops

- A line of `track_i.ww3` with more than five items: `STRSPLIT` writes past `LIST(5)` in the Fortran, and Rust drops the rest ⚠.
- `DSEC21` and `TICK21` in `ww3_trnc` have no leap day in the Fortran and do in Rust: a track across 29 February is not
  reproduced ⚠.
- An empty `track_o.ww3` in `ww3_trck`: the Fortran prints `TTST` unset ⚠.
- Poles and branch cuts in `W3GFCL_R4` ⚠.

## Timing

None.
