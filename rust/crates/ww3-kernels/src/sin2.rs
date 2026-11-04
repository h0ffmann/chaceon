// SPDX-License-Identifier: MPL-2.0
//! `W3SIN2`, ST2's wind input (Tolman and Chalikov 1996, with the swell filter). Translated from
//! `model/src/w3src2md.F90:308-566` at metocean/WW3 `92e2cf66`. The order of operations is the one
//! gfortran 12.2 `-O3 -ffp-contract=off` compiled on aarch64, read from the optimised GIMPLE of
//! `w3sin2` (`rust/port_report/W3SIN2.md`): every `**2` and `**3` is a product, `COS` and `SIN` of
//! `UDIR` are one `sincosf`, `LOG` is `logf`, and nothing fuses. The `BETATB` table is an input:
//! `INPTAB` builds it on the host (`ww3-host/src/inptab.rs`).
//!
//! No `f64` here, and no `TRANS` division: `/(1.-TRANSF)` is `*4.` in the compiled code (`x/0.25` is
//! exact either way). `SIN1A(NK)` is a work array in the Fortran, which `malloc`s it. Its first loop
//! writes one sum per `IK` and nothing reads `S` before the second loop, so the sum is formed where
//! it is used: the same additions in the same order.
//!
//! `MAX` is `fmaxnm` on arm64 (`MAX_EXPR`): a NaN operand loses and `MAX(+0,-0)` is `+0`. `f32::max`
//! agrees on the host, SPIR-V's `FMax` need not, so `fmaxnm` below is written out.
#![allow(clippy::assign_op_pattern, clippy::manual_clamp)] // `x = x + y` keeps the Fortran line.

use crate::real::Real;
use crate::util::sign;
use ww3_libm::{logf, sincosf, sqrtf};

/// `NRSIGA`, `NRDRAG`, `SIGAMX`, `DRAGMX` (`w3src2md.F90:100-103`).
pub const NRSIGA: i32 = 400;
pub const NRDRAG: i32 = 20;
pub const SIGAMX: Real = 40.;
pub const DRAGMX: Real = 1.0E-2;
/// Values in `BETATB(-NRSIGA:NRSIGA+1, NRDRAG+1)`.
pub const BETATB_LEN: usize = ((2 * NRSIGA + 2) * (NRDRAG + 1)) as usize;

const GRAV: Real = crate::constants::GRAV;
const TPI: Real = crate::constants::TPI;
const TRANSF: Real = 0.75;
const PEAKFC: Real = 0.8;

/// Scalars `W3SIN2` reads from `W3GDATMD` and `W3SRC2MD`. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin2Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    /// `DSIGA` and `DDRAG` of `W3SRC2MD`, set by `INPTAB`.
    pub dsiga: Real,
    pub ddrag: Real,
    pub zwind: Real,
    pub fte: Real,
    pub fttr: Real,
    pub xfr: Real,
    pub fpimin: Real,
    pub facti1: Real,
    pub facti2: Real,
    pub fswell: Real,
}

/// The scalars of one `W3SIN2` call: `U`, `UDIR`, `CD`, `Z0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin2Point {
    pub u: Real,
    pub udir: Real,
    pub cd: Real,
    pub z0: Real,
}

/// What one `W3SIN2` call returns besides `S` and `D`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin2Out {
    pub fpi: Real,
    /// Which branches ran, for reading what a capture reached; the arithmetic never reads it. Bit 0:
    /// `|COSFAC|` was below 0.0087 for some bin (`:459`). 1 and 2: `IOMA` clamped to `-NRSIGA` or to
    /// `NRSIGA` (`:469`). 3 and 4: `ICL` clamped to 1 or to `NRDRAG` (`:470`). 5: `OMA < 0` for
    /// some bin (the `MIN(0,INT(SIGN))` term, `:464`). 6: `M1 < 1E-20` (`:503`). 7: `NKFILT` cut at
    /// `NK` (`:516`). 8: `NKFIL2` raised to 0 (`:518`). 9: the first filter loop ran (`:520`).
    /// 10: the transition loop ran (`:525`). 11: `FSWELL*D > D` for some `D` (the filter acted).
    /// 12: `FPISTR` took `FPIMIN` (`:512`).
    pub reach: u32,
}

/// gfortran's `MAX` on two `REAL`s on arm64: `fmaxnm`.
#[inline]
fn fmaxnm(x: Real, y: Real) -> Real {
    if x != x {
        y
    } else if y != y {
        x
    } else if x > y {
        x
    } else if y > x {
        y
    } else {
        // equal: only `+0` against `-0` differs in bits, and `fmaxnm` returns `+0`
        Real::from_bits(x.to_bits() & y.to_bits())
    }
}

/// `BETATB(ioma, icl)`, bounds `(-NRSIGA:NRSIGA+1, NRDRAG+1)` (`w3src2md.F90:107`).
#[inline]
pub fn betatb_at(ioma: i32, icl: i32) -> usize {
    ((ioma + NRSIGA) + (icl - 1) * (2 * NRSIGA + 2)) as usize
}

/// `W3SIN2`, `w3src2md.F90:308-566`, on one spectrum. `sig` is `SIG(0:NK+1)`, `dden` is `DDEN(NK)`,
/// `betatb` is `BETATB(-NRSIGA:NRSIGA+1,NRDRAG+1)`. `a`, `k`, `s`, `d` start at `spec_off`, `cg` at
/// `cg_off` and the result goes to `out[out_off]`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sin2(
    p: &Sin2Params,
    sig: &[Real],
    sig2: &[Real],
    dden: &[Real],
    ecos: &[Real],
    esin: &[Real],
    betatb: &[Real],
    a: &[Real],
    cg: &[Real],
    k: &[Real],
    pt: &Sin2Point,
    s: &mut [Real],
    d: &mut [Real],
    out: &mut [Sin2Out],
    spec_off: usize,
    cg_off: usize,
    out_off: usize,
) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec);
    let (u, udir, cd, z0) = (pt.u, pt.udir, pt.cd, pt.z0);

    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! s {
        ($is:expr) => {
            s[ix!(spec_off, $is)]
        };
    }
    macro_rules! d {
        ($is:expr) => {
            d[ix!(spec_off, $is)]
        };
    }
    macro_rules! a {
        ($is:expr) => {
            a[ix!(spec_off, $is)]
        };
    }

    let mut reach = 0u32;

    // 1.  Preparations
    let (sinu, cosu) = sincosf(udir);

    // 2.  Loop over spectrum
    let facln1 = u / logf(p.zwind / z0);
    let facln2 = logf(z0);

    each!(is, 1, nspec, {
        let mut cosfac = ecos[ix!(0, is)] * cosu + esin[ix!(0, is)] * sinu;
        reach |= (cosfac.abs() < 0.0087) as u32;
        cosfac = sign(fmaxnm(0.0087, cosfac.abs()), cosfac);
        let lambda = TPI / (k[ix!(spec_off, is)] * cosfac.abs());
        let ulam = facln1 * (logf(lambda) - facln2);
        let clam = cd * pow2_(u / ulam);
        let oma = k[ix!(spec_off, is)] * ulam * cosfac / sig2[ix!(0, is)];
        let q1 = oma / p.dsiga;
        let q2 = clam / p.ddrag;
        let mut ioma = (q1 as i32).wrapping_add(0_i32.min(sign(1.1, oma) as i32));
        let mut icl = q2 as i32;
        let rd1 = q1 - ioma as Real;
        let rd2 = q2 - icl as Real;
        reach |= ((oma < 0.) as u32) << 5;
        reach |= ((ioma < -NRSIGA) as u32) << 1 | ((ioma > NRSIGA) as u32) << 2;
        reach |= ((icl < 1) as u32) << 3 | ((icl > NRDRAG) as u32) << 4;
        ioma = (-NRSIGA).max(NRSIGA.min(ioma));
        icl = 1_i32.max(NRDRAG.min(icl));
        let beta = (1. - rd1) * (1. - rd2) * betatb[betatb_at(ioma, icl)]
            + rd1 * (1. - rd2) * betatb[betatb_at(ioma + 1, icl)]
            + (1. - rd1) * rd2 * betatb[betatb_at(ioma, icl + 1)]
            + rd1 * rd2 * betatb[betatb_at(ioma + 1, icl + 1)];
        d!(is) = beta * sig2[ix!(0, is)];
        s!(is) = a!(is) * d!(is);
    });

    // 3.  Calculate FPI. `SIN1A(IK)` is formed in the loop that uses it, as the module comment says.
    let (mut m0, mut m1) = (0., 0.);
    let mut last = 0.;
    each!(ik, 1, nk, {
        let mut sin1a: Real = 0.;
        each!(ith, 1, nth, {
            let is = (ik - 1) * nth + ith;
            sin1a = sin1a + fmaxnm(0., s!(is));
        });
        let sg = sig[ik as usize];
        sin1a = sin1a * dden[ix!(0, ik)] / (cg[ix!(cg_off, ik)] * (sg * sg * sg));
        m0 = m0 + sin1a;
        m1 = m1 + sin1a / sg;
        last = sin1a;
    });
    let sin1a_nk = last / dden[ix!(0, nk)];
    m0 = m0 + sin1a_nk * p.fte;
    m1 = m1 + sin1a_nk * p.fttr;
    let fpi;
    if m1 < 1E-20 {
        reach |= 1 << 6;
        fpi = p.xfr * sig[nk as usize];
    } else {
        fpi = m0 / m1;
    }

    // 4.  Filter for swell
    let ustar = u * sqrtf(cd);
    let fpistr = fmaxnm(p.fpimin, fpi * ustar / GRAV);
    reach |= ((fpistr == p.fpimin) as u32) << 12;
    let fp1str = 3.6E-4 + 0.92 * fpistr - 6.3E-10 / (fpistr * (fpistr * fpistr));
    let fp1 = PEAKFC * fp1str * GRAV / ustar;

    let nkfilt_raw = (p.facti2 + p.facti1 * logf(fp1)) as i32;
    let nkfilt = nk.min(nkfilt_raw);
    reach |= ((nkfilt_raw > nk) as u32) << 7;
    let nkfil2_raw = nk.min((p.facti2 + p.facti1 * logf(TRANSF * fp1)) as i32);
    let nkfil2 = 0_i32.max(nkfil2_raw);
    reach |= ((nkfil2_raw < 0) as u32) << 8;

    each!(is, 1, nkfil2 * nth, {
        reach |= 1 << 9;
        let m = fmaxnm(d!(is), p.fswell * d!(is));
        reach |= ((m > d!(is)) as u32) << 11;
        d!(is) = m;
        s!(is) = a!(is) * d!(is);
    });

    each!(ik, nkfil2 + 1, nkfilt, {
        reach |= 1 << 10;
        // `/(1.-TRANSF)` is `*4.`, exact for the divisor 0.25
        let trans = (sig[ik as usize] / fp1 - TRANSF) * 4.;
        let omt = 1. - trans;
        each!(ith, 1, nth, {
            let is = (ik - 1) * nth + ith;
            let m = fmaxnm(d!(is), p.fswell * d!(is));
            reach |= ((m > d!(is)) as u32) << 11;
            d!(is) = omt * m + trans * d!(is);
            s!(is) = a!(is) * d!(is);
        });
    });

    out[out_off] = Sin2Out { fpi, reach };
}

#[inline]
fn pow2_(x: Real) -> Real {
    x * x
}
