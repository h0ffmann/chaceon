# Glossary

| Term | Meaning |
|---|---|
| FLX1 | WW3 switch: wind stress from the 10 m speed by Wu (1980), `W3FLX1`: `CD = 0.001*(0.8+0.065*U10)` |
| ST1 | WW3 switch: WAM cycle 3 source terms (Komen et al. 1984): wind input `W3SIN1`, dissipation `W3SDS1`, mean parameters `W3SPR1` |
| `SINC1` | `W3SIN1`'s wind input constant, `28 * CINP * DAIR / DWAT` (`w3gridmd.F90:1744`) |
| DB1 | WW3 switch: Battjes and Janssen depth-induced breaking, `W3SDB1` |
| BT4 | WW3 switch: SHOWEX bottom friction over a movable sand bed (Ardhuin et al. 2003), `W3SBT4` |
| IS1 | WW3 switch: scattering of wave action across directions by sea ice (Zieger), `W3SIS1`; `IS1C1` and `IS1C2` turn the ice concentration into its coefficient |
| SHOWEX | Shoaling Waves Experiment, North Carolina, 1999: the field data behind `BT4`'s ripple roughness |
| `ERFTABLE` | `W3SBT4MD`'s table of `(1+erf(x))/2` on `[-4, 4]`, built by `TABU_ERF` |
| ST3 | WW3 switch: Janssen's quasi-linear wind input after WAM cycle 4, with its dissipation: `W3SIN3`, `W3SDS3` |
| `INSIN3` | `W3SRC3MD`'s table setup: calls `TABU_STRESS` and `TABU_TAUHF` |
| `TAUT` | `W3SRC3MD`'s table of friction velocity against wave-supported stress and 10 m wind, built by `TABU_STRESS` |
| Shields parameter | Bed shear stress over the submerged weight of a grain layer; ripples form above a critical value (`PSIC`) |
| IC1 | WW3 switch: ice damping with one uniform exponential decay rate, `W3SIC1` (Rogers and Orzech 2013) |
| `ICEP1` | Ice parameter 1 of `W3IDATMD`: the decay rate `k_i` of wave amplitude under ice, read per grid point; `W3SIC1` returns `D = -2*Cg*k_i` |
| `MAPWN` | `W3GDATMD`'s map from a spectral bin `IS = ITH + (IK-1)*NTH` to its wavenumber band `IK` |
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
| WAM | The Wave Model (WAMDI Group 1988): the spectral model whose cycle 4 input term ST3 converts to WW3's action spectrum |
| `TAUHFT` | `W3SRC3MD`'s and `W3SRC4MD`'s table of the high-frequency wave-supported stress against friction velocity and Charnock parameter, built by `TABU_TAUHF` |
| `W3SDS3`, `SDS3` | ST3's whitecapping dissipation (WAM cycle 4 with the mean wavenumber of Bidlot et al. 2005), `w3src3md.F90`; `SDSC1`, `SDSDELTA1`, `SDSDELTA2` set it in `&SDS3` |
| WAM4 | The fourth cycle of the WAM wave model (WAMDI Group 1988; Komen et al. 1994), whose whitecapping `ST3` carries |
| `FTE`, `FTF`, `FTWN` | `W3GDATMD` factors that add the spectral tail beyond `FREQ(NK)` to the mean energy, frequency and wavenumber sums |
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
| `W3SPR3` | ST3's mean wave parameters (energy, mean frequencies, mean wavenumber, maximum) and the drag at the wind level |
| `CALC_USTAR` | Friction velocity, roughness length and Charnock coefficient from the wind speed and the wave-supported stress, by interpolation in `TAUT`; ST3 and ST4 each have one |
| `LFACTOR` | `W3SRC6MD`'s iteration that finds the reduction of `W3SIN6`'s wind input so that the stress it carries does not exceed the total stress |
| `TAU_WAVE_ATMOS` | `W3SRC6MD`'s stress from the waves to the atmosphere, from the negative part of `W3SIN6`'s input |
| `TAUWINDS` | `W3SRC6MD`'s stress `GRAV*DWAT*SUM(S/C*DSII)` of a wind input spectrum |
| `W3SDS6`, `SDS6` | ST6's whitecapping dissipation, an inherent term T1 plus a cumulative term T2 (Babanin et al. 2010; Rogers et al. 2012), `w3src6md.F90`; `SDSET`, `SDSA1`, `SDSA2`, `SDSP1`, `SDSP2` set it in `&SDS6` |
| `__powisf2` | libgcc's `REAL**INTEGER` with a run-time exponent, a chain of squarings; gfortran calls it at `-O3` |
| `W3SIN2`, `SIN2` | ST2's wind input (`w3src2md.F90`): the growth rate of each bin is read from an interpolation table of the wave age and the drag, with a filter that damps negative input (swell); `&SIN2` sets `ZWND` and `SWELLF` |
| `INPTAB`, `W3BETA`, `BETATB` | ST2's table of the wind-wave interaction parameter against the nondimensional frequency `OMA` and the drag, and the function that fills it |
| FLX2 | WW3 switch: stress of Tolman and Chalikov (1996) with the drag depending on the peak frequency, `W3FLX2`; iterates `UST` `NITTIN` times with `ALPHA = 0.57*(CP/UST)**-1.5` |
| FLX4 | WW3 switch: drag coefficient as a quadratic in the 10 m speed (Hwang 2011), `W3FLX4`, scaled by the namelist `&FLX4 CDFAC` (module variable `FLX4A0`); capped for `U10 >= 50.33` m/s |
| ST2 | WW3 switch: Tolman and Chalikov (1996) source terms, `W3SIN2`, `W3SDS2`, `W3SPR2`, with the stress from a flux routine such as `W3FLX2` |
| `W3SPR2` | ST2's mean wave parameters: energy, mean frequency, mean wavenumber, maximum, Phillips' constant per band `ALFA` and the peak frequency `FP` estimated from `FPI` |
| `W3SWL6`, `SWL6` | ST6's turbulent dissipation of narrow-banded swell (Babanin 2011, section 7.5), `w3swldmd.F90`; `&SWL6 SWLB1, CSTB1` set it, and it runs when `SWLB1 > 0` (`SWL6S6`) |
| `IRANGE` | The helper of `w3swldmd.F90` (and a copy in `w3src6md.F90`) that builds the index vector `X0, X0+DX, ...` for array addressing |
| `SWL6CSTB1` | Namelist `CSTB1`: true keeps `SWLB1` as the constant `B1` of `W3SWL6`, false scales it with the steepness at the spectral peak |
| `W3SDS2`, `SDS2` | Whitecapping dissipation of switch `ST2`, the Tolman and Chalikov (1996) source terms (`w3src2md.F90:582`): a diagonal `D` from a turbulent-scale term and a Phillips-constant term, blended by a frequency filter; `&SDS2` sets `SDSA0`, `SDSA1`, `SDSA2`, `SDSB0`, `SDSB1`, `PHIMIN` |
| `FPI` | In `ST2`, the "peak frequency" of the input (rad/s), from `W3SPR2` |
| `HW` | In `W3SDS2`, 4 times the square root of the energy above `XFH*FPI` (a significant wave height of that part), which sets the turbulent length scale |
| ST6 | WW3 switch: observation-based source terms (Donelan et al. 2006, Babanin 2011, Rogers et al. 2012): wind input `W3SIN6`, dissipation `W3SDS6`, mean parameters `W3SPR6` |
| `W3SPR6` | ST6's mean wave parameters (energy, mean frequency, mean wavenumber, maximum) and the peak frequency from Young's weighted integral |
| FLX3 | WW3 switch: Tolman and Chalikov (1996) stress with a cap on the drag coefficient, `W3FLX3`: `CD_MAX` and `CAP_ID` (0 a minimum, 1 a hyperbolic tangent) |
| `EWN1` | `W3DISPMD`'s table of wavenumbers for `SI*SQRT(DEPTH)` on a grid of `DSIE`, built by `DISTAB`, which the inlined `WAVNU1` of `W3FLX3` reads |
