// SPDX-License-Identifier: MPL-2.0
//! ST2's interpolation table of the wind-wave interaction parameter, built once on the host:
//! `INPTAB` and its function `W3BETA` (`model/src/w3src2md.F90:847-1216`) at metocean/WW3 `92e2cf66`.
//!
//! The order of operations is the optimised GIMPLE of `inptab` (gfortran 12 `-O3 -ffp-contract=off`,
//! `rust/port_report/W3SIN2.md`): `W3BETA` is inlined, the coefficients that a branch does not use
//! are computed only where it needs them, and the table is `W3BETA * 1.E-4` stored as `REAL`.
//! `A0+A4+A5` is `A5 + (A4 + A0)` there.

#![allow(clippy::assign_op_pattern)]

use ww3_kernels::real::Real;
use ww3_kernels::sin2::{BETATB_LEN, DRAGMX, NRDRAG, NRSIGA, SIGAMX, betatb_at};

/// `W3BETA(OMA, CL)`, `w3src2md.F90:1081-1210`.
pub fn w3beta(oma: Real, cl: Real) -> Real {
    let om1 = 1.075 + 75. * cl;
    let om2 = 1.2 + 300. * cl;
    let a1 = 0.25 + 395. * cl;
    let a2 = 0.35 + 150. * cl;
    let a4 = 0.3 + 300. * cl;
    let a9 = 0.35 + 240. * cl;
    let a10 = -0.06 + 470. * cl;
    let a5 = a4 * om1;
    let a0 = 0.25 * (a5 * a5) / a4;
    let b = if oma < -1. {
        -(a1 * (oma * oma)) - a2
    } else if oma < om1 / 2. {
        let a3 = (a0 - a2 - a1) / (a5 + (a4 + a0));
        let a6 = a0 * (1. - a3);
        a3 * oma * (a4 * oma - a5) - a6
    } else if oma < om1 {
        oma * (a4 * oma - a5)
    } else if oma < om2 {
        let a7 = (a9 * ((om2 - 1.) * (om2 - 1.)) + a10) / (om2 - om1);
        let a8 = a7 * om1;
        a7 * oma - a8
    } else {
        a9 * ((oma - 1.) * (oma - 1.)) + a10
    };
    b * 1.0E-4
}

/// `DSIGA`, `DDRAG` and `BETATB(-NRSIGA:NRSIGA+1,NRDRAG+1)`, column-major as the module holds it.
pub struct Inptab {
    pub dsiga: Real,
    pub ddrag: Real,
    pub betatb: Vec<Real>,
}

/// `INPTAB`, `w3src2md.F90:847-886`.
pub fn inptab() -> Inptab {
    let dsiga = SIGAMX / NRSIGA as Real;
    let ddrag = DRAGMX / NRDRAG as Real;
    let mut betatb = vec![0.; BETATB_LEN];
    for isiga in -NRSIGA..=NRSIGA + 1 {
        let siga = isiga as Real * dsiga;
        for idrag in 1..=NRDRAG + 1 {
            let drag = idrag as Real * ddrag;
            betatb[betatb_at(isiga, idrag)] = w3beta(siga, drag);
        }
    }
    Inptab { dsiga, ddrag, betatb }
}
