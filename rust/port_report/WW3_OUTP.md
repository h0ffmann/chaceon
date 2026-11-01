# WW3_OUTP: `ww3_outp`, the point output postprocessor

`rust/crates/ww3-host/src/outp.rs` translates `ww3_outp` (`model/src/ww3_outp.F90:36-1015`) and its internal
`W3EXPO` (`:1029-2844`) at `92e2cf66` for `switches/switch_chaceon_shrd`, with the `READ` branch of `W3IOPO`
(`w3iopomd.F90:1039-1472`), `PRT1DS`, `PRT2DS` (`w3arrymd.F90:1364-1623`, `:1941-2379`) and `STME21`
(`w3timemd.F90:681-752`). `fortfmt.rs` is gfortran's formatted output for the edit descriptors these programs use and
`finp.rs` is `NEXTLN` (`w3servmd.F90:221-334`) with the list-directed `READ`. The binary is `rust/tools/ww3_outp`:
`ww3_outp <dir>` reads `<dir>/ww3_outp.inp`, `<dir>/mod_def.ww3` and `<dir>/out_pnt.ww3`, writes the log to standard
output, and writes `tabNN.ww3` or `ww3.YYMMDDHH.spc` into `<dir>`.

**Status: `host`** (ADR-0005) for the options below. Nothing ran on a GPU; this is a host program.

## What is reached, what is refused

| `ITYPE` | `OTYPE` | State |
|---|---|---|
| 0 | | reached: lists the output times |
| 1 | 1 (print plots) | reached: `PRT1DS` with automatic or fixed scale, `PRT2DS` normalised or scaled |
| 1 | 2 (nondimensional spectral table) | refused, exit 64: it calls `W3SPR4`, `W3SIN4` and the flux routine of the switch |
| 1 | 3 (spectra transfer file) | reached, formatted and unformatted |
| 2 | 1, 2, 5, 6 | reached, one point and several, `FLAGLL` true and false |
| 2 | 3, 4 (nondimensional) | refused, exit 64: the same routines as `ITYPE` 1 `OTYPE` 2 |
| 3 (source terms) | all | refused, exit 64: `W3SPR4`, `W3SIN4`, `W3SDS4`, `W3SNL1`, `W3SBT1`, `W3SDB1` are written here for `W3SRCE`'s arrays, not for one point's spectrum; calling them from the host needs a second wiring ⚠ |
| 4 (partitions, bulletins) | all | refused, exit 64: `W3PART` and `W3BULL` are not translated |
| any | | a `mod_def.ww3` that is not rectilinear or curvilinear (`GTYPE` 3 for `ww3_tp2.6`): refused, exit 64 |
| any | | a table unit of 5, 6, 10 or 20: refused, because the Fortran `OPEN` would close a unit the program holds |

A refusal prints what the Fortran printed up to that point, then `ww3_outp: not implemented: <what>` on standard
error. `scripts/outp_pair.sh` prints it as `REFUSED` and exits 2, so it is a listed line.

## Result

For each input the reference image runs `ww3_outp` on `work/pair/<case>/mod_def.ww3` and `out_pnt.reference`, and Rust's
`ww3_outp` runs on the same files. `cmp` judges the standard output log (`outp.log`), the exit status (`exit.code`)
and every file the program wrote (v, `just outp-all`, 2026-11-01, linux/arm64 image on Apple Silicon). The `out_pnt.ww3`
is the one Rust and the reference both write byte-identically (`WW3_SHEL_TS1.md`). `ww3_ts1` is the pair directory
`default_2500m`.

### Inputs of WW3's regtests

| Case and input | Files compared | Bytes | Result |
|---|---:|---:|---|
| `ww3_tp1.1` `ww3_outp` | 3 | 7 503 | identical |
| `ww3_tp1.4` `ww3_outp_spec` | 2 | 15 278 | identical |
| `ww3_tp1.4` `ww3_outp_tab50` | 3 | 5 794 | identical |
| `ww3_tp1.4` `ww3_outp_tab51` | 3 | 5 757 | identical |
| `ww3_tp1.5` `ww3_outp_spec` | 2 | 15 278 | identical |
| `ww3_tp1.5` `ww3_outp_tab50` | 3 | 5 794 | identical |
| `ww3_tp1.5` `ww3_outp_tab51` | 3 | 5 757 | identical |
| `ww3_tp1.6` `ww3_outp_p05` | 3 | 4 493 | identical |
| `ww3_tp1.6` `ww3_outp_p10` | 3 | 4 493 | identical |
| `ww3_tp1.6` `ww3_outp_p15` | 3 | 4 493 | identical |
| `ww3_tp1.6` `ww3_outp_p20` | 3 | 4 493 | identical |
| `ww3_tp1.6` `ww3_outp` | 2 | 28 153 | identical |
| `ww3_tp1.7` `ww3_outp` | 3 | 2 926 744 | identical |
| `ww3_tp1.8` `ww3_outp` | 2 | 15 532 | identical |
| `ww3_tp1.9` `ww3_outp` | 2 | 145 049 | identical |
| `ww3_tp2.2` `ww3_outp_spec` | 2 | 11 018 | identical |
| `ww3_tp2.2` `ww3_outp_tab51` | 3 | 1 537 | identical |
| `ww3_tp2.2` `ww3_outp_tab52` | 3 | 1 537 | identical |
| `ww3_tp2.2` `ww3_outp_tab53` | 3 | 1 537 | identical |
| `ww3_ts1` `ww3_outp_spec` | 2 | 64 049 | identical |
| `ww3_ts1` `ww3_outp_tab50` | 3 | 171 964 | identical |

21 inputs, 56 files, 3 446 253 bytes compared, 0 differ. The files
include the log and the exit status of each run. The byte total is the sum of the column. `ww3_tp2.6` is refused (`GTYPE` 3).
Inputs for cases with no pair directory (`mww3_test_*`, `ww3_tbt1.1`, `ww3_tbt2.1`, `ww3_tic2.2`, `ww3_tic2.3`, `ww3_ts2`,
`ww3_tr1`, `ww3_tpt1.1`, `ww3_tp2.7`, `ww3_tp2.8`, `ww3_tp2.10`, `ww3_tp2.11`, `ww3_tp2.17`) were not run ⚠: none has an
`out_pnt.reference` here.

### Inputs written for this change

The regtests give no input for `ITYPE` 2 `OTYPE` 1, 5 or 6, for the unformatted transfer file, for scaled print plots,
for the error exits or for a spherical grid with several points. These do. They are not WW3 regtests; the first line
of each names its pair case.

| Input (`regtests/outpf_inputs/`) | Files compared | Bytes | Result |
|---|---:|---:|---|
| `ww3_outp_v_c_badotype` | 3 | 1 391 | identical |
| `ww3_outp_v_c_badtype` | 2 | 1 281 | identical |
| `ww3_outp_v_c_contents` | 2 | 3 355 | identical |
| `ww3_outp_v_c_nolast` | 2 | 1 360 | identical |
| `ww3_outp_v_c_print_scaled` | 2 | 3 547 | identical |
| `ww3_outp_v_c_spc_formatted` | 3 | 12 263 | identical |
| `ww3_outp_v_c_spc_unformatted` | 3 | 6 651 | identical |
| `ww3_outp_v_c_t2o1_multi` | 3 | 3 188 | identical |
| `ww3_outp_v_c_t2o1_one` | 3 | 1 909 | identical |
| `ww3_outp_v_c_t2o2_multi` | 3 | 3 503 | identical |
| `ww3_outp_v_c_t2o5_multi` | 3 | 3 392 | identical |
| `ww3_outp_v_c_t2o5_one` | 3 | 1 963 | identical |
| `ww3_outp_v_c_t2o6_multi` | 3 | 3 039 | identical |
| `ww3_outp_v_c_t2o6_one` | 3 | 1 831 | identical |
| `ww3_outp_v_s_print_normalized` | 2 | 5 952 | identical |
| `ww3_outp_v_s_t2o1_multi` | 3 | 2 018 | identical |
| `ww3_outp_v_s_t2o1_one` | 3 | 1 429 | identical |
| `ww3_outp_v_s_t2o2_multi` | 3 | 2 283 | identical |
| `ww3_outp_v_s_t2o5_multi` | 3 | 2 192 | identical |
| `ww3_outp_v_s_t2o5_one` | 3 | 1 498 | identical |
| `ww3_outp_v_s_t2o6_multi` | 3 | 1 887 | identical |
| `ww3_outp_v_s_t2o6_one` | 3 | 1 333 | identical |

22 inputs, 61 files, 67 265 bytes, 0 differ.
Refused, listed: `ww3_outp_v_c_nondim`, `ww3_outp_v_c_nonwave_type3`, `ww3_outp_v_c_partitions`, and the regtest input of `ww3_tp2.6`.

## Formatted output

`fortfmt.rs` prints a `REAL(4)` the way libgfortran does: it promotes to `double`, and the digits are the correctly
rounded decimal expansion with an exact tie going to the even digit (`libgfortran/io/write_float.def`, `snprintf` with
`%+-#.*e` and `%+-#.*f` ⚠ not re-read at this pin). Rust's `{:.N}` and `{:.Ne}` on that `f64` give the same digits.
Checked against the C library's `snprintf` on 29 729 124 value and precision pairs (random bit patterns, ties such as
0.125 and 2.5, negative zero), 0 differ (v, a throwaway program compiled on the host with `rustc` 1.99.1, not kept: the
repository has no Rust tests). What the module adds is the field rules: asterisks when the value does not fit, the
optional leading zero dropped only to make it fit, `-0.00` for a negative value that rounds to zero, a decimal point for
`F6.0`, and the exponent forms of `E` and `ES`. A list-directed integer is twelve characters wide in this build (v, the
reference's `ww3.*.hs` rows are `225*12` bytes).

## Traps met

- `W3EXPO` reads the last frequency's `EBX` and `EBY` in the tail of `ETX` and `ETY` (`:1583`): the loop variable leaks.
- The inlined `WAVNU1` in `W3EXPO` (`:1508-1525`) has no `I1 >= 1` guard, unlike `WAVNU1`; `Dispersion::tables` gives it
  the tables.
- `PTNME` is 40 characters and is printed whole by `A`, but `A10` in the transfer file cuts it to ten.
- `ww3_outp.inp` is read by `NEXTLN` and then by a list-directed `READ` that continues over records and stops at `/`.

## Faults

A comparison that cannot fail proves nothing, so a fault went through each stage and was then restored (source edit,
`OUTP_TAG` naming the work directory, 2026-11-01):

| Stage | Fault | Case | Result |
|---|---|---|---|
| Formatter | `Fw.d` truncates instead of rounding (`fortfmt.rs`) | `ww3_tp1.4` `ww3_outp_tab50.inp` | DIFFERENT, 88 of 3 476 bytes |
| Unit conversion | wind direction `MOD(271.-WDO*RADE,360.)` for 270 (`outp.rs`) | `default_2500m` `ww3_outp_spec.inp` | DIFFERENT, 13 of 64 047 bytes of the log |
| Record | `WAO` and `WDO` read in the other order from `out_pnt.ww3` (`outp.rs`) | `default_2500m` `ww3_outp_spec.inp` | DIFFERENT, 25 667 of 64 047 bytes of the log |

The last two first ran on `ww3_tp1.4` with `ww3_outp_tab50.inp` and came out identical: that case has no wind, and
`OTYPE` 2 prints neither speed nor direction, so it cannot see them. They were moved to the case that can. Each fault
was restored and `diff` of the saved copy was empty; `just outp-all` was then run again and is the table above.

## Not verified

- The options refused above.
- Inputs of cases that have no `out_pnt.reference` in `work/pair/`.
- A grid that is not rectilinear or curvilinear.
- Reading an `out_pnt.ww3` written with `OFILES(2)` 1 (one file per time) ⚠.
- A device: none. Timing: none.
