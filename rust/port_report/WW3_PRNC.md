# WW3_PRNC: `ww3_prnc`, the NetCDF forcing preprocessor

`rust/crates/ww3-host/src/prnc.rs` translates `ww3_prnc` (`model/src/ww3_prnc.F90:22-2515` at `92e2cf66`) for
`switches/switch_chaceon_shrd`, with `INTERP` (`:2551-2686`), `CHECK_ERROR` (`:2694-2713`), the namelist module
`W3NMLPRNC` (`w3nmlprncmd.F90:81-648`), the time routines `U2D`, `D2J`, `J2D`, `D2T` and `T2D` (`w3timemd.F90`), `STRSPLIT`
(`w3servmd.F90:1439-1495`), the `WRITE` branches of `W3FLDO` and `W3FLDG` (shared with `ww3_prep`) and, from `gridsru.rs`,
`W3GSUC` and `W3GRMP` for the interpolation weights. The binary is `rust/tools/ww3_prnc`: `ww3_prnc <dir>` reads
`<dir>/mod_def.ww3`, `<dir>/ww3_prnc.nml` (or `ww3_prnc.inp`) and the NetCDF file the input names, and writes the forcing file
(`wind.ww3`, `current.ww3`, `ice.ww3`, ...) and `ww3_prnc.nml.log` in `<dir>`. A file name in the input is joined to `<dir>`, as the
Fortran's `FNMPRE` (`./`) joins it to the working directory.

| File | Holds |
|---|---|
| `rust/crates/ww3-host/src/prnc.rs` | the program, `INTERP`, `CHECK_ERROR`, the namelist and `ww3_prnc.inp` readers, `U2D`, `D2J`, `J2D`, `D2T`, `T2D` |
| `rust/crates/ww3-host/src/ncread.rs` | the binding: 13 `libnetcdf` read functions, errors returned as the library's status code |
| `rust/crates/ww3-host/src/gridsru.rs` | `W3GSUC`, `W3GRMP` (shared with `ww3_grid`; the near-pole branch with `W3SPLX` is new here) |
| `rust/tools/ww3_prnc/` | the binary; Cargo feature `netcdf` (off by default, like `ww3_ounf`'s) |
| `regtests/prnc_inputs/` | 89 cases: CDL, `ww3_prnc.nml` or `.inp`, `case.conf`; written by `scripts/prnc_cdl_gen.py` |
| `scripts/prnc_pair.sh`, `prnc_all.sh`, `prnc_variants.sh`, `prnc_faults.sh` | the verdict, the two suites, the fault runs |

**Status: `translated`** (ADR-0005). The paths below are byte-identical to the reference, but the program is not fully reached: the
tidal analysis (`AT`), the input types `F1`, `F2` and `DAT`, unstructured grids and the error branches listed under "Refused" and
"Not reached" are not. `INTERP` and `CHECK_ERROR` are fully reached and are `host` in `rust/ports.tsv`. Nothing here ran on a GPU;
this is host code.

## The inputs of the regtests are not in this repository

`WW3/regtests` holds 21 `ww3_prnc.nml` and 21 `ww3_prnc.inp` files in nine regtests (`ww3_tic1.4`, `ww3_tp2.8`, `ww3_tp2.15`,
`ww3_tp2.17`, `ww3_tp2.18`, `ww3_tp2.21`, `ww3_ufs1.1`, `ww3_ufs1.2`, `ww3_ufs1.3`). They name ten NetCDF files (`ice.nc`, `ice1.nc`,
`ice5.nc`, `wind.nc`, `current.nc`, `level.nc`, `curlev.nc`, `gfs_wind.nc`, `Iroise_AI_UV.nc`, `ww3.201809_wnd.nc`; 21 references)
and `find WW3/regtests -name '*.nc'` finds none (v, 2026-11-01). The copy has no script that fetches them either (`regtests/bin` has no
download step, `grep` for `ftp`, `curl`, `wget` in `run_test` and `run_cmake_test` finds nothing). Nothing was downloaded from a
third party.

So the data here are synthetic. `scripts/prnc_cdl_gen.py` writes CDL files, `ncgen` (the reference image's `netcdf-bin` 4.9.0) turns
them into NetCDF, and the reference and Rust read the same file. What the cases keep from the regtests is the shape: the namelist
(field, grid form, variable and dimension names), the grid the regtest runs on (`ww3_grid` of the reference on the regtest's own
namelist and input), and the layout of the file (1D or 2D longitude and latitude, `time` first, the node dimension of the
unstructured regtests). They do not keep the values, the sizes, or whatever else those files hold, so a result here says the
translation is right for these shapes, not that it reproduces a regtest ⚠.

## Result

`scripts/prnc_pair.sh <case>` runs `ww3_grid` once per grid (cached in `work/prnc_grid/`), then the reference's `ww3_prnc` **twice**
(`work/prnc/<case>/ref`, `ref2`), and Rust's once on the same `mod_def.ww3`, input and NetCDF file. `cmp` judges the exit status,
the log (standard output), the error output and every file the program wrote, forcing file and `ww3_prnc.nml.log` included. The two
reference runs must agree byte for byte first, and did in every case but one (below). No tolerance exists in the script.

The 11 cases shaped like a regtest namelist (v, `just prnc-all`, 2026-11-01, linux/arm64 image on Apple Silicon):

| Case | Shape of | Grid | Field, grid | File | Bytes | Log bytes | Result |
|---|---|---|---|---|---:|---:|---|
| `cart_ice_ll` | `ww3_tic1.4` `ww3_prnc_ice.nml` | `ww3_tic1.4` (Cartesian, 153 x 3) | `ICE_CONC`, `LATLON` | `ice.ww3` | 3 768 | 1 268 | identical |
| `cart_ice_param1` | `ww3_tic1.4` `ww3_prnc_ice1.nml` | `ww3_tic1.4` | `ICE_PARAM1`, `LATLON` | `ice1.ww3` | 3 768 | 1 268 | identical |
| `cart_ice_param5` | `ww3_tic1.4` `ww3_prnc_ice5.nml` | `ww3_tic1.4` | `ICE_PARAM5`, `LATLON` | `ice5.ww3` | 3 768 | 1 268 | identical |
| `ll_currents` | `ww3_tp2.8` `ww3_prnc.nml` | `ww3_tp2.8` (spherical, 103 x 119) | `CURRENTS`, `LATLON` | `current.ww3` | 294 312 | 14 484 | identical |
| `adri_wind_ll` | `ww3_tp2.15` `ww3_prnc.nml` | `ww3_tp2.15` (curvilinear, 43 x 42) | `WINDS`, `LATLON` | `wind.ww3` | 29 008 | 1 352 | identical |
| `ai_one_dim_nodes` | `ww3_tp2.17` `ww3_prnc_current.nml` | `ww3_tp2.8` | `CURRENTS`, `ASIS`, dimension `time` | `current.ww3` | 196 224 | 1 237 | identical |
| `ai_one_dim_level` | `ww3_tp2.17` `ww3_prnc_level.nml` | `ww3_tp2.8` | `WATER_LEVELS`, `ASIS` | `level.ww3` | 98 152 | 1 200 | identical |
| `ai_one_dim_wind` | `ww3_tp2.17`, `ww3_tp2.21` | `ww3_tp2.8` | `WINDS`, `ASIS` | `wind.ww3` | 196 224 | 1 237 | identical |
| `glo_wind_0_360` | `ww3_ufs1.1` `ww3_prnc_glo_5deg_wind.nml` | `ww3_ufs1.1` `glo_5deg` (closed, 72 x 35) | `WINDS`, `LATLON` | `wind.ww3` | 40 432 | 1 358 | identical |
| `glo_current_0_360` | `ww3_ufs1.1` `..._current.nml` | `ww3_ufs1.1` `glo_5deg` | `CURRENTS`, `LATLON` | `current.ww3` | 40 432 | 1 358 | identical |
| `glo_ice_conc` | `ww3_ufs1.1` `..._ice.nml` | `ww3_ufs1.1` `glo_5deg` | `ICE_CONC`, `LATLON` | `ice.ww3` | 20 256 | 1 285 | identical |

`ww3_tp2.17` and `ww3_tp2.21` are unstructured-grid regtests (`LONGITUDE = 'time'`, `LATITUDE = ''`); `ModDef::read` does not read an
unstructured `mod_def.ww3`, so the three `ai_one_dim_*` cases run on `ww3_tp2.8`'s structured grid. The Fortran then reads one row
and leaves the rest of the array as it was (see "Fortran defects"). `ww3_tp2.18` is the tidal analysis, refused.

The 70 further cases that end identical, and the 8 that end in a refusal, are `just prnc-variants`:

| Group | Cases | What they exercise |
|---|---|---|
| Fields, `LL` on the `ww3_tp2.8` grid | `ll_wind_1d`, `ll_water_levels`, `ll_ice_conc`, `ll_ice_berg`, `ll_ice_param1` to `5`, `ll_mud_density`, `ll_mud_thickness`, `ll_mud_viscosity`, `ll_atm_momentum`, `ll_air_density`, `ll_wns_shift`, `ll_wns_shift_all` | all 16 field ids; `WNS` with a third variable; `ISI` with two |
| Input layout | `ll_cur_2d_packed`, `ll_4d_missing_value`, `ll_partial_cover`, `ll_two_fillvalues`, `ll_negative_zero_and_fill_zero`, `ll_wind_nc4`, `ll_wind_64bit_offset`, `adri_wind_ll_2d_curved`, `adri_wind_ll_lat_desc` | 2D longitude and latitude, packed `short` with `scale_factor`, `add_offset`, `_FillValue` and `missing_value`, NaN, four dimensions, uncovered model points, NetCDF 4 and 64-bit offset files, a curvilinear model grid with a curved and with a descending-latitude input |
| Time | `ll_days_units_lon_names`, `ll_minutes_short_units_MT`, `ll_hours_short_hms_calendar`, `ll_int_time`, `ll_shift_back`, `ll_shift_year`, `ll_wns_shift`, `ll_wns_shift_all` | `days`, `minutes`, `hours`, `seconds` units in the long form and in some short forms, the variable and dimension `MT`, a missing and a non-standard `calendar`, an `int` time, `TIMESTART`, `TIMESTOP`, `TIMESHIFT` forward, backward and by a year |
| Global, closure and poles | `glo_wind_neg180`, `glo_current_10deg_noclose`, `glo_wrap_10deg`, `glo_wrap_10deg_neg`, `glo_nodes_holes`, `glo_nodes_holes_dense`, `glo_nodes_holes_ice` | longitudes from 0 and from -180, input closed in longitude (`NXI * SXI` = 360) and not closed, model nodes on input nodes with 35 % and 60 % of the input missing, input cells within 10 degrees of a pole |
| `AI` | `ai_xy_wind`, `ai_xy_larger_file`, `ai_xy_ice_berg_4d`, `ai_xy_rho`, `adri_ai`, `inp_ai_one_dim`, `inp_ai_xy` | dimensions `x`, `y`; a file larger than the grid; a missing value and a NaN; four dimensions |
| `ww3_prnc.inp` | `inp_ll_cur`, `inp_ll_lev`, `inp_ll_wns_notime_nohdr`, `inp_comment_char`, `inp_ai_*`, `cart_ice_ll_inp` | the comment character, `FLTIME` and `FLHDR` false, the time line, `nml_wins_over_inp` (both files present: the namelist wins) |
| Namelist forms | `ll_nml_forms` | lower-case names, `.true.`, one-line group, comments, `FILE%VAR = 'U', 'V'` |
| Errors, same bytes and status | `err_nofile` (59), `err_novar` (59), `err_no_time_var` (59), `err_edge_ai_small_file` (59, after the header), `err_nofill` (27), `err_units` (44), `err_lat_reversed` (32), `err_small_grid` (36), `err_no_field` (242), `err_no_grid` (243), `err_tidal_on_wind` (2), `err_tidal_not_asis` (3), `err_inp_field_id` (30), `err_inp_type_id` (31), `err_inp_illegal_time` (35), `err_inp_short` (1), `err_inp_bad_logical` (42), `err_inp_dims_not_found` (56) | 18 exit statuses and their messages |

In total, 81 cases end identical: 64 forcing files, 10 656 836 bytes; 81 logs, 1 717 611 bytes; 81 empty error outputs; 81 exit
statuses; 68 `ww3_prnc.nml.log`, 67 606 bytes; 0 differ (sum of the logs of `just prnc-all` and `just prnc-variants`, 2026-11-01).
One more case, a global 0.25 degree wind file (1440 x 721, two times, 16 620 928 bytes of NetCDF) on `ww3_tp2.21`'s 1 degree grid
(360 x 171), is too large to commit: `python3 scripts/prnc_cdl_gen.py --big work/prnc_big`, then
`PRNC_CASES=work/prnc_big PRNC_KIND=big bash scripts/prnc_all.sh` gives `wind.ww3` 985 072 bytes, the log and the namelist log
identical (v). No timing was taken.

The exit statuses 242 and 243 are `EXTCDE ( 1010 )` and `EXTCDE ( 1011 )` taken modulo 256 by the operating system; Rust passes the
same numbers to `exit`.

## Faults

A comparison that cannot fail proves nothing, so one fault per stage went into the source and through the pair
(`bash scripts/prnc_faults.sh`; each fault is a one-line perl substitution, the build goes to its own target directory, the source is
restored from a copy), 2026-11-01:

| Stage | Fault | Case | Differing bytes |
|---|---|---|---|
| Header | `GTYPE` written as `GTYPE + 1` | `adri_wind_ll` | 1 of 29 008 (file) |
| `J2D` | one second added | `ll_wns_shift` | 1 of 14 686 (log) |
| Weights | `RD12` taken from `RW(3)` | `adri_wind_ll` | 21 747 of 29 008 |
| Velocity correction | the clamp 1.5 changed to 1.4 | `adri_wind_ll` | 2 982 of 29 008 |
| Scale and offset | the offset left out | `ll_cur_2d_packed` | 193 115 of 294 312 |
| Closure | `ICLOSE_SMPL` replaced by none | `glo_wrap_10deg` | 203 of 20 240 (file), 362 of 1 316 (log) |
| Pole | the `W3SPLX` branch left out | `glo_wind_0_360` | 1 440 of 40 432 |
| Fill to zero (`AI`) | the fill value becomes 1 | `ai_xy_wind` | 6 of 196 224 |
| NaN to fill | the NaN test removed | `ai_xy_wind` | 2 of 196 224 |
| Time shift | the sign of `TIMEDELAY` flipped | `ll_shift_back` | 3 of 294 312 (file), 3 of 14 658 (log) |
| Log | `Time :` written as `Tim  :` | `ll_wind_1d` | 3 of 14 484 (log) |
| Time units | hours divided by 24.5 | `ll_wind_1d` | 4 of 294 312 (file), 10 of 14 484 (log) |
| Fill value | the one of the first variable kept for both | `ll_two_fillvalues` | 2 212 of 294 312 |
| `INTERP` | first, second, third, fourth corner as the fallback, each plus 1 | `glo_nodes_holes_dense` | 3 269, 1 960, 1 113, 46 of 20 240 |
| `INTERP` | all corners missing gives 1 | `glo_nodes_holes_dense` | 2 360 of 20 240 |

Two of these were first **not caught**, and the cases were wrong, not the program: `closure` passed on `glo_wind_0_360`, whose model
nodes (0 to 355 every 5 degrees) never lie in the cell between the last input column (358) and the first, so the case that has input
columns every 10 degrees (`glo_wrap_10deg`) was added; and the third and fourth `INTERP` fallbacks and the all-missing exit passed
on `glo_nodes_holes` (35 % missing, a pattern that never puts two neighbours both missing) until `glo_nodes_holes_dense` (60 % missing from a fixed-seed
generator, plus holes arranged in the first column) was added. With it every branch of `INTERP` is reached, which is why its row is `host`.
The fault on `J2D` is caught by the log only: the file holds `TIMESHIFT`, not the file time, because the delay is computed from the
same time it corrects.

## Refused

Each form below is accepted by the Fortran and stopped by Rust with status 64 and `ww3_prnc: <form> is not implemented` on standard
error (`prnc_pair.sh` exits 3 and prints `REFUSED`). The first eight were run (v, `just prnc-variants`, reference status in
brackets); the rest stop in the code and were not run ⚠.

| Form | Message | Run |
|---|---|---|
| `FORCING%FIELD%DATA_ASSIM` | `the field DATA_ASSIM (assimilation data, DAT)` | `refuse_dat_nml` (59) |
| `ww3_prnc.inp` field `DAT` | `the field DAT (assimilation data, input type 5)` | `refuse_inp_dat` (1) |
| `FORCING%TIDAL` set, `AT` | `a tidal analysis (TIDAL, input type AT, ITYPE 6)` | `refuse_tidal_at` (0) |
| `ww3_prnc.inp` type `AT` | `input type AT (tidal analysis, ITYPE 6)` | `refuse_inp_at` (59) |
| `ww3_prnc.inp` type `F1` | `input type F1 (grid from file, W3FLDP)` | `refuse_inp_f1` (1) |
| `ww3_prnc.inp` type `F2` | `input type F2 (two grids from file, W3FLDP)` | `refuse_inp_f2` (1) |
| a key that is not in `FORCING_NML` or `FILE_NML` | `the key FORCING%NOPE of FORCING_NML` | `refuse_nml_unknown_key` (1) |
| a time units attribute with a field `I` cannot read | `a time units attribute with a field the I edit descriptor cannot read (U2D, status 45)` | `refuse_time_units_digits` (45) |
| a namelist value that is not a logical, or a string value without quotes | `the value ... of ... in ww3_prnc.nml` | ⚠ |
| a `ww3_prnc.nml` without both groups; `FILE%VAR` that is not one to three quoted names | named in the message | ⚠ |
| `TIMESTART`, `TIMESTOP` or `TIMESHIFT` that is not two integers | `TIMESTART that is not two integers` | ⚠ |
| a `calendar` or `units` attribute longer than 50 characters | named in the message | ⚠ |
| a field variable with more than four dimensions, a dimension name longer than 20 characters, a longitude or latitude variable with more than two dimensions | named in the message | ⚠ |
| a field variable with no dimension named `time` or `MT` | `... (NTI is never set in the Fortran)` | ⚠ |
| a `start` or `count` longer than the variable's rank (the 1D form on a rank-1 variable, a 2D grid on a rank-2 variable) | `a start or count longer than the rank of the variable (NF90_GET_VAR)` | ⚠ |
| `ww3_prnc.inp` with more than two dimension names or three variable names | named in the message | ⚠ |
| a build without the Cargo feature `netcdf` | `built without the netcdf feature` | ⚠ |

An unstructured grid is **not** a status 64: `ModDef::read` asserts with its own message, as for `ww3_prep` ⚠. `F1`, `F2` and `DAT` also
need `W3FLDP` and the data records (`W3FLDD`); the tidal analysis needs `W3TIDEMD`, `VUF_SET_PARAMETERS` and the harmonic fit.
None of these is in a regtest that Rust could run here.

## Fortran defects translated

Each is reproduced and each has a case unless marked ⚠.

- **One `FILLVALUE` for all fields.** The loop over the variables (`:811-831`) overwrites it, so the fill value of the last variable
  is applied to the first (`ll_two_fillvalues`: `U` has -999, `V` has -888, and the -999 in `U` is data). (v)
- **The third field of `WNS` is not scaled.** `AC` is read raw (`:1927`), without `scale_factor`, `add_offset`, the NaN test or
  the fill test (`ll_wns_shift_all`: the attributes are on it and ignored). (v)
- **The 1D form on a structured grid reads one row.** With `LONGITUDE = 'time'` and no latitude (the form of `ww3_tp2.17` and
  `ww3_tp2.21`) the read has `count = (MXM,1)`, so only `XC(:,1)` is read; the rest of `XC` keeps its zeros and then takes
  `0 * scale_factor + add_offset` from the `WHERE` over the whole array (`ai_one_dim_level`: `scale_factor` 0.5, `add_offset` 2.0, the
  other 118 rows hold 2.0). (v)
- **The error "GRID DIMENSIONS ... NOT FOUND" prints an uninitialised string** in the namelist form (`STRDIMSNAME` is set only by
  `ww3_prnc.inp`). The reference's two runs of the same input differ in the bytes after the message (heap addresses), so
  no comparison can judge it: Rust prints 1024 NUL bytes and the same status 56. The `.inp` form prints the names and is
  byte-identical (`err_inp_dims_not_found`). (v: the reference differed from itself, `work/prnc/err_dims_not_found`, 2026-11-01)
- **`NTI` is never set when the field has no dimension named `time` or `MT`.** The Fortran loops over garbage; Rust refuses. ⚠
- **`SECOND` in `J2D` and `D2J`, and `DSEC21`, are `REAL`.** A delay of 31 million seconds is not exact in 24 bits; the output is
  what the rounding gives (`ll_shift_year`). (v)
- **The check for an illegal shifted time in `ww3_prnc.inp` prints `TIME`, which was never set** (`WRITE (NDSE,1035) TIME`); the
  static zeros print as `00000000 000000` (`err_inp_illegal_time`). (v)
- **`U2D` reads fixed columns** from the position of the last `-` and the first `:`, so a units string with a time zone after the
  seconds is read from the same columns and the rest ignored. ⚠ (ran: the long form of `seconds`, `days` and `hours`, the short form of `minutes` with `h:m:s` and of `hours` with `hh:mm:ss`;
  not run: the long form of `minutes`, the short forms of `seconds` and `days`, `hours` with `h:m:s`, `minutes` with `hh:mm:ss`)
- **The file name is cut at 80 characters** (`NAMEF`), and the log prints all 80 with the trailing blanks.
- **A `_FillValue` or `missing_value` is mandatory** for every field variable (status 27 otherwise, `err_nofill`); `NaN` data are
  replaced by the fill value, so a file with a fill value of 0 gets zeros there, and a negative zero in the data equals that fill value
  (`ll_negative_zero_and_fill_zero`, `ll_4d_missing_value`). (v)

## Translation notes

- **`REAL` is `f32`, double precision is `f64`.** Data, weights, `X0I`, `XNI`, `SXI`, the scale and offset, `TIMEDELAY` and `SECOND` are `f32`;
  the time variable, the Julian days and the coordinates of the search utility are `f64`. `XC * XCFAC + XCOFF` and `RD * XC` are a
  multiply and an add with no fused multiply-add. A coordinate read from a `double` variable is narrowed once, by the library
  (`nc_get_vara_float`), as `NF90_GET_VAR` into a `REAL` does.
- **The Fortran interface reverses the dimension order of the C functions.** `ncread.rs` takes C order; `prnc.rs` pads `start` and
  `count` to the rank with 1 and reverses them, as `nf90_get_var` does. A variable `float U(time, latitude, longitude)` is
  `DIMSVAR(1:3) = (longitude, latitude, time)` in the Fortran.
- **The log is part of the result.** `NDSO` and `NDSE` are unit 6, so messages, warnings and errors are in one stream, and `cmp` of
  it is part of the verdict. The formats are copied from `:2310-2475`; `A` items are padded to the length of the Fortran variable
  (`NAMEF` 80, `IDSTR1` 14, `GNAME` 30, the message of `NF90_STRERROR` 80). Rust keeps the log in a buffer and writes it when the
  run ends or stops, before a refusal message.
- **`CHECK_ERROR` writes list-directed** (`WRITE (NDSE,*)`): one blank in front of each record and an `I11` after a blank that
  follows the text of the integer item. The `LINE NUMBER` is the line of the `CHECK_ERR` call in `ww3_prnc.F90`, which `cpp` puts in
  with `__LINE__`; `prnc.rs` passes the same numbers.
- **The output file is written one record at a time**, so an error in the time loop leaves the records written so far
  (`err_edge_ai_small_file`: the header only, identical).
- **The near-pole branch of `W3GRMP`** was not translated for `ww3_grid` and stopped with exit 2. A global input reaches it (any input
  cell whose centre is within 10 degrees of a pole), so `gridsru.rs` now projects with `W3SPLX` (`:5971-5995`, `C0 = 0`) before `GETPQR`.
  `sin` and `cos` are the platform's, which is the reference's glibc in the image (v, `glo_wind_0_360` and the others with poles;
  `libm_bits` was not run on `sin` and `cos` ⚠). `W3GSUC` takes the arrays as `(NXI,NYI)` (`IJG` true); the search utility keeps
  `(NY,NX)`, so `prnc.rs` transposes the coordinates once.
- **The velocity correction** `WNT1` applies to winds and momentum and `CRT1` to currents, only after interpolation, as in the
  switch (`:2078-2155`).
- **The number of `NetCDF` calls** is small and every status goes to `CHECK_ERR`; the opened file is never closed in the Fortran, Rust
  closes it at the end.

## Not reached, not translated, not checked

- Types `AT`, `F1`, `F2`, `DAT`, unstructured grids, `ww3_prnc` under MPI, the tidal files `tidana.dat` and `times.*`
  (`O15`, `T` are off in the switch).
- The reference regtests' own data, and the grids `glo_15mxt` and `glox_10m` of `ww3_ufs1.1` and `ww3_ufs1.2`: no run on them.
- A `NaN` as `_FillValue`; a `time` variable that is not one-dimensional; a longitude or latitude variable with three dimensions; a
  compressed or chunked NetCDF 4 file (the `nc4` case is uncompressed) ⚠.
- The units forms listed under "Fortran defects"; the error 45 of `U2D`, the namelist read errors (exit 1 and 4 with a `gfortran`
  `iomsg`), a missing `mod_def.ww3`, a year 0 in `D2J`, a negative Julian day.
- `ww3_prnc` was not timed. The only large run is the 1440 x 721 file above, for identity.

## Reproduce

```bash
just prnc-pair ll_wind_1d
just prnc-all                              # the 11 cases shaped like a regtest namelist
just prnc-variants                         # the other 78 cases: 70 identical, 8 refused
bash scripts/prnc_faults.sh                # the faults, one build each
python3 scripts/prnc_cdl_gen.py            # rewrites regtests/prnc_inputs/ (deterministic)
python3 scripts/prnc_cdl_gen.py --big work/prnc_big && PRNC_CASES=work/prnc_big PRNC_KIND=big bash scripts/prnc_all.sh
```
