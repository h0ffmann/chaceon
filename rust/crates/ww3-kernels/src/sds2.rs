// SPDX-License-Identifier: MPL-2.0
//! `W3SDS2`, ST2's whitecapping dissipation of Tolman and Chalikov (1996). Translated from
//! `model/src/w3src2md.F90:582-825` at metocean/WW3 `92e2cf66` (`rust/port_report/W3SDS2.md`). The
//! `W3_S`, `W3_T`, `W3_T0`, `W3_T1` and `W3_T2` blocks are left out: all output.
//!
//! The Fortran's `EBD(NK)` is a work array. A kernel may not allocate, and each `EBD(IK)` is a plain
//! sum over the directions of one band, so `ebd` recomputes it where the source reads it: the same
//! additions in the same order, hence the same bits.
#![allow(clippy::assign_op_pattern, clippy::manual_range_contains)]

use crate::constants::{G1PI1I, G2PI3I, GRAV, TPIINV};
use crate::real::Real;
use crate::util::pow2;
use ww3_libm::{logf, powf, sqrtf};

/// Scalars of `W3GDATMD` that `W3SDS2` reads (`w3gridmd.F90:2056-2171`, `:3444-3448`). Shared with
/// the device. `SIG(0:NK+1)` and `DDEN(NK)` are separate buffers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds2Params {
    pub nk: i32,
    pub nth: i32,
    pub fte: Real,
    pub dth: Real,
    pub fpimin: Real,
    pub facti1: Real,
    pub facti2: Real,
    pub xfh: Real,
    pub xf1: Real,
    pub xf2: Real,
    pub sdsaln: Real,
    pub cdsa0: Real,
    pub cdsa1: Real,
    pub cdsa2: Real,
    pub cdsb0: Real,
    pub cdsb1: Real,
    pub cdsb2: Real,
    pub cdsb3: Real,
}

/// The scalar inputs of one call: `FPI` and `USTAR`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds2Point {
    pub fpi: Real,
    pub ustar: Real,
}

/// `EBD(IK)` (`:714-719`): `0.` plus the `NTH` values of band `ik`, in order. `a` starts at `o`.
#[inline]
fn ebd(nth: usize, a: &[Real], o: usize, ik: usize) -> Real {
    let mut e: Real = 0.;
    let mut ith = 1;
    while ith <= nth {
        e = e + a[o + (ith - 1) + (ik - 1) * nth];
        ith += 1;
    }
    e
}

/// `MOD(X, 1.)`: gfortran calls `fmodf`, which is exact, so `x - trunc(x)` is the same number
/// below 2^23. From there on `x` is an integer and the result is zero.
#[inline]
fn mod1(x: Real) -> Real {
    if x > -8388608. && x < 8388608. {
        x - (x as i32) as Real
    } else if x != x || x - x != 0. {
        Real::NAN // a NaN or an infinity (`fmodf(inf, 1.)` is NaN)
    } else {
        0.
    }
}

/// `W3SDS2` on one spectrum. `a`, `s` and `d` start at `o` (`NSPEC` values, `(NTH,NK)`); `cg`, `k`
/// and `alfa` hold `NK` values from `v`. `sig` is `SIG(0:NK+1)`, indexed by `IK`, and `dden` is
/// `DDEN(NK)`, so `DDEN(IK)` is `dden[IK - 1]`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sds2(
    p: &Sds2Params,
    sig: &[Real],
    dden: &[Real],
    pt: &Sds2Point,
    a: &[Real],
    cg: &[Real],
    k: &[Real],
    alfa: &[Real],
    s: &mut [Real],
    d: &mut [Real],
    o: usize,
    v: usize,
) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let (fpi, ustar) = (pt.fpi, pt.ustar);
    // 1.a HW (`:710-731`)
    let fhw = p.xfh * fpi;
    let xhw0 = p.facti2 + logf(fhw) * p.facti1;
    let xr = xhw0 + 0.5;
    // `INT` is `fcvtzs`; below 1 the Fortran would index `EBD` out of bounds, so the band is
    // held at 1 there (never reached by a capture ⚠).
    let ikhw_raw = xr as i32;
    let ikhw_raw = if (nk as i32) < ikhw_raw { nk as i32 } else { ikhw_raw };
    let ikhw = if ikhw_raw < 1 { 1 } else { ikhw_raw as usize };
    let ebd_nk = ebd(nth, a, o, nk);
    let ebd_cg = ebd_nk / cg[v + nk - 1];
    let hw;
    if sig[nk + 1] > fhw {
        let mut xhw = 1. - mod1(xr);
        if ikhw == nk {
            // `MAX(0., XHW-0.5)` is fmaxnm: NaN gives 0., and so does -0.
            let t = xhw - 0.5;
            xhw = if t > 0. { t } else { 0. };
        }
        let e = ebd(nth, a, o, ikhw);
        let mut h = ((e * xhw) * dden[ikhw - 1]) / cg[v + ikhw - 1];
        let mut ik = ikhw + 1;
        while ik <= nk {
            h = h + (ebd(nth, a, o, ik) * dden[ik - 1]) / cg[v + ik - 1];
            ik += 1;
        }
        hw = 4. * sqrtf(h + p.fte * ebd_cg);
    } else {
        // `EHIGH = EBD(NK)/CG(NK) * SIG(NK)*DTH * (SIG(NK)/FHW)**5`: `x**5` is `(x*x)*x*(x*x)`
        // in GIMPLE, `p3 = x*p2`, `p5 = p3*p2`.
        let sk = sig[nk];
        let x = sk / fhw;
        let p2 = x * x;
        let p3 = x * p2;
        let p5 = p3 * p2;
        let ehigh = ((sk * ebd_cg) * p.dth) * p5;
        hw = 4. * sqrtf((fhw * 0.25) * ehigh);
    }
    // 1.b PHI (`:735-737`); `MAX(FPIMIN, X)` is fmaxnm, NaN gives FPIMIN.
    let x = ((fpi * TPIINV) * ustar) / GRAV;
    let fpit = if x > p.fpimin { x } else { p.fpimin };
    let phi = (p.cdsb1 * fpit + p.cdsb0) + p.cdsb2 / powf(fpit, p.cdsb3);
    // 1.c filter (`:739-744`). `AFILT + BFILT*SIG` is `SIG*BFILT - AF1*BFILT` in the GIMPLE: the
    // same number, and the sign of a NaN comes from the operand that carries it.
    let af2 = fpi * p.xf2;
    let af1 = fpi * p.xf1;
    let bfilt = 1. / (af2 - af1);
    let afb = af1 * bfilt;
    // 1.d constants (`:746-749`): `-2.*USTAR*HW*PHI` is a negated product.
    let cdist = -(((ustar * 2.) * hw) * phi);
    let cdish = (ustar * ustar) * G2PI3I;
    let cdisp = ustar * G1PI1I;
    // 2. (`:757-774`) D(1,IK), then 3. the copy over the directions and `S = D*A`.
    let mut ik = 1;
    while ik <= nk {
        let sg = sig[ik];
        let t = sg * bfilt - afb;
        let m = if t > 0. { t } else { 0. }; // MAX(0., .), fmaxnm
        let filt = if m < 1. { m } else { 1. }; // MIN(1., .), fminnm
        let kk = k[v + ik - 1];
        let a1 = (cdist * (1. - filt)) * pow2(kk);
        let d1;
        if filt > 0. {
            let r = alfa[v + ik - 1] / p.sdsaln;
            let sg3 = sg * pow2(sg);
            let b = (((p.cdsa0 * filt) * cdish) * sg3) * powf(r, pow_of(p, cdisp, sg));
            d1 = a1 - b;
        } else {
            d1 = a1;
        }
        let mut ith = 1;
        while ith <= nth {
            let i = o + (ith - 1) + (ik - 1) * nth;
            d[i] = d1;
            s[i] = d1 * a[i];
            ith += 1;
        }
        ik += 1;
    }
}

/// `POW = MIN(25., CDSA1 / (CDISP*SIG(IK))**CDSA2)` (`:759`); `fminnm` gives 25. for NaN.
#[inline]
fn pow_of(p: &Sds2Params, cdisp: Real, sg: Real) -> Real {
    let q = p.cdsa1 / powf(sg * cdisp, p.cdsa2);
    if q < 25. { q } else { 25. }
}
