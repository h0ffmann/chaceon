# rigs: OpenRig specs for the port loop

[OpenRig](https://github.com/mvschwarz/openrig) runs a team of Claude Code seats from a declarative spec: a
RigSpec (`rig.yaml`) for the team, an AgentSpec (`agent.yaml`) per role, and workflow specs for the steps a
piece of work goes through. Why these rigs, and what two waves of parallel ports taught, is in
[docs/AGENT_LOOP_202611.md](../docs/AGENT_LOOP_202611.md). `AGENTS.md` binds every seat.

| Path | What |
|---|---|
| `chaceon-port/rig.yaml` | lead, four porters, auditor: routines to status `routine` |
| `chaceon-integrate/rig.yaml` | lead, integrator, regtest runner, auditor: packages to `host` |
| `agents/<role>/` | the AgentSpecs: `lead` (profiles `port` and `integrate`), `porter`, `auditor`, `integrator`, `regtest` |
| `workflows/port-routine.yaml` | claim, port, audit, merge, with a triage loop |
| `workflows/wire-package.yaml` | scope, wire, run, audit, merge, with a triage loop |
| `workflows/ship-wave.yaml` | verify, docs, review, human gate, publish |

Scripts the seats call: `scripts/port_backlog.py` (what ww3-gpu ported and chaceon has not),
`scripts/rig_worktrees.sh` (a worktree per seat, `WW3/` at the pin), `scripts/merge_additive.py` (the
appended lists), `scripts/wave_verify.sh` (every gate of a wave). Claims are in `rust/port_claims.tsv`.

## Run a port wave

```bash
npm install -g @openrig/cli && rig setup --providers claude && rig doctor
git switch -c goal/<id>-<slug> main                      # the wave's integration branch
bash scripts/rig_worktrees.sh goal/<id>-<slug> p1 p2 p3 p4 audit
rig spec validate rigs/chaceon-port/rig.yaml
rig up rigs/chaceon-port/rig.yaml --plan                 # then without --plan; never --cwd
rig send orch-lead@chaceon-port "Start a port wave: <package or routines>"
```

The lead claims routines and starts one `chaceon-port-routine` instance per routine. At the end it starts
`chaceon-ship-wave`, which stops at a human gate: `rig queue` shows it, and resolving it lets the lead push
and open the pull request. `rig down chaceon-port --snapshot` stops the team; `rig up chaceon-port` resumes it.

## Run an integration

```bash
bash scripts/rig_worktrees.sh goal/<id>-<slug> wire regtest audit
rig up rigs/chaceon-integrate/rig.yaml
rig send orch-lead@chaceon-integrate "Integrate <package>"
```

Both rigs can run at once on different integration branches; they share the auditor's worktree
`.worktrees/audit`, so give one of them another seat name first if they do.

## Models and permissions

Lead, auditor and integrator run the larger model at high effort, porters and the regtest runner the smaller
one: a port is long and mechanical once the traps are in the brief, and a check is where a second read pays.
Every seat runs Claude Code's auto permission mode (`builtin:auto`); porters, the regtest runner and the
auditor are also denied `git push` and `gh pr`, and those that only read are denied `git commit`.
