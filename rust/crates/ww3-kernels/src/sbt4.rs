// SPDX-License-Identifier: MPL-2.0
//! `W3SBT4`, SHOWEX bottom friction over a movable bed, and `TABU_ERF`, its table. Translated from
//! `model/src/w3sbt4md.F90:205-275` and `:279-576` at metocean/WW3 `92e2cf66`. The order of
//! operations is the one gfortran 12.2 `-O3 -ffp-contract=off` compiled on aarch64, read from
//! `__w3sbt4md_MOD_w3sbt4` in the reference's `ww3_shel` (`rust/port_report/W3SBT4.md`): every
//! `**2` is a product, `**1.4` and `**SBTCX(2)` call `powf`, `SQRT(TPI)` is a folded constant, and
//! nothing fuses.
// `x = x + y` and `-1.*XERFMAX` keep the Fortran line; `clamp` passes NaN through where
// `MAX(MIN(...))` (`fmaxnm`, `fminnm`) does not.
#![allow(clippy::assign_op_pattern, clippy::neg_multiply, clippy::manual_clamp)]

use crate::constants::{GRAV, SED_SG, TPI};
use crate::real::Real;
use crate::sin4::SIZEFWTABLE;
use ww3_libm::{erff, expf, log10f, powf, sinhf, sqrtf};

/// `SIZEERFTABLE`, `w3sbt4md.F90:108`. `ERFTABLE` is `(0:SIZEERFTABLE)`.
pub const SIZEERFTABLE: i32 = 300;
/// `XERFMAX`, `w3sbt4md.F90:111`.
pub const XERFMAX: Real = 4.;
/// `ABMIN` of `CONSTANTS`, the lower end of `FWTABLE`.
const ABMIN: Real = -1.;
/// The three-point quadrature offsets `XSUB`, `w3sbt4md.F90:392`. `WSUB` is declared and never used.
const XSUB: [Real; 3] = [-0.001, 0.001, 0.];

/// `DELXERF`, `w3sbt4md.F90:263`, which `W3SBT4` reads back from the module.
#[inline]
pub fn delxerf() -> Real {
    (2. * XERFMAX) / SIZEERFTABLE as Real
}

/// `TABU_ERF`, `w3sbt4md.F90:263-272`: `ERFTABLE(0:SIZEERFTABLE)` into `t`.
///
/// `2**(1/2)` is the integer `2**0`, so 1, and `y` is the normal cumulative distribution of
/// `x*sqrt(2)`, not of `x`; the source's comment and its arithmetic disagree, and the arithmetic is
/// what runs. gfortran peels `I=0` and folds `erf(-4.)` to `-1.` with MPFR, so that entry does not
/// call `erff` (`f98cc` in the reference's `ww3_shel`).
pub fn tabu_erf(t: &mut [Real]) {
    let d = delxerf();
    let mut i = 0;
    while i <= SIZEERFTABLE {
        let x = -1. * XERFMAX + i as Real * d;
        let e = if i == 0 { -1. } else { erff(x) };
        let y = if x < 0. { (1. - e.abs()) / 2. } else { (1. + e) / 2. };
        t[i as usize] = y;
        i += 1;
    }
}

/// Scalars of `W3GDATMD` and the module that `W3SBT4` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sbt4Params {
    pub nk: i32,
    pub nth: i32,
    /// `SBTCX(1:10)`; `[0]` is `SBTCX(1)`.
    pub sbtcx: [Real; 10],
    /// `DELAB` of `CONSTANTS`, set by `TABU_FW`.
    pub delab: Real,
    /// `DELXERF` of the module, set by `TABU_ERF`.
    pub delxerf: Real,
}

/// The read-only tables of one grid.
#[derive(Clone, Copy)]
pub struct Sbt4Tables<'a> {
    /// `SIG(0:NK+1)`.
    pub sig: &'a [Real],
    /// `DDEN(NK)` (`w3gdatmd.F90:2038`).
    pub dden: &'a [Real],
    /// `ECOS(NSPEC+NTH)` and `ESIN`: the momentum loop reads them at `IS`, not `ITH`.
    pub ecos: &'a [Real],
    pub esin: &'a [Real],
    /// `FWTABLE(0:SIZEFWTABLE)`.
    pub fwtable: &'a [Real],
    /// `ERFTABLE(0:SIZEERFTABLE)`.
    pub erftable: &'a [Real],
}

/// The scalar inputs of one call: `DEPTH`, `D50` and `PSIC`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sbt4Point {
    pub depth: Real,
    pub d50: Real,
    pub psic: Real,
}

/// What `W3SBT4` returns besides `S`, `D` and `BEDFORM`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sbt4Out {
    /// `TAUBBL(1:2)`.
    pub taubbl: [Real; 2],
    /// Which roughness was used, for reading what a capture reached: 0 relict and 1 ripples with
    /// no subgrid (`w3sbt4md.F90:499-513`), 2 the subgrid mixture (`:517-544`).
    pub branch: i32,
}

/// `W3SBT4`, `w3sbt4md.F90:279-576`, on one spectrum. `a`, `s` and `d` start at `o`, `cg` and `wn`
/// at `k_off`, `CBETA(NK)` at `w_off` in `w`, `BEDFORM(1:3)` at `b_off` in `bedform`: intent
/// `INOUT` in the source, written and never read.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sbt4(
    p: &Sbt4Params,
    t: &Sbt4Tables,
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    pt: &Sbt4Point,
    bedform: &mut [Real],
    s: &mut [Real],
    d: &mut [Real],
    w: &mut [Real],
    o: usize,
    k_off: usize,
    w_off: usize,
    b_off: usize,
) -> Sbt4Out {
    let (depth, d50, psic) = (pt.depth, pt.d50, pt.psic);
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let sbtcx = &p.sbtcx;
    // 1. Grain size.
    let mut dd50 = d50.max(1e-5);
    dd50 = dd50.min(1.);
    let background = sbtcx[5].max(sbtcx[6] * dd50);
    // 2. Subgrid loop. DSUM(NK) of the source is only ever the value of its own band: a scalar.
    let mut taubbl: [Real; 2] = [0., 0.];
    let mut shields: [Real; 3] = [0., 0., 0.];
    let (mut uorb, mut aorb, mut uorb2) = (0., 0., 0.);
    let (mut lx, mut ly) = (0., 0.);
    let mut isub = 0;
    while isub < 3 {
        // 2.a Bulk parameters.
        let dsub = depth * (1. + XSUB[isub]);
        uorb = 0.;
        aorb = 0.;
        let mut ax: Real = 0.;
        let mut ay: Real = 0.;
        let mut ik = 0;
        while ik < nk {
            let sig = t.sig[ik + 1];
            if wn[k_off + ik] * dsub < 6. {
                let mut eb: Real = 0.;
                let mut ebx: Real = 0.;
                let mut eby: Real = 0.;
                let mut ith = 0;
                while ith < nth {
                    let is = o + ith + ik * nth;
                    eb = eb + a[is];
                    ebx = ebx + a[is] * t.ecos[ith];
                    eby = eby + a[is] * t.esin[ith];
                    ith += 1;
                }
                let sh = sinhf(wn[k_off + ik] * dsub);
                let cbeta = 0.5 * (sig * sig) / (GRAV * (sh * sh));
                w[w_off + ik] = cbeta;
                let factor = (t.dden[ik] / cg[k_off + ik]) * 2. * cbeta * GRAV;
                let varu = eb * factor;
                uorb = uorb + varu;
                aorb = aorb + varu / (sig * sig);
                ax = ax + (ebx * factor);
                ay = ay + (eby * factor);
            } else {
                w[w_off + ik] = 0.;
            }
            ik += 1;
        }
        // RMS orbital amplitudes.
        uorb2 = 2. * uorb;
        uorb = sqrtf(uorb2.max(1.0e-7));
        aorb = sqrtf((2. * aorb).max(1.0e-7));
        // Potential ripple wavelength.
        let r = sqrtf(ax * ax + ay * ay + 1e-12);
        lx = aorb * 1.7 * ax / r;
        ly = aorb * 1.7 * ay / r;
        // 2.b Skin roughness and the Shields parameter.
        let fw = fw_of(p, t, aorb / dd50);
        let psi = fw * uorb2 / (2. * GRAV * (SED_SG - 1.) * dd50);
        shields[isub] = psi / psic;
        isub += 1;
    }
    let dpsi = (shields[1] - shields[0]) / (XSUB[1] - XSUB[0]) * sbtcx[4];
    let ksubn;
    let branch;
    if dpsi.abs() < 0.0001 * shields[2] || dpsi.abs() < 1.0e-8 {
        // No subgrid.
        if shields[2] > sbtcx[2] {
            let ksubr = aorb * sbtcx[0] * powf(shields[2], sbtcx[1]);
            let ksubs = aorb * 0.0655 * powf(uorb2 / ((SED_SG - 1.) * GRAV * aorb), 1.4);
            ksubn = ksubr + ksubs;
            branch = 1;
            bedform[b_off + 1] = lx;
            bedform[b_off + 2] = ly;
        } else {
            ksubn = background.max(aorb * sbtcx[3]);
            branch = 0;
            bedform[b_off + 1] = -lx;
            bedform[b_off + 2] = -ly;
        }
    } else {
        // Subgrid.
        let psix = (sbtcx[2] - shields[2]) / dpsi;
        let psixt = ((psix + XERFMAX) / p.delxerf).max(0.);
        let inde = ((SIZEERFTABLE - 1).min(psixt as i32)).max(0);
        let deli1 = (1. as Real).min(psixt - inde as Real);
        let deli2 = 1. - deli1;
        let proba2 = (t.erftable[inde as usize] * deli2 + t.erftable[inde as usize + 1] * deli1)
            .min(1.)
            .max(0.);
        let proba1 = 1. - proba2;
        let psin2 = (shields[2] + expf(-(0.5 * (psix * psix))) / sqrtf(TPI) * dpsi / (proba2 + 0.0001))
            .max(sbtcx[2]);
        ksubn = proba1 * background.max(aorb * sbtcx[3])
            + proba2
                * aorb
                * (sbtcx[0] * powf(psin2, sbtcx[1])
                    + 0.0655 * powf(uorb2 / ((SED_SG - 1.) * GRAV * aorb), 1.4));
        branch = 2;
        if proba2 > 0.5 {
            bedform[b_off + 1] = lx;
            bedform[b_off + 2] = ly;
        } else {
            bedform[b_off + 1] = -lx;
            bedform[b_off + 2] = -ly;
        }
    }
    bedform[b_off] = ksubn;
    // 2.c Friction factor of the full roughness.
    let fw = fw_of(p, t, aorb / ksubn);
    // 5. Outputs and the momentum lost to the bottom.
    let mut ik = 0;
    while ik < nk {
        let sig = t.sig[ik + 1];
        let const2 = t.dden[ik] / cg[k_off + ik] * GRAV / (sig / wn[k_off + ik]);
        let dsum = -(fw * uorb * w[w_off + ik]);
        let mut ith = 0;
        while ith < nth {
            let is = ith + ik * nth;
            d[o + is] = dsum;
            let temp2 = const2 * d[o + is] * a[o + is];
            taubbl[0] = taubbl[0] - temp2 * t.ecos[is];
            taubbl[1] = taubbl[1] - temp2 * t.esin[is];
            s[o + is] = d[o + is] * a[o + is];
            ith += 1;
        }
        ik += 1;
    }
    Sbt4Out { taubbl, branch }
}

/// The `FWTABLE` lookup of `w3sbt4md.F90:478-482` and `:550-554`, at `AORB/roughness = ratio`.
#[inline]
fn fw_of(p: &Sbt4Params, t: &Sbt4Tables, ratio: Real) -> Real {
    let xi = ((log10f(ratio.max(0.3)) - ABMIN) / p.delab).max(1.);
    let ind = (SIZEFWTABLE - 1).min(xi as i32);
    let deli1 = (1. as Real).min(xi - ind as Real);
    let deli2 = 1. - deli1;
    t.fwtable[ind as usize] * deli2 + t.fwtable[ind as usize + 1] * deli1
}
