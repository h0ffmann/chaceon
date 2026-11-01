# WW3_OUNF: `ww3_ounf`, the gridded NetCDF postprocessor

`rust/tools/ww3_ounf` translates `ww3_ounf` (`model/src/ww3_ounf.F90`, 4150 lines at `92e2cf66`) for
`switches/switch_chaceon_shrd`: the program body (`:21-858`), `W3EXNC` (`:860-3486`), `W3CRNC` (`:3505-3990`) and
`S2GRID` (`:4007-4070`), with what they call: the `READ` branch of `W3IOGO` (`w3iogomd.F90:2654-4016`), `W3FLDTOIJ`
and `W3FLGRDFLAG` (`:759-1189`, `:583-744`), the namelist reader of `W3NMLOUNF` (`w3nmlounfmd.F90:75-424`),
`UV_TO_MAG_DIR` of `W3SERVMD` (`:1921-2019`), the metadata module `W3OUNFMETAMD` (`INIT_META`, `DEFAULT_META`,
`DEFAULT_CRS_META`, `GETMETA`, `ADD_PARTNO`, `PARTNO_STRING_SUB`, `WRITE_META`) and the calendar routines `T2D`,
`TSUB`, `TSUBSEC`, `T2ISO`, `STME21` and the ISO forms of `U2D` (`w3timemd.F90`). The binary is `ww3_ounf <dir>`: it
reads `<dir>/mod_def.ww3`, `<dir>/out_grd.ww3` and `<dir>/ww3_ounf.nml` and writes the NetCDF files into `<dir>`, as the
Fortran program does in its working directory. It calls the system NetCDF C library, `libnetcdf.so.19`
([`VERSIONS.md`](../../VERSIONS.md), ADR-0001), through 19 hand-written `extern "C"` declarations; the Cargo feature
`netcdf` turns that on, so `cargo check --workspace` needs no NetCDF on the host.

| File | Holds |
|---|---|
| `rust/tools/ww3_ounf/src/exnc.rs` | the program flow, `W3EXNC`, `W3CRNC`, `S2GRID`, `UV_TO_MAG_DIR`, `WRITE_META`, `GETMETA` |
| `rust/tools/ww3_ounf/src/nc.rs` | the binding: 19 `libnetcdf` functions, C dimension order, errors stop the run |
| `rust/crates/ww3-host/src/ounf.rs` | `W3FLDTOIJ`, the namelist, `W3IOGO`'s reader, `T2D`, `TSUB`, `TSUBSEC`, `T2ISO`, `STME21`, `F12.0` |
| `rust/crates/ww3-host/src/ounfmeta.rs` | `DEFAULT_META`, generated from the Fortran by `scripts/ounf_meta_gen.py` (`--check` finds it stale) |
| `docker/Dockerfile.netcdf`, `scripts/rust_netcdf.sh` | the build image (`libnetcdf-dev`) and the run image (the reference plus `ncdump`) |

**Status: `host`** (ADR-0005) for the `RECT` and `CURV` grids of the cases below. The file is the verdict: `cmp` of every
NetCDF file, nothing excluded. Nothing here ran on a GPU; this is a host program.

## The verdict, and why it is `cmp`

A NetCDF file may carry a creation time or HDF5 internals that change between two runs, in which case `cmp` would
fail for a correct port and a comparison of contents would be needed. It was measured first: the reference's
`ww3_ounf` ran twice on one input, two seconds apart, and `cmp` found the files identical: 43 files from 15 inputs,
18 678 308 bytes, 0 differences, among them the classic format and `FILE%NETCDF = 4` (HDF5), `FIELD%TYPE` 2 and 4, one
file per field, `TIMESPLIT` 10, and a curvilinear grid (v, `just ounf-selfcmp`, which runs `scripts/ounf_selfcmp.sh --all`).
The files' global attributes (`ncdump -h`) hold no creation date, and the library writes no timestamp into the HDF5
objects. So `scripts/ounf_pair.sh` uses `cmp`, no tool compares contents, and
**there is nothing the comparison excludes**. A `scripts/ncbits.sh` was not written because it is not needed ⚠: it
would be if a later libnetcdf or a different HDF5 changed this.

## Result

For each case the reference's `ww3_ounf` and Rust's read the same `mod_def.ww3` and `out_grd.ww3` (the reference run of
`tp_pair.sh` or `ts1_pair.sh`, kept in `work/pair/<case>`) and the same `ww3_ounf.nml` as shipped in the regtest's
`input/`, in two directories. Rust's files are written by Rust only, and `cmp` judges each file (v, `just ounf-all`,
2026-11-01, linux/arm64 image on Apple Silicon). The program's own log is also identical in all of them (v, the `log:`
line of the same run).

| Case | `FIELD%LIST` | `FIELD%TYPE` | Files | Bytes | Result |
|---|---|---|---:|---:|---|
| `calm_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `circle_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `default_20m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `default_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `default_6m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `default_8m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `filipot_20m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `filipot_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `flatswell_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `light_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `noshelter_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `romero2_20m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `sharp_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `storm_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `storm_6m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `strain_20m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `t700_2500m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `t700_8m` | DPT WND ICE HS MSS MSD FAW WCC WCF WCH WCM FOC TAW CHA FWS WBT | 4 | 1 | 16 384 | identical |
| `ww3_tp1.1` | HS | 4 | 1 | 27 320 | identical |
| `ww3_tp1.10` | HS DIR SPR | 4 | 1 | 104 584 | identical |
| `ww3_tp1.2` | HS | 4 | 1 | 22 600 | identical |
| `ww3_tp1.3` | DPT HS FC CFX | 4 | 1 | 104 848 | identical |
| `ww3_tp1.4` | HS T01 DIR | 4 | 1 | 9 004 | identical |
| `ww3_tp1.5` | HS T01 FP DIR | 4 | 1 | 11 368 | identical |
| `ww3_tp1.6` | DPT CUR HS | 4 | 1 | 5 412 | identical |
| `ww3_tp1.7` | DPT HS T0M1 DIR SPR HIG EF P2L | default 3 | 3 | 4 168 396 | identical |
| `ww3_tp1.8` | DPT WND WLV HS T02 DIR SPR TAW TWO BHD SXY FOC USS USF | 4 | 2 | 61 976 | identical |
| `ww3_tp1.9` | DPT HS T0M1 | 4 | 1 | 16 560 | identical |
| `ww3_tp2.1` | HS FP DIR | 4 | 1 | 383 952 | identical |
| `ww3_tp2.15_leg1` | HS WND T02 DP DIR FP MXE MXES MXH MXHC SDMH SDMHC | 4 | 1 | 870 536 | identical |
| `ww3_tp2.15_leg2` | HS WND T02 DP DIR FP MXE MXES MXH MXHC SDMH SDMHC | 4 | 1 | 1 621 896 | identical |
| `ww3_tp2.15` | HS WND T02 DP DIR FP MXE MXES MXH MXHC SDMH SDMHC | 4 | 1 | 2 373 256 | identical |
| `ww3_tp2.2` | HS T01 DIR SPR | 4 | 1 | 1 476 300 | identical |
| `ww3_tp2.3` | HS DIR SPR DP EF TH1M STH1M | 4 | 2 | 845 056 | identical |
| `ww3_tp2.4_curv` | DPT HS | 4 | 1 | 989 072 | identical |
| `ww3_tp2.4` | DPT HS | 4 | 1 | 814 660 | identical |

36 cases, 40 files, 14 201 708 bytes compared, 0 differ (v, `just ounf-all`; the bytes are the sum of the column). The case `ww3_tp2.6` is refused, below.

The 18 `ww3_ts1` rows are the 18 namelist and depth variants of `just ts1-pair`; each requests one point
(`FILE%IX0 = IXN = IY0 = IYN = 2`). `ww3_tp2.6` is refused: its grid is unstructured and `ModDef::read` stops with
"only rectilinear and curvilinear grids are implemented" (v, `just ounf-pair ww3_tp2.6`, exit 3, the reference ran).
`ww3_tp1.7` is the one regtest that leaves `FIELD%TYPE` at its default 3; `ww3_tp1.8` is the one that reaches group 6.

## Fields, types, files, partitions

The regtests ask for 43 of the output fields, most with `FIELD%TYPE = 4`. To reach the rest, the reference's `ww3_shel`
ran again on the inputs of a pair case with a list of 96 field names in `TYPE%FIELD%LIST` (`work/ounf/all.list`, from
`scripts/ounf_extra_all.sh`), and both `ww3_ounf` read that `out_grd.ww3` with `FIELD%LIST` set to the same list. For
`ww3_tp1.7` the main file holds 142 variables (v, `ncdump -h`): groups 1, 2, 4, 5, 6, 7, 8 and 9, the partitioned
fields of partitions 0 to 2, and the frequency files `ef`, `p2l` and `wn` beside it. The names a grid cannot hold (a
spectral field with no bounds, `USF`, `USP`, `TH2M`) are reported by both programs as not available.
`scripts/ounf_extra_all.sh` runs five such inputs, `ww3_tp1.7`, `ww3_tp2.3`, `ww3_tp2.4_curv`, `default_2500m` and
`ww3_tp1.8`, each with nine namelist variants, the third a curvilinear grid.

45 runs, 706 files, 456 543 832 bytes, 0 differ, and the program log identical in all 45 (v, `bash scripts/ounf_extra_all.sh`,
2026-11-01).

## Namelist variants

`scripts/ounf_pair.sh <case> <variant>` appends lines to the namelist, the same on both sides. The variants cover
`FIELD%TYPE` 2, 3 and 4; `FILE%NETCDF` 3 and 4; one file per field (`SAMEFILE = F`); `TIMESPLIT` 0, 4, 8 and 10;
magnitude and direction in place of vectors (`VECTOR = F`); `NOVAL`; no `MAPSTA` variable; time in seconds; `INT64`
time; another epoch; a file prefix; a sub-grid (`IX0`..`IYN`); one partition; and more partitions than the grid holds.

168 runs (8 cases, 21 variants), 356 files, 145 368 465 bytes, 0 differ, and the program log identical in all 168 (v,
`just ounf-variants`, 2026-11-01). The cases are `ww3_tp1.6`, `ww3_tp1.7`, `ww3_tp1.8`, `ww3_tp2.4_curv`, `ww3_tp2.15`,
`ww3_tp2.3`, `ww3_tp1.3` and `default_2500m`.

## Faults

A comparison that cannot fail proves nothing, so a fault went through each stage and was then restored
(`scripts/ounf_pair.sh` after a source edit, built into its own directory with `NETCDF_TARGET=linux-netcdf-fault`,
`OUNF_TAG` naming the work directory, 2026-11-01):

| Stage | Fault | Case | Result |
|---|---|---|---|
| Reading `out_grd.ww3` | the two records of a two-record field (`CUR`) stored in the other order, in `GrdIn::read` | `ww3_tp1.6` | DIFFERENT, 240 of 5 412 bytes |
| A field's scale factor | `HS`'s `FSC` 0.002 written as 0.0021 in `ounfmeta.rs` | `ww3_tp1.7` (`FIELD%TYPE` 3, so shorts) | DIFFERENT, 8 969 of 388 392 bytes |
| An attribute | the global `maximum_altitude` "9000 m" written as "9001 m" | `ww3_tp2.4` | DIFFERENT, 1 of 814 660 bytes |
| Rounding to `short` | `NINT` as a truncation, in `nint` | `ww3_tp1.7` | DIFFERENT in all three files: 74 681 of 1 890 040, 20 003 of 1 889 964 and 14 083 of 388 392 bytes |
| Storage | the deflate level of the field variables 1 written as 2 | `ww3_tp2.4`, `FILE%NETCDF = 4` | DIFFERENT, 196 382 of 238 493 bytes |

Each fault was restored and `diff` of the saved copy was empty (`ounfmeta.rs`: `scripts/ounf_meta_gen.py --check`
passes). A sixth difference was found by the run, not planted: the first version of `WN` ignored the one-point shift of
the notes below, and `ww3.201206_wn.nc` came out DIFFERENT, 136 097 of 1 889 868 bytes.

## Translation notes

- **The dimension order.** `NF90_*` reverses the dimension list and `start`/`count` against the C functions. Rust calls
  C, so a variable defined as `(DIMID(2),DIMID(3),DIMID(4))` is `[time, lat, lon]`, and a field array in Fortran
  order (`IX` fastest) is the row-major `[y][x]` block of one record.
- **The call order is part of the bytes.** Redefining, defining, ending define mode and writing happen in the Fortran's
  order, including a `REDEF`/`ENDDEF` pair around every later field, because the classic format moves data when the
  header grows and HDF5 lays objects out in call order. The files are identical, so the order is right.
- **Attribute types.** `NF90_PUT_ATT` with a default `REAL` is `NC_FLOAT`, with a default `INTEGER` `NC_INT`
  (`valid_min` of the status map, and of a short field after `NINT`), with a text `len_trim` bytes. `NF90_FILL_SHORT`
  is a two-byte integer, so `_FillValue` of a short variable is `NC_SHORT` (v, `ncdump`: `-32767s`).
- **`NINT` saturates.** On aarch64 gfortran compiles `NINT` of a `REAL` to `fcvtas`: halves away from zero, and a
  value out of range is clamped. `P2L` with `FIELD%TYPE` 3 has `valid_max` `NINT(1.E12/0.0004)`, and the reference
  writes 2 147 483 647 (v, `ncdump`). `ww3_kernels::nint` adds one and wraps there, so `exnc.rs` has its own.
- **`FIELD%TYPE` 3 is `short` for most fields and `float` for a few** (`PHIAW`, `PHIOC`, `FIC`), because `W3EXNC` sets
  `NCVARTYPE` field by field, while `INIT_META` fixed the metadata once, with the namelist's 3. So at type 3 `EF`
  has the logarithmic units, scale and range (`NCVARTYPE.LE.3`), and `P2L` has linear units and a `valid_max` of
  1.E12 (`NCVARTYPE.EQ.2` is false there) although its data are `log10` values (the data branch tests the per-field
  `NCVARTYPE`, which is 2). Rust does the same (v, `ncdump` of `ww3_tp1.7`: `p2l:units = "Pa2 m2 s"`, `_FillValue = -32767s`).
- **`SINGLEFILE` is not set.** `ww3_ounf.F90:427` reads it from the old `.inp` input only. The reference reads it as
  true: `w3exnc` loads it from `MAIN__`'s stack frame (`ldr w0, [x0, #112]` at `0xa944` and `0xb0e4` of the
  installed `ww3_ounf`), and runs with `TIMESPLIT` 8 or 10 write every time into the file named after the first. ⚠
  This is the content of a stack slot the program never writes; it was the same with 3 sizes of environment
  and in every run here. Rust sets it true. As a consequence a file left from an earlier run makes the
  reference stop at the first time with "NetCDF: Not a valid ID" (exit 59), and Rust stops with the same message (v).
- **`WN` is shifted by one sea point.** The field is `WN(0:NK+1,0:NSEA)` and `WN(IK,:)` is `NSEA+1` values from index 0,
  of which `W3S2XY` takes the first `NSEA`: sea point `J` gets `WN(IK,J-1)`, and point 1 gets `WN(IK,0)`, which nothing
  sets. The reference does this and Rust reproduces it with that element 0 ⚠ (the heap zero page; the files are
  identical, so it was 0 in these runs).
- **`SNAMEP` is `CHARACTER(15)`**: `quaternary_swell` loses its last letter in the names of partition 4 (v,
  `ncdump`: `phs4:long_name = "sea surface quaternary_swel wave ..."`). Two `long_name` literals of 84 characters
  (`SEA8DP`, `SW8DP`) are cut to 80 by the assignment; the generator prints a note for each.
- **`CUSTOMFRQ` and `FIRSTTIME` have initial values**, so they keep their value from one time to the next; `CUSTOMFRQ`
  turns on for the first `USP` request and stays on.
- **File names** use `REAL(TIME(1))`: `FLOOR(REAL(TIME(1))/NINT(10.**(8-S3)))`, and 20080523 is not a `REAL`, so
  `TIMESPLIT` 7 would name a file from a rounded date (not run ⚠). `TIMESPLIT` 9 divides by zero in the Fortran and is refused.
- **`F12.0`** of a latitude is the `f64` of the `f32`, rounded half to even by `format!("{:.0}")` (a value exactly half way does not occur in these files ⚠), with the point
  appended; a negative value that rounds to zero keeps its sign (`-0.`, v, `southernmost_latitude` of `ww3_tp1.7`).
- **`D50`, `IC1`, `IC5`, `AHS`, `OHS`, `THM`, `ACHA`, `OCHA`** are names of other switches. `W3FLDTOIJ` here warns
  "NOT RECOGNIZED", as the reference does. `USF` and `USP` need `US3DF(1)` and `USSPF(1)` of the grid.
- The status map is written from the first time step of each file, and `MAPSTA`/`MAPST2` come from `MOD(MAPTMP+2,8)-2`.

## Not reached, not translated, not checked

**Stops with a message, not translated (a clear refusal, never silent):**

- `ww3_ounf.inp` without `ww3_ounf.nml`; `ounfmeta.inp` and `NC_globatt.inp` in the directory; `FIELD%FCVARS`
  (forecast period and reference time); a calendar other than `standard`; `TIMESPLIT` 9; user-defined output
  fields (`U1`, `U2`, group 10); unstructured and SMC grids and rotated poles (`mod_def.ww3` reader, and the switch).
- A key of `ww3_ounf.nml` that the program does not know, and any group but `FIELD_NML`, `FILE_NML` and an empty
  `SMC_NML`.
- A field whose records the reader does not know: it stops at the record with the lengths it expected.

**Translated and never reached by a case or a variant:**

- `USP` (`CUSTOMFRQ`, the `USSP_WN` frequencies): no grid has `USSPF`. `TH2M` and `STH2M` the same.
- `PTMETH` other than 1: the comment text for methods 2 to 5, the `F5.3` of the cutoff frequency and the "unknown"
  message ⚠.
- A lower-case `FIELD%TIMEUNIT` or `TIMEVAR` (converted to upper case), and a `FIELD%PARTITION` with a non-digit, which
  stops the program as the Fortran's `READ ... '(I1)'` does.
- `FILE%IX0`..`IYN` outside the grid beyond the clamp the Fortran applies to the upper ends.

**Not compared:**

- The reference writes `ww3_ounf.nml.log`; Rust does not.
- The exit status of a failed run: Rust aborts (`panic = "abort"` in the workspace profile, 133 on the stale file), the
  reference stops through `EXTCDE` (59).
- No timing was taken. The five-case sweep of `ounf_extra_all.sh` and the 168 variant runs took about two hours with
  other jobs sharing Docker.
- Anything on a GPU. `W3OUTG` runs on the host in `iogo.rs`; this program reads its file.

## Reproduce

```bash
just netcdf-image
just ounf-pair ww3_tp2.4
just ounf-pair ww3_tp1.7 nc4type4
just ounf-all
just ounf-variants
bash scripts/ounf_extra_all.sh
just ounf-selfcmp ww3_tp1.7 nc4
```
