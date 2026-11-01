# 2026-11-01: `ww3_prep` byte-identical on 14 regtest inputs, and `ww3_tp1.6` with no Fortran

2026-11-01, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-2719-ww3-prep`.
Results: `rust/port_report/WW3_PREP.md`.

## Question

`ww3_tp1.6` was the one case of the Rust-only chain that still copied a Fortran-written input, `current.ww3`
from `ww3_prep`. Can Rust write it, bit for bit, and which other `ww3_prep` inputs of the regtests can it write?

## Result

Yes for the input type `AI` (a field already on the model grid), every field id and free-format or formatted
input. 14 forcing files from 6 regtests are byte-identical, 406 220 bytes (v, `just prep-all`); 12 more files from
inputs built for the other field ids, layouts and formats are byte-identical, 215 484 bytes (v,
`just prep-variants`). `scripts/rust_only.sh` now runs Rust's `ww3_prep` and compares `current.ww3` too:
`bash scripts/rust_only.sh ww3_tp1.6 ww3_tp1.6` gives six files byte-identical, with no Fortran program in the run
(v). `bash scripts/rust_only_all.sh` after the change: 17 cases, 81 files, 805 162 628 bytes, 0 differ (v; it was 80 files and 805 162 020 bytes before, and the difference is `current.ww3`). The 14 files agreed at the first full sweep once the reading of the input was right, so seven faults went
through the pair before the zeros were believed: seven caught, then restored (v, the report has the table).

## What it took

- The comparison uses the reference's `mod_def.ww3`, so a case whose `ww3_grid` needs a switch Rust's lacks
  (`BT8`, `IC2`) can still be judged on `ww3_prep`. `ww3_tbt1.1`, `ww3_tbt2.1`, `ww3_tic2.2`, `ww3_tic2.3` and
  `ww3_ts3` all ran this way, and `ww3_grid` of the reference accepted every one of their namelists.
- The program has almost no arithmetic for `AI`. The work is in the reads: a list-directed `READ` takes items
  from the next records until it has enough and drops the rest of the last record; `NDSF` equal to the input unit
  puts the data in `ww3_prep.inp` itself; a formatted `F8.2` field with no decimal point takes the last two
  digits as the fraction; a blank field is zero.
- A first sweep failed on 7 of the 14 inputs because a `NEXTLN` was missing before the `NDSF` line; the
  inputs with a comment line there stopped with an error where the Fortran went on.
- `RHO` reads two fields and writes the zeros of the unread third: the Fortran does that, and Rust does too.
- Two traps of the harness: the host awk wrote `3,00` for `3.00` under the Portuguese locale, and the
  Fortran accepted that input (status 0). The reason is likely that `libgfortran` ends a numeric field at a
  comma, so `3` is the value and `00` begins the next field ⚠ (not probed). Rust refuses a comma in a numeric field. The variant builder sets `LC_ALL=C`.

## What did not get done

- `LL`, `F1`, `F2`, `DAT`, binary input and a format that reverses are refused with status 64. `LL` needs
  `W3GSUC` and `W3GRMP` for the model grid (`gridsru.rs` has the `REAL(8)` versions `ww3_grid` uses) and `F1`
  and `F2` need `W3FLDP`; none is in a single-grid regtest input. The `mww3_test_*` inputs need `ww3_multi`.
- The `IDLA` 3 fault is not caught by `ww3_tp1.6`, whose field is uniform in `y`; `ww3_tic2.2` catches it.
- No timing was taken. Nothing ran on a GPU.
