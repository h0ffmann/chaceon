# Feature parity: chaceon and ww3-gpu

2026-11-09. chaceon and [ww3-gpu](https://github.com/h0ffmann/ww3-gpu) port the same model,
WAVEWATCH III® (WW3), with the same rule: the port gives the Fortran's bits. They differ in how. chaceon
replaces WW3 with Rust, and no Fortran runs in its process. ww3-gpu keeps WW3 and lets the Fortran call
C++ written with Kokkos, a C++ library that compiles one source for CPU threads (OpenMP) and for NVIDIA
GPUs (CUDA). This page compares what each one covers and how far each claim is checked. It is a survey, not
a plan: nothing here was run for it.

## Sources

| Side | Read at | Ledger |
|---|---|---|
| chaceon | `fe531e1`, 2026-11-04 | [`rust/ports.tsv`](../rust/ports.tsv), 188 rows, 175 distinct routines |
| ww3-gpu | `58167d7`, 2026-10-22 | `kokkos/ports.tsv`, 316 rows, 316 distinct routines |

Every count on this page was made by joining the two ledgers on the `module:routine` name, after removing
repeated rows (v, a one-off script over both files at the commits above). 85 routines are in both ledgers,
90 only in chaceon's and 231 only in ww3-gpu's (v). Where a ledger and a prose page of the same repository
disagree, the ledger wins here, and the disagreement is named.

**The two references are not the same WW3.** chaceon compares with metocean/WW3 7.14 at `92e2cf66`.
ww3-gpu compares with NOAA-EMC/WW3 at `03adda91`, through the fork h0ffmann/WW3. Line numbers differ
between the two, and MetOcean's changes are only in chaceon's reference (`AGENTS.md`, "Porting"). A row
below matches routines by name, not by source text.

## How the two ladders line up

Each ledger has its own steps. They are not the same evidence, so a step in one column is not a step in
the other.

| chaceon status | What it means | Nearest ww3-gpu level | What that means |
|---|---|---|---|
| `translated` | Rust exists, not fully reached | `translated` (`ported` in the `status` column, `wired` no) | A Kokkos version exists in `kokkos/src` |
| `routine` | Every call captured from the running reference is bit-identical on the host, and in SPIR-V where the routine runs on the device ([ADR-0007](ADRs/ADR-0007-routine-parity-by-capture.md)) | `wired` | WW3 at the fork's pin calls the Kokkos code behind a run-time flag |
| `host` | Whole runs byte-identical to the reference, native on the CPU ([ADR-0005](ADRs/ADR-0005-three-verdicts.md)) | `regtested` (L2) | WW3's regtests ran with the Kokkos code and agreed with the Fortran of the same executable, on the CPU |
| `spirv` | `host`, and the same files from SPIR-V on CPU Vulkan (lavapipe) | none | |
| `cuda` | Whole runs on a real NVIDIA GPU | `gpu` | The L2 replay on the CUDA backend |

No routine has reached `cuda` or `gpu` (v, both ledgers). `routine` and `wired` sit on the same row
because both come after a translation and before a whole-run verdict, but `routine` is checked against
captures and `wired` is not checked by itself: a wired ww3-gpu routine with no regtest has, at most, the
byte-identical `smoke` and `full` checkpoints its `evidence` cell names.

## The projects

| Feature | chaceon | ww3-gpu |
|---|---|---|
| Language of the port | Rust, edition 2024; kernels `#![no_std]` | C++20 with Kokkos 5.2.2 (v, `VERSIONS.md`, `kokkos/CMakeLists.txt:23`) |
| Shape | Replaces WW3: no Fortran in a chaceon process | Inside WW3: the fork's Fortran calls the C++ through a C interface, one flag per part |
| Programs without Fortran | `ww3_grid`, `ww3_strt`, `ww3_prep`, `ww3_shel` in sequence, and the post-processors below | None: every program is WW3's |
| Reference | metocean/WW3 7.14, `92e2cf66` | NOAA-EMC/WW3 `03adda91`; the fork adds 5 687 lines and removes none (v, its README, 2026-10-18) |
| Reference machine | Apple Silicon, linux/arm64 in Docker, gfortran `-O3 -ffp-contract=off` | x86_64, gfortran and g++ 16.2.0, `-ffp-contract=off` (v, the `regtested` cells) |
| Parity on the CPU | Byte-identical to a separately compiled reference | Byte-identical to the Fortran path of the same executable |
| Parity on a GPU | `cuda`: bits are the target; `spirv`: a tolerance fixed before the first run | A tolerance, `nccmp-tol` with `tolerances.txt` (ww3-gpu ADR-0014) |
| CPU parallelism | `ww3_shel --threads N` and, with the feature `mpi`, `mpirun -n R`, the same bytes (v, `just mpi-pair`) | Kokkos OpenMP; MPI keeps the Fortran path (ww3-gpu ADR-0017), though the per-point ST4 parts were byte-identical on 36 MPI regtest entries |
| GPU backends | Rust-CUDA (PTX) and rust-gpu (SPIR-V, Vulkan) | Kokkos CUDA for Ada (`sm_89`) only; Triton kernels allowed under conditions (ww3-gpu ADR-0015) |
| Spectrum resident on the device | Yes, `VA` in SPIR-V on CPU Vulkan, files byte-identical | Yes, `--kokkos resident`, on the OpenMP backend |
| Has run on a real GPU | No: the PTX has never executed | One kernel: the DIA benchmark on an RTX 4090, 2026-09-15. The full model has not |
| Timings | None | DIA kernel 0.047 ms for 1 000 points on the RTX 4090; full model on the CPU 1.65 times less wall time than NOAA WW3 at 32 threads ⚠ (its own record: regtests pending) |
| Tests of its own | None ([ADR-0002](ADRs/ADR-0002-no-ci-no-rust-tests.md)) | None for the port (ww3-gpu ADR-0010); GoogleTest for runtime infrastructure only (ADR-0019) |
| CI | None | GitHub Actions |
| Whole-model evidence | Whole-run `cmp` on its own case sets: 96 files for the Rust-only chain, 214 comparisons for boundaries, 74 for tracks (v, README) | WW3's regtest matrix: 100 serial and 25 OpenMP `NL1` entries byte-identical; the wider matrix postponed (ww3-gpu ADR-0016) |
| Formal methods | Lean 4, one proof about `f32` division ([`LEAN_202610.md`](LEAN_202610.md)) | No proof language; SMT queries and exhaustive sweeps (ww3-gpu ADR-0001) |
| Size of the port | 39 637 Rust lines, tools included (v, README) | 62 257 C++ lines in 355 files, at 2026-10-14 ⚠ (older than the ledger) |
| Licence | MPL-2.0 ([ADR-0006](ADRs/ADR-0006-licence.md)) | MIT, with LGPL-3.0 kernels |
| Book, course, site | No | Yes: 17 lessons, a PDF book, a website |

## Physics, by switch

A switch is WW3's compile-time choice of a scheme. The table gives, for each, the furthest status in each
ledger and the number of routines at it. "None" means no row in that ledger.

### Source terms

| Switch | What it is | chaceon | ww3-gpu |
|---|---|---|---|
| `ST4` | Ardhuin et al. (2010) input and dissipation | `W3SIN4`, `W3SDS4`, `W3SPR4`, `CALC_USTAR` at `spirv`; `INSIN4` and three `TABU_*` at `routine` | All 8 `wired`; `smoke` and `full` byte-identical, 2026-10-15; no regtest |
| `ST1` | WAM cycle 3 (Komen et al. 1984) | 3 at `routine` | 3 `wired` |
| `ST2` | Tolman and Chalikov (1996) | 5 at `routine` | 5 `wired` |
| `ST3` | Janssen's WAM cycle 4 input | 7 at `routine` | 7 `wired` |
| `ST6` | Observation-based (Babanin, Rogers, Zieger) | 7 at `routine` | 8 `wired` (`RRANGE` too) |
| `ST0` | No input or dissipation | None | 1 `wired` |
| `SWL6` | ST6's swell dissipation, `W3SWL6` | 2 at `routine` | 2 `wired`, 1 `translated` |
| `FLX1` to `FLX4` | Wind stress formulations | 1 each at `routine` | 1 each `wired` |
| `FLX5` | Stress taken from the atmospheric model | None | 1 `wired` |
| `FLD1`, `FLD2` | Stress from the wave field (`FLD2` is in GFS-Wave's switch) | None | 6 `wired` |
| `LN1` | Cavaleri and Malanotte-Rizzoli linear input | `spirv` | `wired` |
| `NL1` | Discrete Interaction Approximation (DIA) | `W3SNL1` at `spirv`, `INSNL1` at `host` | Both `regtested`: 100 matrix entries byte-identical |
| `NL2` | Exact transfer, Webb, Resio and Tracy (WRT) | None | None |
| `NL3` | Generalised Multiple DIA | None | 6 `translated` |
| `NL4` | Two-scale approximation | None | 2 `translated` |
| `NL5` | Generalised kinetic equation (GKE) | None | 13 `translated` (`W3SNL5MD` and `W3GKEMD`) |
| `NLS` | Nonlinear high-frequency filter | None | 1 `translated` |
| `TR1` | Triads, lumped triad approximation | None | 1 `translated` |
| `BT1` | JONSWAP bottom friction | `spirv` | `wired` |
| `BT4` | SHOWEX movable-bed friction | 3 at `routine`, called from `W3SRCE` in a `BT4` build | 3 `translated` |
| `BT8`, `BT9` | Muddy-bed damping | None | 4 `translated` |
| `DB1` | Battjes and Janssen depth breaking | `spirv` | `wired` |
| `REF1` | Reflection from shorelines and icebergs | None | 1 `translated` |
| `IC1` | Uniform ice damping | `routine` | `translated` |
| `IC3`, `IC4`, `IC5` | Viscoelastic, frequency-dependent and effective-medium ice damping | None | 26 `translated` |
| `IS1` | Ice scattering (Zieger) | `routine` | `translated` |
| `IS2` | Ice scattering and floe break-up | None | 5 `translated` |
| `IG1` | Infragravity waves | None | 1 `translated` |
| `UOST` | Unresolved obstacles source term (Mentaschi et al. 2018) | None | 7 `translated` |

`kokkos/PORT_STATUS.md` says `NL2`, `NL4`, `NL5` and `NLS` are "not started", and its table puts `ST0` to
`ST6` and `FLX1` to `FLX5` in an uncommitted pull request. Its ledger, which is newer, has the rows above.

### Propagation and grids

| Feature | chaceon | ww3-gpu |
|---|---|---|
| `PR3` with `UQ` (third order, ULTIMATE QUICKEST) | `W3XYP3`, `W3KTP3`, `W3QCK1` to `W3QCK3` at `spirv`; `W3MAP3`, `W3MAPT`, `W3CFLXY` at `host` | `W3MAP3`, `W3MAPT` `regtested`; `W3XYP3`, `W3KTP3`, `W3QCK1` to `W3QCK3` `wired` |
| `PR1`, `PR2` (first order, second order) | None | 4 `translated` |
| Spherical Multiple-Cell (SMC) grids | None | 22 `translated` (`W3PSMCMD`, `W3SMCOMD`) |
| Unstructured grids, with `PDLIB` | None; planned in [`UNSTRUCTURED_GRIDS_202610.md`](UNSTRUCTURED_GRIDS_202610.md) | 56 `translated`; `W3XYPUG` and `W3XYPFSN2` `regtested` |
| Grid search and metrics (`W3GSRUMD`) | 18 at `host`, 1 `translated` | 1 `translated` |
| Dispersion (`W3DISPMD`) | `DISTAB`, `WAVNU1` at `host`, `WAVNU2` at `routine` | `DISTAB`, `WAVNU2` `regtested`, 3 `translated` |
| Wave setup (`W3WAVSET`) | None | 17 `translated` |
| Tides (`W3TIDEMD`) | None | 14 `translated` |

### The time loop and state

| Part | chaceon | ww3-gpu |
|---|---|---|
| `W3SRCE`, the source-term integration | `spirv` | `wired`, batched over points; `CALC_FPI` `wired` |
| `W3WAVE`, the time step | `translated`; `W3GATH`, `W3SCAT` at `host` | `translated`; `W3GATH`, `W3SCAT` `regtested` |
| Updates (`W3UPDTMD`) | 8 at `host`: winds, currents, start, transparencies, gradients, boundary points, ice, water levels | `W3UWND`, `W3DZXY` `regtested`; `W3UTRN` `wired`; 4 `translated` |
| Multi-grid `ww3_multi` | None | `WMIOHG`, `WMIOEG` `regtested` on a fork branch, `mww3_test_01` to `_05` |
| Coupling | None | None |

### Files and programs

| Part | chaceon | ww3-gpu |
|---|---|---|
| Grid file `W3IOGR` | `host` | `translated` |
| Restart `W3IORS` | `host` | `translated` |
| Point output `W3IOPO`, `W3IOPE`, `W3IOPP` | `host` | `translated` |
| Field output `W3OUTG`, `W3IOGO` and helpers | 7 at `host` | `W3OUTG` `wired`, 7 `translated` |
| Boundary file `W3IOBC` | `host` | `translated` |
| Track file `W3IOTR` | `host` | None |
| Partitioning `W3PART` and helpers | 2 at `host`, 3 `translated` | 8 `translated` |
| `W3CPRT` | `translated` | `wired` |
| `ww3_grid`, `ww3_strt`, `ww3_bound`, `ww3_outf`, `ww3_outp`, `ww3_trck`, `ww3_trnc`, `ww3_ounf` | `host`; `ww3_outp`'s `W3EXPO` is `translated` | Not ported: WW3's Fortran |
| `ww3_prep`, `ww3_prnc`, `ww3_ounp`, `ww3_shel` | `translated`, with `host` callees; their whole-run `cmp` is in the README | Not ported |
| `ww3_bounc` | Not ported | Not ported |

## What each one has that the other does not

**chaceon only.** A chain of programs with no Fortran in it, from namelists to restarts, byte-identical on
96 files. NetCDF post-processing (`ww3_ounf`, `ww3_ounp`), text output (`ww3_outf`, `ww3_outp`) and tracks
(`ww3_trck`, `ww3_trnc`) byte-identical. Byte-identical MPI and thread runs of the Rust model. A
whole run in SPIR-V with the spectrum resident, byte-identical on CPU Vulkan. Capture replays, which check
a routine against every call the reference made, before any whole run. MetOcean's 7.14 changes, and a Lean
proof.

**ww3-gpu only.** A kernel timed on a real GPU, and a full-model CPU speed-up ⚠. WW3's own regtest matrix
as evidence. Much wider translation: SMC and unstructured grids, `PR1` and `PR2`, `NL3` to `NL5`, triads,
reflection, `IC3` to `IC5`, `IS2`, mud, tides, wave setup, `UOST`, `ww3_multi`. Every source-term package
`wired` into a running WW3, against `routine` in chaceon. CI, a book and a website.

**Neither.** A whole model on a GPU, `NL2`, `ww3_bounc`, coupling.

## How to bring this page up to date

Read both ledgers at their current commits, join them on the lower-case `routine` column, and count each
module's routines at their furthest status. In ww3-gpu, take `regtested` when that column starts with
`yes`, else `wired` when that one does, else `translated`. Then reread the two README status sections for
the project rows. Change the date at the top and the commits under **Sources**.
