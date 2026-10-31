// SPDX-License-Identifier: MPL-2.0
//! Translated from `model/src/constants.F90:61-82` at metocean/WW3 `92e2cf66`: the `REAL, PARAMETER`
//! scalars. Bits checked against gfortran's folding (`docs/log/2026-10-30-scaffold.md`).
#![allow(clippy::excessive_precision, clippy::approx_constant)]

use crate::real::Real;

pub const GRAV: Real = 9.806;
pub const DWAT: Real = 1000.;
pub const DAIR: Real = 1.225;
pub const NU_AIR: Real = 1.4E-5;
pub const NU_WATER: Real = 1.31E-6;
pub const SED_SG: Real = 2.65;
pub const KAPPA: Real = 0.40;

pub const PI: Real = 3.141592653589793;
pub const TPI: Real = 2.0 * PI;
pub const HPI: Real = 0.5 * PI;
pub const TPIINV: Real = 1. / TPI;
pub const HPIINV: Real = 1. / HPI;
pub const RADE: Real = 180. / PI;
pub const DERA: Real = PI / 180.;

pub const RADIUS: Real = 4.0E7 * TPIINV;

// gfortran folds GRAV**2 and TPI**3 with one rounding each; the products below round more often
// and give the same bits for these two values only.
pub const G2PI3I: Real = 1. / ((GRAV * GRAV) * (TPI * TPI * TPI));
pub const G1PI1I: Real = 1. / (GRAV * TPI);
