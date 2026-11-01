# 2026-11-01: `ww3_ounf` byte-identical on 36 regtests

2026-11-01, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-3056-ww3-ounf`.
Results: `rust/port_report/WW3_OUNF.md`.

## Question

`ww3_ounf` turns `out_grd.ww3` into the NetCDF files a user reads, and it is the first step of this port that needs a
file format library. Can Rust write the same files, and is `cmp` a fair judge of a NetCDF file?

## Result

Yes, and `cmp` is fair. The reference's `ww3_ounf` writes the same bytes twice: 43 files from 15 inputs, two runs two
seconds apart, classic and HDF5 formats, 0 differences (v, `just ounf-selfcmp`). So the verdict is `cmp` of the whole
file and nothing is excluded; no `scripts/ncbits.sh` was written, because a comparison of contents is only needed when
two runs of the Fortran differ.

Rust's `ww3_ounf` is byte-identical on 36 cases, 40 files, 14 201 708 bytes (v, `just ounf-all`), on 168 namelist-variant
runs, 356 files, 145 368 465 bytes (v, `just ounf-variants`), and on 45 runs over every field the grids can hold, 706 files, 456 543 832 bytes (v,
`scripts/ounf_extra_all.sh`). Five faults (reading `out_grd.ww3`, a scale factor, an attribute, the rounding to `short`, the deflate level)
were each caught and then restored (v).
One case, `ww3_tp2.6`, is refused: its grid is unstructured.

## What it took

- The binding is 19 hand-written `extern "C"` declarations, behind a Cargo feature that is off by default, so no crate,
  no bindgen and no libnetcdf on a host that only runs `cargo check`. The build image (`docker/Dockerfile.netcdf`) has
  the same `libnetcdf19` 4.9.0-3+b1 and HDF5 1.10.8 as the reference.
- The first sweep of 37 cases matched at once except one: `P2L` at `FIELD%TYPE` 3 has `valid_max = NINT(1.E12/0.0004)`,
  which the reference's aarch64 build saturates to 2 147 483 647 and `ww3_kernels::nint` wraps. `exnc.rs` has its own.
- `TIMESPLIT` 8 and 10 showed that the reference ignores the split. `SINGLEFILE` is read only from the `.inp` input; the
  namelist path leaves it as the stack had it, and `w3exnc` reads it as true (a load at `0xa944` of the installed
  binary). Rust sets it true ⚠. A file left from an earlier run then stops the reference (NetCDF: Not a valid ID), and
  Rust stops with the same message.
- Running every field exposed two more WW3 behaviours: `WN(IK,:)` starts at index 0, so the wavenumber field is
  shifted by one sea point; and `SNAMEP` is 15 characters, so partition 4 is named `quaternary_swel`.
- The program log, which was not asked for, is identical too, once `W3FLDTOIJ`'s switch-dependent names (`USF`, `USP`,
  `D50`, `IC1`, ...) follow the switch.

## What did not get done

- `ww3_ounf.inp` without the namelist, `ounfmeta.inp`, `NC_globatt.inp`, forecast variables, a calendar other than
  `standard`, user-defined fields, and unstructured, SMC and rotated-pole grids stop with a message.
- `USP` (the `USSP_WN` frequencies) was never in a grid; `USF` is in `ww3_tp1.8` only ⚠.
- `SINGLEFILE` and the unset element `WN(IK,0)` are values the Fortran never wrote; the reference had `true` and `0` in
  every run here ⚠.
- A pure Rust NetCDF writer was not tried: the plan allowed the binding, and the files are libnetcdf's own layout ⚠.
- The sweeps took about two hours, with other jobs sharing Docker; no timing was taken.

## What it changes

A Rust run can now end in the NetCDF files of `RECT` and `CURV` grids without the Fortran `ww3_ounf`; `ww3_prep` is
still Fortran where a case has a current file. Batch 4 still lacks `ww3_ounp`, `ww3_outf`, `ww3_outp` and the point and
boundary I/O.
