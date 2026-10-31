# AGENTS.md

Instructions for any AI coding agent working in chaceon. Read this before writing, building or
measuring anything. This repository is the Rust sibling of `h0ffmann/ww3-gpu`: the same discipline,
a different language and no CI.

## Repo invariants

Non-negotiable here; a skill or task may make these stricter, never looser.

- **Pure Rust.** chaceon is a Rust implementation of WAVEWATCH III® (WW3). It contains no Fortran,
  no C++, no Kokkos and no foreign call into Fortran, and no Fortran runs inside a chaceon process.
  Bindings to system libraries a GPU or a file format needs (the CUDA driver, the Vulkan loader,
  NetCDF) are allowed and listed in `VERSIONS.md` ([ADR-0001](docs/ADRs/ADR-0001-pure-rust-metocean-reference.md)).
- **Full GPU.** The time loop runs on the device, through Rust-CUDA and rust-gpu. The host reads
  inputs and writes files ([plan](docs/RUST_GPU_PLAN_202610.md), "What full GPU means here").
- **metocean/WW3 7.14 is the reference.** `WW3/` is tag `7.14.mos-92e2cf6`, commit `92e2cf66`,
  read-only. It is what a translation copies and what an output is compared with. Never edit it,
  never copy its source into this repository, and move the pin only when the owner asks.
- **No Rust tests.** No `#[test]`, `#[cfg(test)]`, `tests/`, `benches/`, doctest or
  `dev-dependencies`. The evidence is WW3's regtests compared with the reference
  ([ADR-0002](docs/ADRs/ADR-0002-no-ci-no-rust-tests.md)). `just check` refuses them.
- **No CI.** No `.github/`, no workflow, no hosted runner, no bot. The gates below run by hand,
  before a commit.
- **Translate, don't improve.** A translated routine reproduces the Fortran's arithmetic at the
  pin: no refactor, no reordered reduction, no changed limiter, no fused multiply-add. Loop nesting
  changes only where a kernel may not allocate or a device needs it. An improvement is a separate
  change, asked about first, with its own evidence.
- **Floats follow the Fortran's kinds.** `REAL` is `f32`, double precision is `f64`, and a value
  changes kind exactly where gfortran changes it. `f64` is never demoted to fit a device
  ([`docs/FLOAT_REPRESENTATION_202610.md`](docs/FLOAT_REPRESENTATION_202610.md), read before
  translating anything).
- **Bits first, tolerance by exception.** `host` must be byte-identical to the reference. `cuda`
  aims at the same. `spirv` is judged by tolerance. A tolerance is never widened to pass, and a
  comparison tool or tolerance file changes only with a person's review
  ([ADR-0005](docs/ADRs/ADR-0005-three-verdicts.md)).
- **No timing without parity.** A speed figure names the machine and the command, gives the median
  of three runs, and needs a passed comparison on its commit.
- **Write for scientists, carry evidence.** Readers know wave modelling, numerical methods and HPC.
  A claim carries `(v)` with what was checked, or `⚠` when it was not. A number comes with the
  command that reproduces it. Never remove a `⚠` without doing the check.
- **Commits carry `Tested:`** and never a cost.

## Where a change belongs

| Change | Where |
|---|---|
| A translated routine | `rust/crates/`, a row in `rust/ports.tsv`, a report in `rust/port_report/` |
| The GPU entry points | `rust/gpu-cuda/`, `rust/gpu-spirv/` (Batch 0): thin, no physics |
| Running and comparing a regtest | `regtests/`, never `WW3/` |
| A measurement | `bench/results/`, per `bench/README.md` |
| A switch file | `switches/` |
| The Fortran reference build | `docker/Dockerfile.reference` |
| Something tried, a negative result included | a dated entry in `docs/log/`: append, never rewrite |
| A plan or survey | `docs/NAME_YYYYMM.md` |
| A decision several plans follow | an ADR in `docs/ADRs/`, Proposed until the owner accepts it |
| Compound loop artifacts | `docs/compound/plans/`, `docs/compound/solutions/` |
| A pinned version | `VERSIONS.md` |
| A new abbreviation or routine name | `docs/GLOSSARY.md` |

Layout follows ww3-gpu: `kokkos/` became `rust/`. There is no `WW3-noaa/`, because the reference is
`WW3/` itself, and no `picasso/`, proposal, course, book or website.

## Commands and gates

```bash
git clone --recurse-submodules git@github.com:h0ffmann/chaceon.git && cd chaceon
just check        # refuses Rust tests, then cargo check
just port-status  # lines per batch, routines per status
just f64-census   # double precision per WW3 file
just ref-build    # the Fortran reference, linux/arm64 image
```

| You changed | Run |
|---|---|
| `rust/` | `just check`, and for a translated routine `just snl1-cmp`-style replays on every capture |
| `rust/crates/ww3-kernels`, `ww3-libm`, `rust/gpu-*` | also `just spirv`, `just ptx`, `just spirv-replay`; `just libm-bits` for `ww3-libm`, which takes about ten minutes |
| `*.sh`, `justfile` | `just lint-sh` |
| A comparison, a tolerance | stop: a person reviews it |

A missing tool is a reason to install it, not to skip the gate.

## Porting

- **Source.** The Fortran at the pin, read from `WW3/model/src`. Cite `model/src/<file>.F90:<line>`
  at `92e2cf66`, never memory. ww3-gpu's C++ is at NOAA's pin and misses MetOcean's changes: use it
  to find traps, not as a source.
- **Order.** Batches in the plan's order; inside a batch, the measured profile at this pin, most
  expensive first.
- **Reference target.** The executable WW3's CMake builds with gfortran `-O3` and
  `-ffp-contract=off` on Apple Silicon (ADR-0005), not the source read by eye. When `host` differs by a ULP, read the optimised GIMPLE first. The `ww3-port` skill has
  the traps.
- **A difference is a defect of the port** until two runs of the Fortran itself differ on that file.
- **Kernels.** `#![no_std]`, no allocation, no I/O, no panic path that depends on data, the same
  source for all three targets. Nothing physical lives in `gpu-cuda` or `gpu-spirv`.
- **Rust-CUDA settings are part of the physics:** `fma_contraction(false)`, `override_libm(false)`,
  `ftz(false)`, `fast_sqrt(false)`, `fast_div(false)`. The defaults of the first two are wrong for us.
  The H100 and RTX 4090 need its LLVM 7 backend: LLVM 21 output is accepted for sm_100 and later
  only. Assemble with `ptxas --fmad=false` and load the binary, as `just ptx` does (ADR-0007).
- **Evidence before a whole model runs** is a capture of the routine from the running reference, a
  replay on the host in call and batch layouts and on the compiled SPIR-V, and a fault run through
  the comparison (ADR-0007). Read which branches the captures reached, and add namelist variants for
  the rest. It gives the status `routine`, never `host`, `cuda` or `spirv`.
- **Every routine** has a row in `rust/ports.tsv` and a report in `rust/port_report/<ROUTINE>.md`:
  what was translated, its comparison, its timing. `translated` never means `host`.
- **Licence.** Every file, translated or not, starts with `// SPDX-License-Identifier: MPL-2.0`
  ([ADR-0006](docs/ADRs/ADR-0006-licence.md)).

## Running on the GPU

Never run on a device without the owner supervising that run. Compiling for a device is allowed,
and so is running SPIR-V on a CPU Vulkan implementation: `spirv_replay` refuses any other device.
The verdict an agent can reach alone is `host`. This is ww3-gpu's rule, carried over.

## Third-party issues

An issue, pull request or discussion of a repository the project does not own is never cited,
linked or named by number here, because GitHub backlinks it. Cite what it describes: the code at a
commit, a release, a paper. `h0ffmann/*` and `marola-dev/*` are cited as usual.

## Writing

Plain and direct. No em dash in prose. Expand each abbreviation in `docs/GLOSSARY.md` at first use.
Comments say why or name a trap; they never restate the code. No model name or session link in
code, comments, PR titles or documents.

## Engineering work

Size the loop to the change, as ww3-gpu does: a small fix is work, gates, PR; a routine or a tool
is `/ce-plan`, `/ce-work`, `/ce-code-review`; a batch is `/ce-brainstorm` first. Artifacts go under
`docs/compound/`. `/ce-simplify-code` never touches translated code, and "verify" never means a new
test.

## Commits and pull requests

- A subject, a paragraph saying what and why, and a `Tested:` trailer naming the commands and their
  result. Agent commits also carry the harness's `Co-Authored-By`.
- An agent's branch is `goal/<id>-<short-kebab-slug>`: the issue number, or the last 8 characters
  of the session id plus a random 4-digit number. Never `claude/...`.
- The default branch takes changes by pull request. There is no CI to wait for, so the `Tested:`
  trailer is the only record that the gates ran.
- Never rewrite history on someone else's branch.

## When something here is wrong

Update this file in the same change, along with the skill or document that repeats the rule.
