// SPDX-License-Identifier: MPL-2.0
//! `W3SPR1`, the mean parameters of WAM cycle 3, translated from `model/src/w3src1md.F90:87-240` at
//! metocean/WW3 `92e2cf66`. Every operation keeps the Fortran's order: the sums run in `IK`, `ITH`
//! order, `EB*DDEN/CG` associates to the left, and `SQRT(WN)` is `sqrtf`. `**2` is a product.
// `x = x + y` keeps the Fortran line.
#![allow(clippy::assign_op_pattern)]

use crate::constants::TPIINV;
use crate::real::Real;
use ww3_libm::sqrtf;

/// What `W3SPR1` reads from `W3GDATMD`: `NK`, `NTH`, `FTE`, `FTF` and `FTWN`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr1Params {
    pub nk: i32,
    pub nth: i32,
    pub fte: Real,
    pub ftf: Real,
    pub ftwn: Real,
}

/// The four scalars one `W3SPR1` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr1Out {
    pub emean: Real,
    pub fmean: Real,
    pub wnmean: Real,
    pub amax: Real,
}

/// One spectrum. `sig` is `SIG(0:NK+1)`, `dden` is `DDEN(1:NK)` from index 0 of the slice (as in
/// `w3spr4`), `eb` is the Fortran's automatic array `EB(NK)` at `w_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3spr1(
    p: &Spr1Params,
    sig: &[Real],
    dden: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    eb: &mut [Real],
    spec_off: usize,
    cg_off: usize,
    w_off: usize,
) -> Spr1Out {
    let (nk, nth) = (p.nk, p.nth);
    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }

    let (mut emean, mut fmean, mut wnmean, mut amax) = (0., 0., 0., 0_f32);

    // 1.  Integral over directions
    each!(ik, 1, nk, {
        eb[ix!(w_off, ik)] = 0.;
        each!(ith, 1, nth, {
            let av = a[ix!(spec_off, ith + (ik - 1) * nth)];
            eb[ix!(w_off, ik)] = eb[ix!(w_off, ik)] + av;
            // `MAX(AMAX, A)` is `fmaxnm`: +0 against -0 gives +0, and a NaN `A` is skipped. `f32::max`
            // matches on the host, but SPIR-V's `FMax` returns -0 there (`spr1_poke`, 125 calls).
            if amax < av {
                amax = av;
            }
        });
    });

    // 2.  Integrate over directions
    each!(ik, 1, nk, {
        eb[ix!(w_off, ik)] = eb[ix!(w_off, ik)] * dden[ix!(0, ik)] / cg[ix!(cg_off, ik)];
        let e = eb[ix!(w_off, ik)];
        emean = emean + e;
        fmean = fmean + e / sig[ik as usize];
        wnmean = wnmean + e / sqrtf(wn[ix!(cg_off, ik)]);
    });

    // 3.  Add tail beyond discrete spectrum
    let eband = eb[ix!(w_off, nk)] / dden[ix!(0, nk)];
    emean = emean + eband * p.fte;
    fmean = fmean + eband * p.ftf;
    wnmean = wnmean + eband * p.ftwn;

    // 4.  Final processing
    let fmean = TPIINV * emean / fmean.max(1.0e-7);
    let q = emean / wnmean.max(1.0e-7);
    let wnmean = q * q;

    Spr1Out { emean, fmean, wnmean, amax }
}
