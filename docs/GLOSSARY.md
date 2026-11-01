# Glossary

| Term | Meaning |
|---|---|
| DB1 | WW3 switch: Battjes and Janssen depth-induced breaking, `W3SDB1` |
| DIA | Discrete Interaction Approximation of the nonlinear transfer; WW3's `NL1`, `W3SNL1` |
| FMA | Fused multiply-add: `a*b+c` with one rounding |
| GIMPLE | GCC's intermediate form, read to see what `-O3` computes |
| `host`, `cuda`, `spirv` | The three verdicts of ADR-0005 |
| `W3XYP3`, `W3KTP3` | WW3's third-order propagation in space and in spectral space (`PR3`) |
| `W3QCK1`, `W3QCK2`, `W3QCK3` | The ULTIMATE QUICKEST step, `UQ`: uniform, varying grid, with transparencies |
| `out_grd.ww3` | WW3's field output, one record per field per output time |
| `lavapipe` | Mesa's CPU implementation of Vulkan, the only place this repository runs SPIR-V |
| `NoContraction` | SPIR-V decoration that stops a driver fusing a multiply and an add |
| CRD | Contour residual distribution: the family of schemes WW3 uses on triangles |
| `PDLIB` | WW3's domain decomposition for unstructured grids |
| `out_pnt.ww3` | WW3's point output: spectra and scalars at output points |
| lavapipe | Mesa's Vulkan implementation on the CPU |
| libdevice | NVIDIA's device math library |
| libmvec | glibc's vector math library |
| PR3 | WW3 switch: third-order propagation, `W3PRO3` |
| PTX | NVIDIA's virtual instruction set, what Rust-CUDA emits |
| `routine` | Status of ADR-0007: every captured call bit-identical |
| reference | metocean/WW3 at `92e2cf66`, built by `docker/Dockerfile.reference` |
| Rust-CUDA | Rust compiler backend for PTX (`rustc_codegen_nvvm`) |
| rust-gpu | Rust compiler backend for SPIR-V (`rustc_codegen_spirv`) |
| sm_89, sm_90 | NVIDIA shader models: RTX 4090, H100 |
| SPIR-V | Khronos' binary shader language, what Vulkan consumes |
| `spirv-opt` | The SPIR-V optimiser of SPIRV-Tools; it folds floats unless `NoContraction` is set |
| `NoContraction` | SPIR-V decoration: no fusing or reassociating of that operation |
| `spirv_exact` | `rust/tools/spirv_exact`: adds `NoContraction`, widens `f32` division |
| ST4 | WW3 switch: the Ardhuin et al. source terms, `W3SIN4`, `W3SDS4` |
| round to odd | A rounding that lets a later rounding to fewer bits stay correct (Boldo and Melquiond, 2008) |
| ULP | Unit in the last place |
| `VA` | WW3's action spectrum per sea point |
| `mod_def.ww3` | WW3's model definition file: grid, spectral discretisation, namelist constants and the source-term tables, written by `ww3_grid` and read by every other program |
| `W3GRID`, `W3IOGR` | The grid preprocessor's subroutine (`w3gridmd.F90`) and the routine that writes and reads `mod_def.ww3` (`w3iogrmd.F90`) |
| `READNL` | `W3GRID`'s scan of the namelist file `GRID%NML` for a group named `&SIN4`, `&MISC` and so on |
| `INA2R`, `INA2I` | WW3's reads of a real or integer array from a file, by layout (`IDLA`) and format (`IDFM`) indicators |
| `GSU`, `W3GRMP` | The grid search utility object (`w3gsrumd.F90`) and its bilinear-weights call, which `ww3_grid` uses for output boundary points |
| `ICLOSE` | Grid closure: none (-1), simple (2) or tripole (8) |
| `TRFLAG` | Obstruction-transparency flag of `&MISC FLAGTR`: 0 none, odd between grid points, even at grid points |
| MPI, rank | Message Passing Interface; a rank is one of the processes `mpirun -n R` starts |
| `DIST`, `SHRD`, `OMPG` | WW3 switches for distributed memory (MPI), shared memory and OpenMP over grid points; picked at compile time there, at run time here |
| `IAPPRO` | WW3's table from spectral component to the rank that propagates it (`W3INIT`, `w3initmd.F90:853`) |
| `W3GATH`, `W3SCAT` | Gather one component's values over all sea points into the field `W3XYP3` takes, and scatter the result back (`w3wavemd.F90`) |
| rsmpi | The `mpi` crate: Rust bindings to the system MPI library through bindgen |
| Lean 4 | A proof assistant and programming language; a theorem is accepted only when its kernel checks the proof |
| mathlib | Lean's community library of mathematics, pinned in `VERSIONS.md` |
| `lake` | Lean's build tool; `lake exe cache get` downloads mathlib already built |
| elan | Lean's toolchain manager, installed in `docker/Dockerfile.lean` |
| Aeneas | A tool that translates safe Rust to a pure model in Lean (and other provers) to verify it |
| `sorry` | Lean's placeholder for a missing proof; `just lean` refuses any |
| midpoint | The real number halfway between two adjacent `f32` values; a quotient on one side of it rounds one way |
| OpFDiv, OpFConvert | SPIR-V's float division and float-width conversion |
