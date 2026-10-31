// SPDX-License-Identifier: MPL-2.0
//! The two float kinds of WW3, named so a translated line says which Fortran kind it holds.
//! Not generic: WW3's gfortran build has no precision switch (`docs/ADRs/ADR-0004`).

/// Fortran default `REAL`: 4 bytes in every build at the pin.
pub type Real = f32;
/// Fortran `DOUBLE PRECISION`, `REAL*8`, `REAL(KIND=8)`.
pub type Dbl = f64;
