# An agent loop for the port, on OpenRig

**Status:** proposed, 2026-11-04. The specs are in `rigs/`; no rig has been launched yet (⚠).

The port moves one routine at a time, and each routine needs the same steps: read the Fortran at the pin,
translate, capture from the running reference, replay on the host and in SPIR-V, run faults, write the
ledger row, the report and the log. Two waves of ten parallel agents on 2026-11-04 took Batch 7 from 3 828 to
6 670 Fortran lines at status `routine` (v, `just port-summary`, PRs #18 and #19). This document says what
those waves taught, and how [OpenRig](https://github.com/mvschwarz/openrig) turns them into a loop that runs
without a person driving each step, while every decision `AGENTS.md` reserves for a person stays with one.

## What two waves showed

Measured on the second wave (17 routines of `ST2`, `ST6`, `FLX2` to `FLX4`, ten agents, one per routine,
Apple Silicon with eight cores and 16 GB):

- The wave lasted as long as its largest routine: `W3SIN6` took 8 254 s, `W3FLX3` 5 635 s (v, the agents'
  run times). A wave of independent agents waits for its slowest member.
- Ten builds at once are slow: one agent's first `just spirv` took 30 minutes cold (its report); alone it
  takes minutes ⚠ (not timed here).
- Every merge after the first stopped on conflicts in the same nine or ten files, the lists every port
  appends to (`lib.rs`, `ports.tsv`, `justfile`, `capture.rs`, entry points, `spirv_replay`, `ts1_variants.sh`,
  `rust_in_ref.sh`, `GLOSSARY.md`). A three-way merge that applies each branch's insertions at their anchors
  resolved all of them with no hand edit (v, ten merges); it is now `scripts/merge_additive.py`.
- Every agent found the `WW3/` submodule empty in its worktree and git refusing the file transport a local
  submodule clone needs. Each worked around it differently. `scripts/rig_worktrees.sh` now does it once.
- One shared scratch directory let one agent's fault script copy an old kernel over another agent's
  working copy (no harm reached the merge, v). Each seat now has its own.
- Two agents defined the same variant name differently in the first wave; the parallel agents appended six
  duplicate glossary rows in the second; cargo re-sorted `Cargo.lock` after the merges once.
- The first wave filled the disk and killed Docker mid-build; 29 GB of old outputs had to go.
- Every agent's report needed checking: the coordinator reran every comparison on the merged branch (148
  gates, 0 failed, on the second wave) before writing a number into the README.

## The loop

Resident seats with a queue replace one-shot agents. A porter takes its next packet when its last one merges,
so a slow routine holds one seat, not the wave. An auditor reads each port from the Fortran before it merges,
a read not anchored on the porter's. The lead does what the coordinator did by hand: choosing, claiming,
building reference images, merging, rerunning every gate and writing the documents.

```text
   backlog (scripts/port_backlog.py) ──► lead: claim ──► porter: port ──► auditor: audit ──► lead: merge
                                            ▲               │  ▲              │
                                            │               ▼  └── lead: triage ◄┘ (failed)
                                            │
   wave done ──► lead: wave_verify.sh ──► docs ──► auditor: review ──► HUMAN GATE ──► push, PR
```

Each arrow is a step of an OpenRig workflow (`rigs/workflows/`), carried by a queue packet: the daemon records
who holds each step, and a routine that fails audit goes round again under a hop limit.

## Which rigs now

| Rig | Seats | Takes | Why now |
|---|---|---|---|
| `chaceon-port` | lead, 4 porters, auditor | routines to `routine` | 231 routines h0ffmann/ww3-gpu has ported are not in chaceon yet (v, `python3 scripts/port_backlog.py` at `fe531e1`), most of them Batch 7 and 8 |
| `chaceon-integrate` | lead, integrator, regtest, auditor | packages to `host` | 33 ledger rows of Batch 7 are at `routine` (v, `rust/ports.tsv`) and no model run uses them yet; wiring is serial, since every package edits `srce.rs` |

Four porters, not ten: with resident seats the throughput comes from the queue, not from width, and four
builds at once leave the machine usable. This is a judgment from the numbers above, not a measurement ⚠: run
one wave and compare routines merged per hour.

Not now:

- A device rig (verdict `cuda`). `AGENTS.md` forbids running on a GPU without the owner, so a seat could only
  prepare a run; the run itself stays a person's.
- A Batch 8 rig (unstructured grids, SMC, `ww3_multi`). Most of the backlog is there (`w3profsmd`, `pdlib_*`,
  `w3psmcmd`), but it needs the regtests of
  [UNSTRUCTURED_GRIDS_202610.md](UNSTRUCTURED_GRIDS_202610.md) running in the reference image first. The port
  rig takes those routines once their captures exist.

## What stays with a person

- Every comparison tool and tolerance (ADR-0005): the auditor lists each new one; it never approves one.
- The push and the pull request: the `signoff` step of `chaceon-ship-wave` is a human gate (`human@kernel`).
  Porter and auditor seats are also denied `git push` and `gh pr` in their Claude permissions.
- Any run on a GPU, any ADR, any move of the pin.

## Unchecked ⚠

- The specs follow OpenRig's reference at commit `a3e5e92` (2026-11-04) and pass a check of its documented
  rules (required keys, unknown keys, ids, edges, `agent_ref` paths, profiles, startup files). `rig spec
  validate` and `rig spec preflight` have not run on them, nor `rig workflow validate`.
- The `openrig-user` skill is expected from `rig setup`'s home skill folder, not imported from OpenRig's
  `shared` agent, which a project cannot reference by a relative path.
- A workflow routes a step to a role, not to a seat: a fix after a failed audit could reach another porter,
  whose worktree lacks the work. The lead sends rework to the original seat by name.
