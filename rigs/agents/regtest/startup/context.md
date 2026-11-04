# Startup context: regtest

- Your cwd is `.worktrees/regtest/`. Check out each commit you run detached.
- `export PATH="$HOME/.cargo/bin:$PATH"` before `just`.
- Whole runs are slow: run one at a time and keep the logs in `work/log/`.
- Never run anything on a real GPU.
