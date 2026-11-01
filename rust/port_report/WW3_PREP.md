# WW3_PREP: `ww3_prep`, the input preprocessor

`rust/crates/ww3-host/src/prep.rs` translates `ww3_prep` (`model/src/ww3_prep.F90:17-1755` at `92e2cf66`) for
`switches/switch_chaceon_shrd`, with `W3FLDO` and `W3FLDG` in their `WRITE` branches (`w3fldsmd.F90:88-527`,
`:954-1470`), `W3FLDD` in its `WRITE` branch (`:1472-1743`), `W3FLDP` (`:1745-2136`), `INA2R` and `INA2I`
(`w3arrymd.F90:76-461`) for `IDFM` 1, 2 and 3 and `NEXTLN` (`w3servmd.F90:221-334`). The grid search that
`LL`, `F1` and `F2` need is `gridsru.rs`: `GSU_CREATE`, `W3GFCL_R8`, `W3CKCL_R8`, `GETPQR` and `W3GRMP_R8` from
`w3gsrumd.F90`, which `ww3_grid` already used for output boundary points, extended to a grid stored
`XG(I,J)`, to the land mask, to the nearest-neighbour fallback of a fully masked cell (`W3NNSC`, `W3SORT`,
`W3ISRT`, `W3DIST`) and to a cell near a pole (`W3SPLX`). The binary is `rust/tools/ww3_prep`:
`ww3_prep <dir>` reads `<dir>/mod_def.ww3` and `<dir>/ww3_prep.inp` and writes the field file in `<dir>`. A file
name in the input is joined to `<dir>`, as the Fortran's `FNMPRE` (`./`, `w3odatmd.F90:330`) joins it to the
working directory; an unnamed `OPEN` of unit `n` is `<dir>/fort.<n>`, as the runtime's.

**Status: `host`** (ADR-0005) for every input type (`AI`, `LL`, `F1`, `F2`, `DAT`), every field id, free-format,
formatted (with format reversal) and binary input, on the cases below. The row `w3prep` of `rust/ports.tsv`
stays `translated`: branches of the program have not run against the reference, listed under "Not reached". Nothing ran on a GPU; this is a host program.

## What it does

`AI` interpolates nothing: it reads each field with `INA2R` (`VSC = 1.`, `VOF = 0.`, so `1.*x + 0.` is applied
and a negative zero becomes `+0.`) and copies it. `LL`, `F1` and `F2` read the field on the input grid and
interpolate it to the model grid with four weights and four addresses a point (`RD11 RD21 RD12 RD22`,
`IX1 IX2 IY1 IY2`), summed in the order of `ww3_prep.F90:1233-1259`; a two-component field (`IFLD` >= 3 or
icebergs) also has its speed corrected for conservation of velocity by `FACTOR = MIN(1.5, A2/A1)`
(`WNT1` and `CRT1` are on in this switch, `:1318-1327`, `:1344-1353`, `:1370-1379`; `WNT2` and `CRT2`, which use `A3`, are off). `DAT` copies records of
assimilation data. `W3FLDG` writes a time record (two default `INTEGER`) and one record per field. `REAL` is
`f32`, the file is big-endian, a record is its length (4 bytes), the data, the length again.

| Type | `ITYPE` | Grid of the input | Reads | Source (`ww3_prep.F90`) |
|---|---|---|---|---|
| `AI` | 1 | the model's | one array a field | `:509-533`, `:1208-1226` |
| `LL` | 2 | regular longitude-latitude, `X0I XNI NXI Y0I YNI NYI` | one array a field; `W3GSUC` on the generated coordinates and `W3GRMP` once a model point | `:534-556`, `:587-727` |
| `F1` | 3 | any, its latitudes, longitudes and land mask in files | latitude array, longitude array, integer mask, then the field; `W3FLDP` | `:728-906` |
| `F2` | 4 | two such grids, averaged where both cover a point | the same for each, then one field from each; `ICE`, `LEV` and `ISI` only | `:728-927` |
| `DAT` | 5 | none: `DATTYP`, `RECLDT`, `NODATA`, then per time `NDAT` records of `RECLDT` values | `DT0`, `DT1` or `DT2` is the field id, the file `data0.ww3`, ... | `:557-577`, `:1168-1200` |

| Field id | `IFLD` | File | Records after the time | Source (`ww3_prep.F90`) |
|---|---|---|---|---|
| `IC1` to `IC5`, `MDN`, `MTH`, `MVS`, `ICE`, `LEV` | -7 to 2 | `ice1.ww3`, ..., `muddens.ww3`, `mudthk.ww3`, `mudvisc.ww3`, `ice.ww3`, `level.ww3` | 1: `FA` = field 1 | `:1210-1215`, `w3fldsmd.F90:1043-1046` |
| `WND`, `CUR`, `TAU` | 3, 4, 6 | `wind.ww3`, `current.ww3`, `momentum.ww3` | 2: `FX` = field 1, `FY` = field 2 | `:1216-1224` |
| `WNS` | 3 | `wind.ww3` | 3: field 3 is read (`:1143-1147`) and written | |
| `ISI` | 1 | `ice.ww3` | 2: `FX`, `FY`; `FA` is not written | |
| `RHO` | 7 | `density.ww3` | 1: reads two fields, writes `FA` = `AC`, which stays zero | `:1122`, `:1221` |
| `DAT` | 5 | `data0.ww3`, `data1.ww3`, `data2.ww3` | per time: `TD, ND` (3 integers) and `DATA(RECLDT,ND)`; nothing when `ND` is 0 | `w3fldsmd.F90:1629` |

`RHO` writes zeros whatever the input holds: `IFLD` 7 is not `<= 2`, so `FA` takes `AC`, and `W3FLDG` writes `FA`
for a field that is not two-dimensional. That is what the Fortran does and what Rust does (v, `rho` below).

## Result

For each case the reference image runs `ww3_grid` and `ww3_prep` on a regtest's namelist and input (or on a
namelist and input in `regtests/prep/input`). Rust's `ww3_prep` reads **the reference's `mod_def.ww3`** (so a case
whose `ww3_grid` needs a switch the Rust `ww3_grid` lacks still tests `ww3_prep`) and the same `ww3_prep.inp` and
data files; its field file is deleted before it runs and `cmp` judges it, whole (v, `just prep-all`, `just
prep-variants` and `just prep-interp`, 2026-11-01, linux/arm64 image on Apple Silicon). The Fortran's log
(unit 6) is not compared: Rust prints one summary line, listed in the tables as "reached".

### The regtests' inputs

| Regtest | Input | File | Time records | `IDLA`, `IDFM` | Bytes | Result |
|---|---|---|---|---|---|---|
| `ww3_tp1.6` | `ww3_prep_curr.inp` | `current.ww3` | 1, from the input | 3, 1 (inline) | 608 | identical |
| `ww3_tbt1.1` | `ww3_prep_mudd.inp` | `muddens.ww3` | 1, from the file | 3, 1 | 1 512 | identical |
| `ww3_tbt1.1` | `ww3_prep_mudt.inp` | `mudthk.ww3` | 1, from the file | 3, 1 | 1 512 | identical |
| `ww3_tbt1.1` | `ww3_prep_mudv.inp` | `mudvisc.ww3` | 1, from the file | 3, 1 | 1 512 | identical |
| `ww3_tbt2.1` | `ww3_prep_conc.inp` | `ice.ww3` | 3, from the file | 1, 2 | 61 932 | identical |
| `ww3_tbt2.1` | `ww3_prep_mud1.inp` | `muddens.ww3` | 2, from the file | 1, 2 | 41 304 | identical |
| `ww3_tbt2.1` | `ww3_prep_mud2.inp` | `mudthk.ww3` | 2, from the file | 1, 2 | 41 304 | identical |
| `ww3_tbt2.1` | `ww3_prep_mud3.inp` | `mudvisc.ww3` | 2, from the file | 1, 2 | 41 304 | identical |
| `ww3_tic2.2` | `ww3_prep_ice.inp` | `ice.ww3` | 1, from the input | 3, 1 | 40 072 | identical |
| `ww3_tic2.2` | `ww3_prep_ice5.inp` | `ice5.ww3` | 1, from the input | 3, 1 | 40 072 | identical |
| `ww3_tic2.3` `scat` | `ww3_prep_ice.inp` | `ice.ww3` | 1, from the input | 3, 1 | 40 072 | identical |
| `ww3_tic2.3` `scat` | `ww3_prep_ice1.inp` | `ice1.ww3` | 1, from the input | 3, 1 | 40 072 | identical |
| `ww3_tic2.3` `scat` | `ww3_prep_ice5.inp` | `ice5.ww3` | 1, from the input | 3, 1 | 40 072 | identical |
| `ww3_ts3` | `ww3_prep_wind.inp` | `wind.ww3` | 1, from the input | 2, 1 (inline) | 14 872 | identical |

14 files, 406 220 bytes compared, 0 differ (sum of the column; v, `just prep-all`, 2026-11-01). The time records are Rust's count of the log line. `ww3_tbt1.1` has three times in each data file but writes one: the data are one line of 360 values, `IDLA` 3 reads three rows of 120 and drops the rest of a line after each, so the second and third rows take the next time stamps as data and the six lines of the file are used up by the first record. The next time read finds the end of the file, which ends the program normally. Rust follows the same records and the bytes agree (v, `just prep-pair ww3_tbt1.1 ww3_prep_mudt.inp`).

The regtests hold 20 `ww3_prep` inputs: the 14 above, and 6 in `mww3_test_01`, `_04`, `_05` and `_08`, which need
`ww3_multi` and are not run (v, `ls WW3/regtests/*/input/ww3_prep*`). All 14 are type `AI`. No regtest input of a
single-grid case has `LL`, `F1`, `F2`, `DAT`, binary input or a format that reverses, so everything below the
table is an input made here.

### Inputs no regtest has, on ww3_ts3's wind

| Variant | What it exercises | File | Bytes |
|---|---|---|---|
| `lev` | `LEV`, one field | `level.ww3` | 7 468 |
| `wns` | `WNS`, three fields (stability) | `wind.ww3` | 22 276 |
| `tau` | `TAU` | `momentum.ww3` | 14 872 |
| `rho` | `RHO`, the zeros above | `density.ww3` | 7 468 |
| `isi` | `ISI`, icebergs | `ice.ww3` | 14 872 |
| `nohdr` | `FLHDR` false: no header record | `wind.ww3` | 14 824 |
| `idla1` | `IDLA` 1, 43 values a row | `wind.ww3` | 14 872 |
| `idla4` | `IDLA` 4 | `wind.ww3` | 14 872 |
| `times` | `FLTIME` true, three times from the stream, a blank line between them | `wind.ww3` | 44 520 |
| `fmtf` | `IDFM` 2, `(1000(F8.2))`, time `(I8,1X,I6)` | `wind.ww3` | 14 872 |
| `fmtimp` | `IDFM` 2, `(43F8.2)` on integers: the implied decimal point | `wind.ww3` | 14 872 |
| `fmte` | `IDFM` 2, `(1000E12.4)`, two times | `wind.ww3` | 29 696 |
| `comma` | `IDFM` 2, every decimal point a comma: each field is cut at its comma and the columns shift | `wind.ww3` | 14 872 |
| `commai` | `IDFM` 2, the time `19680,06` in `I8`: the integer 19680 | `wind.ww3` | 14 872 |
| `ll_same` | `LL` on the model's own grid, 43 by 43 | `wind.ww3` | 14 872 |
| `dat_one` | `DAT`, `DATTYP` 0, two records of three values | `data0.ww3` | 100 |

`just prep-variants` builds them from `ww3_ts3`'s wind input (two 43 x 43 arrays) and runs each through the same pair on `ww3_ts3`'s grid. 16 byte-identical files, 260 200 bytes (v, `just prep-variants`, 2026-11-01).

### Interpolation, binary input, format reversal and `DAT`

`just prep-interp` writes the inputs to `regtests/prep/input` with `scripts/prep_interp.sh gen` and judges each
by `prep_pair.sh`. Five model grids: `sph`, 24 by 18 points of 0.5 degrees from 10 E, 40 N with a block of land
of 4 by 4 points; `glob`, 24 by 7 points of 15 degrees with simple closure; `pol`, 10 by 12 points of 1 degree from
78 N; `wrap`, 20 by 12 points from 355 E; and `ww3_ts3`'s Cartesian grid. The fields are quantised to multiples
of 1/32, so the text and the binary files of a case hold the same numbers. "Reached" is Rust's count of what
the run did: model points outside the input grid, cells with a land corner (of which all four, fallen back on
the nearest sea points: corrected, with a single neighbour, lost), points covered by two grids, address
clamps, points where the speed factor hit 1.5, and cells near a pole.

| Case | Model | Input | `IDLA`, `IDFM` | Times | Bytes | Reached | What it exercises |
|---|---|---|---|---|---|---|---|
| `ll_wnd` | sph | `WND LL` | 1,1 | 1 | 3 536 | none | two components, `WNT1` factor |
| `ll_cur` | sph | `CUR LL` | 1,1 | 1 | 3 536 | none | `CUR` (`CRT1`) |
| `ll_tau` | sph | `TAU LL` | 1,1 | 1 | 3 536 | none | `TAU` |
| `ll_wns` | sph | `WNS LL` | 1,1 | 1 | 5 272 | none | three fields |
| `ll_rho` | sph | `RHO LL` | 1,1 | 1 | 1 800 | none | zeros written |
| `ll_isi` | sph | `ISI LL` | 1,1 | 1 | 3 536 | none | icebergs |
| `ll_ice` | sph | `ICE LL` | 1,1 | 1 | 1 800 | none | one component |
| `ll_lev` | sph | `LEV LL` | 1,1 | 1 | 1 800 | none | `LEV` |
| `ll_mdn` | sph | `MDN LL` | 1,1 | 1 | 1 800 | none | `MDN` |
| `ll_ic2` | sph | `IC2 LL` | 1,1 | 1 | 1 800 | none | `IC2` |
| `ll_outside` | sph | `WND LL` | 1,1 | 1 | 3 536 | 315 outside | input grid inside the model: points outside, factor 0 |
| `ll_chk` | sph | `WND LL` | 1,1 | 1 | 3 536 | factor 1.5 at 410 | neighbouring points of opposite sign: factor limited to 1.5 |
| `ll_chkc` | sph | `CUR LL` | 1,1 | 1 | 3 536 | factor 1.5 at 229 | the same for `CUR` |
| `ll_times` | sph | `WND LL` | 1,1 | 3 | 10 512 | none | `FLTIME` true, three times, a blank line |
| `ll_idla2` | sph | `WND LL` | 2,1 | 1 | 3 536 | none | `IDLA` 2, 10 values a line |
| `ll_idla3` | sph | `WND LL` | 3,1 | 1 | 3 536 | none | `IDLA` 3 |
| `ll_idla4` | sph | `WND LL` | 4,1 | 1 | 3 536 | none | `IDLA` 4, 10 values a line |
| `ll_fmt` | sph | `WND LL` | 1,2 | 1 | 3 536 | none | `(17F8.3)` |
| `ll_rev` | sph | `WND LL` | 1,2 | 1 | 3 536 | none | `(10F8.3)`: 17 values need two records, format reversal |
| `ll_rev2` | sph | `WND LL` | 2,2 | 1 | 3 536 | none | `IDLA` 2, `(10F8.3)`, one `READ` for 238 values |
| `ll_bin` | sph | `WND LL` | 2,3 | 2 | 7 024 | none | binary file, one record for the array, time records, `FLTIME` true |
| `ll_clamp` | sph | `WND LL` | 1,1 | 1 | 3 536 | none | `IDLA` 7 and `IDFM` 9 are 1 |
| `llg_wnd` | glob | `WND LL` | 1,1 | 1 | 1 424 | none | closed longitudes, `ICLO_SMPL`, points beyond the last column |
| `llg_ice` | glob | `ICE LL` | 1,1 | 1 | 744 | none | closed |
| `llg_open` | glob | `ICE LL` | 1,1 | 1 | 744 | 14 outside | the same grid not closed: 14 points outside |
| `llc_wnd` | cart | `WND LL` | 1,1 | 1 | 14 872 | factor 1.5 at 1; 1849 near a pole | Cartesian model grid (`FLAGLL` false) |
| `llc_cur` | cart | `CUR LL` | 1,1 | 1 | 14 872 | 760 outside; factor 1.5 at 304; 1089 near a pole | Cartesian, input grid inside the model |
| `llp_wnd` | pol | `WND LL` | 1,1 | 1 | 1 040 | 90 near a pole | model 78 to 89 N, input to 89 N |
| `llp_pole` | pol | `WND LL` | 1,1 | 1 | 1 040 | 20 outside; 100 near a pole | input grid to 90 N, 20 points outside |
| `llw_wnd` | wrap | `WND LL` | 1,1 | 1 | 2 000 | none | model longitudes 355 to 374, input -10 to 20 |
| `f1_ice` | sph | `ICE F1` | 1,1 | 1 | 1 800 | 25 land corner (4 all four: 4 corrected, 0 lost) | lat-lon and mask inline, mask with a 4 by 4 block of land |
| `f1_wnd` | sph | `WND F1` | 1,1 | 1 | 3 536 | 25 land corner (4 all four: 4 corrected, 0 lost); factor 1.5 at 1 | two components |
| `f1_wns` | sph | `WNS F1` | 1,1 | 1 | 5 272 | 25 land corner (4 all four: 4 corrected, 0 lost); factor 1.5 at 1 | three fields, `IDLALL` 9 and `IDFMLL` 9 are 1 |
| `f1_chk` | sph | `CUR F1` | 1,1 | 1 | 3 536 | 25 land corner (4 all four: 4 corrected, 0 lost); factor 1.5 at 184 | `CUR`, factor limited |
| `f1_bigland` | sph | `ICE F1` | 1,1 | 1 | 1 800 | 186 land corner (119 all four: 76 corrected, 43 lost) | land block of 10 by 10: cells fallen back on the neighbours, some with none |
| `f1_allland` | sph | `ICE F1` | 1,1 | 1 | 1 800 | 336 land corner (336 all four: 0 corrected, 336 lost) | all land: every cell lost |
| `f1_names` | sph | `ICE F1` | 1,1 | 1 | 1 800 | 25 land corner (4 all four: 4 corrected, 0 lost) | `FROM` `NAME`: three text files |
| `f1_units` | sph | `ICE F1` | 1,1 | 1 | 1 800 | 25 land corner (4 all four: 4 corrected, 0 lost) | `FROM` `UNIT`: `fort.21`, `fort.22`, `fort.23` |
| `f1_fmt` | sph | `ICE F1` | 1,2 | 1 | 1 800 | 25 land corner (4 all four: 4 corrected, 0 lost) | formatted lat-lon `(18F10.4)`, mask `(18I2)` and data |
| `f1_bin` | sph | `ICE F1` | 2,3 | 1 | 1 800 | 25 land corner (4 all four: 4 corrected, 0 lost) | binary lat-lon, mask (one record a row) and data (one record) |
| `f1_idla` | sph | `ICE F1` | 4,1 | 1 | 1 800 | 29 land corner (9 all four: 9 corrected, 0 lost) | lat-lon `IDLA` 2, mask `IDLA` 3, data `IDLA` 4 inline |
| `f1g_ice` | glob | `ICE F1` | 1,1 | 1 | 744 | 10 land corner (3 all four: 3 corrected, 0 lost) | `CLO` true on the global grid |
| `f1g_open` | glob | `ICE F1` | 1,1 | 1 | 744 | 5 outside; 10 land corner (3 all four: 3 corrected, 0 lost) | `CLO` false |
| `f1c_ice` | cart | `ICE F1` | 1,1 | 1 | 7 468 | 144 land corner (90 all four: 86 corrected, 4 lost); 1681 near a pole | Cartesian, land block of 5 by 5 |
| `f1c_bigland` | cart | `ICE F1` | 1,1 | 1 | 7 468 | 1681 land corner (1553 all four: 529 corrected, 1024 lost); 1681 near a pole | Cartesian, land block of 20 by 20 |
| `f1p_ice` | pol | `ICE F1` | 1,1 | 1 | 552 | 25 land corner (16 all four: 16 corrected, 0 lost); 64 near a pole | near the pole, with a land block |
| `f2_ice` | sph | `ICE F2` | 1,1 | 1 | 1 800 | 32 outside; 61 land corner (13 all four: 13 corrected, 0 lost); 304 by two grids | two grids of 18 by 16 and 12 by 10, averaged where both cover |
| `f2_lev` | sph | `LEV F2` | 1,1 | 1 | 1 800 | 32 outside; 61 land corner (13 all four: 13 corrected, 0 lost); 304 by two grids | `LEV`, the grids swapped |
| `f2_isi` | sph | `ISI F2` | 1,1 | 1 | 3 536 | 32 outside; 61 land corner (13 all four: 13 corrected, 0 lost); 304 by two grids | `ISI`, three fields in the file |
| `f2_times` | sph | `ICE F2` | 1,1 | 2 | 3 552 | 32 outside; 61 land corner (13 all four: 13 corrected, 0 lost); 304 by two grids | `FLTIME` true, two files with times |
| `ai_bin2` | cart | `WND AI` | 2,3 | 2 | 29 696 | none | `IDLA` 2, a time record before each field, `FLTIME` true |
| `ai_bin1` | cart | `WND AI` | 1,3 | 1 | 14 872 | none | `IDLA` 1, one record a row |
| `ai_bin3` | cart | `WND AI` | 3,3 | 1 | 14 872 | none | `IDLA` 3 |
| `ai_bin4` | cart | `WND AI` | 4,3 | 1 | 14 872 | none | `IDLA` 4, one record |
| `ai_bin2f` | cart | `WND AI` | 2,3 | 1 | 14 872 | none | `IDLA` 2, no time record |
| `ai_ww3hdr` | cart | `WND AI` | 2,3 | 1 | 14 872 | none | a WAVEWATCH III file read back: its header record is checked and skipped |
| `ai_rev` | cart | `WND AI` | 1,2 | 1 | 14 872 | none | `(10F8.3)` on 43 values a row |
| `ai_revgrp` | cart | `WND AI` | 2,2 | 1 | 14 872 | none | `IDLA` 2, `(1X,2(F8.3))`: reversal restarts at the group |
| `dat_0` | cart | `DT0 AI` | 1,1 | 1 | 100 | none | `DATTYP` 0, two records of three values |
| `dat_1` | cart | `DT1 AI` | 1,1 | 1 | 124 | none | `DATTYP` 1, three records of four values over lines of 4, 2, 3 and 4 values |
| `dat_2` | cart | `DT2 AI` | 1,1 | 1 | 84 | none | `DATTYP` 2, one record of two |
| `dat_times` | cart | `DT0 AI` | 1,1 | 3 | 140 | none | `FLTIME` true: 2, 0 and 1 records at three times |
| `dat_ndat0` | cart | `DT0 AI` | 1,1 | 1 | 48 | none | `NDAT` 0: nothing written but the header |
| `dat_fmt` | cart | `DT0 AI` | 1,2 | 1 | 100 | none | `IDFM` 2, `(3F8.3)` (the time format reads the data) |
| `dat_bin` | cart | `DT0 AI` | 1,3 | 2 | 140 | none | binary: time, `NDAT` and data records |

65 files from 65 inputs, 306 992 bytes, 0 differ (v, `just prep-interp`, 2026-11-01).

## Stops

An input the Fortran stops on goes through the same pair with `PREP_ERROR=1`: Rust must stop with the same exit
status, and whatever field file the reference had written (its header) must be byte-identical.

| Input | Stops on | Where | Exit status of both |
|---|---|---|---|
| `e_badvalue` | a real that is not a number (`abc`) in a list-directed read | `w3arrymd.F90:275`, `LIBERROR_READ_VALUE` | 146 |
| `e_eof` | the second field is shorter than the grid | `w3arrymd.F90:271`, end of file | 255 |
| `e_field` | field id `XXX` | `ww3_prep.F90:428` | 1 |
| `e_type` | format id `XX` | `ww3_prep.F90:445` | 2 |
| `e_f2wnd` | `WND` with `F2` | `ww3_prep.F90:483` | 3 |
| `e_time` | time 1000 0 | `ww3_prep.F90:494` | 4 |
| `e_nxi` | `LL` with `NXI` 1 | `ww3_prep.F90:541` | 5 |
| `e_nyj` | `F1` with `NYJ` 1 | `ww3_prep.F90:758` | 10 |
| `e_dattyp` | `DATTYP` 3 | `ww3_prep.F90:563` | 6 |
| `e_recldt` | `RECLDT` 0 | `ww3_prep.F90:567` | 7 |
| `e_open` | data file that does not exist | `ww3_prep.F90:1518` | 49 |
| `e_openll` | lat-lon file that does not exist | `ww3_prep.F90:1510` | 47 |
| `e_openmask` | mask file that does not exist | `ww3_prep.F90:1514` | 48 |
| `e_binline` | `IDFM` 3 with the data inline | `ww3_prep.F90:970` | 20 |
| `e_dateof` | `NDAT` 2, one record | `ww3_prep.F90:1534` | 53 |
| `e_datndat` | `NDAT` not an integer | `ww3_prep.F90:1530` | 52 |
| `e_badtime` | a time that is not an integer in the data file | `ww3_prep.F90:1522` | 50 |
| `e_hdrfield` | WAVEWATCH III header of field `CUR` on a `WND` input | `ww3_prep.F90:992` | 21 |
| `e_hdrsize` | header of 42 by 43 on a 43 by 43 input | `ww3_prep.F90:992` | 21 |
| `e_dattype` | `DAT` read with the integer time format | `ww3_prep.F90:1534` | 53 |
| `e_shortrec` | binary record shorter than the 238 values read from it | `w3arrymd.F90:275`, `LIBERROR_SHORT_RECORD` | 152 |
| `e_mismatch` | real array read with `(17I8)` | `w3arrymd.F90:275`, `LIBERROR_FORMAT` | 142 |
| `e_mismatchi` | integer mask read with `(18F2.0)` | `w3arrymd.F90:441` or `:445`, `LIBERROR_FORMAT` | 142 |
| `e_shorttime` | binary time record of 4 bytes | `ww3_prep.F90:1522` | 50 |
| `e_f2time` | the two files hold different times | `ww3_prep.F90:1526` | 51 |

25 stops, 25 with the same exit status (v, `just prep-interp`). The Where column is the `EXTCDE` call that ends the Fortran; the status is the number it passes (1 to 53). Inside `INA2R` and `INA2I` (`w3arrymd.F90`) the call passes the runtime's own `IOSTAT`, and the shell sees it modulo 256: `255` is -1, the end of a file, `146` is `LIBERROR_READ_VALUE` (5010), `152` is `LIBERROR_SHORT_RECORD` (5016) and `142` is `LIBERROR_FORMAT` (5006).

## Refused

Each input is accepted by the Fortran (status 0) and stopped by Rust with status 64 and
`ww3_prep: <form> is not implemented`; `prep_pair.sh` exits 3 and prints `REFUSED`. Run (v, `just prep-interp`):

| Input | Form | Where Rust stops |
|---|---|---|
| `r_null` | a null value (`a , , c`) in a list-directed read | `split_line` |
| `r_edit` | the edit descriptor `TR1` in `(TR1,17F8.3)` | `parse_format` |
| `r_slash` | the edit descriptor `/` in `(17F8.3,/)` | `parse_format` |

Stopped in the code and not run against a reference that accepts it ⚠: an unstructured grid (`ModDef::read`
panics with its own message, not status 64); two input files of an `F2` input on one unit number (the reference
reopens the unit and then stops on the second time read, status 50, so it does not accept the input); a
WAVEWATCH III header on a `DAT` input file (the Fortran compares it with `NXI` and `NYI`, which `DAT` never
sets); a negative `NDAT` (the Fortran writes from an unallocated array); a string that continues on the next
record, a repeat count on a string and `Infinity` or `NaN` in a numeric field.

## Faults

A comparison that cannot fail proves nothing, so a fault in each stage went through the pair and was then
restored (`scripts/interp_faults.sh`, one perl substitution on one source file, `PREP_TAG` or `STRT_TAG` naming
it, 2026-11-01). The script puts the file back from a copy and checks it with `cmp`.

| Stage | Fault | Case | Differing bytes |
|---|---|---|---|
| Binary read | `REAL` read little-endian | `ai_bin2` | 29 108 of 29 696 |
| Binary header | the header record of a WAVEWATCH III file not recognised | `ai_ww3hdr` | Rust stops with status 152 |
| Format reversal | reversion to the start of the format, not the last group | `ai_revgrp` | 12 061 of 14 872 |
| Fixed-width read | a comma ends the field but the whole width is consumed | variant `comma` | 13 520 of 14 872 |
| Array layout | `IDLA` 3 reads the rows top to bottom | `ai_bin3` | 6 276 of 14 872 |
| `DAT` record | the count `ND` written as 0 | `dat_0` | 1 of 100 |
| `LL` weights | `RD12` and `RD22` swapped | `ll_wnd` | 2 524 of 3 536 |
| `LL` addresses | `IY2` taken from the lower row | `ll_wnd` | 2 572 of 3 536 |
| Closure | simple closure not set for a closed input | `llg_wnd` | 56 of 1 424 |
| Points outside | a point outside the input grid given a weight | `ll_outside` | 945 of 3 536 |
| Velocity factor | the limit 1.5 written as 1.4 | `ll_chk` | 2 490 of 3 536 |
| Masked cell | weights of a partly masked cell not renormalised | `f1_ice` | 70 of 1 800 |
| Masked cell | the Fortran's overwrite of `IX1` "fixed" (`IX2` instead) | `f1_ice` | 16 of 1 800 |
| Nearest points | `W3SORT` skipped | `f1_bigland` | 115 of 1 800 |
| Distance | `W3DIST` without the `ASIN` | `f1_bigland` | 52 of 1 800 |
| Distance | the Cartesian `W3DIST` with `DX` for `DY` in one term | `f1c_bigland` | 1 349 of 7 468 |
| Two grids | the factor `RD11` not divided by the overlap count | `f2_ice` | 877 of 1 800 |
| Two grids | the second grid's factor `XD22` not divided by the overlap count | `f2_lev` | 715 of 1 800 |

Equivalent edits, which no input can catch: `iy2 = js[3]` against `js[2]` (the same index in a rectilinear
cell), and the type-2 clamps listed under "Not reached" in `WW3_STRT.md`.

## Translation notes

- Everything is `f32` or `i32`, except the grid search, which is `REAL(8)` as the Fortran's. Sums run left to
  right as written (`:1233-1259`), the squares are `x*x`, `SQRT` is the correctly rounded `sqrtf`, and
  contraction is off, so no fused multiply-add.
- **A comma ends a fixed-width numeric field and is consumed with it.** The reference reads the `F8.2` fields of
  `    3,33   12,50` as 0.03 and 33.12, and the `I8` time `19680,06` as 19680; the next field starts after the
  comma (v, `just prep-variants`, the variants `comma` and `commai`). The earlier version of this port refused the
  input.
- `READ (NDSI,*)` is a list: items run on to the next record until enough are read, and the rest of the last
  record is dropped. A record of an unformatted file is read whole: the rest is dropped, a shorter one is
  `LIBERROR_SHORT_RECORD` (exit 152). A formatted `READ` that runs out of descriptors starts a new record and
  restarts the format at its last group of the outermost parentheses (Fortran 2003, 10.4): `(1X,2(F8.3))` reads
  the blank in the first record only (v, variant `ai_revgrp`). A format with a descriptor other than `I`, `F`, `E`,
  `D` and `nX` is refused.
- A real item with an `I` descriptor, and an integer with an `F`, stops the Fortran with `LIBERROR_FORMAT` (exit
  142); a value that cannot be read is `LIBERROR_READ_VALUE` (exit 146); the end of the file is -1 (exit 255).
  On an error `ww3_prep` calls `EXTCDE ( n )`; Rust exits with the same `n` (1 to 54), and the exit status of
  every stop in the table above equals the reference's.
- `NDSF` equal to `NDSI` (10) means the data follow in `ww3_prep.inp` itself, after `NEXTLN`. For `F1` and `F2`
  the same is true of the latitude-longitude file and the mask file (`NDSLL` = 10), and the lines come in the
  order the Fortran reads them.
- A binary data file is opened and its first record is read as `TSTR, TSFLD, NXT, NYT`: an empty file ends the
  program with no output (`END=888`), a record shorter than 24 bytes or another string rewinds the file, and a
  WAVEWATCH III header of another field or size stops with status 21. The header record is skipped when it
  matches (v, variant `ai_ww3hdr`).
- `NXI` and `NYI` are `NXJ(1)` and `NYJ(1)` (an `EQUIVALENCE`, `:291`): they are the model's size for `AI`, the
  input's for `LL`, `F1` and `F2`. With one component `NXJ(2)` is `NXJ(1)`, and the second field is read from
  the first one's file.
- `ICLO` is simple closure (`ICLO_SMPL`) for a spherical grid when `NXI*SXI` is 360 degrees to within a tenth of
  a step (`LL`, `:597-600`) or when the input says the longitudes are closed (`F1`, `F2`, `CLO`).

### Fortran defects translated

- **`W3FLDP` overwrites the first address of a fully masked cell** (`w3fldsmd.F90:2024-2030`). With two nearest
  sea points, the second one's `IX1`, `IY1` replace the first's, `IX2` and `IY2` stay 1, and `RD22` (the second
  weight) multiplies the field at `(1,1)`. The result is a weighted mean of the second point and the corner of
  the array. Rust does the same (v, `f1_ice`: 4 cells; `f1_bigland`: 76 cells; the fault `f1_nn_ix2` that
  "fixes" it differs on 16 bytes).
- **`DAT` reads its data records with the time format** `FORMT` (`ww3_prep.F90:1185`), not the data format
  `FORMF`, which `DAT` ignores. With `IDFM` 2 and a time in the stream (`FLTIME` true) the time format has to
  read integers and the data reals, so the reference stops on the first record (exit 53, v, `e_dattype`).
- **`NDAT` is read list-directed** (`:1173`) whatever `IDFM` is, except 3.
- **`W3GRMP` takes its near-pole branch on a Cartesian grid** (`w3gsrumd.F90:2718`): `D90-ABS(LAT0) > NEAR_POLE` is
  asked whatever the coordinates are, and a Cartesian `y` in metres is never within 10 of 90, so every point of an
  `LL` or `F1` input on `ww3_ts3`'s grid goes through `W3SPLX` with metres taken as degrees (1 849 of 1 849
  points of `llc_wnd`). Rust does the same, and the bits agree (v, `llc_wnd`, `llc_cur`, `f1c_ice`, `f1c_bigland`).
- **The guards on `IY21`, `IY22` against 1 and `MYM`** (`:1266-1276`) cannot be reached: `W3GRMP` returns
  `JS` in `1..NYI`.

## Not reached, not translated, not checked

- Branches of the program that no input here reached ⚠: a fully masked cell with exactly one sea neighbour (the
  counter "with one neighbour" is 0 in every row; two neighbours and none are reached); the two address guards
  `IY21 < 1` and `IY22 > MYM` (`:1266-1276`), which `W3GRMP` cannot trigger because it returns `JS` in `1..NYI`;
  a tripole closure (`ICLO_TRPL`, which `W3GSUC` of `ww3_grid` stops on with its own message); the
  closest-cell search of `W3GFCL` (`FNCL`, `DCIN` > 0; neither `ww3_prep` nor `ww3_grid` asks for it); a
  WAVEWATCH III header on a `DAT` binary file; two data files on one unit number; an unstructured grid.
  `WNT2`, `CRT2`, `O3`, `O15` and `T*` are off in this switch, so they have no code.
- The `mww3_test_*` inputs, which need `ww3_multi`.
- `scripts/rust_only_all.sh` is unchanged. `ww3_tp1.6` is the only one of its 17 cases with a forcing file, and
  `bash scripts/rust_only.sh ww3_tp1.6 ww3_tp1.6` is still six files identical with the new `ww3_prep` (v). The other
  regtests with a `ww3_prep` input (`ww3_tbt*`, `ww3_tic*`, `ww3_ts3`) have no pair under `work/pair` and use
  physics, ice and mud, that this change did not try on Rust's `ww3_shel` ⚠.
- Field values with an exponent letter other than `E` or `D`, `Infinity`, `NaN`.
- The Fortran log, which is not compared (Rust prints one line). The file the Fortran writes after an error is
  compared; its log is not.
- No timing was taken.

## Reproduce

```bash
just prep-pair ww3_tp1.6 ww3_prep_curr.inp
just prep-pair ww3_tic2.3 ww3_prep_ice1.inp scat
just prep-all
just prep-variants
just prep-interp                       # gen, then every case; just prep-interp run '^f1_' for a subset
bash scripts/interp_faults.sh          # edits prep.rs, gridsru.rs and strt.rs and restores them
bash scripts/rust_only.sh ww3_tp1.6 ww3_tp1.6
```
