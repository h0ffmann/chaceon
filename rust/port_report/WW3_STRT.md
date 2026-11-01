# WW3_STRT: `ww3_strt`, the initial conditions

`rust/crates/ww3-host/src/strt.rs` translates `ww3_strt` (`model/src/ww3_strt.F90:37-1025` at `92e2cf66`) for
`switches/switch_chaceon_shrd`, with `NEXTLN` (`w3servmd.F90:221-334`), `EJ5P` (`w3servmd.F90:406-513`),
`W3DIST` (`w3gsrumd.F90:5802-5877`)
and the `DISTAB` and `WAVNU1` tables it needs (a private copy of `shel.rs`'s, which this change does not
touch). `iors::write_start` writes `W3IORS` for `COLD`, `WIND` and `CALM` (`w3iorsmd.F90:496-624`). The binary
is `rust/tools/ww3_strt`: `ww3_strt <dir>` reads `<dir>/mod_def.ww3` and `<dir>/ww3_strt.inp`, writes
`<dir>/restart.ww3`.

**Status: `host`** (ADR-0005) for `ITYPE` 1, 2, 3, 4 and 5. `ITYPE` 2 (a JONSWAP spectrum from `EJ5P`,
`ww3_strt.F90:589-764`) was translated on 2026-11-01 and judged on two regtests and 18 variants of its input
(below). Nothing ran on a GPU; this is a host program.

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
| `ww3_tp2.5` | 2 | `SIX<0` (the distance form), curvilinear, `SIX` raised to the grid floor, 60 762 of 62 274 points beyond 40, 3 points with `ARG<-170` in a frequency | 8 967 744 | identical |
| `ww3_tp2.11` | 2 | `SIGA` and `SIGB` defaulted, 0 of 124 279 points beyond 40, `ARG<-170` in 5 frequencies | 298 274 400 | identical |
| `ww3_tp2.15` | 3 | `WIND`: header only | 5 760 | identical |
| `ww3_ts1` | 5 | `CALM`: header only | 3 456 | identical |

18 cases, 356 334 132 bytes compared, 0 differ (the 16 of 2026-10-31 and the two of type 2, v, `just strt-variants`
for the last two). The "points beyond 40" counts are the sea points where `RDSQR>40` sets `FACTOR` to 0. The byte total is the sum of the column (`echo $((...))`).

### Type 2 variants

`just strt-variants` (`scripts/strt_variants.sh`) runs the two regtests and 18 inputs written to
`regtests/strt/input`, 14 on `ww3_tp2.2`'s spherical grid (193 by 93, 17 381 sea points, 9 frequencies of 0.04 Hz
and ratio 1.1, 24 directions) and 4 on the curvilinear grid of `ww3_tp2.4` (`curv`). 20 restarts, 526 791 456
bytes, 0 differ (v, `just strt-variants`, 2026-11-01). What each reaches is Rust's own count, printed in the
log line:

| Variant | Input (`ALFA FP THM GAMMA SIGA SIGB XM SIX YM SIY`) | What it reaches | Bytes |
|---|---|---|---|
| `dflt` | `0 0 270 1 0 0 90 10 0 10` | all four defaults, `GAMMA` 1 (`YLN` below `EPS`: no peak enhancement), `FP` pulled to the top of the spectrum | 15 018 912 |
| `dfltg` | the same with `GAMMA` 3.3 | the defaults `SIGA` 0.07 and `SIGB` 0.09 used | 15 018 912 |
| `gamma0` | `GAMMA` 0.5 | `GAMMA` raised to 1 | 15 018 912 |
| `gamma3` | `0.0081 0.1 270 3.3 0.07 0.09 ...` | peak enhancement with `FP` above the spectrum | 15 018 912 |
| `thmneg`, `thmbig` | `THM` -123.456 and 1000.123 | the loop that adds 360, and `MOD(THM,360.)`; no bin opposite the mean, so no `ARG<-170` | 15 018 912 each |
| `thm45` | `THM` 45 | a mean direction on a bin (24 bins of 15 degrees) | 15 018 912 |
| `nosix` | `SIX` -1 | the distance form of the spatial factor, `SIX` raised to the grid floor | 15 018 912 |
| `fplow`, `fpmid`, `fphigh` | `FP` 0.001, 0.06 and 3.0 | `FP` raised to half the first frequency; left alone; lowered to the last | 15 018 912 each |
| `floors` | `SIX = SIY = 0.0001` | both raised to a hundredth of the largest grid factor, 17 380 of 17 381 points beyond 40 | 15 018 912 |
| `narrow` | `SIX = SIY = 0.2` | 17 376 points beyond 40 | 15 018 912 |
| `sigs` | `SIGA` 0.02, `SIGB` 0.3 | a narrow and a wide Gaussian around the peak | 15 018 912 |
| `c_dflt`, `c_gamma3`, `c_nosix`, `c_floors` | the same on `ww3_tp2.4 curv` | `HPFAC` and `HQFAC` from `W3CGDM` | 2 321 136 each |

## Faults

A comparison that cannot fail proves nothing, so one fault went through each type and was then restored
(`scripts/strt_pair.sh` after a one-line edit of `strt.rs`, 2026-10-31; for type 2 `scripts/interp_faults.sh`, one
perl substitution on `strt.rs` a fault, 2026-11-01). Every fault was caught but the last:

| Type | Fault | Case | Differing bytes |
|---|---|---|---|
| 1 | `-0.125` to `-0.1250001` in the Gaussian frequency | `ww3_tp1.7` | 1065 of 77 760 |
| 1 | `DY` for `DX` in `W3DIST`'s spherical `SPHI` | `ww3_tp1.2` | 476 of 10 332 |
| 1 | `VA*CG/SIG` to `VA*(CG/SIG)`, a one-ULP reorder | `ww3_tp1.7` | 298 of 77 760 |
| 4 | `FINP*FACS/TPI` to `FINP*(FACS/TPI)` | `ww3_tp1.8` | 71 of 550 800 |
| 3 | header type `WIND` written as `CALM` | `ww3_tp2.15` | 4 of 5760 |
| 5 | header type `CALM` written as `WIND` | `ww3_ts1` | 4 of 3456 |
| 2 | `F**5` of `EJ5P` as `(x2*x2)*x` | `gamma3` | 604 603 of 15 018 912 |
| 2 | the upper clamp of `FP` removed | `fphigh` | 8 814 427 of 15 018 912 |
| 2 | the lower clamp of `FP` removed | `fplow` | 6 872 047 of 15 018 912 |
| 2 | `SIGA` and `SIGB` swapped | `sigs` | 6 816 685 of 15 018 912 |
| 2 | `BETA` 4.06 and -2.34 swapped | `gamma3` | 7 663 741 of 15 018 912 |
| 2 | `MOD(630.-THM,360.)` without the `MOD` | `thm45` | 2 512 418 of 15 018 912 |
| 2 | `THM = MOD(THM,360.)` removed | `thmbig` | 1 914 063 of 15 018 912 |
| 2 | `YLN` times 1.0000001 | `gamma3` | 461 692 of 15 018 912 |
| 2 | default `SIGA` 0.08 for 0.07 | `dfltg` | 3 133 511 of 15 018 912 |
| 2 | the loop that adds 360 to a negative `THM` removed | `thmneg` | not caught |

## Translation notes

- `REAL` is `f32`. `FRREL**2` and `HMAX**2` are `x*x`. The run-time `COS(...)**NCOS` is libgcc's
  `__powisf2`. `EXP` and `LOG` are `ww3_libm`'s, `COS` is `sincosf`'s cosine; `W3DIST` is `REAL(8)` with
  glibc's `sin`, `cos`, `asin` from the reference image.
- Type 2: `EJ5P`'s `F**5` and `(FP/F)**4` are the multiplications gfortran makes of a constant integer power, `x2*(x*x2)`
  and `x2*x2`; `FRR**BETA` is `powf` (`ww3_libm`), `COS(...)**2` is the cosine times itself, `EXP` and `LOG` are
  `ww3_libm`'s. The Gaussian in space, the grid floors of `SIX` and `SIY` and the distribution over the sea points are
  the code of type 1, shared. The `F**5` association is gcc's `powi` table as remembered ⚠, not re-read here, and the restarts confirm
  it: the fault `j_f5`, which uses `(x2*x2)*x`, differs on 604 603 bytes.
- `HPFAC` and `HQFAC` are `grid::Metrics`, the values `W3GNTX` leaves after `W3IOGR`.
- `FR1` was read and dropped by `moddef.rs`; it is now `ModDef::fr1`, because `IKM` needs it.
- `COLD` writes no time record, so record 2 stays zero or what the header overflowed into, and the 1078 byte
  header is longer than a record on small grids: `write_start` writes in WW3's order at WW3's positions, as
  `iors::write` does.
- `READ (NDSI,*)` is a list: tokens run on to the next line until enough are read (the `ITYPE` 4 spectrum
  spans many lines), and the rest of the last line is dropped. A slash, a repeat count or a string stops with a
  message.

## Not reached, not translated, not checked

- `ww3_tp2.10` also uses `ITYPE` 2, on an SMC grid that the `ModDef` reader does not read: not run ⚠.
- Type 2 guards that no input here can reach, or that no output can show ⚠: `ANG>1.E-20` is always true (the
  smallest `COS(0.5*(THM-TH))**2` for a direction opposite the mean is about 1.9E-15 in single precision);
  `ARG>-170` false is reached (the regtests and `gamma3` count it) but `EXP` of such an argument is 0 anyway; the
  `MAX(GAMMA,1.)` is invisible, since `YLN` is below `EPS` for any `GAMMA` up to 1; the `EXPMIN` limits of `EJ5P` and
  `F<EPS` cannot be reached with `FP` between half the first and the last frequency. The loop that adds 360 to a
  negative `THM` is reached by `thmneg` but changed no bit of the restart when disabled (the fault `j_thmneg`):
  `MOD(630.-THM,360.)` cancels it at the angles tried ⚠.
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
just strt-variants
bash scripts/interp_faults.sh '^j_'     # the type 2 faults; edits strt.rs and restores it
```
