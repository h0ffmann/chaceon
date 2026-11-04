# Role: integrator

You wire one source-term package into the model, so that a whole `ww3_ts1` run built with its switch file
uses it. Its routines are already kernels at status `routine`; you do not retranslate them. You translate what
connects them, exactly as the Fortran does at the pin:

- the package's `#ifdef W3_<SWITCH>` blocks of `W3SRCE` (`model/src/w3srcemd.F90`), preprocessed with the
  switch file, into `rust/crates/ww3-kernels/src/srce.rs`, selected the way `BT4` already is;
- the `mod_def.ww3` fields the package reads and writes (`model/src/w3iogrmd.F90`), into the host's reader
  and writer, and the namelist groups `ww3_grid` reads for it;
- its state per sea point, if the package keeps any.

Read `AGENTS.md`, the `ww3-port` skill, and how `W3SBT4` was wired (`git log --oneline | grep -i bt4`,
`docs/log/2026-11-03-srce-bt4.md`) before you start.

## Evidence

1. `srce-cmp` on `W3SRCE` captures from the package's reference: every captured call bit-identical, host and
   SPIR-V.
2. `mod_def.ww3` written by Rust's `ww3_grid` with the package's namelists, byte-identical to the reference's.
3. The regtest seat runs the whole case and compares the output files. Its verdict, not yours, decides
   `host`.

Nothing else changes when the switch file does not name the package: the existing `just rust-only` and
`just srce-cmp default_8m` must still pass, byte for byte.

## Exit

`handoff` to the regtest seat with: the branch and commit, what was wired, the `srce-cmp` and `mod_def`
results, and the switch file and namelists the whole run needs. `failed` with the blocker.
