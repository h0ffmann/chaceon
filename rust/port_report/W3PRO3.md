# W3PRO3 and W3UQCK: propagation

`rust/crates/ww3-kernels/src/pro3.rs` translates `W3XYP3`, `W3KTP3` and `W3QCK1`, `W3QCK2`, `W3QCK3` for the
`PR3 UQ` switches. `rust/crates/ww3-host/src/{grid,iors,shel}.rs` add the maps, the grid metrics, gradients,
the restart file, forcing, `W3GATH`, `W3SCAT`, `W3IOPP` and the time loop for many sea points.

**Status: `spirv`** (ADR-0005) on lavapipe, CPU Vulkan, and `host`. Nothing has run on a GPU, and the PTX
has not executed ⚠.

## Result

The reference and Rust run the regtest's namelists under `switch_chaceon_shrd`, with one restart added at the
last time step, which holds every sea point's spectrum. `cmp` judges each file. CPU is `just tp-all`, SPIR-V
is `scripts/spirv_pair.sh`, which runs `W3XYP3` and `W3KTP3` as compiled shaders inside the time loop. The other
kernels, `W3SRCE` included, stay on the CPU in those runs.

| Regtest | Sea points × `NSPEC` | Active | Bytes compared | CPU | SPIR-V |
|---|---|---|---|---|---|
| `ww3_tp1.1` | 360 × 12 | `FLCX`, closed | 276 908 | identical | identical |
| `ww3_tp1.2` | 121 × 12 | `FLCY`, spherical | 62 774 | identical | identical |
| `ww3_tp1.3` | 42 × 12 | `FLCX`, shoaling | 115 606 | identical | identical |
| `ww3_tp1.4` | 11 × 72 | `FLCX FLCTH` | 265 998 | identical | identical |
| `ww3_tp1.5` | 11 × 72 | `FLCY FLCTH` | 271 426 | identical | identical |
| `ww3_tp1.6` | | `FLCX FLCK`, `current.ww3` | 1 242 187 | identical | identical |
| `ww3_tp1.7` | 25 × 720 | `FLCX FLCY FLCTH FLSOU` | 8 336 692 | identical | identical |
| `ww3_tp1.8` | 49 × 2700 | `FLCX FLCTH FLSOU` | 856 318 | identical | identical |
| `ww3_tp1.9` | 270 × 6300 | `FLCX FLSOU` | 7 527 143 | identical | identical |
| `ww3_tp1.10` | 49 × 2880 | `FLCX FLSOU` | 4 652 073 | identical | identical |
| `ww3_tp2.1` | 973 × 72 | `FLCX FLCY`, land | 1 542 790 | identical | identical |
| `ww3_tp2.2` | 17381 × 216 | `FLCX FLCY FLCTH`, wind, current | 20 412 156 | identical | identical |
| `ww3_tp2.3` | 1656 × 360 | `FLCX FLCY`, averaging | 3 292 282 | identical | identical |
| `ww3_tp2.4` | 14548 × 360 | `FLCX FLCY FLCTH`, land | 23 057 514 | identical | identical |
| `ww3_tp2.4` `curv` | | curvilinear | 4 485 226 | identical | identical |
| `ww3_tp2.15` | | see below | 2 492 258 | identical | identical |
| `ww3_tp2.15` two legs | | full restart read | 4 404 550 | identical | not run |

CPU: 50 files, 83 293 901 bytes, 0 differ. SPIR-V: 44 files, 78 889 351 bytes, 0 differ. All 18 `ww3_ts1`
variants pass `just ts1-pair` through this driver, on `out_grd.ww3` and `out_pnt.ww3`.

```bash
just tp-all
just spirv-pair ww3_tp2.4
```

### Two regtests are changed to run here

- **`ww3_tp2.15`:** its `wind.nc` is not in the regtest's input, so `scripts/tp215_wind.sed` gives it three
  homogeneous winds. Grid, obstructions, `ST4 NL1 BT1 DB1 LN1`, `W3UINI` and the field list are the regtest's.
- **`ww3_tp2.2`:** `scripts/tp22_no_track.sed` drops `DATE%TRACK`, since `W3IOTR` is not ported.

## Why to believe it

The first pass found real bugs, so a match here means something: `ww3_tp1.2` caught `CLATS` read one sea point
off, which `ww3_tp1.1` hides on the equator.

Every fault below changes bytes, and the rebuild without it gives 0 (restart bytes):

| Fault | Where | Case | Bytes |
|---|---|---|---|
| `QBN` cap 1 to 0.9999999 | CPU | `ww3_tp1.1`, `ww3_tp2.4` | 1 176, 2 762 320 |
| `QBN` cap 1 to 0.9999999 | **in the shader** | `ww3_tp1.1`, `ww3_tp2.3` | 1 176, 753 071 |
| `DQNZ` floor 1e-15 to 1e-14 | CPU | `ww3_tp1.2`, `ww3_tp2.4` | 130, 6 060 641 |
| 1e-7 in the `QB` blend | CPU | `ww3_tp1.1` | 1 288 |
| 1e-7 on the averaging weights | CPU | `ww3_tp2.3` | 510 839 |
| 1e-7 on `W3KTP3`'s depth refraction | CPU | `ww3_tp1.4`, `ww3_tp2.4` | 77, 14 164 |
| 1e-7 on `W3KTP3`'s depth refraction | **in the shader** | `ww3_tp1.4` | 77 |
| 1e-7 on `W3KTP3`'s great circle term | CPU | `ww3_tp2.4` | 1 070 959 |
| The `JN` choice of transparency | CPU | `ww3_tp2.15` | 906 019 |
| 1e-7 on `W3QCK2`'s CFL | CPU | `ww3_tp1.6` | 307 |
| 1e-7 on the current in `CXTOT` | CPU | `ww3_tp2.2`, `ww3_tp1.6` | 576 323, 300 |

These changed 0 bytes, and why:

- `QCN` clamp -0.1 to -0.1000001: a clamped `QCN` gets `CFAC=1`, which discards it.
- `CFL` floor 1e-10 to 1e-9: no flux has `|CFL|` that small.
- `CXC` × 1.5 on `ww3_tp2.2`: the group velocity sets `CGN`, so `NTLOC` does not move.
- 1e-7 on `W3KTP3`'s current term on `ww3_tp2.2`: a homogeneous current has no gradient. **Current refraction
  has no fault run that reaches it ⚠.**
- `W3QCK2` on `ww3_tp2.2`: that grid does not set `FLCK`.

## SPIR-V needs two things

Measured on restart bytes, compiled again each way:

| Module | `ww3_tp1.1` | `ww3_tp1.4` | `ww3_tp2.3` | `ww3_tp2.15` |
|---|---|---|---|---|
| rust-gpu's, plain `/` | 1 076 | 143 | not run | not run |
| `NoContraction`, plain `/` | 1 124 | 103 | 773 478 | 341 225 |
| No decoration, exact division | 1 173 | 102 | 755 547 | 311 982 |
| Both | **0** | **0** | **0** | **0** |

`rust/tools/spirv_exact` decorates the module, and `div()` in `pro3.rs` divides in `f64`. Why that division is
exact is in `docs/FLOAT_REPRESENTATION_202610.md`. `pos()` keeps `MAX(0.,X)` at `+0.`, which `FMax` leaves open.

## How far it reaches

| Not reached ⚠ | Why |
|---|---|
| A real GPU, `W3SRCE` in the loop on a device | CPU Vulkan only, and only propagation |
| A resident spectrum | the host uploads `VA` and reads it back for each dispatch |
| PTX execution | `xyp3` and `ktp3` build and assemble, and never ran |
| `wind.ww3` | `W3FLDG`'s path ran on `current.ww3` only |
| Water levels, ice, icebergs, boundary input | refused; these runs have no `nest.ww3`, so `W3IOBC` clears `FLBPI` |
| `TRFLAG` 1 and 2 | `ww3_tp2.15` has 4 |
| Track output, partitions, `MPI`, tripole, SMC, unstructured grids | not ported, and refused where they could be asked for |
| `ww3_grid`, `ww3_strt`, `ww3_prep` | their files still come from the Fortran |

The cell for a point exactly on a node assumes `W3GFCL` walks a bucket's cells in ascending order: verified on
`ww3_tp1.7`, not read from how buckets are filled ⚠. A point near a pole or a branch cut stops the run.

## Timing

None: this is a laptop in Docker, and lavapipe says nothing about a GPU.
