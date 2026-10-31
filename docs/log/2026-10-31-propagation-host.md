# Propagation on the host: 12 regtests

2026-10-31, agent session `cd86e8f5` for the owner, who asked to "convert more"; no issue. Branch
`goal/cd86e8f5-5174-propagation`. Results: `rust/port_report/W3PRO3.md`.

## Question

Does `W3XYP3`, `W3KTP3` and the `UQ` scheme in Rust reproduce the reference when every sea point's spectrum
is compared, not only output points?

## Result

Yes on 12 regtests: 20 files, 43 062 528 bytes, 0 differ (v, `just tp-all`). The comparison adds a restart at
the last step, since `out_grd.ww3` is not ported and a restart holds the whole state.

## What went wrong on the way

- **`CLATS` is `(0:NSEA)`.** It was read from 1, so each sea point had its neighbour's latitude. `ww3_tp1.1`
  matched byte for byte with this bug, on the equator. `ww3_tp1.2` differed by about 0.1%. Running only the
  first case would have shipped it.
- **A wrong record length misled the first reading.** `NSPEC` was taken as 42 when decoding the restart, and
  the difference looked like a tail at the limiter. `NSPEC` is 12 there.
- **`ICLOSE_NONE` is -1**, not 1, and `MAPSF(:,3)` is filled on read, not stored.
- **`GETPQR`'s rounding reaches `PTIFAC`:** `ww3_tp2.4` differed by one ulp in the point weights until the
  Newton step was translated.
- **A point on a node belongs to the first enclosing cell:** on `ww3_tp1.7` the other cell is all land, and
  Rust dropped a point the reference keeps.
- A fault loop run from zsh tested nothing, since the case list did not split. It printed 0 differing
  bytes, and only a missing-file error showed it.

## What it changes

Batch 3 has its propagation on the host. Next: currents, `W3OUTG`, wind fields, obstructions, `ww3_tp2.15`, and
the kernels dispatched in parallel on lavapipe, which is the step that moves this toward a GPU.
