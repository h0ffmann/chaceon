# WW3_GRID: `ww3_grid`, the grid preprocessor

`rust/tools/ww3_grid` translates `ww3_grid` (`model/src/ww3_grid.F90`, a call to `W3GRID`) for
`switches/switch_chaceon_shrd` at `92e2cf66`: `W3GRID` (`w3gridmd.F90:1151-7366`), `READNL`
(`w3gridmd.F90:7392-7705`), the namelist readers of `W3NMLGRID` (`w3nmlgridmd.F90:248-2391`) for the groups the
regtests use, `INA2R` and `INA2I` for free format (`w3arrymd.F90:76-461`), the part of the grid search utility that
output boundary points need (`w3gsrumd.F90`: `GSU_CREATE`, `W3GFCL_R8`, `W3CKCL_R8`, `GETPQR`, `W3GRMP_R8`) and the
`WRITE` branches of `W3IOGR` (`w3iogrmd.F90:115-1781`). The binary is `ww3_grid <dir>`: it reads `<dir>/ww3_grid.nml`,
the namelist file `GRID%NML` names and the depth, coordinate, mask and obstruction files, resolved from `<dir>` as the
Fortran resolves them from the working directory, and writes `<dir>/mod_def.ww3`.

| File | Holds |
|---|---|
| `rust/crates/ww3-host/src/iogr.rs` | `W3IOGR`'s writer, every field of every record |
| `rust/crates/ww3-host/src/moddef.rs` | the reader, which now keeps every field it used to skip |
| `rust/crates/ww3-host/src/gridpre.rs` | `W3GRID`: defaults, namelists, grid, status maps, spectral grid, constants, tables |
| `rust/crates/ww3-host/src/gridio.rs` | namelist groups, `READNL`, list-directed reads, `INA2R`, `INA2I` |
| `rust/crates/ww3-host/src/gridsru.rs` | the grid search utility, for output boundary points |
| `rust/tools/ww3_grid/src/main.rs` | the binary; `--roundtrip <in> <out>` reads and writes a `mod_def.ww3` |

**Status: `host`** (ADR-0005) on the 28 cases below, for `RECT` and `CURV` grids from `ww3_grid.nml`. `UNST` and `SMCG`
grids, the old `ww3_grid.inp`, `IDFM` 2 and 3 (formatted and unformatted data files), tripole closure and a target
near a pole stop with a message. Nothing here ran on a GPU; this is a host program.

## Result

| Regtest | Branches reached (from the Rust log line) | Bytes | Result |
|---|---|---:|---|
| `ww3_ts1` | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 28 497 841 | identical |
| `ww3_ts1` `ST4_T700` namelist | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter, `SDSCUM` 0 | 28 497 841 | identical |
| `ww3_ts1` t700 | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter, `SDSCUM` 0 | 28 497 841 | identical |
| `ww3_ts1` filipot | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter, `SDSBCK` tables | 28 497 841 | identical |
| `ww3_ts1` strain | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 28 497 841 | identical |
| `ww3_ts1` circle | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter, `SDSDTH` 180 | 28 497 841 | identical |
| `ww3_ts1` noshelter | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 no shelter | 28 497 841 | identical |
| `ww3_ts1` flatswell | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 28 497 841 | identical |
| `ww3_ts1` sharp | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 28 497 841 | identical |
| `ww3_ts1` romero2 | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter, `SDSCUM` 0 | 28 497 841 | identical |
| `ww3_ts1`, 50 m | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 28 497 841 | identical |
| `ww3_ts1` t700, 8 m | RECT SPHE closure NONE; depth `IDLA` 3, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter, `SDSCUM` 0 | 28 497 841 | identical |
| `ww3_tp1.1` | RECT SPHE closure SMPL; depth `IDLA` 2, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 25 230 893 | identical |
| `ww3_tp1.2` | RECT SPHE closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 25 218 513 | identical |
| `ww3_tp1.3` | RECT CART closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 1 inbound; ST4 shelter | 25 214 381 | identical |
| `ww3_tp1.4` | RECT CART closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 1 inbound; ST4 shelter | 25 240 073 | identical |
| `ww3_tp1.5` | RECT CART closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 1 inbound; ST4 shelter | 25 240 073 | identical |
| `ww3_tp1.6` | RECT CART closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 1 inbound; ST4 shelter | 25 370 845 | identical |
| `ww3_tp1.7` | RECT SPHE closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 2 inbound (1 connected); ST4 shelter | 27 533 473 | identical |
| `ww3_tp1.8` | RECT CART closure NONE; depth `IDLA` 1, `TRFLAG` 0; `PART`, 1 inbound; ST4 no shelter, `SDSBCK` tables | 54 754 153 | identical |
| `ww3_tp1.9` | RECT CART closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 1 inbound; ST4 shelter | 184 732 605 | identical |
| `ww3_tp1.10` | RECT CART closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 1 inbound; ST4 shelter | 58 781 233 | identical |
| `ww3_tp2.1` | RECT CART closure NONE; depth `IDLA` 4, `TRFLAG` 0; `PART`, 3 inbound (3 connected); ST4 shelter | 25 281 489 | identical |
| `ww3_tp2.2` | RECT SPHE closure NONE; depth `IDLA` 2, `TRFLAG` 0; `PART`, 0 inbound; 2 output boundary lines, 200 points; ST4 shelter | 26 100 097 | identical |
| `ww3_tp2.3` | RECT CART closure NONE; depth `IDLA` 4, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 25 904 637 | identical |
| `ww3_tp2.4` | RECT SPHE closure NONE; depth `IDLA` 1, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 26 407 789 | identical |
| `ww3_tp2.15` | CURV SPHE closure NONE; depth `IDLA` 1, `TRFLAG` 2; mask file; ST4 shelter, `SDSCUM` 0 | 33 916 069 | identical |
| `ww3_tp2.4` `curv` | CURV SPHE closure NONE; depth `IDLA` 1, `TRFLAG` 0; `PART`, 0 inbound; ST4 shelter | 26 045 977 | identical |

28 cases, 982 946 392 bytes compared, 0 differ (v, `just grid-all`, 2026-10-31, linux/arm64 image on Apple Silicon). Rust's `mod_def.ww3` is deleted before each Rust run, and `cmp` judges the file. The bytes are the sum of the column. The `ww3_ts1` variants are `scripts/ts1_variants.sh`'s namelist lines; the `50 m` and `8 m` rows change the depth.

## The writer alone

`just iogr-roundtrip work/grid/*/` reads each reference `mod_def.ww3`, writes it again with `iogr.rs` and compares
the bytes (v, 2026-10-31, linux/arm64 image on Apple Silicon). All 28 files come back identical, 982 946 392 bytes in all (v). That is the writer alone: the reader fills `MAPSF(:,3)` and the grid code is not involved. The comparison can fail: the fault of the next section, in `iogr.rs`, makes `ww3_tp1.1` come back different.

## Faults

A comparison that cannot fail proves nothing, so a fault went through each stage and was then restored
(`scripts/grid_pair.sh` after a source edit, `GRID_TAG` naming the work directory, 2026-10-31):

| Stage | Fault | Case | Result |
|---|---|---|---|
| Writer | `IICEHDISP` and `IICEDDISP` swapped in the `DTCFL` record of `iogr.rs` | `ww3_tp1.1`, pair and round trip | DIFFERENT, 8 of 25 230 893 bytes; the round trip is DIFFERENT too |
| Spectral grid | `DSII(NK) = 0.5*SIG(NK)*(XFR-1.)` without `/XFR` | `ww3_tp1.1` | DIFFERENT, 18 of 25 230 893 bytes |
| A table | one ULP added to `FWTABLE(150)` | `ww3_tp1.1` | DIFFERENT, 1 of 25 230 893 bytes |
| Output boundary weights | `RDBPO/RDTOT` divided by `RDTOT*1.0000001` | `ww3_tp2.2` | DIFFERENT, 948 of 26 100 097 bytes |
| Obstruction | `TRNX = 1.0000001 - OBSX` | `ww3_tp2.15` | DIFFERENT, 1807 of 33 916 069 bytes |
| Not a fault | `GRAV**4` as four products, not the folded power | `ww3_tp1.1` | identical: the products round to the same `f32` as the exact power for `GRAV` and `TPI` (v, a script with exact fractions), so these cases do not tell `fold_pow` from the products ⚠ |

Each fault was restored and `diff` of the saved copy was empty; `ww3_ts1`, `ww3_tp2.2` and `ww3_tp2.15` were then run again and are identical (v).

## Translation notes

- `REAL` is `f32`; `x**2` is `x*x`. A power of a `PARAMETER` (`GRAV**2`, `GRAV**4`, `TPI**4`, `(DAIR/DWAT)**2`) is
  folded by gfortran with one rounding of the exact result, not by repeated products: `fold_pow` does that with
  integer arithmetic. A power of a run-time value (`SIG(NK)**(2+WNMEANP*2)`, `XFR**(-FACHF-2)`) is `powf`, even when
  the exponent is the module variable `FACHF = 5.`.
- `SIN`, `COS` and `TAN` of a `REAL` are the platform's `sinf`, `cosf` and `tanf`, which is glibc's in the reference
  image; `LOG` and `POW` are `ww3-libm`'s.
- `ICLOSE_NONE` is -1, `ICLOSE_SMPL` 2 and `ICLOSE_TRPL` 8 (`w3gsrumd.F90:314-319`), not 0, 1 and 2.
- `MAPSF(:,3)` is zero in the file; `W3IOGR` fills it on read. The writer writes zeros and the reader fills it.
- `W3IOGR` writes `IICEHDISP` before `IICEDDISP` and reads them in the other order. The file holds the writer's
  order, and `ModDef` names them in that order.
- `INSIN4(.TRUE.)` runs inside the writer; Rust runs it in `gridpre.rs`, from the tables of `insin4.rs`. With
  `TAUWSHELTER = 0` the module keeps `TAUHFT2` and `DELTAIL` at zero.
- A list-directed `READ` takes repeat counts (`1080*1` in the depth files) and starts every statement on a new record.
- `W3NMLGRID` reads the excluded-body group with the excluded-point namelist (`w3nmlgridmd.F90:2243`), so bodies in the
  file never reach `W3GRID`: Rust does the same.
- Output boundary points: `W3GRMP` runs in `REAL(8)` on `REAL(4)` targets and the weights are narrowed to `REAL`.

## Not reached, not translated, not checked

**Stops with a message, not translated:**

- `GRID%TYPE` `UNST` and `SMCG`; the old `ww3_grid.inp` input; `IDFM` 2 and 3 (formatted with a `FORMAT`, and unformatted
  data files); `FROM` other than `NAME` or `PART`; an obstruction file sharing the depth file's unit; tripole closure;
  a slash or a quoted item in a list-directed read, and a null value written `r*` (consecutive commas are read as one
  separator, where Fortran leaves the item unchanged ⚠); an output boundary point in a cell within 10 degrees
  of a pole (`W3SPLX` branch of `W3GRMP_R8`); a search utility closure other than none and simple.
- The mask and nearest-neighbour branches and the closest-cell search of the search utility are not translated; `W3GRID`
  does not use them.

**Translated and never reached by a case** (the code compiles the expressions; no run takes the branch):

- Excluded points and bodies: no case has any, and at this pin bodies cannot reach `W3GRID` (see the notes) ⚠.
- `TRFLAG` 1, 3, 4, 5 and 6 (only `FLAGTR` 4, which becomes 2, and 0 occur); the `FLAGTR` clamp.
- `PTM` 2 to 5, with their `WSCUT` and `NOSWLL` overrides; `CALTYPE` other than `standard`; `IUSSP` above 25; a
  `STK_WN` list.
- `&SLN1`, `&SBT1` and `&SDB1` are never in a namelist file of these cases (`ww3_tp1.8` has `&DB1`, which `READNL`
  does not match), so their user-value path is unchecked; their defaults go through the same formulas and are checked.
- Keys of `&SIN4`, `&SDS4`, `&SNL1`, `&MISC` and `&OUTS` that no case sets: for example `ZWND`, `ALPHA0`, `SINTHP`,
  `ZALP`, `SWELLF5`, `Z0RAT`, `SDSP`, `SDSBT`, `SDSABK`, `SDSPBK`, `SDSBINT`, `SDSCOS`, `SDSBRF1`, `WNMEANP`, `FXPM3`,
  `FXFMAGE`, `LAMBDA`, `KDCONV`, and every ice, partitioning and calendar key of `&MISC` except `FLAGTR`, `XP`, `STDX`,
  `STDY` and `STDT`. The defaults of all of them are checked, because every case runs the same formulas on them.
- `RECT` with `CLOS` `SMPL` and a spherical grid: only `ww3_tp1.1`. `ICLOSE_SMPL` with `CURV`: none.
- `FLAGLL` when `GRID%COORD` is neither `SPHE` nor `CART`: Rust takes spherical ⚠.

**Not compared:**

- The Fortran writes `ww3_grid.nml.log` and `ST4TABUHF2.bin` (a cache of `TABU_TAUHF2`, read back when it is present)
  next to `mod_def.ww3`, and prints a long log. Rust writes `mod_def.ww3` only and prints two lines. The reference ran
  in a clean directory every time, so it never read a cache.
- `fold_pow` assumes that gfortran folds a `PARAMETER` power with one rounding (MPFR); the compiler's folding was not
  read ⚠. The bytes do not depend on it for these constants.
- No timing was taken. Rust's `ww3_grid` is not faster or slower on purpose; `ww3_tp1.9` (a 185 MB file) and the `SDSBCK`
  tables of `ww3_tp1.8` and `filipot` are the slow cases ⚠ not measured.

## Reproduce

```bash
just grid-pair ww3_tp2.2
just grid-pair ww3_tp2.4 curv
GRID_VARIANT=filipot just grid-pair ww3_ts1
just grid-all
just iogr-roundtrip work/grid/ww3_tp2.2
```
