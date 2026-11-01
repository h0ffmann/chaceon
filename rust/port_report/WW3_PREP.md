# WW3_PREP: `ww3_prep`, the input preprocessor

`rust/crates/ww3-host/src/prep.rs` translates `ww3_prep` (`model/src/ww3_prep.F90:17-1755` at `92e2cf66`) for
`switches/switch_chaceon_shrd`, with `W3FLDO` and `W3FLDG` in their `WRITE` branches (`w3fldsmd.F90:88-527`,
`:954-1470`), `INA2R` for `IDFM` 1 and 2 (`w3arrymd.F90:76-291`) and `NEXTLN` (`w3servmd.F90:221-334`). The binary
is `rust/tools/ww3_prep`: `ww3_prep <dir>` reads `<dir>/mod_def.ww3` and `<dir>/ww3_prep.inp` and writes the field
file in `<dir>`. A file name in the input is joined to `<dir>`, as the Fortran's `FNMPRE` (`./`,
`w3odatmd.F90:330`) joins it to the working directory.

**Status: `host`** (ADR-0005) for the input type `AI` (`ITYPE` 1: the field is already on the model grid), every
field id, free-format and formatted input. Types `LL`, `F1`, `F2` and `DAT`, binary input (`IDFM` 3) and the
other forms listed under "Refused" stop with status 64 and a message naming the form; the share of
`ww3_prep.F90` that is interpolation (`:585-928`, `:1226-1395`) is therefore not translated, although the row of
`rust/ports.tsv` counts the whole program ⚠. Nothing ran on a GPU; this is a host program.

## What it does

For `AI` the program interpolates nothing. It reads each field with `INA2R` (`VSC = 1.`, `VOF = 0.`, so
`1.*x + 0.` is applied and a negative zero becomes `+0.`), copies it to the output arrays, and `W3FLDG` writes a
time record (two default `INTEGER`) and one record per field. `REAL` is `f32`, the file is big-endian, a record is
its length (4 bytes), the data, the length again.

| Field id | `IFLD` | File | Records after the time | Source (`ww3_prep.F90`) |
|---|---|---|---|---|
| `IC1` to `IC5`, `MDN`, `MTH`, `MVS`, `ICE`, `LEV` | -7 to 2 | `ice1.ww3`, ..., `muddens.ww3`, `mudthk.ww3`, `mudvisc.ww3`, `ice.ww3`, `level.ww3` | 1: `FA` = field 1 | `:1210-1215`, `w3fldsmd.F90:1043-1046` |
| `WND`, `CUR`, `TAU` | 3, 4, 6 | `wind.ww3`, `current.ww3`, `momentum.ww3` | 2: `FX` = field 1, `FY` = field 2 | `:1216-1224` |
| `WNS` | 3 | `wind.ww3` | 3: field 3 is read (`:1143-1147`) and written | |
| `ISI` | 1 | `ice.ww3` | 2: `FX`, `FY`; `FA` is not written | |
| `RHO` | 7 | `density.ww3` | 1: reads two fields, writes `FA` = `AC`, which stays zero | `:1122`, `:1221` |

`RHO` writes zeros whatever the input holds: `IFLD` 7 is not `<= 2`, so `FA` takes `AC`, and `W3FLDG` writes `FA`
for a field that is not two-dimensional. That is what the Fortran does and what Rust does (v, `rho` below).

## Result

For each case the reference image runs `ww3_grid` and `ww3_prep` on a regtest's namelist and input. Rust's
`ww3_prep` reads **the reference's `mod_def.ww3`** (so a case whose `ww3_grid` needs a switch the Rust `ww3_grid`
lacks still tests `ww3_prep`) and the same `ww3_prep.inp`; its field file is deleted before it runs and `cmp`
judges it (v, `just prep-all`, 2026-11-01, linux/arm64 image on Apple Silicon).

| Regtest | Input | File | Time records | `IDLA`, `IDFM` | Bytes | Result |
|---|---|---|---|---|---|---|
| `ww3_tp1.6` | `ww3_prep_curr.inp` | `current.ww3` | 1, from the input | 3, 1 (inline) | 608 | identical |
| `ww3_tbt1.1` | `ww3_prep_mudd.inp` | `muddens.ww3` | 1 | 3, 1 | 1 512 | identical |
| `ww3_tbt1.1` | `ww3_prep_mudt.inp` | `mudthk.ww3` | 1 | 3, 1 | 1 512 | identical |
| `ww3_tbt1.1` | `ww3_prep_mudv.inp` | `mudvisc.ww3` | 1 | 3, 1 | 1 512 | identical |
| `ww3_tbt2.1` | `ww3_prep_conc.inp` | `ice.ww3` | 3, from the file | 1, 2 | 61 932 | identical |
| `ww3_tbt2.1` | `ww3_prep_mud1.inp` | `muddens.ww3` | 2 | 1, 2 | 41 304 | identical |
| `ww3_tbt2.1` | `ww3_prep_mud2.inp` | `mudthk.ww3` | 2 | 1, 2 | 41 304 | identical |
| `ww3_tbt2.1` | `ww3_prep_mud3.inp` | `mudvisc.ww3` | 2 | 1, 2 | 41 304 | identical |
| `ww3_tic2.2` | `ww3_prep_ice.inp` | `ice.ww3` | 1, from the input | 3, 1 | 40 072 | identical |
| `ww3_tic2.2` | `ww3_prep_ice5.inp` | `ice5.ww3` | 1 | 3, 1 | 40 072 | identical |
| `ww3_tic2.3` `scat` | `ww3_prep_ice.inp` | `ice.ww3` | 1 | 3, 1 | 40 072 | identical |
| `ww3_tic2.3` `scat` | `ww3_prep_ice1.inp` | `ice1.ww3` | 1 | 3, 1 | 40 072 | identical |
| `ww3_tic2.3` `scat` | `ww3_prep_ice5.inp` | `ice5.ww3` | 1 | 3, 1 | 40 072 | identical |
| `ww3_ts3` | `ww3_prep_wind.inp` | `wind.ww3` | 1 | 2, 1 (inline) | 14 872 | identical |

14 files, 406 220 bytes compared, 0 differ (sum of the column). The time records are Rust's count of the log line.
`ww3_tbt1.1` has three times in each data file but writes one: the data are one line of 360 values, `IDLA` 3 reads
three rows of 120 and drops the rest of a line after each, so the second and third rows take the next time stamps
as data and the six lines of the file are used up by the first record. The next time read finds the end of the
file, which ends the program normally. Rust follows the same records and the bytes agree (v,
`just prep-pair ww3_tbt1.1 ww3_prep_mudd.inp`).

The regtests hold 20 `ww3_prep` inputs: the 14 above, and 6 in `mww3_test_01`, `_04`, `_05` and `_08`, which need
`ww3_multi` and are not run (v, `ls WW3/regtests/*/input/ww3_prep*`). All 14 run here are type `AI`, so no
regtest input of a single-grid case exercises `LL`, `F1`, `F2` or `DAT`. The `ww3_tic2.3` grid is `scat`; the five
`ww3_grid_*.nml` of that regtest differ in the physics groups, not in the grid ⚠ (not compared one by one).

### Inputs no regtest has

`just prep-variants` builds them from `ww3_ts3`'s wind input (two 43 x 43 arrays) and runs each through the same
pair on `ww3_ts3`'s grid. 12 byte-identical files, 215 484 bytes (v, `just prep-variants`, 2026-11-01):

| Variant | What it exercises | Bytes |
|---|---|---|
| `lev` | `LEV`, one field | 7 468 |
| `wns` | `WNS`, three fields (stability) | 22 276 |
| `tau` | `TAU` | 14 872 |
| `rho` | `RHO`, the zeros above | 7 468 |
| `isi` | `ISI`, icebergs | 14 872 |
| `nohdr` | `FLHDR` false: no header record | 14 824 |
| `idla1`, `idla4` | `IDLA` 1 and 4, 43 values a row | 14 872 each |
| `times` | `FLTIME` true, three times from the stream, a blank line between them | 44 520 |
| `fmtf` | `IDFM` 2, `(1000(F8.2))`, time `(I8,1X,I6)` | 14 872 |
| `fmtimp` | `IDFM` 2, `(43F8.2)` on integers: the implied decimal point | 14 872 |
| `fmte` | `IDFM` 2, `(1000E12.4)`, two times | 29 696 |

## Refused

Each input is accepted by the Fortran (status 0) and stopped by Rust with status 64 and
`ww3_prep: <form> is not implemented`; `prep_pair.sh` exits 3 and prints `REFUSED`. The first four were run
(v, `just prep-variants`); the rest stop in the code and were not run ⚠.

| Form | Where Rust stops | Run |
|---|---|---|
| Type `LL`, `F1`, `F2` | `ww3_prep.F90:534-554`, `:585-928` (the interpolation of `W3GRMP` and `W3FLDP`) | `LL` yes; `F1`, `F2` ⚠ |
| Type `DAT` | `:557-578`, `:1170-1192` | yes |
| A formatted read that needs the format again (more values than descriptors) | `INA2R` with `IDFM` 2 and `IDLA` 2 or 4 | yes (`IDLA` 2, `(43F8.2)`) |
| A comma inside a numeric field of a formatted read | The reference accepts it; the likely rule is that the runtime ends the field at the comma and the columns shift ⚠ (not probed) | yes |
| `IDFM` 3 (binary input) | `:967-997` | ⚠ no binary input file was made |
| A null value (`,,`), a slash or `r*` without a value in a list-directed read | `split_line` | ⚠ |
| A format with a descriptor other than `I`, `F`, `E`, `D` and `nX` | `parse_format` | ⚠ |
| An unstructured grid | `ModDef::read` panics with its own message, not status 64 | ⚠ |

## Faults

A comparison that cannot fail proves nothing, so a fault in each stage went through the pair and was then
restored (`prep_pair.sh` after a one-line edit of `prep.rs`, `PREP_TAG` naming the fault, 2026-11-01). The restored
file was compared with a saved copy by `cmp`.

| Stage | Fault | Case | Differing bytes |
|---|---|---|---|
| Record header | `GTYPE` written as `GTYPE + 1` | `ww3_tp1.6` | 1 of 608 |
| Time stepping | the time of the first record kept for the next | `ww3_tbt2.1` `conc` (3 times) | 4 of 61 932 |
| Array layout | `IDLA` 3 reads the rows bottom to top as top to bottom | `ww3_tic2.2` `ice` | 14 000 of 40 072 |
| Array layout | `IDLA` 4 likewise | variant `idla4` | 1 806 of 14 872 |
| Formatted read | the implied decimal point one digit off | variant `fmtimp` | 13 212 of 14 872 |
| Field selection | `RHO` writes field 1 instead of `AC` | variant `rho` | 6 844 of 7 468 |
| Record order | `FY` written before `FX` | `ww3_tp1.6` | 330 of 608 |

The `IDLA` 3 fault is **not** caught by `ww3_tp1.6`: its field is uniform in `y` (the three rows are equal), so
reversing the rows gives the same bytes (v, run and identical). `ww3_tic2.2` and `idla4` catch it. The evidence
for the `IDLA` 3 row order is `ww3_tic2.2` (and `ww3_ts3` with `IDLA` 4 for the order of a single read), not
`ww3_tp1.6`, and `ww3_tbt1.1`'s fields are constant too.

## Translation notes

- Everything is `f32` or `i32`; there is no arithmetic but `1.*x + 0.` and copies, so no float kind changes and no
  libm call occurs. The one decision is the text to number conversion: the runtime calls `strtof` (correctly
  rounded) for list-directed and formatted reads, as `str::parse::<f32>` does ⚠ (read from libgfortran, not from
  a probe; every value of the 26 files agrees).
- `READ (NDSI,*)` is a list: items run on to the next record until enough are read, and the rest of the last
  record is dropped. This is why `ww3_tp1.6`'s 22-value rows span two lines and why `ww3_ts3`'s arrays end
  mid-line.
- `NDSF` equal to `NDSI` (10) means the data follow in `ww3_prep.inp` itself, after `NEXTLN`; any other
  `NDSF` with `'UNIT'` opens `fort.<NDSF>`, as the runtime does for an unnamed unit.
- A fixed-width field blank is read as zero, blanks inside it are ignored (`BLANK='NULL'`), and a field with no
  decimal point takes the last `d` digits as the fraction.
- On an error the Fortran calls `EXTCDE ( n )`; Rust exits with the same `n` (40 to 54, and `-1` for a read past
  the end of the file in `INA2R`) and prints a shorter message. The Fortran log (unit 6) is replaced by one
  summary line; it is not compared. `O3`, `O15` and `T*` are off in `switch_chaceon_shrd`, so they have no code.
- `WNT1` and `CRT1` (on in this switch) apply only after interpolation (`:1318-1327`, `:1344-1353`), which `AI`
  skips; the correction is therefore not translated ⚠.

## Not reached, not translated, not checked

- Types `LL`, `F1`, `F2` and `DAT`, `IDFM` 3: refused, above. The `mww3_test_*` inputs, which need `ww3_multi`.
- Field values with an exponent letter other than `E` or `D`, `Infinity`, `NaN`: not in any input ⚠.
- No timing was taken.

## Reproduce

```bash
just prep-pair ww3_tp1.6 ww3_prep_curr.inp
just prep-pair ww3_tic2.3 ww3_prep_ice1.inp scat
just prep-all
just prep-variants
bash scripts/rust_only.sh ww3_tp1.6 ww3_tp1.6
```
