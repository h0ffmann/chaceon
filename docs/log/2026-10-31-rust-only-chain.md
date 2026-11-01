# 2026-10-31: a run with no Fortran program

2026-10-31, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-7704-no-fortran-run`.

## Question

Rust had `ww3_grid`, `ww3_strt` and `ww3_shel`, each judged with the Fortran's files as input. Does the chain hold
when each program reads what the Rust program before it wrote?

## Result

Yes on 17 cases: `ww3_ts1` at its default, `ww3_tp1.1` to `ww3_tp1.10`, `ww3_tp2.1` to `ww3_tp2.4`, the curvilinear
`ww3_tp2.4` and `ww3_tp2.15`. All 80 files compared, 805 162 020 bytes, are identical to the reference's (v,
`just rust-only`, after `just tp-all` and `just ts1-pair default 2500`). `ww3_tp1.4` and `ww3_tp2.15` are also identical with the
third step as SPIR-V on lavapipe (v, `RUST_ONLY_SPIRV=1 bash scripts/rust_only.sh <case> <tag>`).

## How it is judged

`scripts/rust_only.sh` starts from an empty directory, the regtest's `input/` and its namelists. `ww3_shel.nml` and
`points.list` are taken from the prepared case because `tp_pair.sh` writes them. Nothing Fortran wrote enters except
`current.ww3`, on `ww3_tp1.6`, the one case that has it: `ww3_prep` is not ported.

**Fault.** Changing `DEPTH%SF` from -2500 to -2400 in the `ww3_grid.nml` of `ww3_tp1.1` and running the same three
programs changes 720 bytes of `mod_def.ww3` and 1 050 of `out_pnt.ww3`; `out_grd.ww3` and both restarts do not change
(v, by hand on a copy of the case; 2026-10-31). So the field reached the model through Rust's own `mod_def.ww3`.

## Whole verification on the integrated branch

MPI, `ww3_grid` and Lean on one branch, on one run, 0 files different (v, 2026-10-31): `just tp-all`; `ww3_ts1` on 18
variants; the 17 cases above; `spirv_pair.sh` on 18 captures; `mpi_pair.sh` on `ww3_tp1.4` and `ww3_tp2.15`;
`grid_pair.sh` on `ww3_tp2.2` and `ww3_tp2.15`. SPIR-V and PTX rebuilt first.

## Limits

⚠ Nothing ran on a real GPU. ⚠ No NetCDF step, so Batch 5's exit is not met. ⚠ Input fields other than `current.ww3`
were not needed by these cases, so `wind.ww3` and the like from a Rust preprocessor remain untested.
