// SPDX-License-Identifier: MPL-2.0
//! `W3SDS1`, the WAM cycle 3 whitecapping source term (switch `ST1`). Translated from
//! `model/src/w3src1md.F90:432-586` at metocean/WW3 `92e2cf66`. The operation order is the one the
//! source writes: `FACTOR = SDSC1 * FMEAN * WNMEAN**3 * EMEAN**2` is left to right with the powers
//! as products, `(WNMEAN*WNMEAN)*WNMEAN` and `EMEAN*EMEAN` (`rust/port_report/W3SDS1.md`).

use crate::real::Real;

/// Scalars of `W3GDATMD` that `W3SDS1` reads: `NK`, `NTH` and `SDSC1`
/// (`w3gridmd.F90:2140`, `TPI * CDIS / APM**2`). Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds1Params {
    pub nk: i32,
    pub nth: i32,
    pub sdsc1: Real,
}

/// The mean parameters of one spectrum: `EMEAN`, `FMEAN` and `WNMEAN` from `W3SPR1`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds1Point {
    pub emean: Real,
    pub fmean: Real,
    pub wnmean: Real,
}

/// `W3SDS1`, `w3src1md.F90:549-556`, on one spectrum. `a`, `s` and `d` start at `o` and hold `NSPEC`
/// values; `k` (`WN2` of `W3SRCE`, one wavenumber per spectral bin) starts at `k_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sds1(
    p: &Sds1Params,
    pt: &Sds1Point,
    a: &[Real],
    k: &[Real],
    s: &mut [Real],
    d: &mut [Real],
    o: usize,
    k_off: usize,
) {
    let wn = pt.wnmean;
    let factor = p.sdsc1 * pt.fmean * (wn * wn * wn) * (pt.emean * pt.emean);
    let nspec = (p.nk * p.nth) as usize;
    let mut is = 0;
    while is < nspec {
        let dd = factor * k[k_off + is];
        d[o + is] = dd;
        s[o + is] = dd * a[o + is];
        is += 1;
    }
}
