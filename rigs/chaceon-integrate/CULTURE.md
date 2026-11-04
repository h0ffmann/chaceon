# chaceon-integrate: team culture

`AGENTS.md` at the repository root is the constitution of this team, and every seat reads it before working.

## The work

One source-term package at a time, from routines at status `routine` to a whole model run at `host`: Rust's
`ww3_grid`, `ww3_strt` and `ww3_shel`, built with the package's switch file, give the reference's output files
byte for byte. The arithmetic of the routines is already proven; what is new here is the glue: the switch
blocks of `W3SRCE`, the `mod_def.ww3` fields, the namelists, the state per sea point.

## Who does what

- The lead picks the package, merges, reruns every gate and ships through the human gate.
- The integrator wires the package and proves `W3SRCE` with captures.
- The regtest seat runs whole cases and gives the file verdict; it never edits the integration.
- The auditor checks the wiring from the Fortran, preprocessed with the switch file.

## Rules

- One package in flight: every package edits `srce.rs` and the `mod_def.ww3` reader.
- A build without the package must not change a byte: `just rust-only` and `just srce-cmp default_8m` pass
  before and after.
- A verdict needs a run where the package acted: count the calls.
- Keep the Fortran's oddities, and mark them ⚠ in the report.
