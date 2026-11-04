# Startup context: chaceon

- Repository root is your cwd. Seats work in `.worktrees/<seat>/` (`scripts/rig_worktrees.sh`), each on
  branch `rig/<seat>`. `work/` is ignored by git and not shared between worktrees.
- Put `$HOME/.cargo/bin` first in `PATH` for `just`: Homebrew's cargo is too old for `resolver = "3"`.
- Ledger: `rust/ports.tsv`. Claims: `rust/port_claims.tsv`. Backlog: `python3 scripts/port_backlog.py`
  (routines h0ffmann/ww3-gpu ported and chaceon has not; set `WW3_GPU_DIR` if its clone is not beside this one).
- Status ladder: translated, routine, host, cuda, spirv. A packet of `chaceon-port` ends at `routine`
  (ADR-0007); a packet of `chaceon-integrate` aims at `host` (ADR-0005).
- Gates: `just check`, `just lint-sh`, `just spirv`, `just ptx`, the routine's `<x>-cmp` on every capture,
  `just spirv-replay`, fault runs. `just srce-cmp default_8m` and `bt4_8m` after a kernel change.
- Reference images: `chaceon-ref:<switch>` and `chaceon-ref-capture:<switch>`, built by `just ref-build`.
  `docker images | grep chaceon` lists them. Keep 25 GB free: a full disk kills Docker mid-build.
- Commits: subject, a what-and-why paragraph, `Tested:` naming commands and results, and the harness's
  `Co-Authored-By` trailer. Branches `goal/<id>-<slug>`, never `claude/...`.
