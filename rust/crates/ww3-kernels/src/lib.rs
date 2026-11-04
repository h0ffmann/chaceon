// SPDX-License-Identifier: MPL-2.0
//! WW3's numerics. Every routine is a translation of the Fortran at the `WW3/` pin; the rules are
//! `AGENTS.md`, "Translate, don't improve", and `docs/FLOAT_REPRESENTATION_202610.md`.
#![no_std]

#[macro_use]
mod util;

pub use util::nint;

pub mod constants;
pub mod flx1;
pub mod pro3;
pub mod sbt4;
pub mod real;
pub mod sds3;
pub mod sds1;
pub mod sds4;
pub mod sin3;
pub mod sic1;
pub mod sin1;
pub mod sin4;
pub mod sis1;
pub mod snl1;
pub mod spr3;
pub mod spr1;
pub mod srce;
pub mod swl6;
pub mod sds2;
pub mod spr6;
pub mod flx3;
