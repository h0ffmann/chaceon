# Startup context: integrator

- Your cwd is `.worktrees/wire/`, on branch `rig/wire`.
- `export PATH="$HOME/.cargo/bin:$PATH"` before `just`.
- A known oddity to keep, not fix: with `IS1`, `W3SRCE` stores `W3SIS1`'s result in `VSIR`
  (`model/src/w3srcemd.F90:2127`) but the attenuation reads `VDIR` (`:2155`).
- Never run anything on a real GPU. No Rust tests of any kind.
