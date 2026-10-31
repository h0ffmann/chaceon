// SPDX-License-Identifier: MPL-2.0
//! The time loop of `ww3_shel` and `W3WAVE` (`model/src/ww3_shel.F90`, `w3wavemd.F90`), with `W3UWND`,
//! `W3FLDH`, `WAVNU1`, `DISTAB` and the point output `W3IOPE`, `W3IOPO`, as far as the first whole case
//! needs them: source terms only, on a rectilinear grid, one sea point, homogeneous wind, cold start.
//! Anything else stops with a message: this is not a general `ww3_shel`.
//!
//! The kernels run on the CPU here. This is the `host` verdict of ADR-0005.

use crate::insin4::wavnu2_cg;
use crate::moddef::ModDef;
use crate::wtime::{Time, dsec21, tick21};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use ww3_kernels::constants::{DAIR, GRAV, PI, TPI};
use ww3_kernels::nint;
use ww3_kernels::real::Real;
use ww3_kernels::sin4::{Sin4Out, Spr4Out};
use ww3_kernels::srce::{SRCE_DIRECT, SrceOut, SrcePoint, SrceWork, w3srce, work_lens, work_small};
use ww3_libm::{atan2f, atanf, sincosf, sqrtf};

const NAR1D: usize = 121;
const DFAC: Real = 6.;
const UNDEF: Real = -999.9;

/// What the case's `ww3_shel.nml` and point list give.
pub struct Run {
    pub time0: Time,
    pub timen: Time,
    /// Start, stride in seconds and end of the field output: it steers the time loop.
    pub field: (Time, i32, Time),
    pub point: (Time, i32, Time),
    /// `(DATE, VALUE1, VALUE2, VALUE3)` of each homogeneous wind.
    pub winds: VecDeque<(Time, Real, Real, Real)>,
    pub xpt: Real,
    pub ypt: Real,
    pub pname: String,
    /// `FLOGRD(5,7)` and `FLOGRD(5,8)`.
    pub flogrd: (bool, bool),
}

/// `DISTAB`'s tables and `WAVNU1`.
struct Dispersion {
    dsie: Real,
    n1max: i32,
    ewn1: Vec<Real>,
    ecg1: Vec<Real>,
}

impl Dispersion {
    fn new() -> Self {
        let n1max = NAR1D as i32 - 1;
        let simax = sqrtf(GRAV * DFAC);
        let dsie = simax / n1max as Real;
        let (mut ewn1, mut ecg1) = (vec![0.; NAR1D + 1], vec![0.; NAR1D + 1]);
        ecg1[0] = sqrtf(GRAV);
        for i in 1..=n1max + 1 {
            let (k, cg) = wavnu2_cg(i as Real * dsie, 1., 1.0e-7, 15);
            ewn1[i as usize] = k;
            ecg1[i as usize] = cg;
        }
        Self { dsie, n1max, ewn1, ecg1 }
    }

    fn wavnu1(&self, si: Real, h: Real) -> (Real, Real) {
        let sqrth = sqrtf(h);
        let six = si * sqrth;
        let i1 = (six / self.dsie) as i32;
        if i1 <= self.n1max && i1 >= 1 {
            let i2 = i1 + 1;
            let r1 = six / self.dsie - i1 as Real;
            let r2 = 1. - r1;
            let (a, b) = (i1 as usize, i2 as usize);
            ((r2 * self.ewn1[a] + r1 * self.ewn1[b]) / h, (r2 * self.ecg1[a] + r1 * self.ecg1[b]) * sqrth)
        } else {
            (si * si / GRAV, 0.5 * GRAV / si)
        }
    }
}

/// `TW0, WX0, WY0, DT0, TWN, WXN, WYN, DTN`.
#[derive(Clone, Copy)]
struct Wind {
    tf0: Time,
    tfn: Time,
    x0: Real,
    y0: Real,
    s0: Real,
    xn: Real,
    yn: Real,
    sn: Real,
}

impl Wind {
    /// `W3FLDH` for `J = 3`.
    fn fldh(&mut self, t0: Time, tn: Time, q: &mut VecDeque<(Time, Real, Real, Real)>) {
        let dera = atanf(1.) / 45.;
        loop {
            self.tf0 = self.tfn;
            if self.tfn[0] != -1 {
                (self.x0, self.y0, self.s0) = (self.xn, self.yn, self.sn);
            }
            if let Some((t, ha, hd, hs)) = q.pop_front() {
                self.tfn = t;
                let (s, c) = sincosf((270. - hd) * dera);
                (self.xn, self.yn, self.sn) = (ha * c, ha * s, hs);
            } else {
                self.tfn = tn;
                tick21(&mut self.tfn, 1.);
            }
            if dsec21(t0, self.tfn) > 0. {
                break;
            }
        }
    }
}

/// `UA0, UAI, UD0, UDI, AS0, ASI`.
#[derive(Default)]
struct Uwnd {
    ua0: Real,
    uai: Real,
    ud0: Real,
    udi: Real,
    as0: Real,
    asi: Real,
}

/// `W3UWND`'s `FLFRST` branch.
fn uwnd_first(w: &Wind) -> Uwnd {
    let ua0 = sqrtf(w.x0 * w.x0 + w.y0 * w.y0);
    let uan = sqrtf(w.xn * w.xn + w.yn * w.yn);
    let d0 = if ua0 > 1.0e-7 { (TPI + atan2f(w.y0, w.x0)) % TPI } else { 0. };
    let dn = if uan > 1.0e-7 { (TPI + atan2f(w.yn, w.xn)) % TPI } else { d0 };
    let ud0 = if ua0 > 1.0e-7 { d0 } else { dn };
    let mut dd = dn - ud0;
    if dd.abs() > PI {
        dd = dd - TPI * if dd.is_sign_negative() { -1. } else { 1. };
    }
    Uwnd { ua0, uai: uan - ua0, ud0, udi: dd, as0: w.s0, asi: w.sn - w.s0 }
}

/// Fortran unformatted output, big-endian like the reference's.
struct Out {
    f: BufWriter<File>,
}

impl Out {
    fn new(path: &str) -> Self {
        Self { f: BufWriter::new(File::create(path).unwrap()) }
    }
    fn record(&mut self, b: &[u8]) {
        let n = (b.len() as i32).to_be_bytes();
        self.f.write_all(&n).unwrap();
        self.f.write_all(b).unwrap();
        self.f.write_all(&n).unwrap();
    }
}

fn be_f(v: &mut Vec<u8>, x: Real) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn text(v: &mut Vec<u8>, s: &str, n: usize) {
    let mut b = s.as_bytes().to_vec();
    b.resize(n, b' ');
    v.extend_from_slice(&b);
}

/// Runs the case. Writes `out_pnt.ww3` into `dir`.
pub fn run(m: &ModDef, mut r: Run, dir: &str) {
    assert_eq!(m.nsea, 1, "only one sea point is implemented");
    assert!(m.flsou && !(m.flcx || m.flcy || m.flcth || m.flck), "only FLSOU is implemented");
    let (nth, nk) = (m.nth as usize, m.nk as usize);
    let nspec = nth * nk;
    let t = m.snl1();
    let p = m.srce(t.params, r.flogrd);
    let tables = m.tables(&t);

    // W3INIT: water depth, then WN and CG on IK=0..NK+1.
    let disp = Dispersion::new();
    let dw = (0. - m.zb[0]).max(0.);
    let depth = m.dmin.max(dw);
    let (mut wn, mut cg) = (vec![0.; nk], vec![0.; nk]);
    let mut cg0 = vec![0.; nk];
    for ik in 1..=nk {
        let (k, c) = disp.wavnu1(m.sig[ik], depth);
        wn[ik - 1] = k;
        cg[ik - 1] = c;
        cg0[ik - 1] = disp.wavnu1(m.sig[ik], m.dmin).1;
    }

    // W3IOPP on a rectilinear grid: the four corners of the cell and their weights.
    let xi = (r.xpt - m.x0) / m.sx;
    let yi = (r.ypt - m.y0) / m.sy;
    let (ix1, iy1) = (xi.floor() as i32 + 1, yi.floor() as i32 + 1);
    let (rx, ry) = (xi - (ix1 - 1) as Real, yi - (iy1 - 1) as Real);
    let corners = [(ix1, iy1), (ix1 + 1, iy1), (ix1 + 1, iy1 + 1), (ix1, iy1 + 1)];
    let mut rd = [(1. - rx) * (1. - ry), rx * (1. - ry), rx * ry, (1. - rx) * ry];
    let mapfs = |ix: i32, iy: i32| -> i32 {
        if ix < 1 || iy < 1 || ix > m.nx || iy > m.ny {
            0
        } else {
            m.mapfs[((iy - 1) + (ix - 1) * m.ny) as usize]
        }
    };
    let is: Vec<i32> = corners.iter().map(|c| mapfs(c.0, c.1)).collect();
    let (mut rds, mut facrd) = (0., 1.);
    for j in 0..4 {
        if is[j] > 0 {
            rds += rd[j];
        } else {
            rd[j] = 0.;
        }
    }
    if rds > 1.0e-7 {
        facrd = 1. / rds;
    }
    let rd: Vec<Real> = rd.iter().map(|x| x * facrd).collect();

    // Work arrays, for one sea point.
    let mut spec = vec![0.; nspec];
    let mut bufs: Vec<Vec<Real>> = (0..17).map(|_| vec![0.; nspec]).collect();
    let (l_snl, l_sds, l_ids) = work_lens(&p);
    let (mut llws, mut w_snl, mut w_sds, mut iw_sds) =
        (vec![0u32; nspec], vec![0.; l_snl], vec![0.; l_sds], vec![0i32; l_ids]);
    let (mut w_small, mut whitecap) = (vec![0.; work_small(&p)], vec![0.; 4]);
    let (mut sin_out, mut spr_out) = ([Sin4Out::default()], [Spr4Out::default()]);
    let mut out = [SrceOut::default()];

    let mut pnt = Out::new(&format!("{dir}/out_pnt.ww3"));
    let mut head = Vec::new();
    text(&mut head, "WAVEWATCH III POINT OUTPUT FILE", 31);
    text(&mut head, "2021-04-06", 10);
    for v in [m.nk, m.nth, 1] {
        head.extend_from_slice(&v.to_be_bytes());
    }
    pnt.record(&head);
    let mut head = Vec::new();
    be_f(&mut head, r.xpt);
    be_f(&mut head, r.ypt);
    text(&mut head, &r.pname, 40);
    pnt.record(&head);

    // W3INIT's output timing. J = 1 writes nothing here, but it splits the time loop like WW3's.
    let mut time = r.time0;
    let mut tonext = [r.field.0, r.point.0];
    let dtout = [r.field.1 as Real, r.point.1 as Real];
    let tolast = [r.field.2, r.point.2];
    let mut flout = [true, true];
    let mut tofrst: Time = [-1, 0];
    for j in 0..2 {
        while dsec21(time, tonext[j]) < 0. {
            tick21(&mut tonext[j], dtout[j]);
        }
        flout[j] = dsec21(tonext[j], tolast[j]) >= 0.;
        if flout[j] && (tofrst[0] == -1 || dsec21(tonext[j], tofrst) > 0.) {
            tofrst = tonext[j];
        }
    }

    let mut wind = Wind { tf0: [-1, 0], tfn: [-1, 0], x0: 0., y0: 0., s0: 0., xn: 0., yn: 0., sn: 0. };
    let (mut ust, mut ustdir): (Real, Real) = (0., 0.);
    let mut time0 = r.time0;
    let timen = r.timen;

    loop {
        // ww3_shel, label 700.
        let mut ttime = timen;
        let dttst = if wind.tfn[0] == -1 { 0. } else { dsec21(time0, wind.tfn) };
        if dttst <= 0. {
            wind.fldh(time0, timen, &mut r.winds);
        }
        if dsec21(wind.tfn, ttime) > 0. {
            ttime = wind.tfn;
        }
        let mut tdn = ttime;
        tick21(&mut tdn, 1.);
        time0 = ttime;
        let tend = time0;

        // W3WAVE.
        let flzero = dsec21(time, tend) == 0.;
        let mut uw: Option<Uwnd> = None;
        let (mut ua, mut ud, mut asv): (Real, Real, Real) = (0., 0., UNDEF);
        loop {
            let tcalc = if tofrst[0] > 0 && dsec21(tend, tofrst) < 0. { tofrst } else { tend };
            let dttst = dsec21(time, tcalc);
            let mut nt = 1 + (dttst / m.dtmax - 0.001) as i32;
            let dtga = dttst / nt as Real;
            let it0 = if dttst == 0. {
                nt = 0;
                0
            } else {
                1
            };
            let mut dtres: Real = 0.;
            for it in it0..=nt {
                let mut dtg = nint(dtga + dtres + 0.0001) as Real;
                dtres = dtres + dtga - dtg;
                if dtres.abs() < 0.001 {
                    dtres = 0.;
                }
                tick21(&mut time, dtg);

                // W3UWND.
                let u = uw.get_or_insert_with(|| uwnd_first(&wind));
                let dt0n = dsec21(wind.tf0, wind.tfn);
                let dt0t = dsec21(wind.tf0, time);
                let rdw = dt0t / dt0n.max(1.0e-7);
                ua = u.ua0 + rdw * u.uai;
                ud = u.ud0 + rdw * u.udi;
                asv = u.as0 + rdw * u.asi;
                let (u10, u10d) = (ua.max(0.001), ud);

                if flzero {
                    break;
                }
                if it == 0 {
                    dtg = 1.;
                }
                let q = SrcePoint {
                    srce_call: SRCE_DIRECT,
                    it,
                    d_inp: dw,
                    u10abs: u10,
                    u10dir: u10d,
                    ustar: ust,
                    ustdir,
                    ice: 0.,
                    dtg,
                    dair: DAIR,
                    coef: 1.,
                };
                let [specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda] =
                    bufs.iter_mut().map(|b| b.as_mut_slice()).collect::<Vec<_>>().try_into().ok().unwrap();
                let mut w = SrceWork {
                    specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda,
                    llws: &mut llws,
                    w_snl: &mut w_snl,
                    w_sds: &mut w_sds,
                    iw_sds: &mut iw_sds,
                    w_small: &mut w_small,
                    whitecap: &mut whitecap,
                    sin_out: &mut sin_out,
                    spr_out: &mut spr_out,
                };
                w3srce(&p, &tables, &q, &mut spec, &wn, &cg, &mut w, &mut out, 0);
                ust = out[0].ustar;
                ustdir = out[0].ustdir;
            }

            // W3WAVE, label 400: outputs at this time.
            let dt1 = if tofrst[0] == -1 { 1. } else { dsec21(time, tofrst) };
            let dt2 = if tdn[0] == -1 { 1. } else { dsec21(time, tdn) };
            if dt1 <= 0. && dt2 != 0. {
                tofrst = [-1, 0];
                for j in 0..2 {
                    if !flout[j] {
                        continue;
                    }
                    if dsec21(time, tonext[j]) == 0. {
                        if j == 1 {
                            write_point(&mut pnt, time, &rd, &is, dw, ua, ud, asv, &spec, &cg, &cg0, m);
                        }
                        tick21(&mut tonext[j], dtout[j]);
                        flout[j] = dsec21(tonext[j], tolast[j]) >= 0.;
                    }
                    if flout[j] && (tofrst[0] == -1 || dsec21(tonext[j], tofrst) > 0.) {
                        tofrst = tonext[j];
                    }
                }
            }
            if dsec21(time, tend) == 0. {
                break;
            }
        }
        if dsec21(time0, timen) <= 0. {
            break;
        }
    }
    pnt.f.flush().unwrap();
}

/// `W3IOPE` and the data records of `W3IOPO`, for one point and one sea point.
#[allow(clippy::too_many_arguments)]
fn write_point(
    pnt: &mut Out,
    time: Time,
    rd: &[Real],
    is: &[i32],
    dw: Real,
    ua: Real,
    ud: Real,
    asv: Real,
    spec: &[Real],
    cg: &[Real],
    cg0: &[Real],
    m: &ModDef,
) {
    let (nth, nk) = (m.nth as usize, m.nk as usize);
    let at = |v: Real, j: usize| if is[j] > 0 { v } else { 0. };
    let sum = |v: Real| rd[0] * at(v, 0) + rd[1] * at(v, 1) + rd[2] * at(v, 2) + rd[3] * at(v, 3);
    let (s, c) = sincosf(ud);
    let wx = rd[0] * at(ua, 0) * if is[0] > 0 { c } else { 1. }
        + rd[1] * at(ua, 1) * if is[1] > 0 { c } else { 1. }
        + rd[2] * at(ua, 2) * if is[2] > 0 { c } else { 1. }
        + rd[3] * at(ua, 3) * if is[3] > 0 { c } else { 1. };
    let wy = rd[0] * at(ua, 0) * if is[0] > 0 { s } else { 0. }
        + rd[1] * at(ua, 1) * if is[1] > 0 { s } else { 0. }
        + rd[2] * at(ua, 2) * if is[2] > 0 { s } else { 0. }
        + rd[3] * at(ua, 3) * if is[3] > 0 { s } else { 0. };
    let wao = sqrtf(wx * wx + wy * wy);
    let wdo = if wao > 1.0e-7 { atan2f(wy, wx) } else { 0. };

    let mut t = Vec::new();
    t.extend_from_slice(&time[0].to_be_bytes());
    t.extend_from_slice(&time[1].to_be_bytes());
    pnt.record(&t);

    let mut b = vec![0u8; 12]; // IW, II, IL
    for v in [sum(dw), wao, wdo, sum(asv), 0., 0., sum(0.), sum(0.5), sum(1000.)] {
        be_f(&mut b, v);
    }
    text(&mut b, "ww3", 13);
    for ik in 0..nk {
        let fac = |j: usize| TPI * m.sig[ik + 1] / if is[j] > 0 { cg[ik] } else { cg0[ik] };
        for ith in 0..nth {
            let a = spec[ith + ik * nth];
            let v = rd[0] * at(a, 0) * fac(0)
                + rd[1] * at(a, 1) * fac(1)
                + rd[2] * at(a, 2) * fac(2)
                + rd[3] * at(a, 3) * fac(3);
            be_f(&mut b, v);
        }
    }
    pnt.record(&b);
}
