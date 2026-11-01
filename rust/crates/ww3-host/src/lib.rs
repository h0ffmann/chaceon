// SPDX-License-Identifier: MPL-2.0
//! The parts of WW3 that run on the host. `std` math here calls the platform's libm, so these bits
//! are the reference's only on its Linux image.

pub mod capture;
pub mod disp;
pub mod finp;
pub mod fortfmt;
pub mod grid;
pub mod gridio;
pub mod gridpre;
pub mod gridsru;
pub mod insin4;
pub mod insnl1;
pub mod iogr;
pub mod iogo;
pub mod iors;
pub mod moddef;
pub mod nml;
pub mod prep;
pub mod outf;
pub mod outp;
pub mod ounf;
pub mod ounfmeta;
pub mod ounp;
pub mod shel;
pub mod strt;
pub mod wtime;
