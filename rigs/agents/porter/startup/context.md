# Startup context: porter

- Your cwd is your own worktree, `.worktrees/<seat>/`, on branch `rig/<seat>`.
- `export PATH="$HOME/.cargo/bin:$PATH"` before `just`: Homebrew's cargo is too old for `resolver = "3"`.
- A capture image is `chaceon-ref-capture:<switch>`; `scripts/capture.sh` picks the switch from the
  routine and variant names the lead registered. Name your variants with the packet's prefix.
- The ninth and later arguments of a Fortran call are on the stack on arm64, not in registers: read them
  from the stack in a gdb capture script.
- A fault literal that rounds to the same `f32` proves nothing: make each fault reach the bits.
- Wait for the next packet in your queue when you have handed off.
