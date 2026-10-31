// SPDX-License-Identifier: MPL-2.0
//! `W3SRCE`, the source-term integrator, with `W3SLN1`, `W3SBT1` and `W3SDB1`. Translated from
//! `model/src/w3srcemd.F90`, `w3sln1md.F90`, `w3sbt1md.F90` and `w3sdb1md.F90` at metocean/WW3 `92e2cf66`,
//! as `switches/switch_chaceon_shrd` builds them (`gfortran -E` gives that text), with MetOcean's
//! `USTAR` floors. The bits are the reference's (`rust/port_report/W3SRCE.md`).
//!
//! `W3SDB1` holds `f64` on the device: `docs/FLOAT_REPRESENTATION_202610.md`. One call of `W3SRCE`
//! keeps no state, so a device runs one invocation per sea point.
#![allow(clippy::assign_op_pattern, unused_assignments)] // `x = x + y` keeps the Fortran line; its macros set every output.

use crate::constants::{DWAT, GRAV, TPI, TPIINV};
use crate::real::{Dbl, Real};
use crate::sds4::{self, Sds4Params, Sds4Point, w3sds4};
use crate::sin4::{Sin4Out, Sin4Params, Sin4Point, Spr4Out, Spr4Params, Spr4Point, w3sin4, w3spr4};
use crate::snl1::{self, Snl1Params, w3snl1};
use crate::util::{pow3, pow4, sign};
use ww3_libm::{expf, logf, sincosf, sqrtf, tanhf};

pub const SRCE_DIRECT: i32 = 0;
pub const SRCE_IMP_POST: i32 = 1;

/// Scalars of `W3GDATMD` that `W3SRCE` and its three small routines read. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SrceParams {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    /// `FDONLY`.
    pub fdonly: u32,
    /// `INFLAGS2(4)`.
    pub inflags2_4: u32,
    pub dmin: Real,
    pub dtmax: Real,
    pub dtmin: Real,
    pub facti1: Real,
    pub facti2: Real,
    pub fachfa: Real,
    pub facp: Real,
    pub xflt: Real,
    pub xrel: Real,
    pub fhmax: Real,
    pub ffxfm: Real,
    pub ffxpm: Real,
    pub ffxfa: Real,
    pub slnc1: Real,
    pub fspm: Real,
    pub fshf: Real,
    pub sbtc1: Real,
    pub sdbc1: Real,
    pub sdbc2: Real,
    pub icescales: [Real; 4],
    pub snl: Snl1Params,
    pub sds: Sds4Params,
    pub sin: Sin4Params,
    pub spr: Spr4Params,
}

/// The tables, read-only, one set for the grid.
pub struct SrceTables<'a> {
    /// `SIG(0:NK+1)`.
    pub sig: &'a [Real],
    pub sig2: &'a [Real],
    pub dden: &'a [Real],
    pub dden2: &'a [Real],
    pub ec2: &'a [Real],
    pub es2: &'a [Real],
    pub esc: &'a [Real],
    pub ecos: &'a [Real],
    pub esin: &'a [Real],
    pub af11: &'a [Real],
    pub pm: &'a [i32],
    pub ic: &'a [i32],
    pub satindices: &'a [i32],
    pub satweights: &'a [Real],
    pub cumulw: &'a [Real],
    pub iktab: &'a [i32],
    pub dcki: &'a [Real],
    pub qbi: &'a [Real],
    pub fwtable: &'a [Real],
    pub tauhft: &'a [Real],
    pub tauhft2: &'a [Real],
    pub taut: &'a [Real],
}

/// The inputs of one call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SrcePoint {
    pub srce_call: i32,
    pub it: i32,
    pub d_inp: Real,
    pub u10abs: Real,
    pub u10dir: Real,
    pub ustar: Real,
    pub ustdir: Real,
    pub ice: Real,
    pub dtg: Real,
    pub dair: Real,
    pub coef: Real,
}

/// What one call returns besides `SPEC` and `WHITECAP`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SrceOut {
    pub ustar: Real,
    pub ustdir: Real,
    pub dtdyn: Real,
    pub fcut: Real,
    pub tauwx: Real,
    pub tauwy: Real,
    pub tauox: Real,
    pub tauoy: Real,
    pub tauwix: Real,
    pub tauwiy: Real,
    pub tauwnx: Real,
    pub tauwny: Real,
    pub phiaw: Real,
    pub charn: Real,
    pub tws: Real,
    pub phioc: Real,
    pub phibbl: Real,
    pub taubbl: [Real; 2],
    pub tauice: [Real; 2],
    pub phice: Real,
    pub tauocx: Real,
    pub tauocy: Real,
    pub wnmean: Real,
}

/// The automatic arrays, one range per sea point. All but the last four hold `NSPEC` per point.
pub struct SrceWork<'a> {
    pub specinit: &'a mut [Real],
    pub dam: &'a mut [Real],
    pub wn2: &'a mut [Real],
    pub vsln: &'a mut [Real],
    pub vsin: &'a mut [Real],
    pub vdin: &'a mut [Real],
    pub vsnl: &'a mut [Real],
    pub vdnl: &'a mut [Real],
    pub vsds: &'a mut [Real],
    pub vdds: &'a mut [Real],
    pub vsbt: &'a mut [Real],
    pub vdbt: &'a mut [Real],
    pub vsdb: &'a mut [Real],
    pub vddb: &'a mut [Real],
    pub vs: &'a mut [Real],
    pub vd: &'a mut [Real],
    pub brlambda: &'a mut [Real],
    pub llws: &'a mut [u32],
    /// `snl1::work_len`.
    pub w_snl: &'a mut [Real],
    /// `sds4::work_len`.
    pub w_sds: &'a mut [Real],
    pub iw_sds: &'a mut [i32],
    /// `work_small`: `W3SPR4`'s two `NK` arrays, `W3SLN1`'s `NTH` and `NK`, `W3SBT1`'s `NK`.
    pub w_small: &'a mut [Real],
    pub whitecap: &'a mut [Real],
    pub sin_out: &'a mut [Sin4Out],
    pub spr_out: &'a mut [Spr4Out],
}

/// Length per point of `SrceWork::w_small`.
#[inline]
pub fn work_small(p: &SrceParams) -> usize {
    (4 * p.nk + p.nth) as usize
}

/// Lengths per point of `w_snl`, `w_sds` and `iw_sds`.
#[inline]
pub fn work_lens(p: &SrceParams) -> (usize, usize, usize) {
    let (lw, li) = sds4::work_len(&p.sds);
    (snl1::work_len(&p.snl), lw, li)
}

/// `W3SLN1`: one spectrum into `s`, starting at `o`. `w` holds `DIRF(NTH)` and `WNF(NK)`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sln1(
    p: &SrceParams,
    sig: &[Real],
    ecos: &[Real],
    esin: &[Real],
    k: &[Real],
    fhigh: Real,
    ustar: Real,
    usdir: Real,
    s: &mut [Real],
    w: &mut [Real],
    o: usize,
    k_off: usize,
    w_off: usize,
) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let o_wnf = w_off + nth;
    let (sinu, cosu) = sincosf(usdir);
    let mut ith = 0;
    while ith < nth {
        w[w_off + ith] = pow4((ecos[ith] * cosu + esin[ith] * sinu).max(0.));
        ith += 1;
    }
    let fac = p.slnc1 * pow4(ustar);
    let ff1 = p.fspm * GRAV / (28. * ustar);
    let ff2 = p.fshf * sig[nk].min(fhigh);
    let ffilt = ff1.max(ff2).min(2. * sig[nk]);
    let mut ik = 0;
    while ik < nk {
        let rfr = sig[ik + 1] / ffilt;
        w[o_wnf + ik] = if rfr < 0.5 { 0. } else { fac / k[k_off + ik] * expf(-(1. / pow4(rfr))) };
        ik += 1;
    }
    let mut ik = 0;
    while ik < nk {
        let mut ith = 0;
        while ith < nth {
            s[o + ith + ik * nth] = w[o_wnf + ik] * w[w_off + ith];
            ith += 1;
        }
        ik += 1;
    }
}

/// `W3SBT1`: one spectrum. `a`, `s` and `d` start at `o`, `cg` and `wn` at `k_off`, `CBETA` at `w_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sbt1(
    p: &SrceParams,
    sig: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    depth: Real,
    s: &mut [Real],
    d: &mut [Real],
    w: &mut [Real],
    o: usize,
    k_off: usize,
    w_off: usize,
) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec as usize);
    if depth * wn[k_off] > 6. {
        let mut is = 0;
        while is < nspec {
            d[o + is] = 0.;
            s[o + is] = 0.;
            is += 1;
        }
        return;
    }
    let factor = p.sbtc1 / depth;
    let mut ik = 1;
    while ik <= nk {
        let i = (ik - 1) as usize;
        if wn[k_off + i] * depth > 6. {
            break;
        }
        w[w_off + i] = factor * (cg[k_off + i] * wn[k_off + i] / sig[ik as usize] - 0.5).max(0.);
        ik += 1;
    }
    let nscut = ((ik - 1) * nth) as usize;
    let mut is = 0;
    while is < nspec {
        // MAPWN(IS) is the wavenumber index of IS.
        d[o + is] = if is < nscut { w[w_off + is / nth as usize] } else { 0. };
        s[o + is] = d[o + is] * a[o + is];
        is += 1;
    }
}

/// `W3SDB1`: one spectrum. On a spectrum with almost no energy it returns with `s` and `d` untouched,
/// as the Fortran does, so they keep the previous step's values.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sdb1(
    p: &SrceParams,
    a: &[Real],
    depth: Real,
    emean: Real,
    fmean: Real,
    wnmean: Real,
    s: &mut [Real],
    d: &mut [Real],
    o: usize,
) {
    let nspec = p.nspec as usize;
    // DBLE(1.E-15): widened from `f32`.
    let thr = 1.0e-15_f32 as Dbl;
    let mut sum = 0_f32;
    let mut is = 0;
    while is < nspec {
        sum = sum + a[o + is];
        is += 1;
    }
    if (sum as Dbl) < thr {
        return;
    }
    let (sdbc1, sdbc2) = (p.sdbc1 as Dbl, p.sdbc2 as Dbl);
    let hm = if p.fdonly != 0 {
        sdbc2 * depth as Dbl
    } else {
        let wn = wnmean as Dbl;
        sdbc2 / wn * ww3_libm::tanh(wn * depth.max(0.) as Dbl)
    };
    let hrms = ww3_libm::sqrt(8.0 * emean as Dbl);
    let (bb, b);
    if hm > thr {
        bb = hrms * hrms / (hm * hm);
        b = ww3_libm::sqrt(bb);
    } else {
        bb = 0.0;
        b = 0.0;
    }
    let mut q0 = 0.0;
    if b <= 0.5 {
        q0 = 0.0;
    } else if b <= 1.0 {
        let t = 2.0 * b - 1.0;
        q0 = t * t;
    }
    let qb: Dbl;
    if b <= 0.2 {
        qb = 0.0;
    } else if b < 1.0 {
        let arg = ww3_libm::exp((q0 - 1.0) / bb);
        let mut q = q0 - bb * (q0 - arg) / (bb - arg);
        let mut i = 0;
        while i < 3 {
            q = ww3_libm::exp((q - 1.0) / bb);
            i += 1;
        }
        qb = q;
    } else {
        qb = 1.0 - thr;
    }
    let cbj: Dbl;
    if bb > thr && (bb - qb).abs() > thr {
        cbj = if bb < 1.0 { 2.0 * sdbc1 * qb * fmean as Dbl / bb } else { 2.0 * sdbc1 * fmean as Dbl * bb };
    } else {
        cbj = 0.0;
    }
    let dv = (-cbj) as Real;
    let mut is = 0;
    while is < nspec {
        d[o + is] = dv;
        s[o + is] = dv * a[o + is];
        is += 1;
    }
}

/// One sea point: `W3SRCE`. `point` selects its ranges of `spec`, `wn1`, `cg1`, `w` and `out`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3srce(
    p: &SrceParams,
    t: &SrceTables,
    pt: &SrcePoint,
    spec: &mut [Real],
    wn1: &[Real],
    cg1: &[Real],
    w: &mut SrceWork,
    out: &mut [SrceOut],
    point: usize,
) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec);
    let so = point * nspec as usize;
    let ko = point * nk as usize;
    let wo = point * 4;
    let (l_snl, l_sds, l_ids) = work_lens(p);
    let (o_snl, o_sds, o_ids) = (point * l_snl, point * l_sds, point * l_ids);
    let o_small = point * work_small(p);
    let o_spr = o_small;
    let o_ln = o_small + 2 * nk as usize;
    let o_bt = o_ln + (nth + nk) as usize;
    let (srce_call, it, u10abs, u10dir, dtg, dair) = (pt.srce_call, pt.it, pt.u10abs, pt.u10dir, pt.dtg, pt.dair);
    let ice_on = p.inflags2_4 != 0;

    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! sp {
        ($is:expr) => {
            spec[ix!(so, $is)]
        };
    }
    macro_rules! wn {
        ($ik:expr) => {
            wn1[ix!(ko, $ik)]
        };
    }
    macro_rules! cg {
        ($ik:expr) => {
            cg1[ix!(ko, $ik)]
        };
    }
    macro_rules! dd {
        ($ik:expr) => {
            t.dden[ix!(0, $ik)]
        };
    }
    macro_rules! sg {
        ($ik:expr) => {
            t.sig[($ik) as usize]
        };
    }
    macro_rules! v {
        ($a:ident, $is:expr) => {
            w.$a[ix!(so, $is)]
        };
    }
    macro_rules! wc {
        ($i:expr) => {
            w.whitecap[wo + $i - 1]
        };
    }

    let depth = p.dmin.max(pt.d_inp);
    macro_rules! scale {
        ($i:expr) => {
            (1. - pt.ice * p.icescales[$i]).min(1.).max(0.)
        };
    }
    let (icescaleln, icescalein, icescalenl, icescaleds): (Real, Real, Real, Real) =
        (scale!(0), scale!(1), scale!(2), scale!(3));
    each!(is, 1, nspec, {
        v!(vsln, is) = 0.;
        v!(vsin, is) = 0.;
        v!(vdin, is) = 0.;
        v!(vsnl, is) = 0.;
        v!(vdnl, is) = 0.;
        v!(vsds, is) = 0.;
        v!(vdds, is) = 0.;
        v!(vsbt, is) = 0.;
        v!(vdbt, is) = 0.;
        v!(vsdb, is) = 0.;
        v!(vddb, is) = 0.;
        v!(brlambda, is) = 0.;
    });
    let drat = dair / DWAT;
    each!(ik, 1, nk, {
        let dam = p.facp / (sg!(ik) * pow3(wn!(ik)));
        let is0 = (ik - 1) * nth;
        each!(ith, 1, nth, {
            v!(dam, ith + is0) = dam;
            v!(wn2, ith + is0) = wn!(ik);
        });
    });

    let mut dtdyn: Real = 0.;
    let mut dttot: Real = 0.;
    let mut nsteps: i32 = 0;
    let mut phiaw: Real = 0.;
    let mut phibbl: Real = 0.;
    let (mut tauwix, mut tauwiy, mut tauwnx, mut tauwny): (Real, Real, Real, Real) = (0., 0., 0., 0.);
    let (mut tauwax, mut tauway): (Real, Real) = (0., 0.);
    let (taubbl1, taubbl2): (Real, Real) = (0., 0.);
    let (mut tauice1, mut tauice2, mut phice): (Real, Real, Real) = (0., 0., 0.);
    let (mut tauwx, mut tauwy): (Real, Real) = (0., 0.);
    let (mut ustar, mut ustdir) = (pt.ustar, pt.ustdir);
    each!(i, 0, 3, {
        w.whitecap[wo + i] = 0.;
    });
    let (mut emean, mut fmean, mut fmean1, mut wnmean, mut amax): (Real, Real, Real, Real, Real) =
        (0., 0., 0., 0., 0.);
    let (mut cd_unused, mut z0, mut charn, mut fmeanws, mut dlwmean): (Real, Real, Real, Real, Real) =
        (0., 0., 0., 0., 0.);

    // W3SPR4 with this call's state, its outputs back into the locals.
    macro_rules! spr4 {
        () => {
            let q = Spr4Point { u: u10abs, udir: u10dir, ustar, usdir: ustdir, tauwx, tauwy };
            w3spr4(
                &p.spr, t.sig, t.dden, t.ecos, t.esin, t.taut, &*spec, cg1, wn1, &*w.llws, &q, &mut *w.w_small,
                &mut *w.spr_out, so, ko, o_spr, point,
            );
            let r = w.spr_out[point];
            emean = r.emean;
            fmean = r.fmean;
            fmean1 = r.fmean1;
            wnmean = r.wnmean;
            amax = r.amax;
            ustar = r.ustar;
            ustdir = r.usdir;
            cd_unused = r.cd;
            z0 = r.z0;
            charn = r.charn;
            fmeanws = r.fmeanws;
            dlwmean = r.dlwmean;
        };
    }
    macro_rules! sin4 {
        () => {
            let q = Sin4Point { ustar, drat, usdir: u10dir, z0 };
            w3sin4(
                &p.sin, t.sig, t.sig2, t.dden, t.dden2, t.ecos, t.esin, t.fwtable, t.tauhft, t.tauhft2, &*spec,
                cg1, &*w.wn2, &q, &mut *w.vsin, &mut *w.vdin, &mut *w.llws, &mut *w.sin_out, so, ko, point,
            );
            let r = w.sin_out[point];
            tauwx = r.tauwx;
            tauwy = r.tauwy;
            tauwax = r.tauwnx;
            tauway = r.tauwny;
        };
    }

    if it == 0 {
        each!(is, 1, nspec, {
            v!(llws, is) = 1;
        });
        ustar = 0.001;
        ustdir = 0.;
    } else {
        spr4!();
        sin4!();
    }
    spr4!();
    let tws = 1. / fmeanws;
    each!(is, 1, nspec, {
        v!(specinit, is) = sp!(is);
    });
    let fage: Real = 0.;
    ustar = ustar.max(0.001);
    let mut fhigh = ((p.ffxfm + fage) * fmean1.max(fmeanws)).max(p.ffxpm / ustar);

    let mut phinl: Real = 0.;
    loop {
        nsteps += 1;
        w3sln1(
            p, t.sig, t.ecos, t.esin, wn1, fhigh, ustar, u10dir, &mut *w.vsln, &mut *w.w_small, so, ko, o_ln,
        );
        sin4!();
        w3snl1(
            &p.snl, t.sig, t.af11, t.pm, t.ic, &*spec, cg1, wnmean * depth, &mut *w.vsnl, &mut *w.vdnl,
            &mut *w.w_snl, so, ko, o_snl,
        );
        let sq = Sds4Point { ustar, usdir: ustdir, depth, dair, dlwmean };
        w3sds4(
            &p.sds, t.sig, t.dden, t.ec2, t.es2, t.esc, t.ecos, t.esin, t.satindices, t.satweights, t.cumulw,
            t.iktab, t.dcki, t.qbi, &*spec, wn1, cg1, &sq, &mut *w.vsds, &mut *w.vdds, &mut *w.brlambda,
            &mut *w.whitecap, &mut *w.w_sds, &mut *w.iw_sds, so, ko, wo, o_sds, o_ids,
        );
        w3sdb1(p, &*spec, depth, emean, fmean, wnmean, &mut *w.vsdb, &mut *w.vddb, so);
        w3sbt1(
            p, t.sig, &*spec, cg1, wn1, depth, &mut *w.vsbt, &mut *w.vdbt, &mut *w.w_small, so, ko, o_bt,
        );

        let nkh = nk.min((p.facti2 + p.facti1 * logf(fhigh.max(1.0e-7))) as i32);
        let nkh1 = nk.min(nkh + 1);
        let nspech = nkh1 * nth;
        let mut dt = (dtg - dttot).min(p.dtmax);
        let afilt = v!(dam, nspec).max(p.xflt * amax);
        if ice_on {
            each!(is, 1, nspech, {
                v!(vsnl, is) = icescalenl * v!(vsnl, is);
                v!(vdnl, is) = icescalenl * v!(vdnl, is);
                v!(vsln, is) = icescaleln * v!(vsln, is);
                v!(vsin, is) = icescalein * v!(vsin, is);
                v!(vdin, is) = icescalein * v!(vdin, is);
                v!(vsds, is) = icescaleds * v!(vsds, is);
                v!(vdds, is) = icescaleds * v!(vdds, is);
            });
        }
        each!(is, 1, nspech, {
            v!(vs, is) = v!(vsln, is) + v!(vsin, is) + v!(vsnl, is) + v!(vsds, is) + v!(vsbt, is);
            v!(vd, is) = v!(vdin, is) + v!(vdnl, is) + v!(vdds, is) + v!(vdbt, is);
            let damax = v!(dam, is).min((p.xrel * v!(specinit, is)).max(afilt));
            let afac = 1. / (v!(vs, is) / damax).abs().max(1.0e-10);
            dt = dt.min(afac / (1. + afac * v!(vd, is).min(0.)).max(1.0e-10));
        });
        dt = dt.max(0.5);
        dtdyn = dtdyn + dt;
        let idt = 1 + (0.99 * (dtg - dttot) / dt) as i32;
        dt = (dtg - dttot) / idt as Real;
        let shave = dt < p.dtmin && dt < dtg - dttot;
        dt = dt.max(p.dtmin.min(dtg - dttot));
        if srce_call == SRCE_IMP_POST {
            dt = dtg;
        }
        let hdt = dt;
        dttot = dttot + dt;

        if srce_call == SRCE_DIRECT {
            if shave {
                each!(is, 1, nspech, {
                    let einc1 = v!(vs, is) * dt / (1. - hdt * v!(vd, is)).max(1.);
                    let einc2 = sign(v!(dam, is).min(einc1.abs()), einc1);
                    sp!(is) = (sp!(is) + einc2).max(0.);
                });
            } else {
                each!(is, 1, nspech, {
                    let einc1 = v!(vs, is) * dt / (1. - hdt * v!(vd, is)).max(1.);
                    sp!(is) = (sp!(is) + einc1).max(0.);
                });
            }
            each!(is, 1, nspech, {
                let einc1 = v!(vsdb, is) * dt / (1. - hdt * v!(vddb, is)).max(1.);
                sp!(is) = (sp!(is) + einc1).max(0.);
            });
        }

        wc!(3) = 0.;
        each!(ik, 1, nk, {
            let factor = dd!(ik) / cg!(ik);
            each!(ith, 1, nth, {
                let is = (ik - 1) * nth + ith;
                phiaw = phiaw + v!(vsin, is) * dt * factor / (1. - hdt * v!(vdin, is)).max(1.);
                phibbl = phibbl - v!(vsbt, is) * dt * factor / (1. - hdt * v!(vdbt, is)).max(1.);
                phinl = phinl + v!(vsnl, is) * dt * factor / (1. - hdt * v!(vdnl, is)).max(1.);
                if v!(vsin, is) > 0. {
                    wc!(3) = wc!(3) + sp!(is) * factor;
                }
            });
        });
        wc!(3) = 4. * sqrtf(wc!(3));
        tauwix = tauwix + tauwx * drat * dt;
        tauwiy = tauwiy + tauwy * drat * dt;
        tauwnx = tauwnx + tauwax * drat * dt;
        tauwny = tauwny + tauway * drat * dt;

        spr4!();
        let fage = p.ffxfa * tanhf(0.3 * u10abs * fmeanws * TPI / GRAV);
        let fh1 = (p.ffxfm + fage) * fmean1;
        ustar = ustar.max(0.001);
        let fh2 = p.ffxpm / ustar;
        fhigh = sg!(nk).min(fh1.max(fh2));
        let nkh = nkh1.min((p.facti2 + p.facti1 * logf(fhigh.max(1.0e-7))) as i32).max(2);

        if dttot >= 0.9999 * dtg {
            let hm = p.fhmax * tanhf(wnmean * pt.d_inp.max(0.)) / wnmean.max(1.0e-4);
            let em = hm * hm / 16.;
            if emean > em && emean > 1.0e-30 {
                each!(is, 1, nspec, {
                    sp!(is) = sp!(is) / emean * em;
                });
                emean = em;
            }
        }

        each!(ik, nkh + 1, nk, {
            each!(ith, 1, nth, {
                sp!(ith + (ik - 1) * nth) = sp!(ith + (ik - 2) * nth) * p.fachfa + 0.;
            });
        });

        sin4!();
        if srce_call == SRCE_IMP_POST {
            break;
        }
        if dttot >= 0.9999 * dtg {
            break;
        }
    }

    dtdyn = dtdyn / nsteps.max(1) as Real;
    let fcut = fhigh * TPIINV;

    let (mut efinish, mut mwxfinish, mut mwyfinish): (Real, Real, Real) = (0., 0., 0.);
    each!(ik, 1, nk, {
        let (mut eband, mut a1band, mut b1band): (Real, Real, Real) = (0., 0., 0.);
        each!(ith, 1, nth, {
            let is = ith + (ik - 1) * nth;
            let diff = v!(specinit, is) - sp!(is);
            eband = eband + diff;
            a1band = a1band + diff * t.ecos[ix!(0, ith)];
            b1band = b1band + diff * t.esin[ix!(0, ith)];
        });
        efinish = efinish + eband * dd!(ik) / cg!(ik);
        mwxfinish = mwxfinish + a1band * dd!(ik) / cg!(ik) * wn!(ik) / sg!(ik);
        mwyfinish = mwyfinish + b1band * dd!(ik) / cg!(ik) * wn!(ik) / sg!(ik);
    });

    let tauox = (GRAV * mwxfinish + tauwix - taubbl1) / dtg;
    let tauoy = (GRAV * mwyfinish + tauwiy - taubbl2) / dtg;
    tauwix = tauwix / dtg;
    tauwiy = tauwiy / dtg;
    tauwnx = tauwnx / dtg;
    tauwny = tauwny / dtg;
    let (sn, cs) = sincosf(ustdir);
    let tauocx = dair * pt.coef * pt.coef * ustar * ustar * cs + DWAT * (tauox - tauwix);
    let tauocy = dair * pt.coef * pt.coef * ustar * ustar * sn + DWAT * (tauoy - tauwiy);
    let phioc = DWAT * GRAV * (efinish + phiaw - phibbl) / dtg;
    let phiaw_out = DWAT * GRAV * phiaw / dtg;
    let phibbl_out = DWAT * GRAV * phibbl / dtg;

    // With IC0 the attenuation is 1: SPEC keeps its value and only the signs of zero remain.
    if ice_on && pt.ice > 0. {
        each!(ik, 1, nk, {
            let factor = dd!(ik) / cg!(ik);
            let factor2 = factor * GRAV * wn!(ik) / sg!(ik);
            each!(ith, 1, nth, {
                let is = ith + (ik - 1) * nth;
                let same = sp!(is) - sp!(is);
                phice = phice + same * factor;
                tauice1 = tauice1 - same * factor2 * t.ecos[ix!(0, is)];
                tauice2 = tauice2 - same * factor2 * t.esin[ix!(0, is)];
            });
        });
        phice = -1. * DWAT * GRAV * phice / dtg;
        tauice1 = tauice1 / dtg;
        tauice2 = tauice2 / dtg;
    }

    if it == 0 {
        each!(is, 1, nspec, {
            sp!(is) = v!(specinit, is);
        });
    }
    each!(is, 1, nspec, {
        sp!(is) = sp!(is).max(0.);
    });

    let _ = (fmean, cd_unused, phinl);
    out[point] = SrceOut {
        ustar,
        ustdir,
        dtdyn,
        fcut,
        tauwx,
        tauwy,
        tauox,
        tauoy,
        tauwix,
        tauwiy,
        tauwnx,
        tauwny,
        phiaw: phiaw_out,
        charn,
        tws,
        phioc,
        phibbl: phibbl_out,
        taubbl: [taubbl1 / dtg, taubbl2 / dtg],
        tauice: [tauice1, tauice2],
        phice,
        tauocx,
        tauocy,
        wnmean,
    };
}
