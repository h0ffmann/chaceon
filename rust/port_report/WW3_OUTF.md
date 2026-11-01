# WW3_OUTF: `ww3_outf`, the field output postprocessor

`rust/crates/ww3-host/src/outf.rs` translates `ww3_outf` (`model/src/ww3_outf.F90:35-528`) and its internal `W3EXGO`
(`:546-2665`) at `92e2cf66` for `switches/switch_chaceon_shrd`, with the `READ` branch of `W3IOGO`
(`w3iogomd.F90:2654-4016`), `W3READFLGRD` (`:333-565`), `W3FLDTOIJ` (`:759-1189`), `PRTBLK` (`w3arrymd.F90:1110-1362`)
and `OUTA2I` (`:625-778`). It shares `fortfmt.rs`, `finp.rs` and `STME21` with `ww3_outp` (`WW3_OUTP.md`). The binary is
`rust/tools/ww3_outf`: `ww3_outf <dir>` reads `<dir>/ww3_outf.inp`, `<dir>/mod_def.ww3` and `<dir>/out_grd.ww3`, writes the
log to standard output, and writes `tabNN.ww3` (`ITYPE` 2) or `ww3.YYMMDDHH.<field>` (`ITYPE` 3 and 4) into `<dir>`.

**Status: `host`** (ADR-0005). Nothing ran on a GPU; this is a host program.

## What is reached, what is refused

| `ITYPE` | State |
|---|---|
| 0 contents | reached |
| 1 print plots | reached: scaled and normalised, scalar, vector and direction fields, a window and a stride |
| 2 statistics | reached: one `tabNN.ww3` per field, the sums in `DOUBLE PRECISION` as the Fortran has them |
| 3 transfer files | reached: `IDFM` 1, 2, 3 and `IDLA` 1 to 5 (the pairs of the table below), `FORMF` `(1X,32I4)` and `(1X,17I7)`, the `eNN.` names of a repeated hour; `(1X,20I6)` is written for `.ust` and `.mss` and no input holds them ⚠ |
| 4 sea-point text | reached |

Every field `W3EXGO` lists under this switch is translated: groups 1, 2, 4 to 9. The ones it does not list (the
spectrum group 3, group 4 beyond 17, `FWS`, `USF`, `P2L`, `TWI`, `FIC`, `USP`, and the `BT4`, `IS2` and `SETUP` fields of
group 1) end the run with the Fortran's own message and exit 1, which Rust reproduces (`ww3_outf_v_spectrum_not_listed`).
Refused, exit 64: a `mod_def.ww3` that is not rectilinear or curvilinear (`ww3_tp2.6`), and group 10 (user-defined
fields). `scripts/outf_pair.sh` prints a refusal as `REFUSED` and exits 2.

## Result

The same way as `ww3_outp`: the reference image runs `ww3_outf` on `work/pair/<case>/mod_def.ww3` and
`out_grd.reference`, Rust runs on the same files, and `cmp` judges the log, the exit status and every file written
(v, `just outf-all`, 2026-11-01).

### Inputs of WW3's regtests

| Case and input | Files compared | Bytes | Result |
|---|---:|---:|---|
| `ww3_tp1.1` `ww3_outf` | 2 | 17 733 | identical |
| `ww3_tp1.2` `ww3_outf` | 2 | 7 883 | identical |
| `ww3_tp1.3` `ww3_outf_depth` | 6 | 8 105 | identical |
| `ww3_tp1.3` `ww3_outf_flds_hrly` | 2 | 140 024 | identical |
| `ww3_tp1.3` `ww3_outf_Hxy` | 14 | 21 636 | identical |
| `ww3_tp1.3` `ww3_outf_whgt_end` | 2 | 6 587 | identical |
| `ww3_tp1.4` `ww3_outf` | 2 | 17 561 | identical |
| `ww3_tp1.5` `ww3_outf` | 2 | 13 676 | identical |
| `ww3_tp1.6` `ww3_outf` | 5 | 4 880 | identical |
| `ww3_tp1.7` `ww3_outf` | 1807 | 483 221 | identical |
| `ww3_tp1.8` `ww3_outf` | 4 | 5 640 | identical |
| `ww3_tp1.9` `ww3_outf` | 5 | 34 190 | identical |
| `ww3_tp2.1` `ww3_outf_flds_hrly` | 20 | 404 222 | identical |
| `ww3_tp2.1` `ww3_outf` | 2 | 247 325 | identical |
| `ww3_tp2.15` `ww3_outf` | 37 | 766 771 | identical |
| `ww3_tp2.2` `ww3_outf` | 2 | 1 312 | identical |
| `ww3_tp2.3` `ww3_outf` | 26 | 530 919 | identical |
| `ww3_tp2.4` `ww3_outf_depth` | 3 | 287 627 | identical |
| `ww3_tp2.4` `ww3_outf` | 6 | 1 147 199 | identical |

19 inputs, 1949 files, 4 146 511 bytes compared, 0 differ; `ww3_tp1.7` alone writes
1 807 files (a file per field per time, 1 200 steps). `ww3_tp2.6` is refused. Inputs for cases with no pair
directory (`mww3_test_*`, `ww3_tbt1.1`, `ww3_tbt2.1`, `ww3_tic2.2`, `ww3_tic2.3`, `ww3_ts3`, `ww3_tp2.5`, `ww3_tp2.7` to `ww3_tp2.11`,
`ww3_tp2.13`, `ww3_tp2.16`) were not run ⚠.

### Inputs written for this change

| Input (`regtests/outpf_inputs/`) | Files compared | Bytes | Result |
|---|---:|---:|---|
| `ww3_outf_v_badtype` | 2 | 686 | identical |
| `ww3_outf_v_contents` | 2 | 2 760 | identical |
| `ww3_outf_v_files_idla1_idfm1` | 5 | 4 853 | identical |
| `ww3_outf_v_files_idla1_idfm2` | 5 | 3 365 | identical |
| `ww3_outf_v_files_idla1_idfm3` | 5 | 2 783 | identical |
| `ww3_outf_v_files_idla2_idfm1` | 5 | 4 845 | identical |
| `ww3_outf_v_files_idla2_idfm2` | 5 | 3 353 | identical |
| `ww3_outf_v_files_idla2_idfm3` | 5 | 2 719 | identical |
| `ww3_outf_v_files_idla3_idfm2` | 5 | 3 365 | identical |
| `ww3_outf_v_files_idla4_idfm2` | 5 | 3 353 | identical |
| `ww3_outf_v_files_idla5_idfm1` | 5 | 3 873 | identical |
| `ww3_outf_v_files_idla5_idfm2` | 5 | 3 873 | identical |
| `ww3_outf_v_files_idla5_idfm3` | 5 | 2 911 | identical |
| `ww3_outf_v_files_window` | 4 | 1 844 | identical |
| `ww3_outf_v_group_flags_short` | 2 | 885 | identical |
| `ww3_outf_v_group_flags` | 3 | 1 328 | identical |
| `ww3_outf_v_not_in_file` | 2 | 4 854 | identical |
| `ww3_outf_v_print_current_direction` | 2 | 3 870 | identical |
| `ww3_outf_v_print_current_normalized` | 2 | 2 152 | identical |
| `ww3_outf_v_print_current_vector` | 2 | 3 876 | identical |
| `ww3_outf_v_print_normalized` | 2 | 8 766 | identical |
| `ww3_outf_v_print_units` | 2 | 8 692 | identical |
| `ww3_outf_v_seapoints` | 5 | 2 292 | identical |
| `ww3_outf_v_spectrum_not_listed` | 2 | 12 446 | identical |
| `ww3_outf_v_statistics_window` | 3 | 1 090 | identical |
| `ww3_outf_v_statistics` | 6 | 2 051 | identical |
| `ww3_outf_v_unknown_name` | 2 | 4 845 | identical |

27 inputs, 98 files, 101 730 bytes, 0 differ. They reach print plots with a stride, a window,
normalised and unit scales, a current as a vector and as a direction, statistics, every `IDFM` and `IDLA` pair above, the
sea-point text, the group-flag input form, an unknown name, a field the file lacks, a field the program does not list, an
illegal `ITYPE` and a group list one line short.

## Traps met

- `W3EXGO`'s four grid arrays and four sea-point arrays are initialised once per call, not per field. A field that sets
  only `XX` and `XY` (`.tbb`, `.mss`) leaves `X1` as the field before it set it, and the map is made from `X1`. The port
  keeps the arrays across fields; no input here holds those two fields, so that branch is read, not run ⚠.
- `XGRD` and `YGRD` are `DOUBLE PRECISION`: the header of an unformatted transfer file holds eight-byte limits where the
  formatted header holds the same values in `F8.2`. The first run wrote four-byte ones and 16 bytes of every header were
  wrong.
- `W3FLDTOIJ` matches on the first six characters of a name, so `STMAXE` and its kin are not names at all: the regtest
  `ww3_tp2.15` asks for five fields, not eleven, and the log says so.
- `.taua` passes `(TAUA, TAUADIR)` to `ATAN2`, not `(TAUADIR, TAUA)`, and keeps `TAUA` in `X1`; the port copies both.
- The sequence of the format statements 990 and 999 puts a blank line between the two error messages: 990 has no closing `/`.
- A list-directed integer is twelve characters wide in this build.

## Faults

A fault went through each stage and was then restored (`OUTF_TAG`, 2026-11-01):

| Stage | Fault | Case | Result |
|---|---|---|---|
| Formatter | `Iw` left-justified (`fortfmt.rs`) | `ww3_tp1.6` `ww3_outf.inp` | DIFFERENT, 24 of 1 317 bytes of the log and 908, 618, 472 bytes of three files |
| Unit conversion | peak direction offset by 0.01 degree before `MOD(630.-RADE*X,360.)` (`outf.rs`) | `ww3_tp2.15` `ww3_outf.inp` | DIFFERENT, 552 of 21 837 bytes of the first `.dir` file, every `.dir` file |
| Record | the two records of the current swapped (`outf.rs`) | `ww3_tp1.6` `ww3_outf.inp` | DIFFERENT, 82 of 1 717 bytes of `.cur` |

Each fault was restored and `diff` of the saved copy was empty; the table above is the run after.

## Not verified

- Group 10 and the fields `W3EXGO` does not list, apart from the error exit of one of them.
- `ww3_outf` on `UNST`, `SMC` or tripole grids.
- Inputs of cases with no `out_grd.reference` here, and `ww3_outf.inp` forms with `IDLA` 5 on a curvilinear grid ⚠ (the
  `XGRD` branch is reached on a rectilinear grid only).
- A device: none. Timing: none.
