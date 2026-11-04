# Lead, chaceon-port: one wave

A wave is a set of routines that share reference builds, usually one or two source-term packages. It ends in
one pull request.

## Before the first packet

1. `git switch main && git pull --ff-only`, then `git switch -c goal/<id>-<slug>` (the integration branch).
2. Choose the routines: `python3 scripts/port_backlog.py`, in the plan's batch order, most expensive first.
   Group them by the switch their captures need. Read each routine's `W3SRCE` call site at the pin.
3. Add the switch files (`switches/`, a paragraph in `switches/README.md`) and the routine and variant cases
   in `scripts/capture.sh`, commit, and build the images: `just ref-build switch_chaceon_<pkg>`, one at a
   time, in the background.
4. Claim every routine of the wave in `rust/port_claims.tsv` and commit.
5. `bash scripts/rig_worktrees.sh <integration-branch> p1 p2 p3 p4 audit`.

## Each routine

`rig workflow instantiate rigs/workflows/port-routine.yaml --root-objective "<packet>" --created-by <you>`,
one instance per routine. The packet text is the whole brief the porter gets; `rigs/agents/porter/guidance/`
holds the rest.

When an instance reaches `merge`:

1. `git merge --no-ff -m "Merge <ROUTINE>" rig/<seat>`.
2. On conflicts: `git diff --name-only --diff-filter=U | xargs python3 scripts/merge_additive.py`, then
   `git grep -l '^<<<<<<<' -- docs justfile rust scripts` must print nothing.
3. `bash -n scripts/*.sh`, `just check`; `git add` the merged files and `rust/Cargo.lock` (cargo re-sorts
   it), commit.
4. Copy the seat's `work/capture/<x>_*.bin` into this checkout's `work/capture/`.
5. Delete the routine's claim row. Refresh the seat's worktree for its next packet.

## End of the wave

Instantiate `rigs/workflows/ship-wave.yaml`. Its `verify` step, `bash scripts/wave_verify.sh <x>...`, reruns every gate on the merged branch: the
builds, every new `<x>-cmp` on every capture, `just spirv-replay` on every new capture and on one capture of
each older kernel (the shader module changed), `just srce-cmp default_8m` and `bt4_8m`. Then the docs:
the README state paragraph, `just port-summary`, the plan's batch entry, duplicate rows in
`docs/GLOSSARY.md`. Then the human gate. After a person resolves it: `git push -u origin <branch>` and
`gh pr create`, the body listing what needs a person's review (every new comparison tool, every translation
that follows the compiled code rather than the source) and every ⚠.
