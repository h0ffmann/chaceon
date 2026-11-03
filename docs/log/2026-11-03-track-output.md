# 2026-11-03: track output, `ww3_trck` and `ww3_trnc`

`ww3_shel` writes `track_o.ww3` (`W3IOTR`), and `ww3_trck` and `ww3_trnc` are ported. All host. Nothing ran on a GPU. No timings.

## Result

| Command | Result |
|---|---|
| `just track-all` | 74 comparisons identical: 8 runs, `ww3_trck` on 8 tracks, `ww3_trnc` on 17 inputs |
| `just track-faults` | 15 faults, 15 caught |
| `just tp-all` | 51 identical; `ww3_tp2.2` runs with its track now |
| `just rust-only` | 96 files identical |
| `scripts/spirv_all.sh`, 18 cases | 51 identical, `track_o.ww3` of `ww3_tp2.2` on SPIR-V among them |
| `scripts/mpi_pair.sh ww3_tp2.2_many ww3_tp2.3` | 10 configurations, 3 repeats each, identical, `track_o.ww3` included |
| `just bound-all`, `just forcing-all`, `just bnd-all` | 214, 48 and 23 identical |
| `scripts/ts1_pair.sh`, 20 cases | 40 identical |

## A claim checked against the file

The report first said the curvilinear case reached ice, dry and disabled cells. Counting the states in its `track_o.ww3` gave
`LND`, `XCL`, `DRY` and `SEA` only: the tracks missed the northern rows where `forcing_gen.py` grows the ice. A track over
every third cell of those rows, hourly, reaches all six states: 113 `ICE`, 52 `DIS`, 44 `DRY`. Still identical.

## `ww3_trnc` and an unset calendar

`ww3_trnc` never reads `mod_def.ww3`, so `CALTYPE` in `W3TIMEMD` stays eight NUL bytes. `TRIM` keeps them, so the reference writes
them as the `calendar` attribute, omits `time:long_name`, and `TSUB` takes the branch with no leap days. Rust does the same.
`DSEC21` and `TICK21` take that branch too in the Fortran, and Rust's do not: a track across 29 February of a leap year is not
reproduced ⚠.

## A stall that was not the program

The last run stopped for about an hour and a half inside one container: OrbStack reported itself running while `docker ps` did
not return. It resumed by itself, and the suite then finished with every comparison identical. A silent monitor was again the
first sign; the next check of a long run looks at the clock of its last output.
