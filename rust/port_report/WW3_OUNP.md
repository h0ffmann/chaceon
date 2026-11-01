# WW3_OUNP: `ww3_ounp`, the point-output NetCDF postprocessor

`rust/tools/ww3_ounp` translates `ww3_ounp` (`model/src/ww3_ounp.F90`, 5580 lines at `92e2cf66`) for
`switches/switch_chaceon_shrd`: the program body (`:22-1417`), `W3EXNC` (`:1439-3045`), `W3CRNC` (`:3068-5543`) and
`CHECK_ERR` (`:5552-5572`), with what they call: the namelist reader of `W3NMLOUNPMD` (`w3nmlounpmd.F90:83-1201`) and
its log `ww3_ounp.nml.log`, the `READ` branch of `W3IOPO` (`w3iopomd.F90:1160-1395`), `STRSPLIT` (`w3servmd.F90:1439-1495`),
and the calendar routines `T2D`, `TSUB`, `TICK21`, `DSEC21`, `STME21` (`w3timemd.F90`). The binary is `ww3_ounp <dir>`: it
reads `<dir>/mod_def.ww3`, `<dir>/out_pnt.ww3` and `<dir>/ww3_ounp.nml` and writes the NetCDF files, `ww3_ounp.nml.log`
and the log on standard output, as the Fortran program does in its working directory. It calls the system NetCDF C
library, `libnetcdf.so.19` ([`VERSIONS.md`](../../VERSIONS.md), ADR-0001), through the `extern "C"` declarations of
`ww3_ounf`'s binding (`rust/tools/ww3_ounf/src/nc.rs`) copied into `rust/tools/ww3_ounp/src/nc.rs` and extended by
`nc_put_vara_int`, `nc_put_vara_text`, `nc_sync` and `nc_inq_varndims`. The calls return their status instead of stopping,
because `ww3_ounp` ignores most of them (`IRET` is overwritten) and `CHECK_ERR` stops on the others; the two bindings could
share one module later. The Cargo feature `netcdf` turns it on, so `cargo check --workspace` needs no NetCDF on the host.

| File | Holds |
|---|---|
| `rust/tools/ww3_ounp/src/exnc.rs` | the program flow with the labels 560, 700 and 888, `W3EXNC`, the global attributes of section 7.1, `CHECK_ERR` |
| `rust/tools/ww3_ounp/src/crnc.rs` | `W3CRNC`, generated from the Fortran by `scripts/ounp_crnc_gen.py` (`--check` finds it stale) |
| `rust/tools/ww3_ounp/src/nc.rs` | the binding: calls return their status, arguments are in the Fortran's order |
| `rust/crates/ww3-host/src/ounp.rs` | the namelist and its log, `W3IOPO`'s reader, the file-name dates, the mean wave parameters of `W3EXNC` |
| `scripts/ounp_pair.sh`, `ounp_all.sh`, `ounp_variants.sh`, `ounp_selfcmp.sh`, `ounp_src.sh` | the comparison, the sweeps, the self-comparison of the reference, the input for `ww3_ts2` |

**Status: `translated`** for the program, `W3EXNC`, `W3CRNC`, the namelist reader and `STRSPLIT`, because only part of each is
reached (the table below; `SOURCE%SPECTRUM`, `INPUT`, `INTERACTIONS`, `DISSIPATION` and `TOTAL` were read at their default only ⚠);
`host` (ADR-0005) for `CHECK_ERR`, whose two branches ran, with two codes (20 and 22). The file is the verdict: `cmp`
of every NetCDF file, of `ww3_ounp.nml.log`, of the standard output and of the exit status, nothing excluded.
Nothing here ran on a GPU; this is a host program.

## What is reached, what is refused

| `POINT%TYPE` | Output | State |
|---|---|---|
| 1 (spectra) | `SPECTRA%OUTPUT` 3 (`efth` file) | reached: the `SHORT` branch of `W3EXNC` (`ww3_ounp.F90:1763-1888`), `SPECTRA%TYPE` 2, 3 and 4, classic and NetCDF-4, one file or one per station, `DIMORDER` true and false |
| 1 | 2 (1-D spectra table), 4 (partitions) | refused, status 64: `W3SPR4`, `W3SIN4`, `W3SDS4` and `W3PART` |
| 1 | 1 (print plots), 5 and above | not refused: the Fortran stops with "ITYPE AND OTYPE COMBINATION NOT RECOGNIZED", status 46, and so does Rust |
| 2 (mean parameters) | `PARAM%OUTPUT` 1, 2, 5, 6 | reached: depth and wind, wave parameters, validation, WMO standard |
| 2 | 3, 4 (nondimensional) | refused, status 64: `W3SPR4`, `W3SIN4`, `W3SDS4` |
| 2 | 7 and above | refused, status 64: the Fortran creates the file and stops with status 30 |
| 3 (source terms) | 2, 3, 4 | refused, status 64: the source terms of the switch |
| 3 | 1; any other `POINT%TYPE` | status 46 as above |

Also refused with status 64, each with its message on standard error: `ww3_ounp.inp` without `ww3_ounp.nml`; a file
`NC_globatt.inp` in the directory; a namelist key or group `ww3_ounp` does not read, or a group missing; `FILE%NETCDF`
other than 3 and 4; `POINT%BUFFER` below 1 (the Fortran divides by it); `POINT%TIMESPLIT` 9 (a division by zero); a point
list with more words than the file has points (the Fortran writes past `POINTLIST` and its reference aborts with
"malloc(): invalid size", v, `ww3_tp2.15` with `list312`); a list index below 1; a calendar other than `standard`; and a
`mod_def.ww3` that is not rectilinear or curvilinear (`ww3_tp2.6`, `GTYPE` 3). A refusal prints what the Fortran printed
up to that point, then `ww3_ounp: not implemented: <what>`. `scripts/ounp_pair.sh` prints it as `REFUSED` and exits 3.

## Result

For each case the reference's `ww3_ounp` and Rust's read the same `mod_def.ww3` and `out_pnt.ww3` (the reference run of
`tp_pair.sh` or `ts1_pair.sh`, kept in `work/pair/<case>`; for `ww3_ts2`, the reference's `ww3_grid`, `ww3_strt` and
`ww3_shel` run by `scripts/ounp_src.sh`) and the same `ww3_ounp.nml` as shipped in the regtest's `input/`, in two
directories, linked and never written. `cmp` judges every NetCDF file, `ww3_ounp.nml.log`, the standard output
(`ounp.log`) and the exit status (`exit.code`) (v, `just ounp-all`, 2026-11-01, linux/arm64 image on Apple Silicon). The
`ww3_ts1` rows are the 20 pair directories of `ts1_pair.sh` (`ww3_ounp_spec.nml`, `TIMESPLIT` 4, `BUFFER` 100), the `ww3_tp2.15`
rows its three runs (the full run and two legs) with the two namelists the regtest has.

| Case | Namelist | TYPE | OUTPUT | SAMEFILE | NETCDF | Stations | NetCDF files | Bytes |
|---|---|---:|---:|---|---:|---:|---:|---:|
| `calm_20m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `calm_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `circle_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `default_20m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `default_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `default_6m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `default_8m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `filipot_20m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `filipot_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `flatswell_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `light_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `noshelter_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `romero2_20m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `sharp_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `storm_20m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `storm_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `storm_6m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `strain_20m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `t700_2500m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `t700_8m` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 136 288 |
| `ts2` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 24 | 1 | 2 863 620 |
| `ww3_tp1.10` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 6 | 1 | 3 818 760 |
| `ww3_tp1.1` | `ww3_ounp.nml` | 1 | 3 | F | 3 | 15 | 15 | 124 440 |
| `ww3_tp1.4` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 11 | 1 | 52 348 |
| `ww3_tp1.5` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 11 | 1 | 52 352 |
| `ww3_tp1.6` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 20 | 1 | 261 736 |
| `ww3_tp1.7` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 5 | 1 | 5 259 012 |
| `ww3_tp1.8` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 1 | 1 | 18 056 |
| `ww3_tp1.9` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 8 | 1 | 209 768 |
| `ww3_tp2.15` | `ww3_ounp_par.nml` | 2 | 2 | T | 3 | 1 | 1 | 6 156 |
| `ww3_tp2.15` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 47 760 |
| `ww3_tp2.15_leg1` | `ww3_ounp_par.nml` | 2 | 2 | T | 3 | 1 | 1 | 5 964 |
| `ww3_tp2.15_leg1` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 24 576 |
| `ww3_tp2.15_leg2` | `ww3_ounp_par.nml` | 2 | 2 | T | 3 | 1 | 1 | 6 060 |
| `ww3_tp2.15_leg2` | `ww3_ounp_spec.nml` | 1 | 3 | T | 3 | 1 | 1 | 36 168 |
| `ww3_tp2.2` | `ww3_ounp.nml` | 1 | 3 | T | 3 | 3 | 1 | 14 904 |

36 runs of 33 cases, 50 NetCDF files, 15 527 440 bytes, 0 differ, and the log, the namelist log and the status identical in
all of them (v, `just ounp-all`; the bytes are the sum of the column). Two refusals and two absences are listed: `ww3_tp2.6` is
refused (its grid is unstructured: `ModDef::read` is not reached, the check stops with "GTYPE 3 in mod_def.ww3", the
reference ran); `ww3_tp2.4` and `ww3_tp2.4_curv` have no `ww3_ounp` input. `ww3_tp1.1` is the one regtest with `SAMEFILE = F`
(15 files, one per station).

### The reference writes the same bytes twice

A NetCDF file may hold a creation time or HDF5 internals that change between runs, in which case `cmp` would fail for a
correct port. It was measured first: the reference's `ww3_ounp` ran twice on one input, two seconds apart, and `cmp` found
every file identical: 14 inputs, 70 NetCDF files, 9 290 867 bytes, 0 differences (v, `just ounp-selfcmp`, 2026-11-01), among them
the classic format and `FILE%NETCDF = 4` (HDF5), one file for all stations and one per station, `SPECTRA%TYPE` 3, the
parameter tables 1, 2, 5 and 6, `TIMESPLIT` 8 and a point list in another order.
So `scripts/ounp_pair.sh` uses `cmp`, no tool compares contents, and **there is nothing the comparison excludes**.

## Namelist variants

`scripts/ounp_pair.sh <case> <variant>` appends lines to the namelist groups, the same on both sides. The variants cover
`FILE%NETCDF` 3 and 4; `SAMEFILE` true and false; `DIMORDER` false (which the Fortran rejects with status 43 for the
classic format); `SPECTRA%TYPE` 2, 3 and 4; `SCALE_FAC`, `OUTPUT_FAC` and the `SOURCE_NML` keys, which only reach the
namelist log; `TIMESPLIT` 0, 1, 4, 5, 7, 8, 10 and 12; `FILE%PREFIX`; the point list in another order, with a duplicate,
with an index past the file, and with none; `TIMECOUNT`, `TIMESTRIDE` and `TIMESTART` before, between and after the
output times; `BUFFER` 1 and 2; `POINT%TYPE` 2 with `PARAM%OUTPUT` 1, 2, 5 and 6 in each of those modes; and the inputs
the program stops on (status 20 for a prefix in a directory that does not exist, 22 for a file with no station, 43, 46).

### Result of the sweep

342 runs on nine cases (`ww3_tp1.1` and `ww3_tp2.15` with every variant, `ww3_tp1.4`, `ww3_tp1.7`, `ww3_tp1.8`, `ww3_tp1.10`, `ww3_tp2.2`,
`default_2500m` and `ww3_ts2` with a third of them): 297 identical, 1079 NetCDF files, 139 120 657 bytes with the logs; 45
refused; 6 in which the Fortran does not end and Rust does not either; 0 differ (v, `just ounp-variants`, 2026-11-01, about 25
minutes). The identical runs include the stops of the Fortran: status 20 (11 runs), 22 (6), 43 (9) and 46 (6), with
their messages.

| Refusal | Runs |
|---|---:|
| `POINT%LIST` with more words than the file has points (the reference aborts) | 13 |
| `PARAM%OUTPUT` 3 | 9 |
| `SPECTRA%OUTPUT` 2 | 9 |
| `POINT%TYPE` 3 | 2 |
| `PARAM%OUTPUT` 4 | 2 |
| `PARAM%OUTPUT` 7 (the Fortran stops with 30) | 2 |
| `SPECTRA%OUTPUT` 4 | 2 |
| `POINT%TIMESPLIT` 9 | 2 |
| `POINT%BUFFER` 0 | 2 |
| `FILE%NETCDF` 2 | 2 |

The six runs that do not end are `POINT%TIMESTRIDE = 0` with a start before the first output time (`ww3_tp1.7`, `ww3_tp1.8`,
`ww3_tp2.15` twice, `default_2500m`, `ww3_ts2`).

## Faults

A comparison that cannot fail proves nothing, so a fault went through each stage and was then restored (a source edit,
built into its own directory with `NETCDF_TARGET=linux-netcdf-fault`, `OUNP_TAG` naming the work directory, 2026-11-01):

| Stage | Fault | Case | Result |
|---|---|---|---|
| Reading `out_pnt.ww3` | wind speed and direction read in the other order, in `Pnt::read` | `default_2500m` | DIFFERENT, 292 of 136 288 bytes (`ww3_tp1.1` first: identical, the wind is zero there) |
| A scale factor | the `0.0004` of the logarithmic spectrum written as `0.0005`, `SPECTRA%TYPE = 3` | `ww3_tp1.1` | DIFFERENT in all 15 files, 480 of 7 848 bytes in each of the last three |
| An attribute | the `standard_name` `sea_surface_wave_frequency` written one character short, in `crnc.rs` | `ww3_tp1.1` | DIFFERENT in all 15 files, 2 of 8 296 bytes |
| Rounding to `short` | `NINT` as a truncation, in `nint_sat`, `SPECTRA%TYPE = 3` | `ww3_tp1.7` | DIFFERENT, 129 871 of 2 645 404 bytes |
| Storage | the deflate level 1 written as 2, `FILE%NETCDF = 4` | `ww3_tp1.1` | DIFFERENT in all 15 files, for example 2 102 of 88 260 bytes |
| A global attribute | `data_type` "OCO spectra 2D" written "OCO spectra 3D" | `ww3_tp1.1` | DIFFERENT in all 15 files, 1 of 8 296 bytes |
| A parameter | `fp * TPIINV` written `fp`, `TYPE` 2 `PARAM%OUTPUT` 2 | `ww3_tp2.15` (`ww3_ounp_par.nml`) | DIFFERENT, 28 of 6 156 bytes |
| The log | the width of the step counter in format 955, 10 written 9 | `ww3_tp1.1` | DIFFERENT, `ounp.log`, 1 572 of 5 233 bytes |
| The status | the exit code 43 written 44, `DIMORDER = F` | `ww3_tp1.1` | DIFFERENT, `exit.code`, 1 of 3 bytes |
| The namelist log | `F8.2` written `F8.3`, the `scale` variant | `ww3_tp1.1` | DIFFERENT, `ww3_ounp.nml.log`, 15 of 812 bytes |

Each fault was restored and `diff` of the saved copy was empty (`python3 scripts/ounp_crnc_gen.py --check` passes). A
fault in `Pnt::read` that swaps two fields passed on `ww3_tp1.1`, where the wind is zero, and was caught on
`default_2500m`: the case a fault is tried on matters.

## Translation notes: what the Fortran does

These are translated, not corrected.

- **`SINGLEFILE` is not set.** `ww3_ounp.F90:550` reads it from the old `.inp` input only, and the namelist path leaves it
  as the program's frame had it. The reference runs as if it were true: with `POINT%TIMESPLIT` 8 over 20 days of output
  it wrote one file named after the first day, and no "New time step" line (v, `ounp_pair.sh ww3_tp1.1 split8`). ⚠ This is
  the content of a stack slot that the program never writes (the same defect as `ww3_ounf`). It was the same with a
  3 kB larger environment. Rust sets it true; the branches that split the files (`:1097-1105`, `:1218-1220`, the `NINT`
  form of `FILETIME` at `:762-777`) are translated and never run ⚠.
- **The station lines of the log, `SAMEFILE` true.** Section 5.6.5 lists `I = 1..NBSTATION` with `NBSTATION = NREQ`, so
  it lists points 1 to `NREQ` that were requested, not the points requested: a list of `3 1 2` prints points 1, 2 and 3
  (v, `ounp_pair.sh ww3_tp1.1 together+list312`).
- **The order of the stations.** The `SHORT` branch (`TYPE` 1) writes the stations in the order of the list; the full
  branch (`TYPE` 2) in the order of the point indices. `station` holds the point index in both.
- **`DIMORDER` false.** `TYPE` 1 writes `NREQ` values with `start = (IOUT, 1)` and no `count`; netcdf-fortran then takes the
  array shape as `count`. `TYPE` 2 writes with `start = (J1, IOUT)` whatever the order, so a station and a time
  swap places and the library refuses the write (the status is not checked). Rust makes the same calls through
  `nc::put_*`, which take the Fortran's `start`, `count` and shape, and the files are identical.
- **`product_name`** is the file name less its first two characters (`GLOBALATT(3:)`), which drops the `./` of `FNMPRE`.
  `field_type` compares the stride with 3600, 7200, 10800, 21600, 32400, 43200 and 86400 and says "n/a" otherwise.
- **Format 2940** prints `NCNAME`, a `CHARACTER(128)`, padded to 128 columns, and it holds the last name assigned in
  sections 5.5 and 5.6.2, not the name of this bunch.
- **Format 942 with no third item** (`TYPE` 2) stops at the `I3` descriptor, so there is no blank line after the heading.
- **`CHECK_ERR`** writes with list-directed output: a blank starts each line, `NF90_STRERROR` is 80 columns and the
  code takes 11 (v, `ounp_pair.sh ww3_tp1.1 together+list0`, status 22; `baddir`, status 20).
- **`W3CRNC` for `TYPE` 1 `OTYPE` 2** sets the deflate of `f` with `VARID(22)`, a variable that does not exist
  (`ww3_ounp.F90:3359`). That block is not translated (refused above), so the defect is only named here.
- **A zero `POINT%TIMESTRIDE` with a start before the first output time** loops for ever in the Fortran (`TICK21` adds
  nothing); Rust loops too. `ounp_pair.sh` stops both at 60 s and prints exit 4 (v, `ounp_pair.sh ts2 stride0`).
- **`FNMPRE` is `./`** (`w3odatmd.F90:330`): the file names, the namelist and its log have it.
- **The point list** (`STRSPLIT`) splits at blanks; only the first word is compared with `all`; an index past `NOPTS` or a
  repeated one is dropped without a message; `NREQ` counts the distinct valid ones.

## Not reached, not translated, not checked

**Translated and never reached:**

- The `FLWW3` branches of `W3CRNC` and `W3EXNC` (`WWIII_param_version`): `FLWW3` is 0 on the namelist path ⚠.
- `NF90_SYNC` (`ww3_ounp.F90:1123-1125`): wherever the log prints `NCFLUSH` it is 833 333 or more (51 runs of `work/ounp`), and no run wrote that many times ⚠.
- The warning of section 5.3 (`NCTYPE` 4, `NFL` above 300, `NREQ` above 9000) and the check of the library version
  (`:672`) ⚠.
- Two stations of one file with one name (section 5.5 skips the second): no `out_pnt.ww3` here has two ⚠.
- The finalisation after the end of file in section 7.4 (`:1242`), which the library refuses with status 0 ⚠.
- `TIMESPLIT` code paths that need `SINGLEFILE` false, above.

**Not translated:** the blocks of `W3EXNC` and `W3CRNC` under `W3_RTD`, `W3_SETUP`, `W3_FLX5`, `W3_IS2`, `W3_BT4`, `W3_ST1`,
`W3_ST2`, `W3_ST3`, `W3_ST6`, `W3_IG1` (the switch defines none); `W3CRNC` for the refused pairs; the old `.inp` reader.

**Not compared:** standard error of the Fortran, which is empty in every run but the aborts; the reference's `ww3_ounp`
for an unstructured, SMC or multi-grid input; no timing was taken; anything on a GPU.

**Regtest inputs this program cannot take here:** `mww3_test_01` to `_04` and `ww3_ufs1.*` (`ww3_multi`), `ww3_tp2.7`,
`ww3_tp2.17`, `ww3_tp2.21` (unstructured grids), `ww3_tp2.10` (SMC), `ww3_tp2.11` (rotated pole, `RTD`), `ww3_tbt1.1` and
`ww3_tbt2.1` (`BT8`), `ww3_tic1.4`, `ww3_tic2.2`, `ww3_tic2.3` (ice, `IS2`/`IC1`/`IC2`), `ww3_tig1.1` (`IG1`),
`ww3_tpt1.1` (`PTM`), `ww3_tp1.11` and `ww3_tp2.8` (boundary and forcing files from `ww3_bounc` and `ww3_prnc`);
`ww3_tp2.6` has an unstructured grid and is refused above.

## Reproduce

```bash
just netcdf-image
just ounp-pair ww3_tp1.1
just ounp-pair ww3_tp1.7 nc4+type3
just ounp-src ww3_ts2 ts2
just ounp-all
just ounp-variants
just ounp-selfcmp
python3 scripts/ounp_crnc_gen.py --check
```

`scripts/rust_netcdf.sh` gained `NETCDF_TIMEOUT` (seconds) for these scripts: a run that does not end stops with status 124.
