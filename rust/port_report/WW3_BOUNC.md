# ww3_bounc: not translated

**Status: not started.** No Rust code exists for `ww3_bounc` (`model/src/ww3_bounc.F90`, 1 005 lines at `92e2cf66`).
What was learned from the reference is recorded here.

## What the reference does at this pin (v, `chaceon-ref-netcdf:switch_chaceon_shrd`)

- `ww3_tp1.11` (`input` and `input2`): the regtest lists `ww3.Point0_196806_spec.nc`, which is not in the copy, but a
  `ww3.Point0_196806_spec.cdl` is. `run_test` builds it with `ncgen -b` in the work directory (`bin/run_test:471-477`);
  `ncgen -b` of that file works here. **The reference `ww3_bounc` then crashes**: `SIGSEGV` at `ww3_bounc.F90:556`, in
  the `NF90_GET_VAR(..., LATS(IP))` of the Cartesian branch (`FLAGLL` false). `LATS` and `LONS` are never allocated
  (`:454` is commented out), and the branch after it uses `LATSTMP`, also unallocated. The grid of `ww3_tp1.11` is
  `CART`, so the reference exits 139 and leaves an empty `nest.ww3`. A Rust port would have to refuse this input:
  status 64 cannot equal the reference's 139.
- `ww3_tp2.8` (spherical, `RECT` 103 by 119, 6 boundary points): the five `ww3.W0052N489_20080310_spec.nc`-style files of
  its `spec.list` are not in the copy and there is no `.cdl`. `ww3_tp2.8` has a `ww3_bounc.inp` and `.nml`.
- `ww3_tp2.17`: `spec.list` names `../input/bound.nc`, absent; the test runs with `MPI` and `PDLIB` switches, which this
  reference binary does not have ⚠ (not read further).
- No download was made, and no input was written under `regtests/bound_inputs/`.

## What it needs

NetCDF reads (`ncgen`-style variables `time`, `frequency`, `direction`, `latitude`/`longitude` or `x`/`y`, `efth`),
`W3CSPC` for a spectrum on another grid, `U2D`, `D2J`, `J2D`, `D2T`, the big-endian `nest.ww3` writer of `W3IOBC.md`
and a spherical input of one's own (a `.cdl` for `ww3_tp2.8`'s 103 by 119 grid with spectra near its boundary points).
`MINLOC(..., MASK=...)` picks the two nearest stations.
