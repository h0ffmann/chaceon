# ADR-0004: Rust's floats follow Fortran's kinds

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

The owner, 2026-10-30: "F64 vs F32 repres. in Rust must be documented". WW3 is an `f32` model with
`f64` in chosen places, some on the device: `W3SDB1` (`w3sdb1md.F90:177-178`) and `W3SRCE`
(`w3srcemd.F90:707`) (v). Mixed-kind Fortran does not read like mixed-kind Rust: `d = a*b` multiplies
in `f32` and widens after, which differs from `(a as f64) * (b as f64)` in the bits (v, run with
gfortran 14.3.0).

## Decision

`Real` is `f32`, `Dbl` is `f64`. Kernels name those aliases and are not generic. A value changes kind
exactly where gfortran changes it. `f64` is never demoted to fit a device: a kernel that needs
`Float64` stops where it is missing. The details and traps are
[`FLOAT_REPRESENTATION_202610.md`](../FLOAT_REPRESENTATION_202610.md).

## Consequences

The port does not gain a whole-model `f64` build. Apple GPUs cannot run the target switch's `DB1`
path ⚠. A change of kind in a kernel is an improvement, so it follows "Translate, don't improve".
