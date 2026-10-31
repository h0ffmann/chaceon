// SPDX-License-Identifier: MPL-2.0
//! `W3OUTG` and `W3IOGO` (`model/src/w3iogomd.F90`), `out_grd.ww3`, for the fields a case asks for.
//! `W3OUTG` loops `IK`, `ITH`, `JSEA`; here each sea point has its own pass, which keeps the order of every
//! per-point sum. A field this file does not hold stops the run in `ids`: partitions, `WBT`, 3-D Stokes.

#![allow(clippy::approx_constant)]

use crate::moddef::ModDef;
use crate::wtime::Time;
use ww3_kernels::constants::{DWAT, GRAV, PI, RADE, TPI, TPIINV};
use ww3_kernels::real::Real;
use ww3_kernels::srce::SrceOut;

pub const UNDEF: Real = -999.9;
const NOGRP: usize = 10;
const NGRPP: usize = 25;

/// `FLOGRD`, 1-based `(IFI, IFJ)` flattened in Fortran's order.
#[derive(Clone)]
pub struct Flags(pub Vec<bool>);

impl Flags {
    pub fn on(&self, i: usize, j: usize) -> bool {
        self.0[(i - 1) + (j - 1) * NOGRP]
    }
    fn set(&mut self, i: usize, j: usize, v: bool) {
        self.0[(i - 1) + (j - 1) * NOGRP] = v;
    }
}

/// `W3FLDTOIJ`, for the fields written here.
const IDS: [(&str, usize, usize); 58] = [
    ("DPT", 1, 1), ("CUR", 1, 2), ("WND", 1, 3), ("AST", 1, 4), ("WLV", 1, 5),
    ("HS", 2, 1), ("LM", 2, 2), ("T02", 2, 3), ("T0M1", 2, 4), ("T01", 2, 5), ("FP", 2, 6),
    ("DIR", 2, 7), ("SPR", 2, 8), ("DP", 2, 9), ("HIG", 2, 10), ("MXE", 2, 11), ("MXES", 2, 12),
    ("MXH", 2, 13), ("MXHC", 2, 14), ("SDMH", 2, 15), ("SDMHC", 2, 16), ("TP", 2, 18),
    ("EF", 3, 1), ("TH1M", 3, 2), ("STH1M", 3, 3), ("CHA", 5, 2), ("CGE", 5, 3), ("FAW", 5, 4),
    ("TAW", 5, 5), ("TWA", 5, 6), ("FWS", 5, 11), ("SXY", 6, 1), ("TWO", 6, 2), ("BHD", 6, 3),
    ("FOC", 6, 4), ("TUS", 6, 5), ("USS", 6, 6), ("P2S", 6, 7), ("P2L", 6, 9), ("DTD", 9, 1),
    ("FC", 9, 2), ("CFX", 9, 3), ("UST", 5, 1), ("WCC", 5, 7), ("WCF", 5, 8), ("WCH", 5, 9),
    ("WCM", 5, 10), ("ABR", 7, 1), ("UBR", 7, 2), ("MSS", 8, 1), ("MSC", 8, 2), ("MSD", 8, 3),
    ("MCD", 8, 4), ("QP", 8, 5), ("FBB", 7, 4), ("ICE", 1, 6), ("WBT", 2, 17), ("USF", 6, 8),
];

/// `W3READFLGRD`'s list, then `W3FLGRDUPDT`, which clears a spectral field that has no bounds.
pub fn ids(list: &str, m: &ModDef) -> Flags {
    let mut f = Flags(vec![false; NOGRP * NGRPP]);
    for name in list.split_whitespace() {
        let e = IDS.iter().find(|x| x.0 == name);
        let e = e.unwrap_or_else(|| panic!("output field {name} is not implemented"));
        f.set(e.1, e.2, true);
    }
    for i in 1..=5 {
        if m.e3df[(i - 1) * 3] <= 0 || m.e3df[(i - 1) * 3 + 2] < m.e3df[(i - 1) * 3 + 1] {
            f.set(3, i, false);
        }
    }
    if m.us3df[0] <= 0 || m.us3df[2] < m.us3df[1] {
        f.set(6, 8, false);
    }
    if m.p2msf[0] <= 0 || m.p2msf[2] < m.p2msf[1] {
        f.set(6, 9, false);
    }
    f
}

/// What `W3WAVE` holds at an output and `W3OUTG` reads. `Scalars` come from the run. Arrays run from
/// `ISEA = 0`, as the state in `shel.rs` does.
pub struct Input<'a> {
    pub time: Time,
    pub va: &'a [Real],
    pub mapsta: &'a [i32],
    pub mapst2: &'a [i32],
    pub cg: &'a [Real],
    pub wn: &'a [Real],
    pub dw: &'a [Real],
    pub wlv: &'a [Real],
    pub ice: &'a [Real],
    pub ua: &'a [Real],
    pub ud: &'a [Real],
    pub asv: &'a [Real],
    pub ust: &'a mut [Real],
    pub ustdir: &'a mut [Real],
    pub src: &'a [SrceOut],
    pub dtdyn: &'a [Real],
    pub fcut: &'a [Real],
    pub cflxy: &'a [Real],
    pub u10: &'a [Real],
    pub u10d: &'a [Real],
    /// `WAVNU1`.
    pub wavnu: &'a dyn Fn(Real, Real) -> (Real, Real),
    pub asf: &'a [Real],
    pub whitecap: &'a [Real],
    pub cx: &'a [Real],
    pub cy: &'a [Real],
}

/// The per-point results of `W3OUTG`.
#[derive(Clone, Default)]
struct Pt {
    hs: Real,
    wlm: Real,
    t02: Real,
    t0m1: Real,
    t01: Real,
    fp0: Real,
    thm: Real,
    ths: Real,
    thp0: Real,
    stmaxe: Real,
    stmaxd: Real,
    hmaxe: Real,
    hcmaxe: Real,
    hmaxd: Real,
    hcmaxd: Real,
    sxx: Real,
    syy: Real,
    sxy: Real,
    bhd: Real,
    tusx: Real,
    tusy: Real,
    ussx: Real,
    ussy: Real,
    prms: Real,
    tpms: Real,
    cge: Real,
    qp: Real,
    aba: Real,
    abd: Real,
    uba: Real,
    ubd: Real,
    mssx: Real,
    mssy: Real,
    mssd: Real,
    mscx: Real,
    mscy: Real,
    mscd: Real,
    wbt: Real,
    /// `US3D(JSEA,IK)`, then `US3D(JSEA,NK+IK)`, for the band of `US3DF`.
    us3: Vec<Real>,
}

fn sq(x: Real) -> Real {
    x * x
}

/// `W3OUTG` for sea point `isea`.
#[allow(clippy::too_many_arguments)]
fn outg(m: &ModDef, f: &Flags, g: &Input, isea: usize, ef: &mut [Real], th1m: &mut [Real], sth1m: &mut [Real], p2: &mut [Real]) -> Pt {
    let (nth, nk) = (m.nth as usize, m.nk as usize);
    let (nspec, nk2) = (nth * nk, nk + 2);
    let a = &g.va[nspec * isea..nspec * (isea + 1)];
    let sig = &m.sig;
    let cg = &g.cg[nk2 * isea..nk2 * (isea + 1)];
    let wn = &g.wn[nk2 * isea..nk2 * (isea + 1)];
    let dw = g.dw[isea];
    let ixy = (m.mapsf[isea - 1 + 2 * m.nsea as usize] - 1) as usize;
    let sta = g.mapsta[ixy];
    let (mut et, mut ewn, mut etr, mut et1, mut et02, mut etx, mut ety, mut etf): (Real, Real, Real, Real, Real, Real, Real, Real) =
        (0., 0., 0., 0., 0., 0., 0., 0.);
    let mut q = Pt::default();
    let mut ettpmm: Real = 0.;
    let (mut etxx, mut etyy, mut etxy): (Real, Real, Real) = (0., 0., 0.);
    let nus = (m.us3df[2] - m.us3df[1] + 1).max(0) as usize;
    if f.on(6, 8) {
        q.us3 = vec![0.; 2 * nus];
    }
    let (mut abr, mut ubr): (Real, Real) = (0., 0.);
    let mut ebd = vec![0.; nk];
    let (mut ab, mut abx, mut aby, mut abxx, mut abyy, mut abxy): (Real, Real, Real, Real, Real, Real) = (0., 0., 0., 0., 0., 0.);
    for ik in 1..=nk {
        (ab, abx, aby, abxx, abyy, abxy) = (0., 0., 0., 0., 0., 0.);
        let (mut abst, mut abx2, mut aby2, mut abyx): (Real, Real, Real, Real) = (0., 0., 0., 0.);
        let factor0 = 0.5f32.max(cg[ik] / sig[ik] * wn[ik]);
        for ith in 0..nth {
            let v = a[ith + (ik - 1) * nth];
            ab += v;
            abx += v * m.ecos[ith];
            aby += v * m.esin[ith];
            abx2 += v * m.ec2[ith];
            aby2 += v * m.es2[ith];
            abyx += v * m.esc[ith];
            if ith < nth / 2 {
                abst += v * a[ith + nth / 2 + (ik - 1) * nth];
            }
            abxx += ((1. + m.ec2[ith]) * factor0 - 0.5) * v;
            abyy += ((1. + m.es2[ith]) * factor0 - 0.5) * v;
            abxy += m.esc[ith] * factor0 * v;
        }
        let factor = m.dden[ik - 1] / cg[ik];
        let e = ab * factor;
        ebd[ik - 1] = e;
        et += e;
        etf += e * cg[ik];
        ewn += e / wn[ik];
        etr += e / sig[ik];
        et1 += e * sig[ik];
        et02 += e * sq(sig[ik]);
        etx += abx * factor;
        ety += aby * factor;
        q.tusx = q.tusx + abx * factor * GRAV * wn[ik] / sig[ik];
        q.tusy = q.tusy + aby * factor * GRAV * wn[ik] / sig[ik];
        etxx += abx2 * factor * sq(wn[ik]);
        etyy += aby2 * factor * sq(wn[ik]);
        etxy += abyx * factor * sq(wn[ik]);
        let factor2 = m.dth * 2. / sq(TPI) * sig[ik] * sq(TPI * sig[ik] / cg[ik]) * abst;
        q.prms = q.prms + factor2 * 2. * m.dsii[ik - 1];
        if f.on(6, 9) && (ik as i32) >= m.p2msf[1] && (ik as i32) <= m.p2msf[2] {
            p2[ik - m.p2msf[1] as usize] = factor2 * 2. * TPI;
        }
        if factor2 > ettpmm {
            ettpmm = factor2;
            q.tpms = TPI / sig[ik];
        }
        if ik == nk {
            let factor2 = sig[ik] * sig[ik] * sig[ik] * sq(sig[ik]) / sq(GRAV) / m.dsii[ik - 1];
            let (ma, mc, mb) = (abx2 * factor * factor2, aby2 * factor * factor2, abyx * factor * factor2);
            q.mscd = 0.5 * (2. * mb).atan2(ma - mc);
            let (sd, cd) = ww3_libm::sincosf(q.mscd);
            q.mscx = ma * sq(cd) + 2. * mb * sd * cd + ma * sq(sd);
            q.mscy = mc * sq(cd) - 2. * mb * sd * cd + ma * sq(sd);
        }
        let kd = 0.001f32.max(wn[ik] * dw);
        let ussco = if kd < 6. {
            let fkd = factor / sq(kd.sinh());
            abr += ab * fkd;
            q.aba += abx * fkd;
            q.abd += aby * fkd;
            ubr += ab * sq(sig[ik]) * fkd;
            q.uba = q.uba + abx * sq(sig[ik]) * fkd;
            q.ubd = q.ubd + aby * sq(sig[ik]) * fkd;
            q.bhd = q.bhd + GRAV * wn[ik] * e / (2. * kd).sinh();
            fkd * sig[ik] * wn[ik] * (2. * kd).cosh()
        } else {
            factor * sig[ik] * 2. * wn[ik]
        };
        abxx = 0f32.max(abxx) * factor;
        abyy = 0f32.max(abyy) * factor;
        abxy = abxy * factor;
        q.sxx += abxx;
        q.syy += abyy;
        q.sxy += abxy;
        ebd[ik - 1] = e / m.dsii[ik - 1];
        if f.on(3, 1) && in_band(m, 0, ik) {
            ef[ik - m.e3df[1] as usize] = ebd[ik - 1] * TPI;
        }
        if f.on(6, 8) && (ik as i32) >= m.us3df[1] && (ik as i32) <= m.us3df[2] {
            let k = ik - m.us3df[1] as usize;
            q.us3[k] = abx * ussco / (m.dsii[ik - 1] * TPIINV);
            q.us3[nus + k] = aby * ussco / (m.dsii[ik - 1] * TPIINV);
        }
        q.ussx = q.ussx + abx * ussco;
        q.ussy = q.ussy + aby * ussco;
        if f.on(3, 2) && in_band(m, 1, ik) {
            th1m[ik - m.e3df[4] as usize] = (630. - RADE * aby.atan2(abx)) % 360.;
        }
        let m1 = (sq(abx) + sq(aby)).sqrt() / 1e-20f32.max(ab);
        if f.on(3, 3) && in_band(m, 2, ik) {
            sth1m[ik - m.e3df[7] as usize] = (2. * (1. - m1)).abs().sqrt() * RADE;
        }
    }

    (q.stmaxe, q.stmaxd, q.hmaxe, q.hcmaxe, q.hmaxd, q.hcmaxd) = (UNDEF, UNDEF, UNDEF, UNDEF, UNDEF, UNDEF);
    if (m.stexu > 0. && m.steyu > 0.) || m.stedu > 0. {
        space_time(m, a, cg, wn, &ebd, sta, (et, et1, et02, etx, ety), &mut q);
    }

    q.mssd = 0.5 * (2. * etxy).atan2(etxx - etyy);
    let (sd, cd) = ww3_libm::sincosf(q.mssd);
    q.mssx = etxx * sq(cd) + 2. * etxy * sd * cd + etyy * sq(sd);
    q.mssy = etyy * sq(cd) - 2. * etxy * sd * cd + etxx * sq(sd);

    // The tail above the last band.
    let cgnk = cg[nk];
    let sn = sig[nk];
    let ft1 = 0.3333 * sq(sn) * m.dth * sn;
    let eband = ab / cgnk;
    et += m.fte * eband;
    ewn += m.ftwl * eband;
    etf += GRAV * m.fttr * eband;
    etr += m.fttr * eband;
    et1 += ft1 * eband;
    // WW3 starts from ET1 here, and what the loop summed into EET1 is lost.
    let eet1 = et1 + ft1 * sq(eband);
    et02 += eband * 0.5 * sq(sq(sn)) * m.dth;
    etx += m.fte * abx / cgnk;
    ety += m.fte * aby / cgnk;
    q.sxx = (q.sxx + m.fte * abxx / cgnk) * DWAT * GRAV;
    q.syy = (q.syy + m.fte * abyy / cgnk) * DWAT * GRAV;
    q.sxy = (q.sxy + m.fte * abxy / cgnk) * DWAT * GRAV;

    (q.hs, q.wlm, q.t0m1, q.t01, q.t02, q.thm, q.ths, q.fp0, q.thp0) =
        (UNDEF, UNDEF, UNDEF, UNDEF, UNDEF, UNDEF, UNDEF, UNDEF, UNDEF);
    q.qp = UNDEF;
    if sta > 0 {
        q.hs = 4. * et.sqrt();
        if et > 1.0e-7 {
            q.qp = (2. / sq(et)) * eet1 * sq(TPIINV);
            q.wlm = ewn / et * TPI;
            q.t0m1 = etr / et * TPI;
            q.ths = RADE * 0f32.max(2. * (1. - 0f32.max((sq(etx) + sq(ety)) / sq(et)).sqrt())).sqrt();
            if q.ths < 0.01 * RADE * m.dth {
                q.ths = 0.;
            }
        } else {
            q.wlm = 0.;
            q.t0m1 = TPI / sig[nk];
            q.ths = 0.;
        }
        q.thm = if etx.abs() + ety.abs() > 1.0e-7 { ety.atan2(etx) } else { 0. };
        abr = (2. * 0f32.max(abr)).sqrt();
        q.abd = if abr >= 1.0e-7 { q.abd.atan2(q.aba) } else { 0. };
        q.aba = abr;
        ubr = (2. * 0f32.max(ubr)).sqrt();
        q.ubd = if ubr >= 1.0e-7 { q.ubd.atan2(q.uba) } else { 0. };
        q.uba = ubr;
        q.cge = DWAT * GRAV * etf;
        if et02 > 1.0e-7 && et > 0. {
            q.t02 = TPI * (et / et02).sqrt();
            q.t01 = TPI * et / et1;
        } else {
            q.t02 = TPI / sig[nk];
            q.t01 = q.t02;
        }
    }

    // The peak.
    let mut ec = ebd[nk - 1];
    let mut ikp = nk;
    for ik in (1..nk).rev() {
        if ec < ebd[ik - 1] {
            ec = ebd[ik - 1];
            ikp = ik;
        }
    }
    if ikp != nk {
        q.fp0 = sig[ikp] * TPIINV;
        let (xl, xh) = (1. / m.xfr - 1., m.xfr - 1.);
        let (xl2, xh2) = (sq(xl), sq(xh));
        let el = if ikp == 1 { -ebd[0] } else { ebd[ikp - 2] - ebd[ikp - 1] };
        let eh = ebd[ikp] - ebd[ikp - 1];
        let denom = xl * eh - xh * el;
        let d = denom.abs().max(1.0e-15).copysign(denom);
        q.fp0 = q.fp0 * (1. + 0.5 * (xl2 * eh - xh2 * el) / d);
    }
    let (mut px, mut py): (Real, Real) = (0., 0.);
    for ith in 0..nth {
        if ikp != nk {
            px += a[ith + (ikp - 1) * nth] * m.ecos[ith];
            py += a[ith + (ikp - 1) * nth] * m.esin[ith];
        }
    }
    if px.abs() + py.abs() > 1.0e-7 && q.fp0 != UNDEF {
        q.thp0 = py.atan2(px);
    }
    if sta <= 0 {
        q.fp0 = UNDEF;
        q.thp0 = UNDEF;
    }
    q.wbt = UNDEF;
    if f.on(2, 17) && sta > 0 {
        q.wbt = calc_wbt(m, g, isea, a, cg, wn, dw);
    }
    q
}

/// `CALC_WBT` for one sea point. `ESIG**4.` has a real exponent, so it stays a `powf`.
fn calc_wbt(m: &ModDef, g: &Input, isea: usize, a: &[Real], cg: &[Real], wn: &[Real], dw: Real) -> Real {
    let (nth, nk) = (m.nth as usize, m.nk as usize);
    let sig = &m.sig;
    let tdpt = dw.max(m.dmin);
    let (sinu, cosu) = ww3_libm::sincosf(g.u10d[isea]);
    let mut esig = vec![0f32; nk];
    for ik in 1..=nk {
        let tc = sig[ik] / wn[ik];
        let factor = sig[ik] / cg[ik] * m.dth;
        for ith in 0..nth {
            let tforce = tc - g.u10[isea] * (cosu * m.ecos[ith] + sinu * m.esin[ith]) * m.btbeta;
            if tforce < 0. {
                esig[ik - 1] += a[ith + (ik - 1) * nth] * factor;
            }
        }
    }
    let mut et: Real = 0.;
    for ik in 0..nk {
        et += esig[ik] * m.dsii[ik];
    }
    et = et + esig[nk - 1] * m.fte / (m.dth * sig[nk]);
    let hs = 4. * 0f32.max(et).sqrt();
    let (mut top, mut bot): (Real, Real) = (0., 0.);
    for ik in 0..nk {
        let e4 = esig[ik].powf(4.);
        top += e4 * sig[ik + 1] * m.dsii[ik];
        bot += e4 * m.dsii[ik];
    }
    let mut sigp = top / 1.0e-10f32.max(bot);
    if sigp < 1.0e-6 {
        sigp = sig[nk];
    }
    let kp = (g.wavnu)(sigp, tdpt).0;
    let mut etp: Real = 0.;
    for ik in 1..=nk {
        if sig[ik] >= 0.7 * sigp && sig[ik] <= 1.3 * sigp {
            etp += esig[ik - 1] * m.dsii[ik - 1];
        }
    }
    let hsp = 4. * 0f32.max(etp).sqrt();
    let wstp = 0.5 * kp * hsp;
    let twbt = 85.1 * (0f32.max(wstp - 0.055) * (1. + hs / tdpt)).powf(2.33);
    1f32.min(twbt)
}

/// The space-time extremes, lines 1711-1939 of `W3OUTG`. `E` is the sums before the tail, as there.
#[allow(clippy::too_many_arguments)]
fn space_time(m: &ModDef, a: &[Real], cg: &[Real], wn: &[Real], ebd: &[Real], sta: i32, e: (Real, Real, Real, Real, Real), q: &mut Pt) {
    let (nth, nk) = (m.nth as usize, m.nk as usize);
    let sig = &m.sig;
    let (et, et1, et02, etx, ety) = e;
    let mut thmp = UNDEF;
    if sta > 0 && etx.abs() + ety.abs() > 1.0e-7 {
        thmp = ety.atan2(etx);
    }
    let (s, c) = ww3_libm::sincosf(thmp);
    let (mut mssxm, mut mssym, mut mssxtm, mut mssytm, mut mssxym): (Real, Real, Real, Real, Real) = (0., 0., 0., 0., 0.);
    for ik in 1..=nk {
        let (mut abx2m, mut aby2m, mut abxm, mut abym, mut abxym): (Real, Real, Real, Real, Real) = (0., 0., 0., 0., 0.);
        for ith in 0..nth {
            let v = a[ith + (ik - 1) * nth];
            let along = m.ecos[ith] * c + m.esin[ith] * s;
            let across = m.esin[ith] * c - m.ecos[ith] * s;
            abx2m += v * sq(along);
            aby2m += v * sq(across);
            abxm += v * along;
            abym += v * across;
            abxym += v * along * across;
        }
        let factor = m.dden[ik - 1] / cg[ik];
        mssxm += abx2m * factor * sq(wn[ik]);
        mssym += aby2m * factor * sq(wn[ik]);
        mssxtm = mssxtm + abxm * factor * wn[ik] * sig[ik];
        mssytm = mssytm + abym * factor * wn[ik] * sig[ik];
        mssxym += abxym * factor * sq(wn[ik]);
    }
    let (mut t02p, mut wl02x, mut wl02y) = (UNDEF, UNDEF, UNDEF);
    let (mut alpxt, mut alpyt, mut alpxy) = (UNDEF, UNDEF, UNDEF);
    if et02 > 1.0e-7 {
        t02p = TPI * (et / et02).sqrt();
    }
    if mssxm > 1.0e-7 {
        wl02x = TPI * (et / mssxm).sqrt();
    }
    if mssym > 1.0e-7 {
        wl02y = TPI * (et / mssym).sqrt();
    }
    if mssxm > 1.0e-7 && et02 > 1.0e-7 {
        alpxt = mssxtm / (mssxm * et02).sqrt();
    }
    if mssym > 1.0e-7 && et02 > 1.0e-7 {
        alpyt = mssytm / (mssym * et02).sqrt();
    }
    if mssxm > 1.0e-7 && mssym > 1.0e-7 {
        alpxy = mssxym / (mssxm * mssym).sqrt();
    }
    let (stex, stey) = if m.stexu > 0. && m.steyu > 0. { (m.stexu, m.steyu) } else { (0., 0.) };
    let sted = if m.stedu > 0. { m.stedu } else { 0. };
    let (mut nv, mut ns, mut nb, mut mu, mut ni, mut mode) = (UNDEF, UNDEF, UNDEF, UNDEF, UNDEF, UNDEF);
    let fine = wl02x > 1.0e-7 && wl02y > 1.0e-7 && t02p > 1.0e-7;
    if fine {
        nv = TPI * (stex * stey * sted) / (wl02x * wl02y * t02p)
            * (1. - sq(alpxt) - sq(alpyt) - sq(alpxy) + 2. * alpxt * alpyt * alpxy).sqrt();
        ns = TPI.sqrt()
            * ((stex * sted) / (wl02x * t02p) * (1. - sq(alpxt)).sqrt()
                + (stey * sted) / (wl02y * t02p) * (1. - sq(alpyt)).sqrt()
                + (stex * stey) / (wl02x * wl02y) * (1. - sq(alpxy)).sqrt());
        nb = stex / wl02x + stey / wl02y + sted / t02p;
    }
    if et1 > 1.0e-7 {
        ni = (et * et02 / sq(et1) - 1.).sqrt();
    }
    if et > 1.0e-7 {
        mu = sq(et1) / GRAV * et.powf(-1.5) * (1. - ni + sq(ni));
    }
    if stex == 0. && stey == 0. {
        mode = (2. * nb.ln()).sqrt();
    } else if sted == 0. {
        mode = (2. * ns.ln() + (2. * ns.ln() + (2. * ns.ln()).ln()).ln()).sqrt();
    } else if fine {
        mode = (2. * nv.ln() + 2. * (2. * nv.ln() + 2. * (2. * nv.ln()).ln()).ln()).sqrt();
    }
    let den = mode - (2. * nv * mode + ns) / (nv * sq(mode) + ns * mode + nb);
    let root = et.sqrt();
    q.stmaxe = root * (mode + 0.5 * mu * sq(mode) + 0.5772 * (1. + mu * mode) / den);
    q.stmaxd = root * (PI * (1. + mu * mode) / 6f32.sqrt() / den);
    let mut phist: Real = 0.;
    if t02p > 1.0e-7 {
        let mut tlphi = 0.3 * t02p;
        let mut phi = [0f32; 21];
        let (x2, x3) = (m.xfr * m.xfr, m.xfr * m.xfr * m.xfr);
        for p in &mut phi {
            let mut ik = 1;
            while ik + 3 <= nk {
                *p = *p
                    + (x3 * ebd[ik + 2] * (x3 * sig[ik] * tlphi).cos()
                        + x2 * ebd[ik + 1] * (x2 * sig[ik] * tlphi).cos()
                        + m.xfr * ebd[ik] * (m.xfr * sig[ik] * tlphi).cos()
                        + ebd[ik - 1] * (sig[ik] * tlphi).cos())
                        * m.dsii[ik - 1];
                ik += 4;
            }
            tlphi += t02p / 20.;
        }
        let low = phi.map(|p| p / et).into_iter().fold(Real::INFINITY, Real::min);
        phist = low.abs();
    }
    let stmaxel = root * (mode + 0.5772 / den);
    let stmaxdl = root * (PI / 6f32.sqrt() / den);
    q.hcmaxe = stmaxel * (1. + phist);
    q.hcmaxd = stmaxdl * (1. + phist);
    q.hmaxe = stmaxel * (2. * (1. + phist)).sqrt();
    q.hmaxd = stmaxdl * (2. * (1. + phist)).sqrt();
}

fn in_band(m: &ModDef, field: usize, ik: usize) -> bool {
    let (lo, hi) = (m.e3df[field * 3 + 1], m.e3df[field * 3 + 2]);
    (ik as i32) >= lo && (ik as i32) <= hi
}

/// `out_grd.ww3`.
pub struct Grd {
    f: std::io::BufWriter<std::fs::File>,
    flags: Flags,
}

impl Grd {
    pub fn new(path: &str, m: &ModDef, flags: Flags) -> Self {
        use std::io::Write;
        let mut g = Self { f: std::io::BufWriter::new(std::fs::File::create(path).unwrap()), flags };
        let mut h = Vec::new();
        let mut text = |s: &str, n: usize| {
            let mut b = s.as_bytes().to_vec();
            b.resize(n, b' ');
            h.extend(b);
        };
        text("WAVEWATCH III GRID OUTPUT FILE", 30);
        text("2019-10-04", 10);
        text(&m.gname, 30);
        for v in [NOGRP as i32, NGRPP as i32, m.nsea, m.nx, m.ny] {
            h.extend(v.to_be_bytes());
        }
        h.extend(UNDEF.to_be_bytes());
        h.extend(m.noswll.to_be_bytes());
        g.rec(&h);
        g.f.flush().unwrap();
        g
    }

    fn rec(&mut self, b: &[u8]) {
        use std::io::Write;
        let n = (b.len() as i32).to_be_bytes();
        self.f.write_all(&n).unwrap();
        self.f.write_all(b).unwrap();
        self.f.write_all(&n).unwrap();
    }

    fn reals(&mut self, v: impl Iterator<Item = Real>) {
        let b: Vec<u8> = v.flat_map(|x| x.to_be_bytes()).collect();
        self.rec(&b);
    }

    /// `W3OUTG`, then `W3IOGO ('WRITE', ...)`.
    pub fn write(&mut self, m: &ModDef, g: &mut Input) {
        use std::io::Write;
        let nsea = m.nsea as usize;
        let (nx, ny) = (m.nx as usize, m.ny as usize);
        let fl = self.flags.clone();
        let band = |j: usize| (m.e3df[j * 3 + 2] - m.e3df[j * 3 + 1] + 1).max(0) as usize;
        let (ne, nt1, ns1) = (band(0), band(1), band(2));
        let np2 = (m.p2msf[2] - m.p2msf[1] + 1).max(0) as usize;
        let (mut ef, mut th1m, mut sth1m) = (vec![UNDEF; ne * nsea], vec![UNDEF; nt1 * nsea], vec![UNDEF; ns1 * nsea]);
        let mut p2 = vec![0.; np2 * nsea];
        let mut pts: Vec<Pt> = Vec::with_capacity(nsea);
        for isea in 1..=nsea {
            let i = isea - 1;
            pts.push(outg(
                m,
                &fl,
                g,
                isea,
                &mut ef[ne * i..ne * isea],
                &mut th1m[nt1 * i..nt1 * isea],
                &mut sth1m[ns1 * i..ns1 * isea],
                &mut p2[np2 * i..np2 * isea],
            ));
        }

        let mut t = Vec::new();
        t.extend(g.time[0].to_be_bytes());
        t.extend(g.time[1].to_be_bytes());
        for b in &fl.0 {
            t.extend((*b as i32).to_be_bytes());
        }
        self.rec(&t);
        let mut b = Vec::with_capacity(4 * nx * ny);
        for iy in 0..ny {
            for ix in 0..nx {
                let c = iy + ix * ny;
                b.extend((g.mapsta[c] + 8 * g.mapst2[c]).to_be_bytes());
            }
        }
        self.rec(&b);

        // W3IOGO's blank at dry points: fields it writes as UNDEF there.
        let mut dry = vec![false; nsea];
        let mut bound = vec![false; nsea];
        for i in 0..nsea {
            bound[i] = g.mapsta[(m.mapsf[i + 2 * nsea] - 1) as usize] == 2;
            if g.mapsta[(m.mapsf[i + 2 * nsea] - 1) as usize] < 0 {
                dry[i] = true;
                g.ust[i + 1] = UNDEF;
                g.ustdir[i + 1] = UNDEF;
                let n = |d: usize, v: &mut [Real]| v[i * d..(i + 1) * d].fill(UNDEF);
                if fl.on(3, 1) {
                    n(ne, &mut ef);
                }
                if fl.on(3, 2) {
                    n(nt1, &mut th1m);
                }
                if fl.on(3, 3) {
                    n(ns1, &mut sth1m);
                }
                if fl.on(6, 9) {
                    n(np2, &mut p2);
                }
                if fl.on(6, 8) {
                    pts[i].us3.fill(UNDEF);
                }
            }
        }
        let gone = |i: usize, off: bool, v: Real| if dry[i] && off { UNDEF } else { v };
        let sea = |f: &dyn Fn(usize) -> Real| (0..nsea).map(f).collect::<Vec<_>>();
        // gfortran joins the two calls into one sincosf.
        let dir = |i: usize| (g.ua[i + 1], ww3_libm::sincosf(g.ud[i + 1]));

        for ifi in 1..=NOGRP {
            for ifj in 1..=NGRPP {
                if !fl.on(ifi, ifj) {
                    continue;
                }
                // `src` for what W3IOGO blanks at a dry point, `sbl` also at a boundary point.
                let src = |f: fn(&SrceOut) -> Real| sea(&|i| gone(i, true, f(&g.src[i + 1])));
                let sbl = |f: fn(&SrceOut) -> Real| sea(&|i| if bound[i] { UNDEF } else { gone(i, true, f(&g.src[i + 1])) });
                let on = |p: fn(&Pt) -> Real, flag: bool| sea(&|i| gone(i, flag, p(&pts[i])));
                let mut w = |v: Vec<Real>| self.reals(v.into_iter());
                match (ifi, ifj) {
                    (1, 1) => w(g.dw[1..=nsea].to_vec()),
                    (1, 2) => {
                        w(g.cx[1..=nsea].to_vec());
                        w(g.cy[1..=nsea].to_vec());
                    }
                    (1, 3) => {
                        w(sea(&|i| dir(i).0 * dir(i).1.1));
                        w(sea(&|i| dir(i).0 * dir(i).1.0));
                    }
                    (1, 4) => w(g.asv[1..=nsea].to_vec()),
                    (1, 5) => w(g.wlv[1..=nsea].to_vec()),
                    (1, 6) => w(g.ice[1..=nsea].to_vec()),
                    (2, 1) => w(on(|p| p.hs, false)),
                    (2, 2) => w(on(|p| p.wlm, true)),
                    (2, 3) => w(on(|p| p.t02, true)),
                    (2, 4) => w(on(|p| p.t0m1, true)),
                    (2, 5) => w(on(|p| p.t01, true)),
                    (2, 6) | (2, 18) => w(on(|p| p.fp0, true)),
                    (2, 7) => w(on(|p| p.thm, true)),
                    (2, 8) => w(on(|p| p.ths, true)),
                    (2, 9) => w(on(|p| p.thp0, true)),
                    (2, 10) => w(vec![UNDEF; nsea]),
                    (2, 11) => w(on(|p| p.stmaxe, true)),
                    (2, 12) => w(on(|p| p.stmaxd, true)),
                    (2, 13) => w(on(|p| p.hmaxe, true)),
                    (2, 14) => w(on(|p| p.hcmaxe, true)),
                    (2, 15) => w(on(|p| p.hmaxd, true)),
                    (2, 16) => w(on(|p| p.hcmaxd, true)),
                    (2, 17) => w(on(|p| p.wbt, true)),
                    (3, 1) => w(transpose(&ef, ne, nsea)),
                    (3, 2) => w(transpose(&th1m, nt1, nsea)),
                    (3, 3) => w(transpose(&sth1m, ns1, nsea)),
                    (5, 1) => {
                        let u = |i: usize, f: Real| g.ust[i + 1] * g.asf[i + 1] * f;
                        let ok = |i: usize| g.mapsta[(m.mapsf[i + 2 * nsea] - 1) as usize] == 1;
                        let dir = |i: usize| ww3_libm::sincosf(g.ustdir[i + 1]);
                        w(sea(&|i| if ok(i) { u(i, dir(i).1) } else { UNDEF }));
                        w(sea(&|i| if ok(i) { u(i, dir(i).0) } else { UNDEF }));
                    }
                    (5, 2) => w(src(|o| o.charn)),
                    (5, 3) => w(on(|p| p.cge, true)),
                    (5, 4) => w(sbl(|o| o.phiaw)),
                    (5, 5) => {
                        w(sbl(|o| o.tauwix));
                        w(sbl(|o| o.tauwiy));
                    }
                    (5, 6) => {
                        w(sbl(|o| o.tauwnx));
                        w(sbl(|o| o.tauwny));
                    }
                    (5, 7..=10) => w(sea(&|i| {
                        if bound[i] { UNDEF } else { gone(i, true, g.whitecap[4 * (i + 1) + ifj - 7]) }
                    })),
                    (5, 11) => w(sea(&|i| g.src[i + 1].tws)),
                    (6, 1) => {
                        w(on(|p| p.sxx, true));
                        w(on(|p| p.syy, true));
                        w(on(|p| p.sxy, true));
                    }
                    (6, 2) => {
                        w(sbl(|o| o.tauox));
                        w(sbl(|o| o.tauoy));
                    }
                    (6, 3) => w(on(|p| p.bhd, true)),
                    (6, 4) => w(sbl(|o| o.phioc)),
                    (6, 5) => {
                        w(on(|p| p.tusx, true));
                        w(on(|p| p.tusy, true));
                    }
                    (6, 6) => {
                        w(on(|p| p.ussx, true));
                        w(on(|p| p.ussy, true));
                    }
                    (6, 7) => {
                        w(on(|p| p.prms, true));
                        w(on(|p| p.tpms, true));
                    }
                    (6, 8) => {
                        let n = pts[0].us3.len() / 2;
                        for part in 0..2 {
                            let all: Vec<Real> = pts.iter().flat_map(|p| p.us3[part * n..(part + 1) * n].to_vec()).collect();
                            w(transpose(&all, n, nsea));
                        }
                    }
                    (6, 9) => w(transpose(&p2, np2, nsea)),
                    (7, 1) => {
                        w(sea(&|i| gone(i, true, pts[i].aba * pts[i].abd.cos())));
                        w(sea(&|i| gone(i, true, pts[i].aba * pts[i].abd.sin())));
                    }
                    (7, 2) => {
                        w(sea(&|i| gone(i, true, pts[i].uba * pts[i].ubd.cos())));
                        w(sea(&|i| gone(i, true, pts[i].uba * pts[i].ubd.sin())));
                    }
                    (7, 4) => w(sbl(|o| o.phibbl)),
                    (8, 1) => {
                        w(on(|p| p.mssx, true));
                        w(on(|p| p.mssy, true));
                    }
                    (8, 2) => {
                        w(on(|p| p.mscx, true));
                        w(on(|p| p.mscy, true));
                    }
                    (8, 3) => w(on(|p| p.mssd, true)),
                    (8, 4) => w(on(|p| p.mscd, true)),
                    (8, 5) => w(on(|p| p.qp, true)),
                    (9, 1) => w(sea(&|i| gone(i, true, g.dtdyn[i + 1]))),
                    (9, 2) => w(sea(&|i| gone(i, true, g.fcut[i + 1]))),
                    (9, 3) => w(sea(&|i| gone(i, true, g.cflxy[i + 1]))),
                    _ => unreachable!(),
                }
            }
        }
        self.f.flush().unwrap();
    }
}

/// `EF(1:NSEA,...)` is written with `NSEA` first; `outg` fills one sea point's band at a time.
fn transpose(v: &[Real], bands: usize, nsea: usize) -> Vec<Real> {
    let mut o = Vec::with_capacity(v.len());
    for b in 0..bands {
        for i in 0..nsea {
            o.push(v[b + i * bands]);
        }
    }
    o
}
