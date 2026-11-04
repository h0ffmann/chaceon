# Glossary

| Term | Meaning |
|---|---|
| DB1 | WW3 switch: Battjes and Janssen depth-induced breaking, `W3SDB1` |
| BT4 | WW3 switch: SHOWEX bottom friction over a movable sand bed (Ardhuin et al. 2003), `W3SBT4` |
| SHOWEX | Shoaling Waves Experiment, North Carolina, 1999: the field data behind `BT4`'s ripple roughness |
| `ERFTABLE` | `W3SBT4MD`'s table of `(1+erf(x))/2` on `[-4, 4]`, built by `TABU_ERF` |
| ST3 | WW3 switch: Janssen's quasi-linear wind input after WAM cycle 4, with its dissipation: `W3SIN3`, `W3SDS3` |
| `INSIN3` | `W3SRC3MD`'s table setup: calls `TABU_STRESS` and `TABU_TAUHF` |
| `TAUT` | `W3SRC3MD`'s table of friction velocity against wave-supported stress and 10 m wind, built by `TABU_STRESS` |
| `TAUHFT` | `W3SRC3MD`'s table of the high-frequency wave-supported stress against friction velocity and Charnock coefficient, built by `TABU_TAUHF` |
| Shields parameter | Bed shear stress over the submerged weight of a grain layer; ripples form above a critical value (`PSIC`) |
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
| `W3SDS1` | ST1 and WAM-3 whitecapping dissipation, the dissipation term of switch `ST1` (`w3src1md.F90:432`, method "WAM-3"): `S = SDSC1 * FMEAN * WNMEAN^3 * EMEAN^2 * K * A` |
| ST4 | WW3 switch: the Ardhuin et al. source terms, `W3SIN4`, `W3SDS4` |
| round to odd | A rounding that lets a later rounding to fewer bits stay correct (Boldo and Melquiond, 2008) |
| ULP | Unit in the last place |
| `VA` | WW3's action spectrum per sea point |
| `mod_def.ww3` | WW3's model definition file: grid, spectral discretisation, namelist constants and the source-term tables, written by `ww3_grid` and read by every other program |
| `W3GRID`, `W3IOGR` | The grid preprocessor's subroutine (`w3gridmd.F90`) and the routine that writes and reads `mod_def.ww3` (`w3iogrmd.F90`) |
| `READNL` | `W3GRID`'s scan of the namelist file `GRID%NML` for a group named `&SIN4`, `&MISC` and so on |
| `ww3_prep`, `W3PREP` | The input preprocessor (`ww3_prep.F90`): reads a water level, current, wind, ice, mud, momentum or density field, or assimilation data, from a file and writes the forcing file `ww3_shel` reads (`current.ww3`, `wind.ww3`, `ice.ww3`, ...) |
| `AI`, `LL`, `F1`, `F2`, `DAT` | The input types of `ww3_prep`: the field is already on the model grid (`AI`), on a regular longitude-latitude grid (`LL`), on a grid given by a file (`F1`), two such grids averaged (`F2`), or it is assimilation data (`DAT`) |
| `ww3_prnc`, `W3PRNC` | The NetCDF forcing preprocessor (`ww3_prnc.F90`): reads a wind, current, ice, level, mud, momentum or air density field from a NetCDF file and writes the same forcing file as `ww3_prep`. Its input is `ww3_prnc.nml`, or the older `ww3_prnc.inp` |
| `AI`, `LL`, `AT` of `ww3_prnc` | Its input types: the field is on the model grid (`AI`, `ITYPE` 1), on a longitude-latitude grid, regular or curvilinear, to be interpolated (`LL`, `ITYPE` 2), or a time series on the model grid to be reduced to tidal constituents (`AT`, `ITYPE` 6, refused here). `FORCING%GRID%ASIS` and `LATLON` of the namelist select `AI` and `LL` |
| CDL, `ncgen` | The text form of a NetCDF file, and the tool that builds the file from it. `regtests/prnc_inputs/` holds CDL because the NetCDF inputs of the regtests are not in the WW3 copy |
| `INTERP` | The bilinear interpolation of `ww3_prnc` from the weights of `W3GRMP`; it skips corners holding the fill value and rescales the weights that remain |
| `W3FLDO`, `W3FLDG` | The routines that open a forcing file and write or read its header (`W3FLDO`) and its fields at one time (`W3FLDG`), `w3fldsmd.F90` |
| `FNMPRE` | The file name preamble (`./` by default) that `ww3_prep` puts before every file name it opens |
| `INA2R`, `INA2I` | WW3's reads of a real or integer array from a file, by layout (`IDLA`) and format (`IDFM`) indicators |
| `GSU`, `W3GRMP` | The grid search utility object (`w3gsrumd.F90`) and its bilinear-weights call, which `ww3_grid` uses for output boundary points and `ww3_prep` for the `LL`, `F1` and `F2` input types |
| `W3FLDP`, `W3FLDD` | `w3fldsmd.F90`: `W3FLDP` computes the interpolation factors from an input grid with a land mask to the model grid (`F1`, `F2`), `W3FLDD` writes the assimilation data records of the `DAT` type |
| `MSKC`, `NNBR`, `W3NNSC` | In `W3GRMP`: how the cell around a target point is masked (none, partly, fully), the number of nearest unmasked points a fully masked cell falls back on, and the table of index offsets (`W3NNSC`) it searches |
| `LMSK`, `MAPOVR` | In `ww3_prep`: the land mask of an input grid as a logical array, and the count of input grids that cover each model point (land points keep `ILAND`) |
| `EJ5P` | The JONSWAP one-dimensional spectrum of `w3servmd.F90`, used by `ww3_strt` start type 2 |
| `WNT1`, `CRT1` | Switches that make `ww3_prep` correct an interpolated wind, momentum or current for conservation of velocity (the factor `MIN(1.5, A2/A1)`) |
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
| `ww3_outp` | WW3's point output postprocessor: reads `out_pnt.ww3`, writes tables, spectra files and print plots |
| `ww3_outf` | WW3's field output postprocessor: reads `out_grd.ww3`, writes print plots, statistics and transfer files |
| `W3EXPO`, `W3EXGO` | The internal subroutines of `ww3_outp` and `ww3_outf` that write one output time |
| `ITYPE`, `OTYPE` | The output type and subtype lines of `ww3_outp.inp`; `ITYPE` is also the first item of `ww3_outf.inp`'s type line |
| `IDLA`, `IDFM` | Layout (1 to 5) and format (free, fixed, unformatted) indicators of a `ww3_outf` transfer file |
| `PRT1DS`, `PRT2DS`, `PRTBLK`, `OUTA2I` | `w3arrymd` print plots of a 1-D spectrum, a 2-D spectrum, a field block, and the writer of an integer array |
| `STME21` | `w3timemd`: a time as the text `YYYY/MM/DD HH:MM:SS UTC` |
| edit descriptor | A Fortran format item such as `F8.2`, `E11.3`, `I4.2`; `rust/crates/ww3-host/src/fortfmt.rs` prints them as gfortran does |
| `ww3_ounf` | WW3's postprocessor for gridded output: reads `mod_def.ww3` and `out_grd.ww3`, writes NetCDF files |
| NetCDF | The self-describing array file format `ww3_ounf` writes; `FILE%NETCDF` 3 is the classic format, 4 is HDF5-based |
| HDF5 | The container under NetCDF-4 files; the reference's HDF5 is 1.10.8 |
| `libnetcdf` | The NetCDF C library, `libnetcdf.so.19` on bookworm, the only foreign library `ww3_ounf` calls (ADR-0001) |
| `NF90_*` | The Fortran 90 interface of NetCDF that `ww3_ounf` calls; it reverses dimension order against the C functions Rust calls |
| `W3EXNC`, `W3CRNC` | `ww3_ounf`'s routines that write one output time and create one file |
| `S2GRID`, `W3S2XY` | Spread the sea-point array of a field onto the `(NX+1,NY)` grid; `S2GRID` also converts directions and replaces `UNDEF` by `NOVAL` |
| `FIELD%TYPE`, `NCVARTYPE` | `ww3_ounf`'s variable type: 2 packed `short` with a scale factor, 3 `short` for most fields and `float` for a few, 4 `float` |
| `NINT` | Fortran's round to the nearest integer, halves away from zero; on aarch64 one `fcvtas`, which saturates out of range |
| `SINGLEFILE` | An uninitialised logical of `ww3_ounf` that the reference reads as true: one file named from the first output time |
| `ww3_ounp` | WW3's postprocessor for point output: reads `mod_def.ww3`, `out_pnt.ww3` and `ww3_ounp.nml`, writes NetCDF files, one for all stations or one per station; `SINGLEFILE` is the same uninitialised logical |
| `POINT%TYPE`, `SPECTRA%OUTPUT`, `PARAM%OUTPUT` | The `ITYPE` and `OTYPE` of `ww3_ounp.nml`: type 1 is spectra (output 3 the file of `efth`), type 2 is mean parameters (output 1 depth and wind, 2 wave parameters, 5 validation, 6 WMO) |
| `DIMORDER` | `ww3_ounp.nml`: true gives variables of shape (time, station) in C order, false (station, time) |
| `FNMPRE` | The file name preamble of WW3, `./`; it is part of the file names `ww3_ounp` writes and of `product_name` less its first two characters |
| ncdump | NetCDF's text dump of a file's header and data, used here to read what a file holds |
| `W3PART` | WW3's spectral partitioning of one spectrum into wind sea and swells (`w3partmd.F90`); `PTMETH` 1 to 5 picks the method |
| `W3CPRT` | Runs `W3PART` on every sea point before an output that needs partitions (`w3iosfmd.F90`) |
| `W3UBPT` | Interpolates the boundary spectra read by `W3IOBC` to a grid's boundary points (`w3updtmd.F90`) |
| `W3CSPC` | Carries a spectrum to another spectral grid, keeping each bin's energy (`w3cspcmd.F90`) |
| `W3ULEV` | Applies a new water level: depths, wavenumbers, dry cells, spectra remapped (`w3updtmd.F90`) |
| `W3UICE` | Applies a new ice concentration to the maps and spectra (`w3updtmd.F90`) |
| `W3IOBC` | Reads, writes and dumps the boundary data files `nest.ww3` and `nestNN.ww3` (`w3iobcmd.F90`) |
| `ww3_bounc` | WW3's pre-processor that turns NetCDF spectra into `nest.ww3` |
| `ww3_bound` | WW3's older pre-processor for the same file, from ASCII or binary spectra |
