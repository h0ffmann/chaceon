# chaceon-port: team culture

`AGENTS.md` at the repository root is the constitution of this team, and every seat reads it before working.
This file adds only what working as a team of seats needs.

## The work

One routine of WAVEWATCH III at the pin, translated to `no_std` Rust that gives the same bits as the
Fortran reference built by gfortran `-O3 -ffp-contract=off` on arm64. A port is done at status `routine`:
every captured call bit-identical on the host, in call and batch layouts, and in compiled SPIR-V on lavapipe,
with faults run through every guard. Nothing is wired into `W3SRCE` here; `chaceon-integrate` does that.

## Who does what

- The lead chooses, prepares, claims, merges, verifies and ships. It never ports.
- A porter ports the routine of its packet and nothing else, in its own worktree.
- The auditor checks a port from the Fortran and the files before the merge, and never edits it.
- A person reviews every comparison tool (ADR-0005) and resolves the human gate before any push.

## Rules learned in the first two waves

- One seat, one worktree, one scratch directory. A shared temporary directory let one seat's fault script
  overwrite another seat's kernel.
- Shared lists are appended at their end; the lead merges them with `scripts/merge_additive.py`.
- Variant names carry the packet's prefix, so two seats never define the same variant differently.
- A disk below 25 GB free stops the wave: Docker dies mid-build.
- A seat's report is model output: the lead reruns its comparison on the merged branch before writing a
  number into a document.

## When blocked

Name the exact command and what it blocks, tell the lead, and keep working on what does not depend on it.
A permission a seat lacks is a question for a person, never a reason to work around it.
