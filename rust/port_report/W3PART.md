# W3PART: spectral partitioning, and the partition fields of out_grd.ww3

`rust/crates/ww3-host/src/partition.rs` translates `W3PART`, `PTSORT`, `PTNGHB`, `PT_FLD` and `PTMEAN`
(`model/src/w3partmd.F90:138-1541` at `92e2cf66`) and `W3CPRT` (`w3iosfmd.F90:108-344`). `iogo.rs` fills `PHS`,
`PTP`, `PLP`, `PDIR`, `PSI`, `PWS`, `PWST`, `PNR`, `PTHP0`, `PQP`, `PPE`, `PGW`, `PSW`, `PTM1`, `PT1`, `PT2` and `PEP`
as `W3OUTG` does (`w3iogomd.F90:2506-2597`) and writes group 4 fields 1 to 17 (`:3355-3388`). `shel.rs` calls `W3CPRT`
when a group 4 field is requested and a field output is due (`w3wavemd.F90:2393-2403`). Host only; no kernel and no
SPIR-V file changed.

**Status: `translated` for `W3PART`, `PT_FLD`, `PTMEAN` and `W3CPRT`, `host` for `PTSORT` and `PTNGHB`** (the ledger
rule: a routine is `host` only when every branch ran). Nothing ran on a GPU ⚠.

## Result

The reference and Rust run the same case, `cmp` judges the files (v, `just bnd-all`, linux/arm64 image):

| Case | Partitioning | `out_grd.ww3` | `out_pnt.ww3` | `restart001.ww3` |
|---|---|---|---|---|
| `ww3_tpt1.1` `PTM1` | `PTMETH` 1, 3 partitions at 9 points | identical, 14 482 bytes | identical | identical |
| `ww3_tpt1.1` `PTM2` | 2, wave-age cut | identical, 14 482 | identical | identical |
| `ww3_tpt1.1` `PTM3` | 3, topographic only | identical, 14 482 | identical | identical |
| `ww3_tpt1.1` `PTM4` | 4, 2 partitions | identical, 8 002 | identical | identical |
| `ww3_tpt1.1` `PTM5` | 5, frequency cut `PTFC` 0.1 | identical, 8 002 | identical | identical |
| `ww3_tp2.15` (changing wind) | 1 | identical, 2 776 706 | identical | identical |

All 18 group 4 fields of `TYPE%FIELD%LIST` are requested (`PHS ... PEP TWS PNR`). `ww3_tpt1.1` carries WW3's
multimodal start spectrum (`ww3_strt.inp`, `ITYPE` 4); `PNR` is 3.0 at all 9 sea points for `PTM1` to `PTM3` and 2.0
for `PTM4` and `PTM5` (v, `scripts/` parser in the log entry). `ww3_tp2.15` has `PNR` 1 and 2 from 276 points.
The reference writes the same bytes twice: `out_grd.reference` of `ww3_tpt1.1` `PTM2` has SHA-256 `e0d755215a258722...`
on two runs (v). The reference binary is built `ST4 PR3 NL1 BT1 DB1`, while WW3's `ww3_tpt1.1` switch is `ST0 PR0`; the
comparison is Fortran against Rust on the same executable, so the sources differ from WW3's own run of this test ⚠.

## Faults

A comparison that cannot fail proves nothing, so one-line edits were run against the saved reference files
(`scripts/bnd_fault.sh`, then restored):

| Edit | Case | `out_grd.ww3` |
|---|---|---|
| `SUMQP` product `f*f*d*s` to `f*(f*d)*s` | `PTM1` | differs |
| `x**5` chain `x2*x3` to `x2*x2` | `PTM1` | differs |
| skip the 5-pass watershed-line fill | `PTM1`, `PTM2`, `ww3_tp2.15` | differs |
| skip the `PTMETH 2` wind-part relabel | `PTM2` | differs |
| `FCDIR` above the highest frequency `SIG(NK+1)` to `SIG(NK)` | `PTM1` | differs |
| `TPIINV**3` as two roundings instead of one | `PTM1` | identical, not caught ⚠ |
| `<=` to `<` in the fill's nearest-neighbour test | `PTM1`, `PTM2` | identical, not caught ⚠ |

The cube needs checking in its own right: the two roundings give the same bits for `TPIINV` ⚠ (the same remark
`constants.rs` makes for `TPI**3`). A tie in the fill test never happened.

## Traps

- gfortran folds `TPIINV**3` with one rounding, so `cube_once` rounds the exact cube once. `TPIINV**2` is one multiply.
- `SIGP**(-5)` expands through gfortran's power tree to `1/(x2*(x*x2))`, not `x4*x`.
- `DO IK=NK-1,2,-1 ... EXIT` leaves `IK` at 1 when it runs out; `RD<0` then sets `IK` to 0 and reads `SIG(0)`.
- `PMAP` is not set for a partition skipped by `HS<HSPMIN` and `PTMETH` 4 or 5, and a partition `IP>0` found while
  `NPO` is 0 is dropped with `NPO` left at 0, as written.
- `NINT(1.+Z*FACT)` is round-half-away; `MOD(630.-ATAN2*RADE,360.)` is `fmodf`, exact; `COS` comes from `sincosf`
  of `ww3-libm`, whose bits equal glibc's `sincosf`, and `cosf` alone is taken to give the same ⚠ (every partition
  case above depends on it and none differs).

## Not reached ⚠

`FLCOMB` true (merging wind-sea partitions, `w3partmd.F90:455-488`), the `STOP` on a NaN depth in `W3CPRT`, an `XP`
array too small (`PTMEAN` label 2000, message translated), a partition with no peak (`IFPMAX` 0: the Fortran reads
`SUMF(-1,IP)`; Rust stops with status 64), `W3IOSF` (type 6 output), the second field output (`FLOGR2`), and group 4
fields 18 to 23 (`SEA8HS` family, `ids` panics). `ww3_outp` `ITYPE` 4 and `ww3_ounp` still call `W3PART` in the Fortran
and stay refused in Rust (`WW3_OUTP.md`): `W3BULL` and the table printing are not translated. Multi-process runs gather
`VA` to rank 0 before the output, where `W3CPRT` runs on all points; WW3 runs it per rank ⚠ (not run under MPI).

## Timing

None: laptop in Docker.
