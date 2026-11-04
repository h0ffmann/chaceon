// SPDX-License-Identifier: MPL-2.0
//! `W3SDS3`, ST3's WAM-cycle 4 whitecapping with Bidlot et al. (2005) mean wavenumber. Translated
//! from `model/src/w3src3md.F90:1253-1433` at metocean/WW3 `92e2cf66` (`rust/port_report/W3SDS3.md`).
//! `CG`, `USTAR`, `USDIR`, `DEPTH`, `IX` and `IY` are arguments no statement reads, so they are not
//! here. The `W3_S`, `W3_T`, `W3_T0` and `W3_T1` blocks are left out: all output.

use crate::constants::TPI;
use crate::real::Real;

/// Scalars of `W3GDATMD` that `W3SDS3` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds3Params {
    pub nk: i32,
    pub nth: i32,
    pub ssdsc1: Real,
    pub ddelta1: Real,
    pub ddelta2: Real,
}

/// The scalar inputs of one call: `EMEAN`, `FMEAN` (`FMEANS` at the call in `W3SRCE`) and `WNMEAN`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds3Point {
    pub emean: Real,
    pub fmean: Real,
    pub wnmean: Real,
}

/// `W3SDS3` on one spectrum. `a`, `s` and `d` start at `o`, `k` (`K(NK)`) at `k_off`. `D` is zeroed
/// first and then written in full, as in the source (`:1378`).
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sds3(
    p: &Sds3Params,
    k: &[Real],
    a: &[Real],
    pt: &Sds3Point,
    s: &mut [Real],
    d: &mut [Real],
    o: usize,
    k_off: usize,
) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let nspec = nk * nth;
    let mut is = 0;
    while is < nspec {
        d[o + is] = 0.;
        is += 1;
    }
    // `MAX(1.E-10, WNMEAN)` (`:1382`).
    let wnmean2 = max_ww3(1.0E-10, pt.wnmean);
    // `WNMEAN**2*EMEAN`, then `SSDSC1 * TPI*FMEAN * ALFAMEAN**2`, left to right (`:1383-1384`).
    let alfamean = (pt.wnmean * pt.wnmean) * pt.emean;
    let factor = ((p.ssdsc1 * TPI) * pt.fmean) * (alfamean * alfamean);
    let mut ik = 1;
    while ik <= nk {
        let kk = k[k_off + ik - 1];
        // `DDELTA1*K(IK)/WNMEAN2 + DDELTA2*(K(IK)/WNMEAN2)**2` (`:1397`).
        let r = kk / wnmean2;
        let factor2 = factor * ((p.ddelta1 * kk) / wnmean2 + p.ddelta2 * (r * r));
        let mut ith = 1;
        while ith <= nth {
            d[o + (ith - 1) + (ik - 1) * nth] = factor2;
            ith += 1;
        }
        ik += 1;
    }
    // `S = D * A` (`:1405`).
    let mut is = 0;
    while is < nspec {
        s[o + is] = d[o + is] * a[o + is];
        is += 1;
    }
}

/// What gfortran's `MAX` compiled to at `:1382`: `fmaxnm s8, s8, s1` with `s8 = WNMEAN`, which
/// returns the other operand when one is NaN (read with gdb `disassemble` on `ww3_shel`, see the
/// port report). A NaN `WNMEAN` gives `1.E-10`; `S` and `D` are NaN through `ALFAMEAN` anyway.
#[inline]
fn max_ww3(x: Real, y: Real) -> Real {
    if y != y || x > y { x } else { y }
}
