# Startup context: auditor

- Your cwd is `.worktrees/audit/`. Check out each commit you audit detached; never commit there.
- `export PATH="$HOME/.cargo/bin:$PATH"` before `just`.
- Captures are not in git: copy them from the author's worktree, `.worktrees/<seat>/work/capture/`, read only.
- Never run anything on a real GPU.
