# 2026-11-01: partition fields and boundary output files byte-identical; ww3_bounc crashes in the reference

**Done.** `W3PART` and its helpers, `W3CPRT`, the group 4 fields of `out_grd.ww3` and the `WRITE` of `W3IOBC`
(`nestNN.ww3`) are in `ww3-host`. `just bnd-all` (v, linux/arm64 image): `out_grd.ww3`, `out_pnt.ww3` and `restart001.ww3`
byte-identical on `ww3_tpt1.1` `PTM1` to `PTM5` and `ww3_tp2.15` with all 18 partition fields, and `nest01.ww3`,
`nest02.ww3` on `ww3_tp2.3` with two boundary-output lines. Details: `rust/port_report/W3PART.md`, `W3IOBC.md`.

**Shared pair directory.** `work/pair` is shared with other worktrees, so the regression of `ww3_shel` was run on copies:
`just bnd-regress` (89 of 91 files identical; the two that differ are `ww3_tp2.6`, an unstructured grid that `moddef.rs`
refuses and never ran, so not a change of this work). The suites `tp_all`, `ts1_pair` and `rust_only_all` were not rerun
because they write `work/pair` and `work/rustonly`; `ww3_grid`, `ww3_strt` and `ww3_prep` were not touched ⚠.

**Negative results.**

- The reference `ww3_bounc` segfaults on a Cartesian grid (`ww3_bounc.F90:556`, unallocated `LATS`/`LONS`), so
  `ww3_tp1.11` has no reference `nest.ww3` at this pin. The regtest's spectra come as a `.cdl` that `run_test` builds with
  `ncgen -b`; `ww3_tp2.8` and `ww3_tp2.17` name `.nc` files that are not in the copy.
- Boundary input in a run (`nest.ww3` read, applied in `W3XYP3`) is not done: it needs a change to the shared kernel.
- `ww3_bounc`, `ww3_bound`, `W3IOBC` `READ`, `W3UBPT`, `W3CSPC`, `ww3_outp` `ITYPE` 4 and `W3IOSF` were not translated.
- Two faults were not caught (`TPIINV**3` rounded twice, `<=` to `<` in the fill, `VA*(SIG2/CG)`), listed in the reports.
