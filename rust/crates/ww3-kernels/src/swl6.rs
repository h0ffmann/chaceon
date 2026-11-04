// SPDX-License-Identifier: MPL-2.0
//! `W3SWL6`, the turbulent dissipation of narrow-banded swell of ST6 (Babanin 2011, section 7.5), and
//! `IRANGE` of the same module. Translated from `model/src/w3swldmd.F90:251-413` (`W3SWL6`) and
//! `:430-460` (`IRANGE`) at metocean/WW3 `92e2cf66`. Every variable is `REAL`, so everything is `f32`.
//! Nothing fuses (`-ffp-contract=off`). The `W3_S` (`STRACE`) and `W3_T6` blocks are left out.
//!
//! The Fortran keeps automatic arrays `ABAND`, `KMAX`, `ANAR`, `BN`, `DDIS` (NK) and `K` (NTH,NK). A
//! kernel may not allocate, so each value is recomputed where it is used, with the same operations in
//! the same order: `ABAND` is a sum over `ITH` from `0.`, so a second evaluation gives the same bits.
//! `FTE` and `FTWN` are imported by the Fortran and never read.

use crate::real::Real;
use ww3_libm::sqrtf;

/// Scalars of `W3GDATMD` that `W3SWL6` reads. `SWL6CSTB1` is a `LOGICAL`, held here as an `i32`.
/// Shared with the device. `SIG(1:NK)` and `DDEN(1:NK)` are separate buffers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Swl6Params {
    pub nk: i32,
    pub nth: i32,
    pub dth: Real,
    pub swl6b1: Real,
    pub swl6cstb1: i32,
}

/// `N = INT(REAL(X1-X0)/REAL(DX))+1`, the length of `IRANGE(X0,X1,DX)` (`w3swldmd.F90:454`).
/// `INT` truncates toward zero, which `as i32` does for an `f32`.
#[inline]
pub fn irange_len(x0: i32, x1: i32, dx: i32) -> i32 {
    (((x1 - x0) as Real) / (dx as Real)) as i32 + 1
}

/// Element `I` (1-based) of `IRANGE(X0,X1,DX)`: `X0 + (I-1)*DX`.
#[inline]
pub fn irange_at(x0: i32, dx: i32, i: i32) -> i32 {
    x0 + (i - 1) * dx
}

/// `ABAND(IK) = SUM(RESHAPE(A,(/NTH,NK/)),1)` for one `IK` (`:356`): in `ITH` order from `0.`.
#[inline]
fn aband(a: &[Real], o: usize, nth: usize, ik: usize) -> Real {
    let mut sum: Real = 0.;
    let mut ith = 0;
    while ith < nth {
        sum += a[o + ith + (ik - 1) * nth];
        ith += 1;
    }
    sum
}

/// `KMAX(IK) = MAXVAL(K,1)` for one `IK` (`:365`), inline in the reference (its disassembly has no call
/// of `_gfortran_maxval_r4`): skip to the first element `>= -Inf`, give the quiet NaN when there is
/// none (every element NaN), then keep an element only if it is strictly greater. A column with a NaN
/// has a NaN `ABAND`, which `:391` rejects, so the NaN branches never reach an output.
#[inline]
fn kmax(a: &[Real], o: usize, nth: usize, ik: usize) -> Real {
    let base = o + (ik - 1) * nth;
    let mut ith = 0;
    while ith < nth && !(a[base + ith] >= Real::NEG_INFINITY) {
        ith += 1;
    }
    if ith >= nth {
        return f32::from_bits(0x7fc0_0000);
    }
    let mut r = Real::NEG_INFINITY;
    while ith < nth {
        let x = a[base + ith];
        if x > r {
            r = x;
        }
        ith += 1;
    }
    r
}

/// `K(ITH,IK)` after the normalisation of `:366-372`.
#[inline]
fn kval(a: &[Real], o: usize, nth: usize, ik: usize, ith: usize, kmax: Real) -> Real {
    if kmax < 1.0E-34 { 1. } else { a[o + ith + (ik - 1) * nth] / kmax }
}

/// `W3SWL6` on one spectrum, `:251-413`. `a`, `s` and `d` start at `o` (NSPEC each), `cg` and `wn` at
/// `k_off` (NK each); `sig` and `dden` hold `SIG(1:NK)` and `DDEN(1:NK)`. `D` is zeroed first
/// (`D = 0.`, `:358`) and then written in full.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3swl6(
    p: &Swl6Params,
    sig: &[Real],
    dden: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
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
    // `B1 = SWL6B1` (`:359`), or the steepness form of `:376-386`.
    let mut b1 = p.swl6b1;
    if p.swl6cstb1 == 0 {
        // `IK = MAXLOC(ABAND,1)` (`:383`): inline in gfortran. The first element `>= -Inf` is the
        // start, a later one wins only if strictly greater, and every element NaN gives 1.
        let mut ikp = 1;
        let mut ik = 1;
        while ik <= nk && !(aband(a, o, nth, ik) >= Real::NEG_INFINITY) {
            ik += 1;
        }
        if ik <= nk {
            ikp = ik;
            let mut lim = aband(a, o, nth, ik);
            while ik <= nk {
                let x = aband(a, o, nth, ik);
                if x > lim {
                    lim = x;
                    ikp = ik;
                }
                ik += 1;
            }
        }
        // `SUM(ABAND*DDEN/CG)`: `(ABAND*DDEN)/CG`, in `IK` order from `0.`.
        let mut sum: Real = 0.;
        let mut ik = 1;
        while ik <= nk {
            sum += (aband(a, o, nth, ik) * dden[ik - 1]) / cg[k_off + ik - 1];
            ik += 1;
        }
        // `2.*SQRT(SUM)*WN(IK)` is `(2.*SQRT(SUM))*WN(IK)`.
        b1 = p.swl6b1 * ((2. * sqrtf(sum)) * wn[k_off + ikp - 1]);
    }
    // `IKN = IRANGE(1,NSPEC,NTH)` (`:354`): `IKN(IK) = 1 + (IK-1)*NTH`, NK entries.
    let mut ik = 1;
    while ik <= nk {
        let ab = aband(a, o, nth, ik);
        // `DDIS = 0.`, set only where `ABAND(IK) > 1.E-30` (`:391`).
        let mut ddis: Real = 0.;
        if ab > 1.0E-30 {
            let km = kmax(a, o, nth, ik);
            // `SUM(K,1)` in `ITH` order from `0.`, `ANAR = 1.0/(SUM*DTH)`.
            let mut sk: Real = 0.;
            let mut ith = 0;
            while ith < nth {
                sk += kval(a, o, nth, ik, ith, km);
                ith += 1;
            }
            let anar = 1.0 / (sk * p.dth);
            // `BN = ANAR*(ABAND*SIG(1:NK)*DTH)*WN**3`: `(ANAR*((ABAND*SIG)*DTH))*WN**3`, and
            // `WN**3` is `(WN*WN)*WN` at -O3.
            let w = wn[k_off + ik - 1];
            let bn = (anar * ((ab * sig[ik - 1]) * p.dth)) * ((w * w) * w);
            // `-(2./3.)*B1*SIG(IK)*SQRT(BN(IK))` is `-((((2./3.)*B1)*SIG)*SQRT(BN))`: a sign flip after
            // the product. The flip is an integer operation so that no compiler moves the negation
            // into an operand, which would change the sign of a NaN (`W3FLX1` has the same trap).
            let t = core::hint::black_box(((2.0f32 / 3.0f32) * b1) * sig[ik - 1] * sqrtf(bn));
            ddis = f32::from_bits(t.to_bits() ^ 0x8000_0000);
        }
        // `D(IKN+(ITH-1)) = DDIS` (`:397-399`).
        let mut ith = 1;
        while ith <= nth {
            d[o + (irange_at(1, nth as i32, ik as i32) + (ith as i32 - 1) - 1) as usize] = ddis;
            ith += 1;
        }
        ik += 1;
    }
    // `S = D * A` (`:401`).
    let mut is = 0;
    while is < nspec {
        s[o + is] = d[o + is] * a[o + is];
        is += 1;
    }
}
