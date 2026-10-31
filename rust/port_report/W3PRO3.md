# W3PRO3 and W3UQCK: propagation

`rust/crates/ww3-kernels/src/pro3.rs` translates `W3XYP3`, `W3KTP3` and `W3QCK1`, `W3QCK2`, `W3QCK3` for the
`PR3 UQ` switches. `rust/crates/ww3-host/src/{grid,iors,shel}.rs` add the maps, gradients, restart file,
`W3GATH`, `W3SCAT`, `W3IOPP` and many sea points.

**Status: `host`** (ADR-0005) on the cases below. The kernels run on the CPU in this driver. Their SPIR-V and
PTX build, and neither has run anywhere ⚠.

## Result

The reference and Rust run the regtest's own namelists under `switch_chaceon_shrd`, with one restart added at
the last time step. That restart holds every sea point's spectrum, so it is the whole-state comparison.

| Regtest | Sea points × `NSPEC` | Active | `out_pnt.ww3` bytes | restart bytes | Verdict |
|---|---|---|---|---|---|
| `ww3_tp1.1` | 360 × 12 | `FLCX`, closed | 62 902 | 44 100 | identical |
| `ww3_tp1.2` | 121 × 12 | `FLCY`, spherical | none | 23 772 | identical |
| `ww3_tp1.3` | 42 × 12 | `FLCX`, shoaling | none | 5 544 | identical |
| `ww3_tp1.4` | 11 × 72 | `FLCX FLCTH` | 193 804 | 6 624 | identical |
| `ww3_tp1.5` | 11 × 72 | `FLCY FLCTH` | 193 804 | 9 504 | identical |
| `ww3_tp1.7` | 25 × 720 | `FLCX FLCY FLCTH FLSOU` | 5 329 030 | 106 560 | identical |
| `ww3_tp1.8` | 49 × 2700 | `FLCX FLCTH FLSOU` | 76 312 | 658 800 | identical |
| `ww3_tp1.9` | 270 × 6300 | `FLCX FLSOU` | 404 789 | 7 106 400 | identical |
| `ww3_tp1.10` | 49 × 2880 | `FLCX FLSOU` | 3 825 607 | 702 720 | identical |
| `ww3_tp2.1` | 973 × 72 | `FLCX FLCY`, land | none | 317 664 | identical |
| `ww3_tp2.3` | 1656 × 360 | `FLCX FLCY`, averaging | none | 2 486 880 | identical |
| `ww3_tp2.4` | 14548 × 360 | `FLCX FLCY FLCTH`, land | 47 392 | 21 460 320 | identical |

20 files, 43 062 528 bytes, 0 differ. `ww3_ts1` still passes through the new driver.

```bash
just tp-all
just tp-pair ww3_tp2.4
```

`FLCK` is set in some of these grids, and `W3INIT` clears it, since they have no currents. `FLSOU` here runs
with no wind.

## Why to believe it

`ww3_tp2.4`'s final restart has 1 878 425 nonzero values of 5 237 280, and 1 879 201 changed since the start.
On each case a fault changes bytes, then restoring it gives 0 (restart bytes differ):

| Fault | Case | Bytes |
|---|---|---|
| `QBN` cap 1 to 0.9999999 | `ww3_tp1.1`, `ww3_tp2.4` | 1 176, 2 762 320 |
| `DQNZ` floor 1e-15 to 1e-14 | `ww3_tp1.2`, `ww3_tp2.4` | 130, 6 060 641 |
| 1e-7 in the `QB` blend | `ww3_tp1.1` | 1 288 |
| 1e-7 on the averaging weights | `ww3_tp2.3` | 510 839 |
| 1e-7 on `W3KTP3`'s depth refraction | `ww3_tp1.4`, `ww3_tp2.4` | 77, 14 164 |
| 1e-7 on `W3KTP3`'s great circle term | `ww3_tp2.4` | 1 070 959 |

Three faults changed 0 bytes:

- The `QCN` clamp -0.1 to -0.1000001: a clamped `QCN` gets `CFAC=1`, which discards it.
- The `JN` choice in `W3QCK3`: every transparency is 1 here, so **obstructions are not tested** ⚠.
- The `CFL` floor 1e-10 to 1e-9: no flux has `|CFL|` that small.

The first pass failed on `ww3_tp1.2`, which found a real bug: `CLATS` is `(0:NSEA)` and was read from 1.
`ww3_tp1.1` passed with that bug, on the equator.

## How far it reaches

| Not reached ⚠ | Why |
|---|---|
| Currents, `W3QCK2`, `W3KTP3`'s `FLCK` branch | `W3UCUR`, `ww3_tp1.6`, `ww3_tp2.2` |
| Obstructions, `W3UTRN` with `TRFLAG` 1 or 2 | no such case yet |
| Input boundaries | runs here have no `nest.ww3`, so `W3IOBC` clears `FLBPI` |
| `out_grd.ww3` | needs `W3OUTG` and `W3IOGO`, so no case is a complete ADR-0005 verdict |
| A full restart read | written and read code exist, the read has not run |
| Wind fields, `ww3_tp2.15`'s physics | homogeneous or no wind so far |
| `ww3_grid`, `ww3_strt` | their files still come from the Fortran |
| `ww3_tp1.6`, `ww3_tp1.11`, `ww3_tp2.2`, and every other regtest | not run through this driver |

## On a device

`xyp3` and `ktp3` are entry points in `rust/gpu-spirv/shader` and `rust/gpu-cuda/kernels`. Each handles one
component or sea point per launch, so they exercise no parallelism yet. The Fortran's components are
independent, so one thread per component needs offsets in `Qck` and the work arrays. `just spirv` and `just ptx`
build both; `ptxas --fmad=false` assembles them. Nothing was dispatched, on lavapipe or on a device ⚠.

## Timing

None, as `WW3_SHEL_TS1.md` says.
