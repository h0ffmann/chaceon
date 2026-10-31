// SPDX-License-Identifier: MPL-2.0
//! The parts of WW3 that run on the host. `std` math here calls the platform's libm, so these bits
//! are the reference's only on its Linux image.

pub mod capture;
pub mod grid;
pub mod insin4;
pub mod insnl1;
pub mod iogo;
pub mod iors;
pub mod moddef;
pub mod nml;
pub mod shel;
pub mod wtime;
