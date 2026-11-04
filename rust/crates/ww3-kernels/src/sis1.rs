// SPDX-License-Identifier: MPL-2.0
//! `W3SIS1`, scattering of wave action by sea ice (switch `IS1`). Translated from
//! `model/src/w3sis1md.F90:74-228` at metocean/WW3 `92e2cf66`, with the `W3_S` and `W3_T` blocks
//! (tracing, test output) left out: the capture switch does not define them.
//! Arithmetic as gfortran `-O3 -ffp-contract=off` compiles it: no fusion, true divisions,
//! `ALPHA * A(IS)` then `/ REAL(NTH-1)` for each target bin.
// `s = s - x` and `s = s + x` keep the Fortran line.
#![allow(clippy::assign_op_pattern)]

use crate::real::Real;

/// Scalars of `W3GDATMD` that `W3SIS1` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sis1Params {
    pub nk: i32,
    pub nth: i32,
    /// `DTMIN`, the smallest time step (`w3gdatmd.F90:2311`).
    pub dtmin: Real,
    /// `IS1C1` and `IS1C2`, the scale and offset of the ice concentration (`:2785-2786`).
    pub is1c1: Real,
    pub is1c2: Real,
}

/// The scalar input of one call: `ICE`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sis1Point {
    pub ice: Real,
}

/// `W3SIS1` on one spectrum. `a` and `s` start at `o`: intent `IN` and `OUT`. Returns whether the
/// scattering ran (`ALPHA > 0 .AND. ICE > 0`, `:191`), for reading what a capture reached.
#[inline]
pub fn w3sis1(p: &Sis1Params, a: &[Real], pt: &Sis1Point, s: &mut [Real], o: usize) -> bool {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let ice = pt.ice;
    // :180
    let mut i = 0;
    while i < nk * nth {
        s[o + i] = 0.;
        i += 1;
    }
    // :186. MAX(0., v) is `v > 0 ? v : 0`: a NaN gives 0.
    let v = p.is1c1 * ice + p.is1c2;
    let alpha = if v > 0. { v } else { 0. };
    if alpha > 0. && ice > 0. {
        let n1 = (nth - 1) as Real;
        let mut ik = 0;
        while ik < nk {
            let mut ith = 0;
            while ith < nth {
                let is = ith + ik * nth;
                // `A(IS).GE.0.` is false for NaN.
                if a[o + is] >= 0. {
                    s[o + is] = s[o + is] - alpha * a[o + is];
                    let mut ith2 = 0;
                    while ith2 < nth {
                        let is2 = ith2 + ik * nth;
                        if is2 != is {
                            s[o + is2] = s[o + is2] + alpha * a[o + is] / n1;
                        }
                        ith2 += 1;
                    }
                }
                ith += 1;
            }
            ik += 1;
        }
        // :208
        let mut i = 0;
        while i < nk * nth {
            s[o + i] = s[o + i] / p.dtmin;
            i += 1;
        }
        return true;
    }
    false
}
