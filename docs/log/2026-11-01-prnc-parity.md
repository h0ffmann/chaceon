# 2026-11-01: `ww3_prnc` byte-identical on 81 cases, inputs rebuilt from CDL

2026-11-01, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-3303-prnc`.
Results: `rust/port_report/WW3_PRNC.md`.

## Question

`ww3_prnc` is the preprocessor of a real forecast's forcing: it turns NetCDF wind, current, ice and level into the
`wind.ww3`, `current.ww3`, ... that `ww3_shel` reads. Can Rust write those files, and the log, bit for bit, for the input
types `AI` and `LL`, from `ww3_prnc.nml` and from the old `ww3_prnc.inp`?

## What was missing first

The regtests have 21 `ww3_prnc.nml` and 21 `ww3_prnc.inp` and name ten NetCDF files, and the copy of WW3 at the pin has none of
them: `find WW3/regtests -name '*.nc'` finds nothing, and no script in the copy downloads them. Nothing was downloaded. The
cases are CDL files that `ncgen` of the reference image builds, with the namelists and grids of the regtests and synthetic
values (`scripts/prnc_cdl_gen.py`). A result here is parity on these shapes, not on a regtest's own data ⚠.

## Result

81 cases end byte-identical: 64 forcing files (10 656 836 bytes), 81 logs (1 717 611 bytes), 81 empty error outputs, 81 exit
statuses and 68 `ww3_prnc.nml.log` (67 606 bytes), 0 differences (v, `just prnc-all` for the 11 cases shaped like a regtest
namelist and `just prnc-variants` for the other 70). 8 cases end in a refusal, status 64, where the reference runs (v). A global
0.25 degree wind file (1440 x 721) on the 1 degree grid of `ww3_tp2.21` is identical too (v, `--big`, not committed). The
reference runs twice on every case and the two agree on every file but one: the error "GRID DIMENSIONS ... NOT FOUND" of the
namelist form prints an uninitialised string, so that case is not in the set (the `.inp` form of it is).

The comparison is `cmp` of the whole of everything the program writes, its log (unit 6, errors included, since `NDSE` is unit
6), its error output and its status, with no tolerance. 18 error exits (statuses 1 to 59, 242, 243) match in text and status.

Eighteen faults, at least one per stage, went through the pair and were caught (`bash scripts/prnc_faults.sh`; the table is in the report).

## What it took

- The log is a result. `ww3_prnc` writes 100 KB of warnings when the input does not cover the model grid, and the formats
  decide the bytes: `A` items print at the length of the Fortran variable (the file name is 80 characters, a field name 14,
  `NF90_STRERROR` 80), `CHECK_ERROR` writes list-directed, so a blank leads each record and an `I11` follows a blank, and
  the line number it prints is the line of the `CHECK_ERR` call in the source.
- The first sweep (67 cases) was clean except for one padding length (`IDSTR1`, 14 characters, six names are 13 characters and the DATA
  statement pads them) and one case where the reference was not deterministic.
- `W3GRMP` near a pole. `ww3_grid` stopped with exit 2 there. A global input has cells within 10 degrees of a pole, and the
  model grids of `ww3_ufs1.1` reach 85 degrees, so `gridsru.rs` now does what the Fortran does: project with `W3SPLX`, then
  `GETPQR`. It is byte-identical (`glo_wind_0_360` and the other global cases), with the platform's `sin` and `cos`.
- `W3GSUC` is called with `IJG` true (arrays `(NXI,NYI)`). The Rust search utility keeps `(NY,NX)`, so the coordinates are
  transposed once.
- Two cases were wrong, not the program. A fault in the closure of the longitude passed on `glo_wind_0_360` because the
  model nodes never lie in the wrap cell; the fault in the third and fourth fallback of `INTERP` passed on a case with 35 %
  of the input missing, in a pattern that never put two neighbours both missing. Cases with input columns 10 degrees apart
  (`glo_wrap_10deg`) and 60 % missing (`glo_nodes_holes_dense`) catch them.
- The Fortran defects that are reproduced (one `FILLVALUE` for all fields, the third field of `WNS` unscaled, the one-row read of
  the 1D form, a `REAL` delay) have a case each; the report lists them.
- Rust build: no new crate. The reads are 13 more `extern "C"` declarations in `ww3-host`'s `ncread.rs`, behind the Cargo
  feature `netcdf` of `ww3-host` and `ww3_prnc` (off by default), so `cargo check --workspace` needs no NetCDF on the host.

## What did not get done

- `AT` (the tidal analysis), `F1`, `F2` and `DAT`, unstructured grids, MPI: refused or not read, with status 64 or a named
  message (report, "Refused"). `ww3_tp2.18` is the tidal regtest.
- The grids `glo_15mxt` and `glox_10m` of `ww3_ufs1.1` and `ww3_ufs1.2`, `Iroise_AI_UV.nc` and the other real files: not run.
- A `NaN` fill value, a compressed NetCDF 4 file, the short forms of some time units, a year 0: not run ⚠.
- No timing was taken. Nothing ran on a GPU.
