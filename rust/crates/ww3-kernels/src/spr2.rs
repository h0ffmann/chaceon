// SPDX-License-Identifier: MPL-2.0
//! `W3SPR2`, the mean parameters of ST2 (Tolman and Chalikov), translated from
//! `model/src/w3src2md.F90:125-288` at metocean/WW3 `92e2cf66`. Every operation keeps the Fortran's
//! order: the sums run in `IK`, `ITH` order, `EB*DDEN/CG` associates to the left, `2.*DTH*SIG*EB*WN**3`
//! is `((2*DTH)*SIG)*EB` times the product tree `(WN*WN)*WN`, `FPISTR**3` is `(F*F)*F` and the final
//! `**2` is a product. `DEPTH` and `U` are in the argument list and never read (`:211-217`).
// `x = x + y` keeps the Fortran line.
#![allow(clippy::assign_op_pattern)]

use crate::constants::{GRAV, TPIINV};
use crate::real::Real;
use ww3_libm::sqrtf;

/// What `W3SPR2` reads from `W3GDATMD`: `NK`, `NTH`, `DTH`, `FTE`, `FTF` and `FTWN`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr2Params {
    pub nk: i32,
    pub nth: i32,
    pub dth: Real,
    pub fte: Real,
    pub ftf: Real,
    pub ftwn: Real,
}

/// The scalar arguments of one call. `depth` and `u` are never read by the routine; the capture keeps
/// them so the comparison can say which winds and depths it covered.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr2Point {
    pub depth: Real,
    pub fpi: Real,
    pub u: Real,
    pub ustar: Real,
}

/// The five scalars one call returns; `ALFA(NK)` goes to its own buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr2Out {
    pub emean: Real,
    pub fmean: Real,
    pub wnmean: Real,
    pub amax: Real,
    pub fp: Real,
}

/// One spectrum. `sig` is `SIG(0:NK+1)`, `dden` is `DDEN(1:NK)` from index 0 of the slice, `eb` is the
/// Fortran's automatic array `EB(NK)` at `w_off`, and `alfa` is `ALFA(NK)` at `w_off` as well.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3spr2(
    p: &Spr2Params,
    sig: &[Real],
    dden: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    pt: &Spr2Point,
    eb: &mut [Real],
    alfa: &mut [Real],
    spec_off: usize,
    cg_off: usize,
    w_off: usize,
) -> Spr2Out {
    let (nk, nth) = (p.nk, p.nth);
    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }

    let ust = pt.ustar.max(0.0001);
    let (mut emean, mut fmean, mut wnmean, mut amax) = (0., 0., 0., 0_f32);

    // 1.  Integral over directions and maximum
    each!(ik, 1, nk, {
        eb[ix!(w_off, ik)] = 0.;
        each!(ith, 1, nth, {
            let av = a[ix!(spec_off, ith + (ik - 1) * nth)];
            eb[ix!(w_off, ik)] = eb[ix!(w_off, ik)] + av;
            // `MAX(AMAX, A)` is `fmaxnm`: +0 against -0 gives +0, and a NaN `A` is skipped. `f32::max`
            // matches on the host, but SPIR-V's `FMax` returned -0 there in W3SPR1's replay.
            if amax < av {
                amax = av;
            }
        });
    });

    // 2.  Integrate over directions
    each!(ik, 1, nk, {
        let s = sig[ik as usize];
        let w = wn[ix!(cg_off, ik)];
        let e = eb[ix!(w_off, ik)];
        let w2 = w * w;
        alfa[ix!(w_off, ik)] = 2. * p.dth * s * e * (w2 * w);
        eb[ix!(w_off, ik)] = e * dden[ix!(0, ik)] / cg[ix!(cg_off, ik)];
        let e = eb[ix!(w_off, ik)];
        emean = emean + e;
        fmean = fmean + e / s;
        wnmean = wnmean + e / sqrtf(w);
    });

    // 3.  Add tail beyond discrete spectrum and get mean pars
    let eband = eb[ix!(w_off, nk)] / dden[ix!(0, nk)];
    emean = emean + eband * p.fte;
    fmean = fmean + eband * p.ftf;
    wnmean = wnmean + eband * p.ftwn;

    let fmean = TPIINV * emean / fmean.max(1.0e-7);
    let q = emean / wnmean.max(1.0e-7);
    let wnmean = q * q;

    // 4.  Estimate peak frequency from FPI
    let fpistr = (pt.fpi * ust / GRAV).max(0.008);
    let f2 = fpistr * fpistr;
    let fp = (3.6E-4 + 0.92 * fpistr - 6.3E-10 / (f2 * fpistr)) / ust * GRAV;
    let fp = fp * TPIINV;

    Spr2Out { emean, fmean, wnmean, amax, fp }
}
