# ADR-0001: Pure Rust; metocean/WW3 7.14 is the reference

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

The owner, 2026-10-30, verbatim: "This repository (and AGENTS.md) must be a PURE rust port of ww3 in
full GPU using rust-gpu and rust-cuda." and "Idea is to start to have Rust routines instead of
Kokkos." and "Keep metocean 7.14 version as reference to compare ww3 fortran vs chaceon Rust".

## Decision

chaceon holds no Fortran, C++ or Kokkos, and calls no Fortran. `WW3/` is a read-only submodule of
metocean/WW3 at `92e2cf66`. Bindings to system libraries are allowed.

## Consequences

ww3-gpu compares two routines inside one executable, chosen at run time. chaceon cannot: reference
and port are two executables, so a build difference can look like physics. ADR-0005 answers that.
Until Batch 5, chaceon reads files written by the Fortran's preprocessors. That is data, not Fortran
in the process.
