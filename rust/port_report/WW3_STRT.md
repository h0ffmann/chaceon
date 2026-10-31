# WW3_STRT: `ww3_strt`, the initial conditions

`rust/crates/ww3-host/src/strt.rs` translates `ww3_strt` (`model/src/ww3_strt.F90:37-1025` at `92e2cf66`) for
`switches/switch_chaceon_shrd`, with `NEXTLN` (`w3servmd.F90:221-334`), `W3DIST` (`w3gsrumd.F90:5802-5877`)
and the `DISTAB` and `WAVNU1` tables it needs (a private copy of `shel.rs`'s, which this change does not
touch). `iors::write_start` writes `W3IORS` for `COLD`, `WIND` and `CALM` (`w3iorsmd.F90:496-624`). The binary
is `rust/tools/ww3_strt`: `ww3_strt <dir>` reads `<dir>/mod_def.ww3` and `<dir>/ww3_strt.inp`, writes
`<dir>/restart.ww3`.

**Status: `host`** (ADR-0005) for `ITYPE` 1, 3, 4 and 5. `ITYPE` 2 (JONSWAP, `EJ5P`) is not translated and
stops with a message: no regtest of the survey uses it ⚠. Nothing ran on a GPU; this is a host program.

## Result

For each case the reference image runs `ww3_grid` and `ww3_strt`, and Rust's `ww3_strt` reads that
`mod_def.ww3` and `ww3_strt.inp`. Rust's `restart.ww3` is deleted before it runs, and `cmp` judges the
file (v, `just strt-pair <regtest> [suffix]`, 2026-10-31, linux/arm64 image on Apple Silicon).

| Regtest | `ITYPE` | Branches reached (from the Rust log line) | Bytes | Result |
|---|---|---|---|---|
| `ww3_tp1.1` | 1 | `SIX<0` (circular), one frequency bin, one direction bin, `SIX` raised to 0.01 `HPFAC`, 335 of 360 points beyond 40 | 30 408 | identical |
| `ww3_tp1.2` | 1 | same, spherical `W3DIST` (`FLAGLL`), 96 of 121 | 10 332 | identical |
| `ww3_tp1.3` | 1 | `SIX<0`, one bin each, 35 of 42 | 3 696 | identical |
| `ww3_tp1.4` | 1 | `SIX<0`, one frequency bin, cosine power, 4 of 11 | 3 744 | identical |
| `ww3_tp1.5` | 1 | the same as `ww3_tp1.4` | 3 744 | identical |
| `ww3_tp1.7` | 1 | `SIX>0` kept, Gaussian frequency, cosine power, 18 of 25 | 77 760 | identical |
| `ww3_tp1.8` | 4 | `FACS` 0.01, user spectrum | 550 800 | identical |
| `ww3_tp1.9` | 4 | `FACS` 1.0288E-07, user spectrum | 6 854 400 | identical |
| `ww3_tp1.10` | 1 | `SIX>0` raised to the grid floor, Gaussian frequency, cosine power, 48 of 49 | 587 520 | identical |
| `ww3_tp2.1` | 1 | `SIX>0` kept, one bin each, 0 of 973 beyond 40 | 280 800 | identical |
| `ww3_tp2.2` | 1 | `SIX<0`, Gaussian frequency, cosine power, 17 257 of 17 381 | 15 018 912 | identical |
| `ww3_tp2.3` | 1 | `SIX<0`, Gaussian frequency, cosine power, 1 452 of 1 656 | 2 387 520 | identical |
| `ww3_tp2.4` | 1 | `SIX<0`, Gaussian frequency, cosine power, 13 451 of 14 548 | 20 952 000 | identical |
| `ww3_tp2.4` `curv` | 1 | the same, curvilinear (`HPFAC`, `HQFAC` from `W3CGDM`), 14 499 of 16 117 | 2 321 136 | identical |
| `ww3_tp2.15` | 3 | `WIND`: header only | 5 760 | identical |
| `ww3_ts1` | 5 | `CALM`: header only | 3 456 | identical |

16 cases, 49 091 988 bytes compared, 0 differ. The "points beyond 40" counts are the sea points where
`RDSQR>40` sets `FACTOR` to 0. The byte total is the sum of the column (`echo $((...))`).

## Faults

A comparison that cannot fail proves nothing, so one fault went through each type and was then restored
(`scripts/strt_pair.sh` after a one-line edit of `strt.rs`, 2026-10-31). Every fault was caught:

| Type | Fault | Case | Differing bytes |
|---|---|---|---|
| 1 | `-0.125` to `-0.1250001` in the Gaussian frequency | `ww3_tp1.7` | 1065 of 77 760 |
| 1 | `DY` for `DX` in `W3DIST`'s spherical `SPHI` | `ww3_tp1.2` | 476 of 10 332 |
| 1 | `VA*CG/SIG` to `VA*(CG/SIG)`, a one-ULP reorder | `ww3_tp1.7` | 298 of 77 760 |
| 4 | `FINP*FACS/TPI` to `FINP*(FACS/TPI)` | `ww3_tp1.8` | 71 of 550 800 |
| 3 | header type `WIND` written as `CALM` | `ww3_tp2.15` | 4 of 5760 |
| 5 | header type `CALM` written as `WIND` | `ww3_ts1` | 4 of 3456 |

## Translation notes

- `REAL` is `f32`. `FRREL**2` and `HMAX**2` are `x*x`. The run-time `COS(...)**NCOS` is libgcc's
  `__powisf2`. `EXP` and `LOG` are `ww3_libm`'s, `COS` is `sincosf`'s cosine; `W3DIST` is `REAL(8)` with
  glibc's `sin`, `cos`, `asin` from the reference image.
- `HPFAC` and `HQFAC` are `grid::Metrics`, the values `W3GNTX` leaves after `W3IOGR`.
- `FR1` was read and dropped by `moddef.rs`; it is now `ModDef::fr1`, because `IKM` needs it.
- `COLD` writes no time record, so record 2 stays zero or what the header overflowed into, and the 1078 byte
  header is longer than a record on small grids: `write_start` writes in WW3's order at WW3's positions, as
  `iors::write` does.
- `READ (NDSI,*)` is a list: tokens run on to the next line until enough are read (the `ITYPE` 4 spectrum
  spans many lines), and the rest of the last line is dropped. A slash, a repeat count or a string stops with a
  message.

## Not reached, not translated, not checked

- `ITYPE` 2 is not translated ⚠ (`EJ5P`, `w3servmd.F90:406-`): three regtests use it (the survey counted
  `ww3_strt.inp` files by type: 31, 3, 2, 5, 3 for types 1 to 5) and none was run here ⚠.
- The log line counts the frequency and direction single-bin branches, `SIX` and `SIY` raised to the grid floor,
  and `RDSQR>40`. It does not count: `THM<0` (every input is in 0 to 360), `HMAX<0`, `SIP<0`, `FACS<=0`, the
  `FP` lower clamp, the `IKM` clamp, `ABS(FRREL)>=10`. They compile the same expressions but no run shows them
  taken ⚠. `SIY` never exceeded its input in these cases ⚠.
- `ITYPE` outside 1 to 5 stops as the Fortran does, with another message and exit code; not run ⚠.
- The `O4`, `O5` and `O6` switches of `switch_chaceon_shrd` print plots to the log in Fortran; Rust prints one line.
  They write no file. The Fortran log is not compared.
- `ww3_tp1.6` also uses `ITYPE` 1 and was not run; the survey's other cases are outside the 16.
- Unstructured grids (`UNGTYPE`) stop in the `ModDef` reader, as before. No timing was taken.

## Reproduce

```bash
just strt-pair ww3_tp1.7
just strt-pair ww3_tp2.4 curv
```
