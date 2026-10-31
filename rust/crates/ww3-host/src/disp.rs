// SPDX-License-Identifier: MPL-2.0
//! `DISTAB` and `WAVNU1` (`model/src/w3dispmd.F90:551-682`, `:84-201`).

use crate::insin4::wavnu2_cg;
use ww3_kernels::constants::GRAV;
use ww3_kernels::real::Real;
use ww3_libm::sqrtf;

const NAR1D: usize = 121;
const DFAC: Real = 6.;

/// `DISTAB`'s tables and `WAVNU1`.
pub struct Dispersion {
    dsie: Real,
    n1max: i32,
    ewn1: Vec<Real>,
    ecg1: Vec<Real>,
}

impl Dispersion {
    pub fn new() -> Self {
        let n1max = NAR1D as i32 - 1;
        let simax = sqrtf(GRAV * DFAC);
        let dsie = simax / n1max as Real;
        let (mut ewn1, mut ecg1) = (vec![0.; NAR1D + 1], vec![0.; NAR1D + 1]);
        ecg1[0] = sqrtf(GRAV);
        for i in 1..=n1max + 1 {
            let (k, cg) = wavnu2_cg(i as Real * dsie, 1., 1.0e-7, 15);
            ewn1[i as usize] = k;
            ecg1[i as usize] = cg;
        }
        Self { dsie, n1max, ewn1, ecg1 }
    }

    pub fn wavnu1(&self, si: Real, h: Real) -> (Real, Real) {
        let sqrth = sqrtf(h);
        let six = si * sqrth;
        let i1 = (six / self.dsie) as i32;
        if i1 <= self.n1max && i1 >= 1 {
            let i2 = i1 + 1;
            let r1 = six / self.dsie - i1 as Real;
            let r2 = 1. - r1;
            let (a, b) = (i1 as usize, i2 as usize);
            ((r2 * self.ewn1[a] + r1 * self.ewn1[b]) / h, (r2 * self.ecg1[a] + r1 * self.ecg1[b]) * sqrth)
        } else {
            (si * si / GRAV, 0.5 * GRAV / si)
        }
    }
}
