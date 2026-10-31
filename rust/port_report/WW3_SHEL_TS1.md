# ww3_ts1 in Rust: the first whole-file verdict

`rust/tools/ww3_shel` runs a case in Rust from a calm start and writes `out_pnt.ww3`. Code:
`rust/crates/ww3-host/src/{moddef,wtime,shel}.rs`, on the kernels of `ww3-kernels` built for the CPU.

**Status: `host`** (ADR-0005), for `out_pnt.ww3` on `ww3_ts1`. Nothing here has run on a GPU ⚠.

## Result

The reference and Rust run the same case and `cmp` judges the file:

| Variant, depth | `out_pnt.ww3` |
|---|---|
| default 2500, 20, 8, 6 m | identical |
| calm, light 2500 m | identical |
| storm 6, 2500 m | identical |
| strain 20 m, circle 2500 m | identical |
| t700 2500, 8 m, romero2 20 m | identical |
| filipot 2500, 20 m | identical |
| noshelter, flatswell, sharp 2500 m | identical |

18 cases, 7 652 218 bytes each, 137 739 924 bytes in all, 2 161 outputs per file. Every byte of 36 hours of
spectra and scalars, with each step's input coming from Rust's own previous step.

```bash
just ts1-pair storm 6
```

## Why to believe it

The match came on the first run, so it was checked:

- The file was deleted, regenerated in about 2 s, and its SHA-256 equals the reference's
  (`d6e64c201bda591d475b...`).
- One reassociated multiplication in `W3SNL1` changes 1 171 911 bytes.
- A factor of 1.0000001 on the wind interpolation changes 1 448 487 bytes.
- `just moddef-cmp` compares what the reader parses with the reference's memory: 24 tables and the whole
  `SrceParams` struct, bit for bit.

## How far it reaches

| Reached | Not reached ⚠ |
|---|---|
| `W3SRCE` and all its kernels, chained | Propagation here: see [`W3PRO3.md`](W3PRO3.md) |
| `mod_def.ww3`, read | `mod_def.ww3` written: it still comes from `ww3_grid` |
| `W3UWND`, `W3FLDH` winds, homogeneous | Wind fields, currents, water levels, ice |
| `TICK21`, `DSEC21`, standard calendar | Other calendars |
| `WAVNU1`, `DISTAB` | |
| `W3IOPE`, `W3IOPO`, write, one point | `out_grd.ww3`, which the reference also writes: not produced yet |
| `W3WAVE`, the time and output logic | More than one sea point here: see `W3PRO3.md` |

The binary refuses what it does not implement, with a message. It reads only the namelist keys these cases use.

`test.comp` was not run, since `cmp` on the only file produced says more. A full ADR-0005 verdict
for `ww3_ts1` also needs `out_grd.ww3` ⚠.

Bits hold on the arm64 reference with contraction off (ADR-0005), for `NK=36`, `NTH=24`.

`ww3_shel <case dir> [--threads N]` (default 1) shares the per-sea-point loops of `W3SRCE` and `W3KTP3` and the
per-component loop of `W3XYP3` over N threads, each with its own scratch; `W3XYP3` gathers its components from
`va` in parallel, and each sea point scatters its own values back, so no thread writes `va` by component. The
kernels and their arithmetic are unchanged, and `--threads 1` takes the serial path. (v) For `ww3_tp1.1`, `ww3_tp1.4`,
`ww3_tp2.3`, `ww3_tp1.8`, `ww3_tp1.6`, `ww3_tp2.15`, `default_2500m` and `storm_6m`, run with
`scripts/rust_in_ref.sh ww3_shel /work/pair/<case> --threads N` for N of 1, 2, 4 and 8, `cmp` finds `out_grd.ww3`,
`out_pnt.ww3` and `restart001.ww3` (where a `.reference` exists) identical to the reference, with the outputs deleted
before each run; a deliberate fault in the threaded scatter made `ww3_tp1.1` differ at N of 2 and not at 1. ⚠ Only
8 threads on an 8-core Docker laptop were tried, and no case is large enough to show a speed-up.

## Ranks: `mpirun -n R ww3_shel`

The owner, 2026-10-31: "chaceon must also allow MPI/OpenMP runtime option". WW3 fixes `SHRD`, `OMPG` or `DIST` when
it is compiled; here one binary takes ranks at run time, and `--threads N` still applies inside each rank.
Build with `--features mpi` (`ww3-host`, `ww3_shel`; rsmpi `mpi` 0.8.0, Open MPI 4.1.4, `VERSIONS.md`); the default
build has no MPI library and `spirv_shel` is untouched. A process not started by a launcher (no `OMPI_COMM_WORLD_SIZE`,
`PMI_SIZE` or `PMIX_RANK`) never calls `MPI_Init` and takes the serial code, as does `R=1`. A rank layout with a device
is refused.

What follows WW3's `DIST` (`W3_DIST W3_MPI`), read at the pin (v):

- Sea points go round-robin, `ISEA = IAPROC + (JSEA-1)*NAPROC` (`w3parall.F90:1256`); `W3KTP3` and `W3SRCE` run on a
  rank's own points (`w3wavemd.F90`, the `INIT_GET_ISEA` loops).
- Each spectral component goes to one rank by `IAPPRO` (`w3initmd.F90:853`, translated as written, integer logic
  only). `W3GATH` and `W3SCAT` become one `MPI_Alltoallv` each: the owners of the points send a component's values
  to the owner of the component, which runs `W3XYP3` on the whole field, and the result goes back
  (`w3wavemd.F90:1859`). The scatter writes a point only where `MAPSTA` is not 0, as the serial path does.
- Rank 0 writes `out_grd.ww3`, `out_pnt.ww3` and the restart. Before an output due now, the other ranks send rank 0
  their points' `VA`, `UST`, `USTDIR`, `DTDYN`, `FCUT`, the `W3SRCE` results and `WHITECAP` (`gather_out`).

Where it departs from WW3: every rank holds the whole state (WW3's ranks hold their `NSEAL` points), so the setup, the
forcing, the maps and `W3CFLXY` are computed on every rank and need no reduction. No global minimum or maximum
crosses ranks, so no reduction order exists to differ. The kernels and the order of every per-point operation are
unchanged, and `IAPPRO` decides only who computes a component. The restart's `VA = MAX(0,VA)` is applied to a rank's own
points on the other ranks.

(v) `bash scripts/mpi_pair.sh` (eight cases, outputs deleted before every run, `cmp` of every `.reference`): for
`ww3_tp1.1`, `ww3_tp1.4`, `ww3_tp1.6`, `ww3_tp1.8`, `ww3_tp2.3`, `ww3_tp2.15`, `storm_6m` and `default_2500m`, R of 1, 2, 3
and 4, and R=2 with `--threads 2`, each three times: 120 runs, 120 byte-identical for `out_grd.ww3`, `out_pnt.ww3`
and `restart001.ww3` where a reference exists. `storm_6m` has one sea point, so R of 2 to 4 leaves ranks without
points and passes. The comparison is with the Fortran reference, not with a Fortran `DIST` build ⚠ (none was built).

| Case | R=1 | R=2 | R=3 | R=4 | R=2, T=2 |
|---|---|---|---|---|---|
| ww3_tp1.1, tp1.4, tp1.6, tp1.8, tp2.3, tp2.15 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| storm_6m, default_2500m | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |

(v) Fault: skipping the scatter of the even components on rank 1 made `ww3_tp1.1` differ at R=2, R=3 and R=2,T=2
(16 345, 16 365 and 16 345 bytes of `out_grd.ww3`) and not at R=1; the source was restored. A first fault, skipping
component 0 only, changed nothing on `ww3_tp1.1`, so a fault must touch a component the case moves. (v) The default
build and the `mpi` build run without `mpirun` give `storm_6m` identical (`scripts/rust_in_ref.sh ww3_shel
/work/pair/storm_6m`).

⚠ Not measured: ranks across hosts, other MPI libraries, `R` above 4, any regtest beyond the eight, and speed. On
`default_2500m` the wall time of a run is 1.6 s at R of 1, 2 and 4 in Docker on a laptop, dominated by starting the container; this
is an observation, not a timing. Ranks run `--oversubscribe` on a few cores.

## Timing

None: this is a laptop in Docker, and a timing needs a named machine and three runs.
