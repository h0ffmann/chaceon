// SPDX-License-Identifier: MPL-2.0
//! `INSNL1`, the DIA's tables. Translated from `model/src/w3snl1md.F90:467-770` at metocean/WW3
//! `92e2cf66`. Runs once on the host; its tables go to the device.

use ww3_kernels::constants::TPIINV;
use ww3_kernels::real::Real;
use ww3_kernels::snl1::{IC_COLS, PM_COLS, Snl1Params};

/// What `W3SNL1` reads besides the spectrum.
pub struct Snl1Tables {
    pub params: Snl1Params,
    pub af11: Vec<Real>,
    pub pm: Vec<i32>,
    pub ic: Vec<i32>,
}

/// libgcc's `__powisf2`: gfortran calls it for a run-time integer exponent, at `-O3` too.
fn powi(mut x: Real, n: i32) -> Real {
    let mut m = n.unsigned_abs();
    let mut y = if m & 1 != 0 { x } else { 1. };
    loop {
        m >>= 1;
        if m == 0 {
            break;
        }
        x *= x;
        if m & 1 != 0 {
            y *= x;
        }
    }
    if n < 0 { 1. / y } else { y }
}

/// `x**11` as `-O3` expands a literal power: GCC's power tree, not `__powisf2`'s chain.
fn pow11(x: Real) -> Real {
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x2 * x3;
    let x6 = x3 * x3;
    x5 * x6
}

/// `sig` is `SIG(0:NK+1)`. `params` brings the namelist scalars; the table sizes, `DAL` and the
/// weights are filled in here.
pub fn insnl1(mut params: Snl1Params, dth: Real, xfr: Real, lam: Real, sig: &[Real]) -> Snl1Tables {
    let (nk, nth, nspec) = (params.nk, params.nth, params.nspec);
    let nfr = nk;

    // 1.  Internal angles of quadruplet.
    let lamm2 = (1. - lam) * (1. - lam);
    let lamp2 = (1. + lam) * (1. + lam);
    let delth3 = ((lamm2 * lamm2 + 4. - lamp2 * lamp2) / (4. * lamm2)).acos();
    let delth4 = (-delth3.sin() * lamm2 / lamp2).asin();

    // 2.  Lambda dependend weight factors.
    let p4 = |x: Real| {
        let x2 = x * x;
        x2 * x2
    };
    let dal1 = 1. / p4(1. + lam);
    let dal2 = 1. / p4(1. - lam);
    let dal3 = 2. * dal1 * dal2;

    // 3.  Directional indices.
    let cthp = (delth4 / dth).abs();
    let ithp = cthp as i32;
    let ithp1 = ithp + 1;
    let wthp = cthp - ithp as Real;
    let wthp1 = 1. - wthp;

    let cthm = (delth3 / dth).abs();
    let ithm = cthm as i32;
    let ithm1 = ithm + 1;
    let wthm = cthm - ithm as Real;
    let wthm1 = 1. - wthm;

    // 4.  Frequency indices.
    let xfrln = xfr.ln();

    let ifrp = ((1. + lam).ln() / xfrln) as i32;
    let ifrp1 = ifrp + 1;
    let wfrp = (1. + lam - powi(xfr, ifrp)) / (powi(xfr, ifrp1) - powi(xfr, ifrp));
    let wfrp1 = 1. - wfrp;

    let ifrm = ((1. - lam).ln() / xfrln) as i32;
    let ifrm1 = ifrm - 1;
    let wfrm = (powi(xfr, ifrm) - (1. - lam)) / (powi(xfr, ifrm) - powi(xfr, ifrm1));
    let wfrm1 = 1. - wfrm;

    // 5.  Range of calculations
    let nfrhgh = nfr + ifrp1 - ifrm1;
    let nfrchg = nfr - ifrm1;
    let nspecy = nfrhgh * nth;
    let nspecx = nfrchg * nth;

    // 7.  Spectral addresses
    let fr_idx = |ifr: i32| {
        [
            ifr + ifrp,
            ifr + ifrp1,
            (ifr + ifrm).max(0),
            (ifr + ifrm1).max(0),
            (ifr - ifrp).max(0),
            (ifr - ifrp1).max(0),
            ifr - ifrm,
            ifr - ifrm1,
        ]
    };
    let th_idx = |ith: i32| {
        let up = |i: i32| if i > nth { i - nth } else { i };
        let dn = |i: i32| if i < 1 { i + nth } else { i };
        [
            up(ith + ithp),
            up(ith + ithp1),
            up(ith + ithm),
            up(ith + ithm1),
            dn(ith - ithp),
            dn(ith - ithp1),
            dn(ith - ithm),
            dn(ith - ithm1),
        ]
    };

    let mut pm = Vec::with_capacity(nspecx as usize * PM_COLS);
    for isp in 1..=nspecx {
        let [if1, if2, if3, if4, ..] = fr_idx(1 + (isp - 1) / nth);
        let [it1, it2, it3, it4, it5, it6, it7, it8] = th_idx(1 + (isp - 1) % nth);
        let addr = |it: i32, if_: i32| it + (if_ - 1) * nth;
        pm.extend([
            addr(it2, if2),
            addr(it1, if2),
            addr(it2, if1),
            addr(it1, if1),
            addr(it8, if4),
            addr(it7, if4),
            addr(it8, if3),
            addr(it7, if3),
            addr(it6, if2),
            addr(it5, if2),
            addr(it6, if1),
            addr(it5, if1),
            addr(it4, if4),
            addr(it3, if4),
            addr(it4, if3),
            addr(it3, if3),
        ]);
    }

    let mut ic = Vec::with_capacity(nspec as usize * IC_COLS);
    for isp in 1..=nspec {
        let [_, _, _, _, if5, if6, if7, if8] = fr_idx(1 + (isp - 1) / nth);
        let [it1, it2, it3, it4, it5, it6, it7, it8] = th_idx(1 + (isp - 1) % nth);
        let addr = |it: i32, if_: i32| it + (if_ - 1) * nth;
        ic.extend([
            addr(it6, if6),
            addr(it2, if6),
            addr(it5, if6),
            addr(it1, if6),
            addr(it6, if5),
            addr(it2, if5),
            addr(it5, if5),
            addr(it1, if5),
            addr(it4, if8),
            addr(it8, if8),
            addr(it3, if8),
            addr(it7, if8),
            addr(it4, if7),
            addr(it8, if7),
            addr(it3, if7),
            addr(it7, if7),
        ]);
    }

    // 8.  Fill scaling array (f**11)
    let mut af11 = vec![0.; nspecx as usize];
    let mut fill = |ifr: i32, v: Real| {
        for ith in 1..=nth {
            af11[(ith + (ifr - 1) * nth - 1) as usize] = v;
        }
    };
    for ifr in 1..=nfr {
        fill(ifr, pow11(sig[ifr as usize] * TPIINV));
    }
    let mut fr = sig[nfr as usize] * TPIINV;
    for ifr in nfr + 1..=nfrchg {
        fr *= xfr;
        fill(ifr, pow11(fr));
    }

    // 9.  Interpolation weights
    let awg = [
        wthp * wfrp,
        wthp1 * wfrp,
        wthp * wfrp1,
        wthp1 * wfrp1,
        wthm * wfrm,
        wthm1 * wfrm,
        wthm * wfrm1,
        wthm1 * wfrm1,
    ];

    params.nfrhgh = nfrhgh;
    params.nfrchg = nfrchg;
    params.nspecx = nspecx;
    params.nspecy = nspecy;
    params.dal1 = dal1;
    params.dal2 = dal2;
    params.dal3 = dal3;
    params.awg = awg;
    params.swg = awg.map(|w| w * w);
    Snl1Tables { params, af11, pm, ic }
}
