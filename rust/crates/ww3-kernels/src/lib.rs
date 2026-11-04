// SPDX-License-Identifier: MPL-2.0
//! WW3's numerics. Every routine is a translation of the Fortran at the `WW3/` pin; the rules are
//! `AGENTS.md`, "Translate, don't improve", and `docs/FLOAT_REPRESENTATION_202610.md`.
#![no_std]

#[macro_use]
mod util;

pub use util::nint;

pub mod constants;
pub mod pro3;
pub mod sbt4;
pub mod real;
pub mod sds4;
pub mod sin4;
pub mod sis1;
pub mod snl1;
pub mod srce;
