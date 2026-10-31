// SPDX-License-Identifier: MPL-2.0
//! The time loop of `ww3_shel` and `W3WAVE` (`model/src/ww3_shel.F90`, `w3wavemd.F90`), with `W3UWND`,
//! `W3FLDH`, `WAVNU1`, `DISTAB` and the point output `W3IOPE`, `W3IOPO`, as far as the first whole case
//! needs them: a rectilinear grid, `PR3 UQ`, homogeneous wind or none, no currents, ice or boundaries.
//! Anything else stops with a message: this is not a general `ww3_shel`.
//!
//! The kernels run on the CPU here. This is the `host` verdict of ADR-0005, and `out_grd.ww3` is not written.

use crate::grid::{Maps, Metrics, unpack, w3dzxy, w3map3, w3mapt, w3utrn};
use crate::insin4::wavnu2_cg;
use crate::iors::{self, State};
use crate::moddef::ModDef;
use crate::wtime::{Time, dsec21, tick21};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use ww3_kernels::constants::{DAIR, GRAV, PI, TPI};
use ww3_kernels::nint;
use ww3_kernels::pro3::{
    Ktp3Params, Ktp3Point, Ktp3Tables, Ktp3Work, Xyp3Params, Xyp3Tables, Xyp3Work, w3ktp3, w3xyp3,
};
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
    pub restart: (Time, i32, Time),
    /// `FLWIND`: `INPUT%FORCING%WINDS` is `H`.
    pub flwind: bool,
    /// `(DATE, VALUE1, VALUE2, VALUE3)` of each homogeneous wind.
    pub winds: VecDeque<(Time, Real, Real, Real)>,
    /// `XPT`, `YPT`, `PNAMES`.
    pub points: Vec<(Real, Real, String)>,
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

/// One output point of `W3IOPP`.
struct Point {
    x: Real,
    y: Real,
    name: String,
    ix: [i32; 4],
    iy: [i32; 4],
    rd: [Real; 4],
}

/// `GETPQR`: Newton on the bilinear map of the cell. A rectangle needs one step, and that step's
/// rounding is what reaches `PTIFAC`.
fn getpqr(xt: f64, yt: f64, xs: &[f64; 4], ys: &[f64; 4]) -> (f64, f64) {
    const EPS: f64 = 1.0e-6;
    for k in 0..4 {
        if (xt - xs[k]).abs() <= EPS && (yt - ys[k]).abs() <= EPS {
            return ([0., 1., 1., 0.][k], [0., 0., 1., 1.][k]);
        }
    }
    let (mut pr, mut qr) = (0.5, 0.5);
    let (dyt, dy1, dy2) = (yt - ys[0], ys[1] - ys[0], ys[3] - ys[0]);
    let dy3 = ys[2] - ys[1] - dy2;
    let (dxt, dx1, dx2) = (xt - xs[0], xs[1] - xs[0], xs[3] - xs[0]);
    let dx3 = xs[2] - xs[1] - dx2;
    for _ in 0..10 {
        let dyp = dyt - dy1 * pr - dy2 * qr - dy3 * pr * qr;
        let dxp = dxt - dx1 * pr - dx2 * qr - dx3 * pr * qr;
        let (mat1, mat2) = (dy1 + dy3 * qr, dy2 + dy3 * pr);
        let (mat3, mat4) = (dx1 + dx3 * qr, dx2 + dx3 * pr);
        let det = mat1 * mat4 - mat2 * mat3;
        let delp = (dyp * mat4 - mat2 * dxp) / det;
        let delq = (mat1 * dxp - dyp * mat3) / det;
        pr += delp;
        qr += delq;
        if delp.abs() < 1.0e-6 && delq.abs() < 1.0e-6 {
            break;
        }
    }
    (pr, qr)
}

/// `W3IOPP` on a rectilinear grid. `W3GRMP` is `REAL(8)` on the `REAL` coordinates of the corners.
fn iopp(m: &ModDef, mapsta: &[i32], pts: &[(Real, Real, String)]) -> Vec<Point> {
    let closed = m.iclose != -1;
    let (nx, ny) = (m.nx, m.ny);
    let xg = |ix: i32| (m.x0 + (ix - 1) as Real * m.sx) as f64;
    let yg = |iy: i32| (m.y0 + (iy - 1) as Real * m.sy) as f64;
    let mut v = Vec::new();
    for (x, y, name) in pts {
        let (mut xt, yt) = (*x as f64, *y as f64);
        if m.flagll {
            while xt < xg(1) {
                xt += 360.;
            }
        }
        // W3GFCL takes the first cell that holds the point within EPS, so a point on a node belongs to
        // the cell before it. Which one matters on a coast: the other may be all land.
        let mut i0 = ((xt - xg(1)) / m.sx as f64).floor() as i32 + 1;
        let mut j0 = ((yt - yg(1)) / m.sy as f64).floor() as i32 + 1;
        if i0 > 1 && (xt - xg(i0)).abs() <= 1.0e-6 {
            i0 -= 1;
        }
        if j0 > 1 && (yt - yg(j0)).abs() <= 1.0e-6 {
            j0 -= 1;
        }
        let ix1 = if closed { i0 } else { i0.min(nx - 1) };
        let iy1 = j0.min(ny - 1);
        if ix1 < 1 || iy1 < 1 || (!closed && ix1 >= nx) {
            continue;
        }
        let xs = [xg(ix1), xg(ix1 + 1), xg(ix1 + 1), xg(ix1)];
        let ys = [yg(iy1), yg(iy1), yg(iy1 + 1), yg(iy1 + 1)];
        let (ixr, jxr) = getpqr(xt, yt, &xs, &ys);
        if ixr > 1. + 1e-6 || jxr > 1. + 1e-6 {
            continue;
        }
        let wrap = |ix: i32| if closed { (ix - 1).rem_euclid(nx) + 1 } else { ix };
        let ix = [wrap(ix1), wrap(ix1 + 1), wrap(ix1 + 1), wrap(ix1)];
        let iy = [iy1, iy1, iy1 + 1, iy1 + 1];
        let rd = [
            ((1. - ixr) * (1. - jxr)) as Real,
            (ixr * (1. - jxr)) as Real,
            (ixr * jxr) as Real,
            ((1. - ixr) * jxr) as Real,
        ];
        let land = (0..4).all(|j| mapsta[((iy[j] - 1) + (ix[j] - 1) * ny) as usize] == 0);
        if !land {
            v.push(Point { x: *x, y: *y, name: name.clone(), ix, iy, rd });
        }
    }
    v
}

/// `ISP` of the clock: `DSEC21` goes into an `INTEGER` in `W3WAVE` and `W3XYP3`.
fn odd_step(time: Time, dtg: Real) -> i32 {
    let dttst = dsec21([time[0], 0], time) as i32;
    nint(dttst as Real / dtg) % 2
}

/// Runs the case. Writes `out_pnt.ww3` and `restartNNN.ww3` into `dir`.
pub fn run(m: &ModDef, mut r: Run, dir: &str) {
    assert!(!m.fldry, "FLDRY is not implemented");
    // W3IOBC clears FLBPI when there is no nest file, and W3XYP3 then has no boundary input.
    assert!(!m.flbpi || !std::path::Path::new(&format!("{dir}/nest.ww3")).exists(), "input boundaries are not implemented");
    let (nth, nk, nsea) = (m.nth as usize, m.nk as usize, m.nsea as usize);
    let (nx, ny) = (m.nx as usize, m.ny as usize);
    let (nspec, nk2) = (nth * nk, nk + 2);
    let nxy = nx * ny;
    let t = m.snl1();
    let p = m.srce(t.params, r.flogrd);
    let tables = m.tables(&t);

    // W3INIT.
    let (mut mapsta, mut mapst2) = unpack(&m.maptmp);
    let maptst = mapsta.clone();
    let mut s: State = iors::read(&format!("{dir}/restart.ww3"), m, r.time0, &mut mapsta, &mut mapst2);
    for (sta, tst) in mapsta.iter_mut().zip(&maptst) {
        if sta.abs() == 2 || tst.abs() == 2 {
            *sta = if *sta < 0 { -tst.abs() } else { tst.abs() };
        }
    }
    let mut dw = vec![0.; nsea + 1];
    let mut dry = vec![0; nxy];
    for (c, v) in mapst2.iter().enumerate() {
        dry[c] = v / 2 % 2;
    }
    for isea in 1..=nsea {
        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
        let d = s.wlv[isea] - m.zb[isea - 1];
        dw[isea] = (0f32).max(d);
        if d <= 0. {
            mapst2[c] -= 2 * dry[c];
            dry[c] = 1;
            mapst2[c] += 2;
            mapsta[c] = -mapsta[c].abs();
            s.va[nspec * isea..nspec * (isea + 1)].fill(0.);
        }
    }
    let disp = Dispersion::new();
    let (mut wn, mut cg) = (vec![0.; nk2 * (nsea + 1)], vec![0.; nk2 * (nsea + 1)]);
    for is in 0..=nsea {
        let depth = if is > 0 { m.dmin.max(dw[is]) } else { m.dmin };
        for ik in 0..nk2 {
            (wn[ik + nk2 * is], cg[ik + nk2 * is]) = disp.wavnu1(m.sig[ik], depth);
        }
    }
    let metrics = Metrics::new(m);
    let pts = iopp(m, &mapsta, &r.points);

    // Scratch of W3SRCE, per call.
    let mut bufs: Vec<Vec<Real>> = (0..17).map(|_| vec![0.; nspec]).collect();
    let (l_snl, l_sds, l_ids) = work_lens(&p);
    let (mut llws, mut w_snl, mut w_sds, mut iw_sds) =
        (vec![0u32; nspec], vec![0.; l_snl], vec![0.; l_sds], vec![0i32; l_ids]);
    let mut w_small = vec![0.; work_small(&p)];
    let mut whitecap = vec![0.; 4 * (nsea + 1)];
    let (mut sin_out, mut spr_out) = ([Sin4Out::default()], [Spr4Out::default()]);
    let mut out = [SrceOut::default()];

    // Scratch of W3XYP3 and W3KTP3.
    let (mut iixy1, mut iixy2, mut iixy3, mut iixy4) =
        (vec![0; nsea], vec![0; nsea], vec![0; nsea], vec![0; nsea]);
    let (mut rdi1, mut rdi2, mut rdi3, mut rdi4) =
        (vec![0.; nsea], vec![0.; nsea], vec![0.; nsea], vec![0.; nsea]);
    let mut mapstx = vec![0; ny * (nx + 4)];
    let (mut vlcflx, mut vlcfly) = (vec![0.; (nx + 1) * ny], vec![0.; (nx + 1) * ny]);
    let (mut cxtot, mut cytot) = (vec![0.; (nx + 1) * ny], vec![0.; nx * ny]);
    let (mut field, mut aq) = (vec![0.; ny * (nx + 3)], vec![0.; ny * (nx + 3)]);
    let mut fla = vec![0.; ny * (nx + 1)];
    let (mut dsdd, mut frk, mut frg, mut fkc) = (vec![0.; nk2], vec![0.; nk], vec![0.; nk], vec![0.; nth]);
    let mut vq_k = vec![0.; nk2 * (nth + 3)];
    let (mut db, mut dm) = (vec![0.; nk2 * (nth + 1)], vec![0.; nk2 * (nth + 2)]);
    let (mut vcflt, mut cflk) = (vec![0.; nk2 * (nth + 1)], vec![0.; nk2 * nth]);
    let mut fla_k = vec![0.; nk2 * (nth + 1)];

    let mut mp = Maps::default();
    let (mut atrnx, mut atrny) = (vec![1.; 3 * nxy], vec![1.; 3 * nxy]);
    let mut maptrn = vec![0u32; nxy];
    let (mut dddx, mut dddy) = (vec![0.; nxy], vec![0.; nxy]);

    // W3INIT's output timing, for J = 1, 2, 4. J = 1 writes nothing here, but it splits the time loop.
    let mut time = r.time0;
    const JS: [usize; 3] = [1, 2, 4];
    let mut tonext = [r.field.0, r.point.0, r.restart.0];
    let dtout = [r.field.1 as Real, r.point.1 as Real, r.restart.1 as Real];
    let tolast = [r.field.2, r.point.2, r.restart.2];
    let mut flout = [false; 3];
    let mut tofrst: Time = [-1, 0];
    for j in 0..3 {
        if dtout[j] <= 0.5 {
            continue;
        }
        loop {
            let d = dsec21(time, tonext[j]);
            if (JS[j] != 4 && d < 0.) || (JS[j] == 4 && d <= 0.) {
                tick21(&mut tonext[j], dtout[j]);
            } else {
                break;
            }
        }
        flout[j] = dsec21(tonext[j], tolast[j]) >= 0.;
        if flout[j] && (tofrst[0] == -1 || dsec21(tonext[j], tofrst) > 0.) {
            tofrst = tonext[j];
        }
    }

    let mut pnt = (flout[1] && !pts.is_empty()).then(|| {
        let mut f = Out::new(&format!("{dir}/out_pnt.ww3"));
        let mut h = Vec::new();
        text(&mut h, "WAVEWATCH III POINT OUTPUT FILE", 31);
        text(&mut h, "2021-04-06", 10);
        for v in [m.nk, m.nth, pts.len() as i32] {
            h.extend_from_slice(&v.to_be_bytes());
        }
        f.record(&h);
        let mut h = Vec::new();
        for q in &pts {
            be_f(&mut h, q.x);
            be_f(&mut h, q.y);
        }
        for q in &pts {
            text(&mut h, &q.name, 40);
        }
        f.record(&h);
        f
    });

    let mut wind = Wind { tf0: [-1, 0], tfn: [-1, 0], x0: 0., y0: 0., s0: 0., xn: 0., yn: 0., sn: 0. };
    let mut time0 = r.time0;
    let timen = r.timen;
    let mut itime = 0;
    let mut ifile = 1;
    let (flcx, flcy, flcth, flsou) = (m.flcx, m.flcy, m.flcth, m.flsou);
    let flck = false; // W3INIT: no currents here.
    let xp = Xyp3Params {
        nx: m.nx,
        ny: m.ny,
        nsea: m.nsea,
        nth: m.nth,
        flcx: flcx as u32,
        flcy: flcy as u32,
        global: (m.iclose != -1) as u32,
        flagll: m.flagll as u32,
        dtcfl: m.dtcfl,
        wdth: m.wdth,
        wdcg: m.wdcg,
        dth: m.dth,
        xfr: m.xfr,
        ..Default::default()
    };
    let kp = Ktp3Params {
        nk: m.nk,
        nth: m.nth,
        nspec: m.nk * m.nth,
        flcth: flcth as u32,
        flck: flck as u32,
        dmin: m.dmin,
        ctmax: m.ctmax,
        fachfa: m.fachfa,
    };

    loop {
        // ww3_shel, label 700.
        let mut ttime = timen;
        if r.flwind {
            let dttst = if wind.tfn[0] == -1 { 0. } else { dsec21(time0, wind.tfn) };
            if dttst <= 0. {
                wind.fldh(time0, timen, &mut r.winds);
            }
            if dsec21(wind.tfn, ttime) > 0. {
                ttime = wind.tfn;
            }
        }
        let mut tdn = ttime;
        tick21(&mut tdn, 1.);
        time0 = ttime;
        let tend = time0;

        // W3WAVE.
        let flzero = dsec21(time, tend) == 0.;
        let mut flfrst = true;
        let mut flmap = itime == 0;
        let mut flddir = itime == 0 && flcth;
        let mut uw: Option<Uwnd> = None;
        let (mut ua, mut ud, mut asv): (Real, Real, Real) = (0., 0., UNDEF);
        let mut u10: Real = 0.01;
        let mut u10d: Real = 0.;
        loop {
            let tcalc = if tofrst[0] > 0 && dsec21(tend, tofrst) < 0. { tofrst } else { tend };
            let dttst = dsec21(time, tcalc);
            let mut nt = 1 + (dttst / m.dtmax - 0.001) as i32;
            let dtga = dttst / nt as Real;
            let it0 = if dttst == 0. {
                if !flzero {
                    itime -= 1;
                }
                nt = 0;
                0
            } else {
                1
            };
            let mut dtres: Real = 0.;
            for it in it0..=nt {
                itime += 1;
                let mut dtg = nint(dtga + dtres + 0.0001) as Real;
                dtres = dtres + dtga - dtg;
                if dtres.abs() < 0.001 {
                    dtres = 0.;
                }
                tick21(&mut time, dtg);

                if r.flwind {
                    // W3UWND.
                    let u = uw.get_or_insert_with(|| uwnd_first(&wind));
                    let dt0n = dsec21(wind.tf0, wind.tfn);
                    let dt0t = dsec21(wind.tf0, time);
                    let rdw = dt0t / dt0n.max(1.0e-7);
                    ua = u.ua0 + rdw * u.uai;
                    ud = u.ud0 + rdw * u.udi;
                    asv = u.as0 + rdw * u.asi;
                    (u10, u10d) = (ua.max(0.001), ud);
                } else if flfrst {
                    s.ust[1..].fill(0.05);
                    s.ustdir[1..].fill(0.05);
                }

                if flmap {
                    w3map3(m, &mapsta, &mut mp);
                    w3utrn(m, &mapsta, &mut atrnx, &mut atrny);
                    w3mapt(m, &atrnx, &atrny, &mut maptrn);
                    flmap = false;
                }
                if flddir {
                    w3dzxy(m, &metrics, &mapsta, &dw, &mut dddx, &mut dddy);
                    flddir = false;
                }
                flfrst = false;

                if flzero {
                    break;
                }
                if it == 0 {
                    dtg = 1.;
                } else {
                    let ntloc = 1 + (dtg / m.dtcfli - 0.001) as i32;
                    let facth = dtg / (m.dth * ntloc as Real);
                    let fack = dtg / ntloc as Real;
                    let itloch = (ntloc + 1 - odd_step(time, dtg)) / 2;
                    let kt = Ktp3Tables {
                        sig: &m.sig,
                        dsip: &m.dsip,
                        ecos: &m.ecos,
                        esin: &m.esin,
                        ec2: &m.ec2,
                        esc: &m.esc,
                        es2: &m.es2,
                        mapwn: &m.mapwn,
                        mapth2: &mp.mapth2,
                        mapwn2: &mp.mapwn2,
                    };
                    let mut spectral = |va: &mut [Real], n: i32| {
                        for _ in 0..n {
                            for isea in 1..=nsea {
                                let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                if mapsta[c] != 1 {
                                    continue;
                                }
                                let q = Ktp3Point {
                                    itime,
                                    facth,
                                    fack,
                                    cthg0: m.cthg0s[isea - 1],
                                    dw: m.dmin.max(dw[isea]),
                                    dddx: dddx[c],
                                    dddy: dddy[c],
                                    ..Default::default()
                                };
                                let mut w = Ktp3Work {
                                    dsdd: &mut dsdd,
                                    frk: &mut frk,
                                    frg: &mut frg,
                                    fkc: &mut fkc,
                                    vq: &mut vq_k,
                                    db: &mut db,
                                    dm: &mut dm,
                                    vcflt: &mut vcflt,
                                    cflk: &mut cflk,
                                    fla: &mut fla_k,
                                };
                                w3ktp3(
                                    &kp,
                                    &kt,
                                    &q,
                                    &cg[nk2 * isea..nk2 * (isea + 1)],
                                    &wn[nk2 * isea..nk2 * (isea + 1)],
                                    &mut va[nspec * isea..nspec * (isea + 1)],
                                    &mut w,
                                );
                            }
                        }
                    };
                    if flcth || flck {
                        spectral(&mut s.va, itloch);
                    }
                    if flcx || flcy {
                        let yfirst = odd_step(time, dtg) == 0;
                        let px = Xyp3Params {
                            nact: mp.nact,
                            ncent: mp.ncent,
                            nmx0: mp.nmx[0],
                            nmx1: mp.nmx[1],
                            nmx2: mp.nmx[2],
                            nmy0: mp.nmy[0],
                            nmy1: mp.nmy[1],
                            nmy2: mp.nmy[2],
                            ..xp
                        };
                        let tb = Xyp3Tables {
                            sig: &m.sig,
                            ecos: &m.ecos,
                            esin: &m.esin,
                            mapsf: &m.mapsf,
                            mapsta: &mapsta,
                            mapaxy: &mp.mapaxy,
                            mapcxy: &mp.mapcxy,
                            mapx2: &mp.mapx2,
                            mapy2: &mp.mapy2,
                            maptrn: &maptrn,
                            clats: &m.clats,
                            dpdx: &metrics.dpdx,
                            dpdy: &metrics.dpdy,
                            dqdx: &metrics.dqdx,
                            dqdy: &metrics.dqdy,
                            gsqrt: &metrics.gsqrt,
                            cg: &cg,
                            atrnx: &atrnx,
                            atrny: &atrny,
                        };
                        let base = ny - 1;
                        for ispec in 0..nspec {
                            // W3GATH.
                            field.fill(0.);
                            for isea in 1..=nsea {
                                let ixy = m.mapsf[isea - 1 + 2 * nsea] as usize;
                                field[ixy + base] = s.va[ispec + nspec * isea];
                            }
                            let mut w = Xyp3Work {
                                iixy1: &mut iixy1,
                                iixy2: &mut iixy2,
                                iixy3: &mut iixy3,
                                iixy4: &mut iixy4,
                                rdi1: &mut rdi1,
                                rdi2: &mut rdi2,
                                rdi3: &mut rdi3,
                                rdi4: &mut rdi4,
                                mapstx: &mut mapstx,
                                vlcflx: &mut vlcflx,
                                vlcfly: &mut vlcfly,
                                cxtot: &mut cxtot,
                                cytot: &mut cytot,
                                aq: &mut aq,
                                fla: &mut fla,
                            };
                            w3xyp3(&px, &tb, ispec as i32 + 1, dtg, yfirst, &mut field, &mut w);
                            // W3SCAT.
                            for isea in 1..=nsea {
                                let ixy = m.mapsf[isea - 1 + 2 * nsea] as usize;
                                if mapsta[ixy - 1] != 0 {
                                    s.va[ispec + nspec * isea] = field[ixy + base];
                                }
                            }
                        }
                    }
                    if flcth || flck {
                        spectral(&mut s.va, ntloc - itloch);
                    }
                }

                if flsou {
                    for isea in 1..=nsea {
                        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                        if mapsta[c] != 1 {
                            s.ust[isea] = UNDEF;
                            s.ustdir[isea] = UNDEF;
                            continue;
                        }
                        let q = SrcePoint {
                            srce_call: SRCE_DIRECT,
                            it,
                            d_inp: dw[isea],
                            u10abs: u10,
                            u10dir: u10d,
                            ustar: s.ust[isea],
                            ustdir: s.ustdir[isea],
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
                            whitecap: &mut whitecap[4 * isea..4 * (isea + 1)],
                            sin_out: &mut sin_out,
                            spr_out: &mut spr_out,
                        };
                        let spec = &mut s.va[nspec * isea..nspec * (isea + 1)];
                        let (a, b) = (1 + nk2 * isea, nk2 * isea + nk + 1);
                        w3srce(&p, &tables, &q, spec, &wn[a..b], &cg[a..b], &mut w, &mut out, 0);
                        s.ust[isea] = out[0].ustar;
                        s.ustdir[isea] = out[0].ustdir;
                    }
                }
            }

            // W3WAVE, label 400: outputs at this time.
            let dt1 = if tofrst[0] == -1 { 1. } else { dsec21(time, tofrst) };
            let dt2 = if tdn[0] == -1 { 1. } else { dsec21(time, tdn) };
            if dt1 <= 0. && dt2 != 0. {
                tofrst = [-1, 0];
                for j in 0..3 {
                    if !flout[j] {
                        continue;
                    }
                    if dsec21(time, tonext[j]) == 0. {
                        if JS[j] == 2 {
                            if let Some(f) = pnt.as_mut() {
                                let w = Winds { ua, ud, asv };
                                write_points(f, time, &pts, m, &mapsta, &s.va, &dw, &cg, &w);
                            }
                        } else if JS[j] == 4 {
                            let path = format!("{dir}/restart{ifile:03}.ww3");
                            ifile += 1;
                            iors::write(&path, m, &mut s, time, &mapsta, &mapst2);
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
    if let Some(f) = pnt.as_mut() {
        f.f.flush().unwrap();
    }
}

/// `UA`, `UD`, `AS`: the wind is homogeneous, at every sea point but not at `ISEA = 0`.
struct Winds {
    ua: Real,
    ud: Real,
    asv: Real,
}

/// `W3IOPE` and the data records of `W3IOPO`.
#[allow(clippy::too_many_arguments)]
fn write_points(
    pnt: &mut Out,
    time: Time,
    pts: &[Point],
    m: &ModDef,
    mapsta: &[i32],
    va: &[Real],
    dw: &[Real],
    cg: &[Real],
    w: &Winds,
) {
    let (nth, nk, ny) = (m.nth as usize, m.nk as usize, m.ny);
    let (nspec, nk2) = (nth * nk, nk + 2);
    let mut t = Vec::new();
    t.extend_from_slice(&time[0].to_be_bytes());
    t.extend_from_slice(&time[1].to_be_bytes());
    pnt.record(&t);

    for q in pts {
        let at = |j: usize| ((q.iy[j] - 1) + (q.ix[j] - 1) * ny) as usize;
        let is: [usize; 4] = std::array::from_fn(|j| m.mapfs[at(j)].max(0) as usize);
        let mut rd = q.rd;
        let (mut rds, mut rdi): (Real, Real) = (0., 0.);
        for j in 0..4 {
            let im = mapsta[at(j)];
            if im > 0 {
                rds += rd[j];
            } else if im < 0 {
                rdi += rd[j];
            } else {
                rd[j] = 0.;
            }
        }
        if rds + rdi > 1.0e-7 {
            let f = 1. / (rds + rdi);
            rd = rd.map(|x| x * f);
        }
        let on = |v: Real, j: usize| if is[j] > 0 { v } else { 0. };
        let sum = |v: Real| rd[0] * on(v, 0) + rd[1] * on(v, 1) + rd[2] * on(v, 2) + rd[3] * on(v, 3);
        let dpo = rd[0] * dw[is[0]] + rd[1] * dw[is[1]] + rd[2] * dw[is[2]] + rd[3] * dw[is[3]];
        let (sn, cs) = sincosf(w.ud);
        let (c, sx) = (|j: usize| if is[j] > 0 { cs } else { 1. }, |j: usize| if is[j] > 0 { sn } else { 0. });
        let wx = rd[0] * on(w.ua, 0) * c(0)
            + rd[1] * on(w.ua, 1) * c(1)
            + rd[2] * on(w.ua, 2) * c(2)
            + rd[3] * on(w.ua, 3) * c(3);
        let wy = rd[0] * on(w.ua, 0) * sx(0)
            + rd[1] * on(w.ua, 1) * sx(1)
            + rd[2] * on(w.ua, 2) * sx(2)
            + rd[3] * on(w.ua, 3) * sx(3);
        let wao = sqrtf(wx * wx + wy * wy);
        let wdo = if wao > 1.0e-7 { atan2f(wy, wx) } else { 0. };

        let mut b = vec![0u8; 12]; // W3IOPO zeroes IW, II, IL before it writes them.
        for v in [dpo, wao, wdo, sum(w.asv), 0., 0., sum(0.), sum(0.5), sum(1000.)] {
            be_f(&mut b, v);
        }
        text(&mut b, "ww3", 13);
        if rds > 1.0e-7 {
            let f = (rds + rdi) / rds;
            rd = rd.map(|x| x * f);
        }
        for ik in 0..nk {
            let fac = |j: usize| TPI * m.sig[ik + 1] / cg[ik + 1 + nk2 * is[j]];
            for ith in 0..nth {
                let i = ith + ik * nth;
                let a = |j: usize| va[i + nspec * is[j]];
                let v = rd[0] * a(0) * fac(0) + rd[1] * a(1) * fac(1) + rd[2] * a(2) * fac(2) + rd[3] * a(3) * fac(3);
                be_f(&mut b, v);
            }
        }
        pnt.record(&b);
    }
}
