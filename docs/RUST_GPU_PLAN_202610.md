# chaceon: WAVEWATCH III in pure Rust on the GPU

2026-10-30. The plan the owner asked for: a Rust port of WW3 in big batches, on rust-gpu and
Rust-CUDA, with no Kokkos, no CI and no Rust tests, compared with metocean/WW3 7.14
(`7.14.mos-92e2cf6`, `92e2cf66`, the `WW3/` submodule). Decisions are in [`ADRs/`](ADRs/README.md);
the rules are in [`AGENTS.md`](../AGENTS.md).

## What "full GPU" means here

Everything inside the time loop runs on the device: source terms, integration, propagation and the
loop's bookkeeping. The host reads inputs, starts the device and writes files. Preprocessors such
as `ww3_grid` are Rust on the CPU, since a GPU buys nothing there. The first target is one process
on one GPU; several GPUs come in Batch 8.

## Reference and verdicts

Every batch ends with WW3's regtests run twice, on the same inputs: the Fortran reference and
chaceon. `test.comp` and `cmp` judge the output files. Three verdicts come from one kernel crate
([ADR-0005](ADRs/ADR-0005-three-verdicts.md)):

| Verdict | Compiled by | Runs on | Pass means | Status today |
|---|---|---|---|---|
| `host` | rustc | Apple Silicon CPU, in the reference's Linux image | Byte-identical to the reference | Exists to prove the translation; never the product |
| `cuda` | Rust-CUDA | H100, RTX 4090 | Byte-identical is the target; tolerance otherwise, every difference recorded | The product |
| `spirv` | rust-gpu | Vulkan | Within tolerance only: the specification rules out identical bits ([details](FLOAT_REPRESENTATION_202610.md#device-float-behaviour)) | The portable backend |

`host` runs first because it separates a translation defect from a device defect. A byte difference
on `host` is a bug in the port.

## Batches

Lines are the Fortran files' lines, comments included (`just port-status`, from
[`rust/port_files.tsv`](../rust/port_files.tsv), which gives every one of the 135 `.F90` files a
batch). The `.f`, `.f90` and `.c` files, `SCRIP/` and `PDLIB/` are not counted in this table; Batch 8 takes them, and the README's Fortran target includes them.
The first five batches give a working forecast for the target switch,
[`switch_chaceon_shrd`](../switches/switch_chaceon_shrd), and the last three cover the rest.

| Batch | Result | Files | Lines | Exit |
|---|---|---|---|---|
| 0 | Both compilers run a kernel; the reference runs | 0 | 0 | The bit audit below |
| 1 | State and reading WW3's files | 15 | 23 113 | `mod_def`, wind and restart read back byte-exact |
| 2 | Source terms and `W3SRCE` on the device | 10 | 9 019 | `ww3_ts1` ST4 (`matrix.base:907`) |
| 3 | Propagation and the whole time loop | 9 | 20 350 | `ww3_tp2.4`, then `ww3_tp2.15` |
| 4 | Outputs and post-processing | 19 | 41 945 | `ww3_tnc1`; NetCDF compared |
| 5 | Preprocessors: no Fortran left in a run | 17 | 35 002 | `ww3_tp2.15` from namelists, Rust only |
| 6 | Speed: resident state, tuning | 0 | 0 | Timings on the H100 |
| 7 | Remaining physics and MetOcean's switch | 38 | 51 579 | Each switch's `matrix.base` entries |
| 8 | Other grids, `ww3_multi`, several GPUs | 20 | 58 760 | The rest of the matrix |
| X | Not planned: ESMF, OASIS, GRIB, `ctest` | 7 | 12 351 | The owner decides |

### Batch 0: prove the ground

Nothing else is worth starting until these hold, because the two compilers are the plan's largest
risk: both projects call themselves early-stage (v, their READMEs, 2026-10-30).

**Done, 2026-10-31:** both compilers build real kernels (`W3SNL1`, `W3SDS4`), `ww3-libm` covers what
they call, and capture and replay work (ADR-0007, logs of [10-30](log/2026-10-30-snl1-parity.md) and
[10-31](log/2026-10-31-sds4-parity.md)). **Open:** items 2 and 3 beyond `ww3_ts1`, the device audit
of item 6, `ww3cmp`.

1. Build the reference with `just ref-build`: native linux/arm64 on Apple Silicon, contraction off.
2. Run `ww3_ts1` ST4, `ww3_tp2.4` and `ww3_tp2.15` on it, twice, to learn which files WW3 itself
   does not reproduce. Keep the outputs by checksum.
3. Profile the reference at this pin. ww3-gpu's profiles are NOAA's pin, and MetOcean's ST4 differs.
4. Add `rust/gpu-cuda/` and `rust/gpu-spirv/`, each a workspace with its own nightly, each building
   `ww3-kernels`. Rust-CUDA uses its LLVM 7 backend, because LLVM 21 reaches sm_100 and later only.
5. Write `ww3-libm`: `no_std` copies of glibc's `expf`, `logf`, `powf`, `tanhf`, `sincosf` and the
   `f64` functions `W3SDB1` needs, translated from ww3-gpu's `libm_device.hpp`, whose device bits
   were never run there either ⚠.
6. **The bit audit.** One small kernel on each backend, supervised by the owner: `+ - * /`, `sqrt`,
   `ww3-libm`, subnormals, NaN, `f64`, FMA detection, over a fixed input set, compared with the host.
7. Settle in writing what the audit shows, and fix the tolerances, in Rust, before any model output
   exists to tempt them. `rust/tools/ww3cmp` replaces ww3-gpu's C++ `nccmp-tol`.

Exit: a table of the bits each backend reproduces. **Stop point:** if Rust-CUDA cannot compile a
real kernel on the H100, the owner chooses before Batch 1 between fixing it upstream and moving to
NVIDIA's cuda-oxide, which ADR-0003 keeps as the fallback.

### Batch 1: state and files

**2026-10-31:** `mod_def.ww3` is read for this switch, and `just ts1-pair` gives the first `host` verdict
on `out_pnt.ww3` (`rust/port_report/WW3_SHEL_TS1.md`). `W3WAVE` is only the part `ww3_ts1` reaches. Next
come more than one sea point, real state structs, restart and `out_grd.ww3`.

`constants` (done), `w3gdatmd`, `w3adatmd`, `w3wdatmd`, `w3odatmd`, `w3idatmd`, `w3dispmd`,
`w3servmd`, `w3timemd`, `w3arrymd`, and the readers `w3iogrmd`, `w3iorsmd`.

- WW3's global pointers (`W3SETG`, `W3SETA`) become one owned struct per grid. Arrays become
  flat `#[repr(C)]` buffers with Fortran's bounds carried explicitly: `SIG(0:NK+1)` is not `SIG[0]`.
- `ww3-io` reads and writes unformatted Fortran records.
- **The shortcut that makes the batches possible:** Batches 1 to 4 read `mod_def.ww3`, `wind.ww3`
  and `restart.ww3` written by the Fortran. A model runs from Batch 3 with no preprocessor ported.
  chaceon still contains no Fortran; it only reads these files.

### Batch 2: source terms on the device

In the order the profile sets, pending Batch 0's new one: `W3SNL1`, `W3SDS4`, `W3SIN4`, `W3SPR4`, `CALC_USTAR`,
`W3SRCE`, `W3SDB1`, `W3SBT1`, `W3SLN1`: all `routine`, with `INSIN4` and the `TABU_` tables. `W3SBT4`
remains, for MetOcean's switch.

- Source: the Fortran at the pin, never ww3-gpu's C++, which translates NOAA's ST4 and misses
  MetOcean's `USTAR` floors and `ISNAN` guards (`w3src4md.F90:680`, `:780`, `:835`, `:1683`;
  `w3srcemd.F90:1048`, `:1071`, `:1178`, `:1891`) (v, ww3-gpu's log 2026-10-29). The C++ is useful
  for one thing: its traps.
- The integrator has a different time step per sea point. On the device that is every point
  looping to its own end, with a batch mask; points never wait for one another in the arithmetic.
- A kernel does not allocate: scratch is a preallocated per-point buffer.

### Batch 3: the time loop

**2026-10-31:** 18 runs pass `just tp-all` on the host with `out_grd.ww3`, `out_pnt.ww3` and a restart
byte-identical, `ww3_tp2.15`'s grid and physics among them under homogeneous wind. With `W3SRCE`, `W3XYP3`
and `W3KTP3` in compiled SPIR-V on lavapipe and `VA` resident, they pass too (`rust/port_report/W3PRO3.md`).
Open: water levels, ice, boundary input, `wind.ww3`, track output, and any real device.

`w3wavemd`, `w3initmd`, `w3updtmd`, `w3pro3md`, `w3uqckmd`, `w3parall`, `w3cspcmd`, `w3flx2md`
(`ww3_tp2.4` needs `FLX2`). `ww3_tp2.4` runs propagation alone (`ST0`, `NL0`); `ww3_tp2.15` has the
target's physics, `PR3 UQ FLX0 LN1 ST4 NL1 BT1 DB1`, so it is the first full model (v, its `switch`).
This is the first run with the whole spectrum resident: a Rust `ww3_shel` that writes `out_grd.ww3`
and `restart.ww3` for `cmp`.

**2026-11-02:** water levels, `IC0` ice, file winds and boundary input from `nest.ww3` run on the host byte-identical, with
`ww3_bound` as the step that writes the file ([log](log/2026-11-02-level-ice-boundary.md), `rust/port_report/W3ULEV.md`,
`W3IOBC.md`, `WW3_BOUND.md`). `ww3_shel` refuses levels, ice and a `nest.ww3` on a device ⚠. Open in this batch: track output.

**2026-11-03:** track output runs on the host byte-identical, with `ww3_trck` and `ww3_trnc`
(`rust/port_report/W3IOTR.md`). Batch 3's open items are now only a real device.

### Runtime parallelism

The owner, 2026-10-31: "chaceon must also allow MPI/OpenMP runtime option." WW3 picks this at compile time, with
`SHRD`, `OMPG` and `DIST`. Here one binary picks it at run time: threads with `--threads N`, and ranks under
`mpirun`. Exit: `out_grd.ww3`, `out_pnt.ww3` and the restart byte-identical to the serial run and the reference,
for several thread and rank counts.

Status: threads done (`ww3_shel <case> --threads N`, verified byte-identical for N of 1, 2, 4 and 8 on eight
cases, see `rust/port_report/WW3_SHEL_TS1.md`). Ranks done on the host CPU: `mpirun -n R ww3_shel <case> [--threads N]`
built with the Cargo feature `mpi` (`ww3-host`, `ww3_shel`), the sea points dealt round-robin and the spectral
components by `IAPPRO` as WW3's `DIST` does, rank 0 writing the files; byte-identical to the reference on eight cases
for R of 1 to 4 and R=2 with two threads (`scripts/mpi_pair.sh`, table in `rust/port_report/WW3_SHEL_TS1.md`).
Not built: a rank layout with a device (`spirv_shel`, `gpu-*` take no ranks), MPI across hosts ⚠, and any speed claim.

### Batch 4: outputs

`ww3_shel` in full, `w3iogomd`, `w3iopomd`, `w3iobcmd`, `w3partmd`, `ww3_ounf`, `ww3_ounp`,
`ww3_outf`, `ww3_outp`, the namelist readers. NetCDF goes through a binding to the C library, unless
Batch 4 finds a pure Rust writer that reproduces the files ⚠. Field computation (`W3OUTG`) runs on
the device, reading only what an output needs.

Status on 2026-11-01: `ww3_outp` and `ww3_outf` have Rust ports, byte-identical (log, exit status and every file) on
21 and 19 inputs of WW3's regtests and on 22 and 27 inputs written here, `rust/port_report/WW3_OUTP.md` and
`WW3_OUTF.md` ([log](log/2026-11-01-outf-outp-parity.md)). Refused with a message: `ww3_outp` `ITYPE` 3 and 4 and the
nondimensional tables, both on `GTYPE` 3 grids, `ww3_outf` group 10. `ww3_ounf`, `ww3_ounp`, `w3iobcmd`, `w3partmd`
and NetCDF are not started.
Status on 2026-11-01: `ww3_ounf` has a Rust port, `rust/tools/ww3_ounf`, through the system `libnetcdf` (19 declarations, the
Cargo feature `netcdf`, [`VERSIONS.md`](../VERSIONS.md)). Its NetCDF files are byte-identical to the reference's on 36 cases
and 40 files, and on namelist variants and every field the grids can hold, classic and HDF5 formats, one file or one per
field, `TIMESPLIT`, partitions ([report](../rust/port_report/WW3_OUNF.md), [log](log/2026-11-01-ounf-parity.md)). `cmp` is
the verdict because the reference writes the same bytes twice. A pure Rust writer was not tried ⚠. Still open in this
batch: `ww3_ounp`, `ww3_outf`, `ww3_outp`, `w3iobcmd`, `w3partmd`, and `ww3_ounf`'s unstructured, SMC and rotated-pole
branches, forecast variables, `ounfmeta.inp` and the old `ww3_ounf.inp`.

Status on 2026-11-01: `ww3_ounp` has a Rust port for `POINT%TYPE` 1 and 2, byte-identical with its NetCDF, logs and
exit status ([report](../rust/port_report/WW3_OUNP.md), [log](log/2026-11-01-ounp-parity.md)). `w3partmd` and the
partition fields of `out_grd.ww3` are ported, and so is the WRITE side of `w3iobcmd`
([log](log/2026-11-01-boundary-partition-parity.md)). Still open: `W3IOBC` READ, boundary input in a run, which needs
a change in `W3XYP3`'s sub-step loop, the `SEA8HS` fields, and `ww3_ounp` types that need the source terms.

### Batch 5: preprocessors

`ww3_grid`, `ww3_strt`, `ww3_prnc`, `ww3_prep`, `ww3_bound`, `ww3_bounc`, `ww3_uprstr`, `w3gridmd`,
`w3gsrumd`. Exit: a regtest from namelists to NetCDF, Rust only, whose `mod_def.ww3` equals the
Fortran's byte for byte. `ww3_grid` also writes `ST4TABUHF2.bin`, so the tables must match too.

Status on 2026-10-31: `ww3_strt` and `ww3_grid` have Rust ports, byte-identical on the regtests of
`rust/port_report/WW3_STRT.md` and `WW3_GRID.md`. Rust's `ww3_grid` does not write `ST4TABUHF2.bin`, the cache
that only `ww3_grid` reads back, and stops on unstructured and SMC grids.
`just rust-only` runs the chain from namelists on 17 cases, byte-identical on every file up to `restart.ww3`
([log](log/2026-10-31-rust-only-chain.md)). The exit above is not met: there is no NetCDF step yet.

Status on 2026-11-01: `ww3_prep` has a Rust port for the input type `AI` (a field already on the model grid) and
every field id, with free-format and formatted input. 14 forcing files of the six regtests that have a `ww3_prep`
input and run on `switch_chaceon_shrd` are byte-identical, and so are 12 more files from inputs built for the other
field ids and layouts; `LL`, `F1`, `F2`, `DAT`, binary input and a reversing format are refused
([log](log/2026-11-01-prep-parity.md), `rust/port_report/WW3_PREP.md`). `just rust-only` no longer copies
`current.ww3` from the Fortran: `ww3_tp1.6` is a run with no Fortran program. The `mww3_test_*` inputs need
`ww3_multi` and are not run.

Status on 2026-11-01: `ww3_prep` takes `LL`, `F1`, `F2`, `DAT`, binary input and format reversal, and `ww3_strt`
takes start type 2 ([log](log/2026-11-01-prep-interp-parity.md)). `ww3_prnc` takes `AI` and `LL`, from `ww3_prnc.nml`
or `ww3_prnc.inp`; `AT`, `F1`, `F2` and `DAT` are refused, and its inputs are synthetic NetCDF built from CDL under
`regtests/prnc_inputs/` because the regtests' files are not in the repository ⚠
([log](log/2026-11-01-prnc-parity.md)). `ww3_bounc` is not ported: the reference's `ww3_bounc` stops with a segmentation
fault on the only input available here, `ww3_tp1.11` (`rust/port_report/WW3_BOUNC.md`). `ww3_bound` is ported, 2026-11-02.

### Batch 6: speed

No new physics. Whole-spectrum copies per step become zero, kernels are grouped to cut dispatch
cost, and the order of work is tuned while the bits stay fixed. Speed needs its parity first, so
timings start here, on the H100, as a machine, a command and a median of three runs. UFSC's H100
access is still open (ww3-gpu #143), so this batch waits on it. The RTX 4090 serves until then.

### Batches 7 and 8: the rest

Batch 7: `ST1`, `ST2`, `ST3`, `ST6`, `NL2` to `NL5`, ice, the fluxes, tides, the switch MetOcean
ships (`BT4`, `WCOR`, `RWND`, `SEED`, `MPI`).

**2026-11-03:** `W3SBT4` (`BT4`) is a kernel at status `routine`: 33 824 captured calls bit-identical on the host and
in SPIR-V on lavapipe, against a reference built with `switches/switch_chaceon_bt4` ([log](log/2026-11-03-sbt4-parity.md),
`rust/port_report/W3SBT4.md`). It is called from the `W3SRCE` kernel when the build is `BT4`: 15 127 captured `W3SRCE`
calls bit-identical on the host and in SPIR-V ([log](log/2026-11-03-srce-bt4.md)). Next for `BT4`: its state per sea
point and its `mod_def.ww3` fields, so that a model run uses it. Batch 8 ([how to run them, and what porting takes](UNSTRUCTURED_GRIDS_202610.md)): PDLIB and the unstructured solvers, the SMC
grid, `ww3_multi`, domain decomposition over several GPUs. MPI needs an ADR of its own: a binding
to C MPI, or GPU-aware transport.

## Risks

| Risk | Effect | Answer |
|---|---|---|
| Rust-CUDA or rust-gpu breaks on a real kernel | Port stops | Batch 0's stop point; kernels stay plain `no_std` Rust, so a compiler can be replaced |
| The two pin different nightlies, `2026-04-02` and `2026-08-15` (v) | A language feature works on only one | Separate workspaces; kernels use only what both accept |
| gfortran's `-O3` choices (power chains, ST4's libmvec loops, per ww3-gpu's `st4_libmvec.hpp:10-13`) | `host` is off by a ULP | Read the optimised code; repeat at this pin ⚠ |
| Two executables, unlike ww3-gpu's one | Build differences count as physics | One named machine; the reference runs twice to measure itself |
| glibc's math is platform-specific ⚠ | macOS libm is not the reference's | `host` runs inside the reference's linux/arm64 image, against its glibc (ADR-0005) |
| gfortran on arm64 fuses `a*b+c` by default (v, probe) | Default reference differs from Rust's arithmetic | The reference is built with `-ffp-contract=off` (ADR-0005) |
| An arm64 reference is not LabECO's operational x86_64 build | Bits proved here are this reference's | Contraction off keeps the arithmetic IEEE; libm results may still differ ⚠ |
| `Float64` is missing on Apple GPUs ⚠ | No `DB1` on the Mac, and no `W3SNL1` either: `ww3-libm`'s `expf` needs `f64` and `i64` | Stop with a message; no demotion |
| SPIR-V is bit-identical on lavapipe (v, [log](log/2026-10-31-spirv-bit-parity.md)), and no device has run ⚠ | `f64` division, subnormals and `OpFConvert`'s rounding are still free on a device | Measure on a real device before fixing tolerances |
| 52 storage buffers in `W3SRCE` ⚠ | A device with a low limit cannot bind them | Read `maxPerStageDescriptorStorageBuffers` on the device; pack tables if needed |
| Soft `fma64` and `sqrtf` are slow on a device ⚠ | Wall time | Replace them on PTX in Batch 6, once `fma.rn` and `sqrt.rn` are shown to give the same bits |
| Rust-CUDA's LLVM 7 path is its legacy one (v) | A language or `core` change may stop building | Pinned nightly; kept plain; cuda-oxide is the fallback |
| Licence | A translation of LGPL code under MPL-2.0 | The owner's decision, ADR-0006; to revisit before distribution |

## Open for the owner

1. **The switch**: `switch_chaceon_shrd` is ww3-gpu's guess at ondaseventos' build ⚠.
2. **GPU supervision**: AGENTS.md keeps ww3-gpu's rule that device runs need the owner. If runs on
   the Mac may go unsupervised, a new ADR should say so.
