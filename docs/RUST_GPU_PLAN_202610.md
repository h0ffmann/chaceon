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
| `host` | rustc | x86_64 CPU | Byte-identical to the reference | Exists to prove the translation; never the product |
| `cuda` | Rust-CUDA | H100, RTX 4090 | Byte-identical is the target; tolerance otherwise, every difference recorded | The product |
| `spirv` | rust-gpu | Vulkan | Within tolerance only: the specification rules out identical bits ([details](FLOAT_REPRESENTATION_202610.md#device-float-behaviour)) | The portable backend |

`host` runs first because it separates a translation defect from a device defect. A byte difference
on `host` is a bug in the port.

## Batches

Lines are the Fortran files' lines, comments included (`just port-status`, from
[`rust/port_files.tsv`](../rust/port_files.tsv), which gives every one of the 135 `.F90` files a
batch). The `.f`, `.f90` and `.c` files, `SCRIP/` and `PDLIB/` are not counted; Batch 8 takes them.
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

1. Build the reference with `just ref-build`, on a native x86_64 machine for any bit verdict.
2. Run `ww3_ts1` ST4, `ww3_tp2.4` and `ww3_tp2.15` on it, twice, to learn which files WW3 itself
   does not reproduce. Keep the outputs by checksum.
3. Profile the reference at this pin. ww3-gpu's profiles are NOAA's pin, and MetOcean's ST4 differs.
4. Add `rust/gpu-cuda/` and `rust/gpu-spirv/`, each a workspace with its own nightly, each building
   `ww3-kernels`. Rust-CUDA uses `ubuntu24-cuda13-llvm21` in its `container/`.
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

`constants` (done), `w3gdatmd`, `w3adatmd`, `w3wdatmd`, `w3odatmd`, `w3idatmd`, `w3dispmd`,
`w3servmd`, `w3timemd`, `w3arrymd`, and the readers `w3iogrmd`, `w3iorsmd`.

- WW3's global pointers (`W3SETG`, `W3SETA`) become one owned struct per grid. Arrays become
  flat `#[repr(C)]` buffers with Fortran's bounds carried explicitly: `SIG(0:NK+1)` is not `SIG[0]`.
- `ww3-io` reads and writes unformatted Fortran records.
- **The shortcut that makes the batches possible:** Batches 1 to 4 read `mod_def.ww3`, `wind.ww3`
  and `restart.ww3` written by the Fortran. A model runs from Batch 3 with no preprocessor ported.
  chaceon still contains no Fortran; it only reads these files.

### Batch 2: source terms on the device

In the order the profile sets, pending Batch 0's new one: `W3SNL1`, `W3SDS4`, `W3SIN4`, `W3SPR4`,
`W3SRCE`, `W3SDB1`, `W3SBT1`, `W3SLN1`, `W3SBT4`, and their setup routines.

- Source: the Fortran at the pin, never ww3-gpu's C++, which translates NOAA's ST4 and misses
  MetOcean's `USTAR` floors and `ISNAN` guards (`w3src4md.F90:680`, `:780`, `:835`, `:1683`;
  `w3srcemd.F90:1048`, `:1071`, `:1178`, `:1891`) (v, ww3-gpu's log 2026-10-29). The C++ is useful
  for one thing: its traps.
- The integrator has a different time step per sea point. On the device that is every point
  looping to its own end, with a batch mask; points never wait for one another in the arithmetic.
- A kernel does not allocate: scratch is a preallocated per-point buffer.

### Batch 3: the time loop

`w3wavemd`, `w3initmd`, `w3updtmd`, `w3pro3md`, `w3uqckmd`, `w3parall`, `w3cspcmd`, `w3flx2md`
(`ww3_tp2.4` needs `FLX2`). `ww3_tp2.4` runs propagation alone (`ST0`, `NL0`); `ww3_tp2.15` has the
target's physics, `PR3 UQ FLX0 LN1 ST4 NL1 BT1 DB1`, so it is the first full model (v, its `switch`).
This is the first run with the whole spectrum resident: a Rust `ww3_shel` that writes `out_grd.ww3`
and `restart.ww3` for `cmp`.

### Batch 4: outputs

`ww3_shel` in full, `w3iogomd`, `w3iopomd`, `w3iobcmd`, `w3partmd`, `ww3_ounf`, `ww3_ounp`,
`ww3_outf`, `ww3_outp`, the namelist readers. NetCDF goes through a binding to the C library, unless
Batch 4 finds a pure Rust writer that reproduces the files ⚠. Field computation (`W3OUTG`) runs on
the device, reading only what an output needs.

### Batch 5: preprocessors

`ww3_grid`, `ww3_strt`, `ww3_prnc`, `ww3_prep`, `ww3_bound`, `ww3_bounc`, `ww3_uprstr`, `w3gridmd`,
`w3gsrumd`. Exit: a regtest from namelists to NetCDF, Rust only, whose `mod_def.ww3` equals the
Fortran's byte for byte. `ww3_grid` also writes `ST4TABUHF2.bin`, so the tables must match too.

### Batch 6: speed

No new physics. Whole-spectrum copies per step become zero, kernels are grouped to cut dispatch
cost, and the order of work is tuned while the bits stay fixed. Speed needs its parity first, so
timings start here, on the H100, as a machine, a command and a median of three runs. UFSC's H100
access is still open (ww3-gpu #143), so this batch waits on it. The RTX 4090 serves until then.

### Batches 7 and 8: the rest

Batch 7: `ST1`, `ST2`, `ST3`, `ST6`, `NL2` to `NL5`, ice, the fluxes, tides, the switch MetOcean
ships (`BT4`, `WCOR`, `RWND`, `SEED`, `MPI`). Batch 8: PDLIB and the unstructured solvers, the SMC
grid, `ww3_multi`, domain decomposition over several GPUs. MPI needs an ADR of its own: a binding
to C MPI, or GPU-aware transport.

## Risks

| Risk | Effect | Answer |
|---|---|---|
| Rust-CUDA or rust-gpu breaks on a real kernel | Port stops | Batch 0's stop point; kernels stay plain `no_std` Rust, so a compiler can be replaced |
| The two pin different nightlies, `2026-04-02` and `2026-08-15` (v) | A language feature works on only one | Separate workspaces; kernels use only what both accept |
| gfortran's `-O3` choices (power chains, ST4's libmvec loops, per ww3-gpu's `st4_libmvec.hpp:10-13`) | `host` is off by a ULP | Read the optimised code; repeat at this pin ⚠ |
| Two executables, unlike ww3-gpu's one | Build differences count as physics | One named machine; the reference runs twice to measure itself |
| glibc picks `expf`'s FMA variant by CPU ⚠ | The reference depends on the machine | Record the CPU; never judge bits on emulated x86_64 |
| gfortran on arm64 may contract by default ⚠ | The Mac's Fortran is not the reference | x86_64 only (ADR-0005) |
| `Float64` is missing on Apple GPUs ⚠ | No `DB1` on the Mac | Stop with a message; no demotion |
| Licence | A translation of LGPL code under MPL-2.0 | ADR-0006 |

## Open for the owner

1. **Licence** (ADR-0006): the repository's `LICENSE` is MPL-2.0, and a translation of WW3 derives
   from LGPLv3 code. Files translated from WW3 are marked `LGPL-3.0-only` until this is settled.
2. **The switch**: `switch_chaceon_shrd` is ww3-gpu's guess at ondaseventos' build ⚠.
3. **GPU supervision**: AGENTS.md keeps ww3-gpu's rule that device runs need the owner. If runs on
   the Mac may go unsupervised, ADR-0003 should say so.
