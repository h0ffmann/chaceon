# ww3_bound: not translated

**Status: not started.** No Rust code exists for `ww3_bound` (`model/src/ww3_bound.F90`, 751 lines at `92e2cf66`).
The only regtest inputs are `WW3/regtests/ww3_tr1/input_bndin/ww3_bound.inp` and `.nml` (`find WW3/regtests -name 'ww3_bound*'`, v);
they were not read further and the reference was not run on them ⚠. It writes the same `nest.ww3` as `ww3_bounc`, from spectra files of WW3's own format;
see `W3IOBC.md` for the file and `WW3_BOUNC.md` for what the sibling needs.
