// SPDX-License-Identifier: MPL-2.0
//! `W3SPR3` and `CALC_USTAR` of `W3SRC3MD`, ST3's mean wave parameters (WAM cycle 4). Translated from
//! `model/src/w3src3md.F90:134-347` and `:1135-1227` at metocean/WW3 `92e2cf66`. Not the ST4
//! `CALC_USTAR` of `sin4.rs`: ST3's has no Newton branch, no `CAPCHNK` and no `ZZ0MAX`.
//!
//! Left out because it has no effect on any output: `ALFA(IK)` (`:273`) is computed and never read,
//! the first `UNZ` (`:246`) is overwritten at `:326`, `Z0 = 0.` (`:324`) is overwritten by
//! `CALC_USTAR`, and so is the floor `USTAR = MAX(0.0001, USTAR)` (`:247`), since `USTAR` is
//! `INTENT(OUT)` in `CALC_USTAR`. The bits are the reference's (`rust/port_report/W3SPR3.md`).
#![allow(clippy::assign_op_pattern)] // `x = x + y` keeps the shape of the Fortran line.

use crate::constants::{GRAV, KAPPA, TPIINV};
use crate::real::Real;
use crate::util::pow2;
use ww3_libm::{expf, powf, sqrtf};

pub const ITAUMAX: i32 = 200;
pub const JUMAX: i32 = 200;
pub const TAUWMAX: Real = 2.2361;

/// Scalars `W3SPR3` and `CALC_USTAR` read. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr3Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub wwnmeanp: Real,
    pub wwnmeanptail: Real,
    pub fte: Real,
    pub sstxftf: Real,
    pub sstxftftail: Real,
    pub sstxftwn: Real,
    pub zzwnd: Real,
    pub aalpha: Real,
    pub deltauw: Real,
    pub delu: Real,
}

/// The scalars of one `W3SPR3` call. `ustar` and `usdir` are `INTENT(INOUT)`; both are overwritten.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr3Point {
    pub u: Real,
    pub udir: Real,
    pub ustar: Real,
    pub usdir: Real,
    pub tauwx: Real,
    pub tauwy: Real,
}

/// The scalars one `W3SPR3` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr3Out {
    pub emean: Real,
    pub fmean: Real,
    pub fmeans: Real,
    pub wnmean: Real,
    pub amax: Real,
    pub ustar: Real,
    pub usdir: Real,
    pub cd: Real,
    pub z0: Real,
    pub charn: Real,
    pub fmeanws: Real,
}

/// `TAUT(i, j)`, with bounds `0:ITAUMAX`, `0:JUMAX`.
#[inline]
pub fn taut_at(i: i32, j: i32) -> usize {
    (i + j * (ITAUMAX + 1)) as usize
}

/// `CALC_USTAR` (`:1135-1227`): `(USTAR, Z0, CHARN)`.
#[inline]
pub fn calc_ustar(p: &Spr3Params, taut: &[Real], windspeed: Real, tauw: Real) -> (Real, Real, Real) {
    let tauw_local = tauw.min(TAUWMAX).max(0.);
    let xi = sqrtf(tauw_local) / p.deltauw;
    let ind = (ITAUMAX - 1).min(xi as i32);
    let deli1 = (xi - ind as Real).min(1.);
    let deli2 = 1. - deli1;
    let xj = windspeed / p.delu;
    let j = (JUMAX - 1).min(xj as i32);
    let delj1 = (xj - j as Real).min(1.);
    let delj2 = 1. - delj1;
    let ustar = (taut[taut_at(ind, j)] * deli2 + taut[taut_at(ind + 1, j)] * deli1) * delj2
        + (taut[taut_at(ind, j + 1)] * deli2 + taut[taut_at(ind + 1, j + 1)] * deli1) * delj1;

    // Determines roughness length. At WINDSPEED = 0 USTAR is 0 and 0/0 is NaN: MIN gives 100.
    let sqrtcdm1 = (windspeed / ustar).min(100.0);
    let z0 = p.zzwnd * expf(-KAPPA * sqrtcdm1);
    let charn = if ustar > 0.001 { GRAV * z0 / pow2(ustar) } else { p.aalpha };
    (ustar, z0, charn)
}

/// One spectrum. `w` holds the Fortran's automatic arrays `EB` and `EB2`, `NK` each, from `w_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3spr3(
    p: &Spr3Params,
    sig: &[Real],
    dden: &[Real],
    taut: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    llws: &[u32],
    pt: &Spr3Point,
    w: &mut [Real],
    out: &mut [Spr3Out],
    spec_off: usize,
    cg_off: usize,
    w_off: usize,
    out_off: usize,
) {
    let (nk, nth) = (p.nk, p.nth);
    let o_eb = w_off;
    let o_eb2 = w_off + nk as usize;
    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! cg {
        ($ik:expr) => {
            cg[ix!(cg_off, $ik)]
        };
    }
    macro_rules! dd {
        ($ik:expr) => {
            dden[ix!(0, $ik)]
        };
    }
    macro_rules! sg {
        ($ik:expr) => {
            sig[($ik) as usize]
        };
    }

    let (mut emean, mut emeanws, mut fmeanws, mut fmean, mut fmeans) = (0., 0., 0., 0., 0.);
    let (mut wnmean, mut amax) = (0., 0_f32);

    // 1.  Integral over directions and maximum
    each!(ik, 1, nk, {
        w[ix!(o_eb, ik)] = 0.;
        w[ix!(o_eb2, ik)] = 0.;
        each!(ith, 1, nth, {
            let is = ith + (ik - 1) * nth;
            let av = a[ix!(spec_off, is)];
            w[ix!(o_eb, ik)] = w[ix!(o_eb, ik)] + av;
            if llws[ix!(spec_off, is)] != 0 {
                w[ix!(o_eb2, ik)] = w[ix!(o_eb2, ik)] + av;
            }
            amax = amax.max(av);
        });
    });

    // 2.  Integrate over directions
    each!(ik, 1, nk, {
        w[ix!(o_eb, ik)] = w[ix!(o_eb, ik)] * dd!(ik) / cg!(ik);
        w[ix!(o_eb2, ik)] = w[ix!(o_eb2, ik)] * dd!(ik) / cg!(ik);
        let (eb, eb2) = (w[ix!(o_eb, ik)], w[ix!(o_eb2, ik)]);
        emean = emean + eb;
        fmean = fmean + eb * powf(sg!(ik), 2. * p.wwnmeanptail);
        fmeans = fmeans + eb * powf(sg!(ik), 2. * p.wwnmeanp);
        wnmean = wnmean + eb * powf(wn[ix!(cg_off, ik)], p.wwnmeanp);
        emeanws = emeanws + eb2;
        fmeanws = fmeanws + eb2 * powf(sg!(ik), 2. * p.wwnmeanptail);
    });

    // 3.  Add tail beyond discrete spectrum and get mean pars
    let eband = w[ix!(o_eb, nk)] / dd!(nk);
    emean = emean + eband * p.fte;
    fmean = fmean + eband * p.sstxftftail;
    fmeans = fmeans + eband * p.sstxftf;
    wnmean = wnmean + eband * p.sstxftwn;
    let eband = w[ix!(o_eb2, nk)] / dd!(nk);
    emeanws = emeanws + eband * p.fte;
    fmeanws = fmeanws + eband * p.sstxftftail;

    // 4.  Final processing
    let fmean = if fmean < 1.0e-7 {
        TPIINV * sg!(nk)
    } else {
        TPIINV * powf(fmean.max(1.0e-7) / emean.max(1.0e-7), 1. / (2. * p.wwnmeanptail))
    };
    let fmeans = if fmeans < 1.0e-7 {
        TPIINV * sg!(nk)
    } else {
        TPIINV * powf(fmeans.max(1.0e-7) / emean.max(1.0e-7), 1. / (2. * p.wwnmeanp))
    };
    let wnmean = powf(wnmean.max(1.0e-7) / emean.max(1.0e-7), 1. / p.wwnmeanp);
    let fmeanws = if fmeanws < 1.0e-7 || emeanws < 1.0e-7 {
        TPIINV * sg!(nk)
    } else {
        TPIINV * powf(fmeanws.max(1.0e-7) / emeanws.max(1.0e-7), 1. / (2. * p.wwnmeanptail))
    };

    // 5.  Cd and z0
    let tauw = sqrtf(pow2(pt.tauwx) + pow2(pt.tauwy));
    let (ustar, z0, charn) = calc_ustar(p, taut, pt.u, tauw);
    let unz = pt.u.max(0.01);
    let cd = pow2(ustar / unz);
    out[out_off] = Spr3Out {
        emean,
        fmean,
        fmeans,
        wnmean,
        amax,
        ustar,
        usdir: pt.udir,
        cd,
        z0,
        charn,
        fmeanws,
    };
}
