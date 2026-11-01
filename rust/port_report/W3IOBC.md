# W3IOBC: boundary data files

`rust/crates/ww3-host/src/iobc.rs` translates `W3IOBC` with `INXOUT = 'WRITE'` (`model/src/w3iobcmd.F90:186-267`,
`:317-335`, `:414-440` at `92e2cf66`): the files `nest01.ww3`, `nest02.ww3` ... that `ww3_shel` writes at the times of
`DATE%BOUNDARY` for a grid with `OUTBND` points. `shel.rs` has a fourth output stream for it (`J` = 5,
`FLOUT(5) = FLBPO`, `w3initmd.F90:1099`). Host only; no kernel and no SPIR-V file changed.

**Status: `translated`**, not `host`: `READ` and `DUMP` are not translated. Nothing ran on a GPU ⚠.

## Result

`ww3_tp2.3` with two `OUTBND` lines (5 points and 4 points, appended to the regtest's `ww3_grid.nml`; the regtest has
none) and `DATE%BOUNDARY = '19680606 000000' '43200' '19680611 000000'`: `nest01.ww3` (80 142 bytes) and
`nest02.ww3` are byte-identical to the reference's, with `out_grd.ww3` and `restart001.ww3` still identical (v,
`just bnd-all`, `scripts/bnd_pair.sh`). The reference's `nest01.ww3` has SHA-256 `0440b0c9b9db9165...` on two runs (v).

| Fault | `nest01.ww3` |
|---|---|
| a factor 1.0000001 on `ABPOS` | differs |
| `VA*SIG2/CG` to `VA*(SIG2/CG)` | identical, not caught ⚠ (the garden-sprinkler spectra have few non-zero bins) |

## What stops

- A `nest.ww3` in the run directory with `FLBPI` set: `ww3_shel` stops. The boundary spectra enter in `W3XYP3`'s
  sub-step loop (`w3pro3md.F90:1337-1360`, step 3.c): the value at each boundary point is replaced after both sweeps of
  every `ITLOC`, and the next sweep reads it. That loop is in `ww3-kernels` `w3xyp3`, which is shared with the SPIR-V
  build, and this change leaves kernels alone. A host-side replacement after the call is exact only when every
  component has `NTLOC` 1, which a regtest like `ww3_tp1.11` (`DTXY` 80 against `DTMAX` 400) does not give.
- `READ` (with `W3CSPC` for a spectrum on another grid), `W3UBPT` (`w3updtmd.F90:1313-1502`) and `DUMP` (`ww3_multi`
  only) are not translated.
- `W3IOBC` `WRITE` under `PDLIB`, `SMC`, `RTD` and `DIST`: not in this switch.

## Timing

None.
