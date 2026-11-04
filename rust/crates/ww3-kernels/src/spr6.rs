// SPDX-License-Identifier: MPL-2.0
//! `W3SPR6`, ST6's mean wave parameters, translated from `model/src/w3src6md.F90:121-257` at
//! metocean/WW3 `92e2cf66`. The Fortran uses array statements: every `SUM` runs in index order from
//! zero (gfortran does not reassociate a `REAL` reduction without `-ffast-math`), `SUM(A,1)*DDEN/CG`
//! associates to the left, `EB**4` is `(EB*EB)*(EB*EB)` and `**2` is a product.
// `x = x + y` keeps the Fortran line.
#![allow(clippy::assign_op_pattern)]

use crate::constants::TPIINV;
use crate::real::Real;
use ww3_libm::sqrtf;

/// `HSMIN` (`w3src6md.F90:211`), the significant wave height below which no peak is sought.
pub const HSMIN: Real = 0.05;

/// What `W3SPR6` reads from `W3GDATMD`: `NK`, `NTH`, `DTH`, `FTE`, `FTF` and `FTWN`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr6Params {
    pub nk: i32,
    pub nth: i32,
    pub dth: Real,
    pub fte: Real,
    pub ftf: Real,
    pub ftwn: Real,
}

/// The five scalars one `W3SPR6` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr6Out {
    pub emean: Real,
    pub fmean: Real,
    pub wnmean: Real,
    pub amax: Real,
    pub fp: Real,
}

/// gfortran's `MAX(c, x)` on arm64 is `fmaxnm`: a NaN `x` gives `c`. Written as a comparison because
/// SPIR-V's `FMax` does not promise that (`rust/port_report/W3SPR1.md`). `c` is a positive constant.
#[inline]
fn mx(c: Real, x: Real) -> Real {
    if x > c { x } else { c }
}

/// One spectrum. `sig` is `SIG(0:NK+1)`; `dden` and `dsii` are `(1:NK)` from index 0 of the slice;
/// `eb` is the Fortran's automatic array `EB(NK)` at `w_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3spr6(
    p: &Spr6Params,
    sig: &[Real],
    dden: &[Real],
    dsii: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    eb: &mut [Real],
    spec_off: usize,
    cg_off: usize,
    w_off: usize,
) -> Spr6Out {
    let (nk, nth) = (p.nk, p.nth);
    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }

    // 1.  Integrate over directions: EB = SUM(A,1) * DDEN / CG, AMAX = MAXVAL(A)
    let mut amax = Real::NEG_INFINITY;
    each!(ik, 1, nk, {
        let mut s = 0.;
        each!(ith, 1, nth, {
            let av = a[ix!(spec_off, ith + (ik - 1) * nth)];
            s = s + av;
            if amax < av {
                amax = av;
            }
        });
        eb[ix!(w_off, ik)] = s * dden[ix!(0, ik)] / cg[ix!(cg_off, ik)];
    });

    // 2.  Integrate over wavenumbers
    let (mut emean, mut fmean, mut wnmean) = (0., 0., 0.);
    each!(ik, 1, nk, {
        emean = emean + eb[ix!(w_off, ik)];
    });
    each!(ik, 1, nk, {
        fmean = fmean + eb[ix!(w_off, ik)] / sig[ik as usize];
    });
    each!(ik, 1, nk, {
        wnmean = wnmean + eb[ix!(w_off, ik)] / sqrtf(wn[ix!(cg_off, ik)]);
    });

    // 3.  Add tail beyond discrete spectrum
    let eband = eb[ix!(w_off, nk)] / dden[ix!(0, nk)];
    emean = emean + eband * p.fte;
    fmean = fmean + eband * p.ftf;
    wnmean = wnmean + eband * p.ftwn;

    // 4.  Final processing
    let fmean = TPIINV * emean / mx(1.0e-7, fmean);
    let q = emean / mx(1.0e-7, wnmean);
    let wnmean = q * q;

    // 5.  Peak frequency from a weighted integral, Young (1999) p239
    let mut fp = 0.;
    if 4.0 * sqrtf(emean) > HSMIN {
        each!(ik, 1, nk, {
            let mut s = 0.;
            each!(ith, 1, nth, {
                s = s + a[ix!(spec_off, ith + (ik - 1) * nth)];
            });
            eb[ix!(w_off, ik)] = s * sig[ik as usize] / cg[ix!(cg_off, ik)] * p.dth;
        });
        let (mut num, mut den) = (0., 0.);
        each!(ik, 1, nk, {
            let e = eb[ix!(w_off, ik)];
            let e2 = e * e;
            let e4 = e2 * e2;
            num = num + sig[ik as usize] * e4 * dsii[ix!(0, ik)];
        });
        each!(ik, 1, nk, {
            let e = eb[ix!(w_off, ik)];
            let e2 = e * e;
            let e4 = e2 * e2;
            den = den + e4 * dsii[ix!(0, ik)];
        });
        fp = num / mx(1.0e-10, den);
        fp = fp * TPIINV;
    }

    Spr6Out { emean, fmean, wnmean, amax, fp }
}
