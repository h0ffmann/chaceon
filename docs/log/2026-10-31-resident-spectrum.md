# The whole model core on one resident spectrum

2026-10-31, agent session `cd86e8f5` for the owner, who said "merged go next"; no issue. Branch
`goal/cd86e8f5-8124-resident-spectrum`. Results: `rust/port_report/W3PRO3.md`.

## Question

Does `W3SRCE` keep its bits chained in the time loop as SPIR-V, next to propagation, with no per-step copy of `VA`?

## Result

Yes on lavapipe, on 20 runs: `out_grd.ww3`, `out_pnt.ww3` and the restart are byte-identical (v,
`scripts/spirv_pair.sh`). `w3xyp3_va` holds `W3GATH` and `W3SCAT`, and the CPU path uses it too. `VA` moves only at
the start and at outputs, and the tool prints the bytes.

## What went wrong on the way

- **A script edited while it ran.** `tp_pair.sh` was changed during `tp_all.sh`, bash read the new bytes at the old
  offset, and one `ww3_tp1.6` CPU run ended in a syntax error. No output differed; only that run was missing, and
  the exit status of 1 is what showed it, since no `DIFFERENT` line appeared. Do not edit a running script.
- **`--threads` and `ww3_strt` came from two delegated tasks**, each with its own worktree; they were read,
  integrated and verified again here.

## What it changes

A device run needs no new copy, and `--threads` and `ww3_strt` are in. Next: MPI, `W3ULEV` and `W3UICE`, `wind.ww3`, a
real device.
