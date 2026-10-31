// SPDX-License-Identifier: MPL-2.0
//! The time loop of `ww3_shel` and `W3WAVE` (`model/src/ww3_shel.F90`, `w3wavemd.F90`), with `W3UWND`,
//! `W3FLDH`, `WAVNU1`, `DISTAB` and the point output `W3IOPE`, `W3IOPO`, as far as the first whole case
//! needs them: rectilinear or curvilinear grids, `PR3 UQ`, winds and currents, no ice, water levels or
//! boundary input.
//! Anything else stops with a message: this is not a general `ww3_shel`.
//!
//! The kernels run on the CPU here, or propagation runs through a `Device`.

use crate::grid::{Maps, Metrics, unpack, w3dzxy, w3map3, w3mapt, w3utrn};
use crate::insin4::wavnu2_cg;
use crate::iogo::{self, Grd, Input};
use crate::iors::{self, State};
use crate::moddef::{ModDef, Records};
use crate::wtime::{Time, dsec21, tick21};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use ww3_kernels::constants::{DAIR, DERA, GRAV, PI, RADIUS, TPI, TPIINV};
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
const U10MIN: Real = 1.;
const U10MAX: Real = 20.;

/// What the case's `ww3_shel.nml` and point list give.
pub struct Run {
    pub time0: Time,
    pub timen: Time,
    /// Start, stride in seconds and end of the field output: it steers the time loop.
    pub field: (Time, i32, Time),
    pub point: (Time, i32, Time),
    pub restart: (Time, i32, Time),
    pub wind: Option<Forcing>,
    pub current: Option<Forcing>,
    /// `XPT`, `YPT`, `PNAMES`.
    pub points: Vec<(Real, Real, String)>,
    /// `FLOGRD(5,7)` and `FLOGRD(5,8)`.
    pub flogrd: (bool, bool),
    /// `TYPE%FIELD%LIST`.
    pub fields: String,
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

/// Where a forcing field comes from.
enum Source {
    /// `(DATE, VALUE1, VALUE2, VALUE3)`, for `W3FLDH`.
    Homogeneous(VecDeque<(Time, Real, Real, Real)>),
    /// `current.ww3` or `wind.ww3`, for `W3FLDG`.
    File(Records),
}

/// `TC0, CX0, CY0, TCN, CXN, CYN` or `TW0, WX0, WY0, DT0, TWN, WXN, WYN, DTN`, with the grids `(NX,NY)`.
pub struct Forcing {
    src: Source,
    tf0: Time,
    tfn: Time,
    x0: Vec<Real>,
    y0: Vec<Real>,
    s0: Vec<Real>,
    xn: Vec<Real>,
    yn: Vec<Real>,
    sn: Vec<Real>,
}

impl Forcing {
    pub fn homogeneous(q: VecDeque<(Time, Real, Real, Real)>, n: usize) -> Self {
        Self::new(Source::Homogeneous(q), n)
    }

    /// `W3FLDO ('READ', ...)`.
    pub fn file(path: &str, n: usize) -> Self {
        let mut r = Records::open(path);
        r.next();
        Self::new(Source::File(r), n)
    }

    fn new(src: Source, n: usize) -> Self {
        let z = || vec![0.; n];
        Self { src, tf0: [-1, 0], tfn: [-1, 0], x0: z(), y0: z(), s0: z(), xn: z(), yn: z(), sn: z() }
    }

    fn shift(&mut self) {
        self.tf0 = self.tfn;
        if self.tfn[0] != -1 {
            self.x0.copy_from_slice(&self.xn);
            self.y0.copy_from_slice(&self.yn);
            self.s0.copy_from_slice(&self.sn);
        }
    }

    /// `W3FLDH` for `J` of 2 or 3, or `W3FLDG`.
    fn next(&mut self, t0: Time, tn: Time) {
        let dera = atanf(1.) / 45.;
        loop {
            self.shift();
            let got = match &mut self.src {
                Source::Homogeneous(q) => q.pop_front().map(|(t, ha, hd, hs)| {
                    let (s, c) = sincosf((270. - hd) * dera);
                    (t, vec![ha * c], vec![ha * s], hs)
                }),
                Source::File(r) => {
                    let t = r.try_next().map(|mut c| [c.i(), c.i()]);
                    let n = self.xn.len();
                    t.map(|t| (t, r.next().fs(n), r.next().fs(n), 0.))
                }
            };
            let Some((t, x, y, hs)) = got else {
                self.tfn = tn;
                tick21(&mut self.tfn, 1.);
                if matches!(self.src, Source::File(_)) {
                    break;
                }
                if dsec21(t0, self.tfn) > 0. {
                    return;
                }
                continue;
            };
            self.tfn = t;
            if x.len() == 1 {
                self.xn.fill(x[0]);
                self.yn.fill(y[0]);
            } else {
                self.xn.copy_from_slice(&x);
                self.yn.copy_from_slice(&y);
            }
            self.sn.fill(hs);
            if dsec21(t0, self.tfn) > 0. {
                break;
            }
        }
        if matches!(self.src, Source::File(_)) && self.tf0[0] == -1 {
            self.tf0 = t0;
            self.x0.copy_from_slice(&self.xn);
            self.y0.copy_from_slice(&self.yn);
            self.s0.copy_from_slice(&self.sn);
        }
    }
}

/// `UA0, UAI, UD0, UDI, AS0, ASI` or `CA0, CAI, CD0, CDI`, by `ISEA`.
struct Polar {
    a0: Vec<Real>,
    ai: Vec<Real>,
    d0: Vec<Real>,
    di: Vec<Real>,
    s0: Vec<Real>,
    si: Vec<Real>,
}

/// The `FLFRST` branch of `W3UWND` and `W3UCUR`.
fn polar(m: &ModDef, f: &Forcing) -> Polar {
    let nsea = m.nsea as usize;
    let z = || vec![0.; nsea + 1];
    let mut p = Polar { a0: z(), ai: z(), d0: z(), di: z(), s0: z(), si: z() };
    for isea in 1..=nsea {
        let (ix, iy) = (m.mapsf[isea - 1] as usize, m.mapsf[isea - 1 + nsea] as usize);
        let c = (ix - 1) + (iy - 1) * m.nx as usize;
        let (x0, y0, xn, yn) = (f.x0[c], f.y0[c], f.xn[c], f.yn[c]);
        let a0 = sqrtf(x0 * x0 + y0 * y0);
        let an = sqrtf(xn * xn + yn * yn);
        let d0 = if a0 > 1.0e-7 { (TPI + atan2f(y0, x0)) % TPI } else { 0. };
        let dn = if an > 1.0e-7 { (TPI + atan2f(yn, xn)) % TPI } else { d0 };
        let u0 = if a0 > 1.0e-7 { d0 } else { dn };
        let mut dd = dn - u0;
        if dd.abs() > PI {
            dd = dd - TPI * if dd.is_sign_negative() { -1. } else { 1. };
        }
        (p.a0[isea], p.ai[isea], p.d0[isea], p.di[isea]) = (a0, an - a0, u0, dd);
        (p.s0[isea], p.si[isea]) = (f.s0[c], f.sn[c] - f.s0[c]);
    }
    p
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

/// `W3CKCL` away from a pole and a branch cut: the point is on one side of all four edges.
fn in_cell(xt: f64, yt: f64, xs: &[f64; 4], ys: &[f64; 4]) -> bool {
    const EPS: f64 = 1.0e-6;
    if (0..4).any(|i| (xt - xs[i]).abs() <= EPS && (yt - ys[i]).abs() <= EPS) {
        return true;
    }
    let mut sign1 = 0f64;
    for i in 0..4 {
        let j = (i + 1) % 4;
        let (v1x, v1y) = (xs[j] - xs[i], ys[j] - ys[i]);
        let (v2x, v2y) = (xt - xs[i], yt - ys[i]);
        let mut cross = v1x * v2y - v1y * v2x;
        if cross.abs() < EPS {
            cross = 0.;
        }
        if sign1.abs() <= EPS {
            if cross.abs() > EPS {
                sign1 = 1f64.copysign(cross);
            }
        } else if cross.abs() > EPS && 1f64.copysign(cross) != sign1 {
            return false;
        }
    }
    true
}

/// `W3IOPP`. `W3GRMP` is `REAL(8)` on the `REAL` coordinates of the corners. A curvilinear grid looks at
/// every cell, where WW3 goes through buckets: the first cell that holds the point is the same.
fn iopp(m: &ModDef, mapsta: &[i32], pts: &[(Real, Real, String)]) -> Vec<Point> {
    let closed = m.iclose != -1;
    let (nx, ny) = (m.nx, m.ny);
    let at = |ix: i32, iy: i32| ((iy - 1) + (ix - 1) * ny) as usize;
    let xg = |ix: i32| (m.x0 + (ix - 1) as Real * m.sx) as f64;
    let yg = |iy: i32| (m.y0 + (iy - 1) as Real * m.sy) as f64;
    let xmin = m.xgrd.iter().copied().fold(Real::INFINITY, Real::min);
    let l360 = xmin >= 0.;
    let wrap360 = |x: f64| {
        let x = x % 360.;
        if closed || l360 {
            if x < 0. { x + 360. } else { x }
        } else if x > 180. {
            x - 360.
        } else {
            x
        }
    };
    let mut v = Vec::new();
    for (x, y, name) in pts {
        let (xt, yt) = (*x as f64, *y as f64);
        let cell = if m.gtype == 2 {
            let xw = if m.flagll { wrap360(xt) } else { xt };
            let mut found = None;
            'search: for iy1 in 1..ny {
                for ix1 in 1..nx {
                    let c = [at(ix1, iy1), at(ix1 + 1, iy1), at(ix1 + 1, iy1 + 1), at(ix1, iy1 + 1)];
                    let xs = c.map(|k| if m.flagll { wrap360(m.xgrd[k] as f64) } else { m.xgrd[k] as f64 });
                    let ys = c.map(|k| m.ygrd[k] as f64);
                    if in_cell(xw, yt, &xs, &ys) {
                        assert!(90. - (ys.iter().sum::<f64>() / 4.).abs() > 10., "a point near the pole");
                        found = Some((ix1, iy1, getpqr(xw, yt, &xs, &ys)));
                        break 'search;
                    }
                }
            }
            match found {
                Some(f) => f,
                None => continue,
            }
        } else {
            let mut xr = xt;
            if m.flagll {
                while xr < xg(1) {
                    xr += 360.;
                }
            }
            // W3GFCL takes the first cell that holds the point within EPS, so a point on a node belongs
            // to the cell before it. Which one matters on a coast: the other may be all land.
            let mut i0 = ((xr - xg(1)) / m.sx as f64).floor() as i32 + 1;
            let mut j0 = ((yt - yg(1)) / m.sy as f64).floor() as i32 + 1;
            if i0 > 1 && (xr - xg(i0)).abs() <= 1.0e-6 {
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
            let (ixr, jxr) = getpqr(xr, yt, &xs, &ys);
            if ixr > 1. + 1e-6 || jxr > 1. + 1e-6 {
                continue;
            }
            (ix1, iy1, (ixr, jxr))
        };
        let (ix1, iy1, (ixr, jxr)) = cell;
        let wrap = |ix: i32| if closed { (ix - 1).rem_euclid(nx) + 1 } else { ix };
        let ix = [wrap(ix1), wrap(ix1 + 1), wrap(ix1 + 1), wrap(ix1)];
        let iy = [iy1, iy1, iy1 + 1, iy1 + 1];
        let rd = [
            ((1. - ixr) * (1. - jxr)) as Real,
            (ixr * (1. - jxr)) as Real,
            (ixr * jxr) as Real,
            ((1. - ixr) * jxr) as Real,
        ];
        let land = (0..4).all(|j| mapsta[at(ix[j], iy[j])] == 0);
        if !land {
            v.push(Point { x: *x, y: *y, name: name.clone(), ix, iy, rd });
        }
    }
    v
}

/// `MAPSTX`: `MAPSTA` with two ghost columns on each side.
fn mapstx_fill(m: &ModDef, mapsta: &[i32], x: &mut [i32]) {
    let (nx, ny) = (m.nx as usize, m.ny as usize);
    let global = m.iclose != -1;
    x[2 * ny..2 * ny + nx * ny].copy_from_slice(mapsta);
    for i in 0..2 * ny {
        x[i] = if global { mapsta[i + (nx - 2) * ny] } else { 0 };
        x[i + nx * ny + 2 * ny] = if global { mapsta[i] } else { 0 };
    }
}

/// `W3UINI`: a fetch-limited JONSWAP with `COS**2` spreading, at every sea point.
fn w3uini(m: &ModDef, g: &Metrics, u10: &[Real], u10d: &[Real], cg: &[Real], va: &mut [Real]) {
    let (nsea, nk, nth) = (m.nsea as usize, m.nk as usize, m.nth as usize);
    let (nspec, nk2) = (nk * nth, nk + 2);
    va.fill(0.);
    let mut a = vec![0.; nk];
    let mut d1 = vec![0.; nth];
    for isea in 1..=nsea {
        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
        let mut xgr = 0.5 * (g.hpfac[c] * g.hpfac[c] + g.hqfac[c] * g.hqfac[c]).sqrt();
        if m.flagll {
            xgr = xgr * RADIUS * DERA;
        }
        let u10c = u10[isea].min(U10MAX).max(U10MIN);
        let xstar = GRAV * xgr / (u10c * u10c);
        let fstar = 3.5 / xstar.powf(0.33);
        let gamma = (1f32).max(7.0 / xstar.powf(0.143));
        let alfa = 0.076 / xstar.powf(0.22);
        let fp = fstar * GRAV / u10c;
        let yln = gamma.ln();
        for ik in 1..=nk {
            let fr = m.sig[ik] * TPIINV;
            let fr2 = fr * fr;
            let aa = alfa * 0.06175 / (fr2 * fr * fr2);
            let q = fp / fr;
            let q2 = q * q;
            let bb = (-50f32).max(-1.25 * (q2 * q2));
            let r = (fr - fp) / (0.07 * fp);
            let cc = (-50f32).max(-0.5 * (r * r));
            a[ik - 1] = aa * (bb + cc.exp() * yln).exp();
        }
        let mut d1int: Real = 0.;
        for ith in 0..nth {
            let v = (0f32).max((m.th[ith] - u10d[isea]).cos());
            d1[ith] = v * v;
            d1int += d1[ith];
        }
        d1int = d1int * m.dth;
        let f1 = TPIINV / d1int;
        for ik in 1..=nk {
            let f2 = f1 * a[ik - 1] * cg[ik + nk2 * isea] / m.sig[ik];
            for ith in 0..nth {
                va[ith + (ik - 1) * nth + nspec * isea] = f2 * d1[ith];
            }
        }
    }
}

/// `W3CFLXY`, for every sea point.
#[allow(clippy::too_many_arguments)]
fn w3cflxy(m: &ModDef, g: &Metrics, cg: &[Real], cx: &[Real], cy: &[Real], flcur: bool, dtg: Real, out: &mut [Real]) {
    let (nsea, nk, nth) = (m.nsea as usize, m.nk as usize, m.nth as usize);
    let nk2 = nk + 2;
    for isea in 1..=nsea {
        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
        let mut v: Real = 0.;
        for ik in 1..=nk {
            for ith in 0..nth {
                let mut cxtot = m.ecos[ith] * cg[ik + nk2 * isea] / m.clats[isea - 1];
                let mut cytot = m.esin[ith] * cg[ik + nk2 * isea];
                if flcur {
                    cxtot = cxtot + cx[isea] / m.clats[isea - 1];
                    cytot = cytot + cy[isea];
                }
                let cp = cxtot * g.dpdx[c] + cytot * g.dpdy[c];
                let cq = cxtot * g.dqdx[c] + cytot * g.dqdy[c];
                v = v.max(cp * dtg).max(cq * dtg);
            }
        }
        out[isea] = v;
    }
}

/// `ISP` of the clock: `DSEC21` goes into an `INTEGER` in `W3WAVE` and `W3XYP3`.
fn odd_step(time: Time, dtg: Real) -> i32 {
    let dttst = dsec21([time[0], 0], time) as i32;
    nint(dttst as Real / dtg) % 2
}

/// Runs propagation somewhere other than the CPU. `xyp3` advances every component, `vq` holding
/// `Q(1-NY:NY*(NX+2))` of each in turn; `ktp3` advances every sea point of `pts`, with one block per point
/// in `cg`, `wn` and `va`.
pub trait Device {
    fn xyp3(&mut self, p: &Xyp3Params, t: &Xyp3Tables, dtg: Real, yfirst: bool, comps: usize, vq: &mut [Real]);
    fn ktp3(&mut self, p: &Ktp3Params, t: &Ktp3Tables, pts: &[Ktp3Point], cg: &[Real], wn: &[Real], va: &mut [Real]);
}

/// Runs the case. Writes `out_grd.ww3`, `out_pnt.ww3` and `restartNNN.ww3` into `dir`.
pub fn run(m: &ModDef, mut r: Run, dir: &str, mut device: Option<&mut dyn Device>) {
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
    let mut src = vec![SrceOut::default(); nsea + 1];
    // W3WAVE, FLCOLD.
    let cold = s.rstype != 2;
    let mut fliwnd = s.rstype == 1;
    let mut dtdyn = vec![0.; nsea + 1];
    let flcur = r.current.is_some();
    let z = || vec![0.; nsea + 1];
    let (mut ua, mut ud, mut asv, mut u10, mut u10d) = (z(), z(), z(), z(), z());
    asv[1..].fill(UNDEF);
    let (mut cx, mut cy) = (z(), z());
    let (mut dcxdx, mut dcxdy, mut dcydx, mut dcydy) = (vec![0.; nxy], vec![0.; nxy], vec![0.; nxy], vec![0.; nxy]);
    let mut pwind: Option<Polar> = None;
    let mut pcur: Option<Polar> = None;
    let mut fcut = vec![if cold { m.sig[nk] * TPIINV } else { 0. }; nsea + 1];
    let mut cflxy = vec![0.; nsea + 1];
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
    let mut all: Vec<Real> = Vec::new();

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

    let flags = iogo::ids(&r.fields, m);
    let mut grd = flout[0].then(|| Grd::new(&format!("{dir}/out_grd.ww3"), m, flags.clone()));
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

    let mut time0 = r.time0;
    let timen = r.timen;
    let mut itime = 0;
    let mut ifile = 1;
    let (flcx, flcy, flcth, flsou) = (m.flcx, m.flcy, m.flcth, m.flsou);
    let flck = m.flck && flcur; // W3INIT.
    let xp = Xyp3Params {
        nx: m.nx,
        ny: m.ny,
        nsea: m.nsea,
        nth: m.nth,
        flcx: flcx as u32,
        flcy: flcy as u32,
        global: (m.iclose != -1) as u32,
        flagll: m.flagll as u32,
        flcur: flcur as u32,
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
        flcur: flcur as u32,
        dmin: m.dmin,
        ctmax: m.ctmax,
        fachfa: m.fachfa,
    };

    loop {
        // ww3_shel, label 700.
        let mut ttime = timen;
        for f in [r.current.as_mut(), r.wind.as_mut()].into_iter().flatten() {
            let dttst = if f.tfn[0] == -1 { 0. } else { dsec21(time0, f.tfn) };
            if dttst <= 0. {
                f.next(time0, timen);
            }
            if dsec21(f.tfn, ttime) > 0. {
                ttime = f.tfn;
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
        let mut flddir = itime == 0 && (flcth || flck);
        let mut ugdtupdate = false;
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

                if let Some(f) = r.current.as_ref() {
                    // W3UCUR.
                    if flfrst {
                        pcur = Some(polar(m, f));
                    }
                    let c = pcur.as_ref().unwrap();
                    let rd = dsec21(f.tf0, time) / dsec21(f.tf0, f.tfn).max(1.0e-7);
                    for i in 1..=nsea {
                        let cabs = c.a0[i] + rd * c.ai[i];
                        let (sn, cs) = sincosf(c.d0[i] + rd * c.di[i]);
                        cx[i] = cabs * cs;
                        cy[i] = cabs * sn;
                    }
                    w3dzxy(m, &metrics, &mapsta, &cx, &mut dcxdx, &mut dcxdy);
                    w3dzxy(m, &metrics, &mapsta, &cy, &mut dcydx, &mut dcydy);
                } else if flfrst {
                    ugdtupdate = true;
                    cflxy.fill(0.);
                }
                if let Some(f) = r.wind.as_ref() {
                    // W3UWND.
                    if flfrst {
                        s.asf[1..].fill(1.);
                        pwind = Some(polar(m, f));
                    }
                    let u = pwind.as_ref().unwrap();
                    let rdw = dsec21(f.tf0, time) / dsec21(f.tf0, f.tfn).max(1.0e-7);
                    for i in 1..=nsea {
                        ua[i] = u.a0[i] + rdw * u.ai[i];
                        ud[i] = u.d0[i] + rdw * u.di[i];
                        asv[i] = u.s0[i] + rdw * u.si[i];
                        u10[i] = ua[i].max(0.001);
                        u10d[i] = ud[i];
                    }
                } else if flfrst {
                    u10[1..].fill(0.01);
                    u10d[1..].fill(0.);
                    s.ust[1..].fill(0.05);
                    s.ustdir[1..].fill(0.05);
                }

                if fliwnd {
                    w3uini(m, &metrics, &u10, &u10d, &cg, &mut s.va);
                    fliwnd = false;
                }
                if flmap {
                    mapstx_fill(m, &mapsta, &mut mapstx);
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
                    if flags.on(9, 3) && ugdtupdate {
                        w3cflxy(m, &metrics, &cg, &cx, &cy, flcur, dtg, &mut cflxy);
                    }
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
                    let mut spectral = |va: &mut [Real], n: i32, device: &mut Option<&mut dyn Device>| {
                        for _ in 0..n {
                            if let Some(d) = device.as_deref_mut() {
                                let (mut pts, mut ids) = (Vec::new(), Vec::new());
                                let (mut cgb, mut wnb, mut vab) = (Vec::new(), Vec::new(), Vec::new());
                                for isea in 1..=nsea {
                                    let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                    if mapsta[c] != 1 {
                                        continue;
                                    }
                                    pts.push(Ktp3Point {
                                        itime,
                                        facth,
                                        fack,
                                        cthg0: m.cthg0s[isea - 1],
                                        dw: m.dmin.max(dw[isea]),
                                        dddx: dddx[c],
                                        dddy: dddy[c],
                                        cx: cx[isea],
                                        cy: cy[isea],
                                        dcxdx: dcxdx[c],
                                        dcxdy: dcxdy[c],
                                        dcydx: dcydx[c],
                                        dcydy: dcydy[c],
                                    });
                                    ids.push(isea);
                                    cgb.extend_from_slice(&cg[nk2 * isea..nk2 * (isea + 1)]);
                                    wnb.extend_from_slice(&wn[nk2 * isea..nk2 * (isea + 1)]);
                                    vab.extend_from_slice(&va[nspec * isea..nspec * (isea + 1)]);
                                }
                                d.ktp3(&kp, &kt, &pts, &cgb, &wnb, &mut vab);
                                for (b, isea) in ids.iter().enumerate() {
                                    va[nspec * isea..nspec * (isea + 1)]
                                        .copy_from_slice(&vab[nspec * b..nspec * (b + 1)]);
                                }
                                continue;
                            }
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
                                    cx: cx[isea],
                                    cy: cy[isea],
                                    dcxdx: dcxdx[c],
                                    dcxdy: dcxdy[c],
                                    dcydx: dcydx[c],
                                    dcydy: dcydy[c],
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
                                    0,
                                    &cg[nk2 * isea..nk2 * (isea + 1)],
                                    &wn[nk2 * isea..nk2 * (isea + 1)],
                                    &mut va[nspec * isea..nspec * (isea + 1)],
                                    &mut w,
                                );
                            }
                        }
                    };
                    if flcth || flck {
                        spectral(&mut s.va, itloch, &mut device);
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
                            mapstx: &mapstx,
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
                            cx: &cx,
                            cy: &cy,
                            atrnx: &atrnx,
                            atrny: &atrny,
                        };
                        let base = ny - 1;
                        if let Some(d) = device.as_deref_mut() {
                            let lq = ny * (nx + 3);
                            all.resize(lq * nspec, 0.);
                            all.fill(0.);
                            for isea in 1..=nsea {
                                let ixy = m.mapsf[isea - 1 + 2 * nsea] as usize;
                                for ispec in 0..nspec {
                                    all[lq * ispec + ixy + base] = s.va[ispec + nspec * isea];
                                }
                            }
                            d.xyp3(&px, &tb, dtg, yfirst, nspec, &mut all);
                            for isea in 1..=nsea {
                                let ixy = m.mapsf[isea - 1 + 2 * nsea] as usize;
                                if mapsta[ixy - 1] != 0 {
                                    for ispec in 0..nspec {
                                        s.va[ispec + nspec * isea] = all[lq * ispec + ixy + base];
                                    }
                                }
                            }
                        }
                        for ispec in 0..if device.is_some() { 0 } else { nspec } {
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
                                vlcflx: &mut vlcflx,
                                vlcfly: &mut vlcfly,
                                cxtot: &mut cxtot,
                                cytot: &mut cytot,
                                aq: &mut aq,
                                fla: &mut fla,
                            };
                            w3xyp3(&px, &tb, ispec as i32 + 1, dtg, yfirst, 0, &mut field, &mut w);
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
                        spectral(&mut s.va, ntloc - itloch, &mut device);
                    }
                    ugdtupdate = false;
                }

                if flsou {
                    for isea in 1..=nsea {
                        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                        if mapsta[c] != 1 {
                            s.ust[isea] = UNDEF;
                            s.ustdir[isea] = UNDEF;
                            dtdyn[isea] = UNDEF;
                            fcut[isea] = UNDEF;
                            continue;
                        }
                        let q = SrcePoint {
                            srce_call: SRCE_DIRECT,
                            it,
                            d_inp: dw[isea],
                            u10abs: u10[isea],
                            u10dir: u10d[isea],
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
                        dtdyn[isea] = out[0].dtdyn;
                        fcut[isea] = out[0].fcut;
                        src[isea] = out[0];
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
                        if JS[j] == 1 {
                            if let Some(f) = grd.as_mut() {
                                let mut g = Input {
                                    time,
                                    va: &s.va,
                                    mapsta: &mapsta,
                                    mapst2: &mapst2,
                                    cg: &cg,
                                    wn: &wn,
                                    dw: &dw,
                                    wlv: &s.wlv,
                                    ua: &ua,
                                    ud: &ud,
                                    asv: &asv,
                                    ust: &mut s.ust,
                                    ustdir: &mut s.ustdir,
                                    src: &src,
                                    dtdyn: &dtdyn,
                                    fcut: &fcut,
                                    cflxy: &cflxy,
                                    u10: &u10,
                                    u10d: &u10d,
                                    wavnu: &|si, h| disp.wavnu1(si, h),
                                    ice: &s.ice,
                                    asf: &s.asf,
                                    whitecap: &whitecap,
                                    cx: &cx,
                                    cy: &cy,
                                };
                                f.write(m, &mut g);
                            }
                        } else if JS[j] == 2 {
                            if let Some(f) = pnt.as_mut() {
                                let w = Winds { ua: &ua, ud: &ud, asv: &asv, cx: &cx, cy: &cy };
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

/// `UA`, `UD`, `AS`, `CX`, `CY`, from `ISEA = 0`.
struct Winds<'a> {
    ua: &'a [Real],
    ud: &'a [Real],
    asv: &'a [Real],
    cx: &'a [Real],
    cy: &'a [Real],
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
        let (sc, ua) = (is.map(|i| sincosf(w.ud[i])), is.map(|i| w.ua[i]));
        let wx = rd[0] * ua[0] * sc[0].1 + rd[1] * ua[1] * sc[1].1 + rd[2] * ua[2] * sc[2].1 + rd[3] * ua[3] * sc[3].1;
        let wy = rd[0] * ua[0] * sc[0].0 + rd[1] * ua[1] * sc[1].0 + rd[2] * ua[2] * sc[2].0 + rd[3] * ua[3] * sc[3].0;
        let wao = sqrtf(wx * wx + wy * wy);
        let wdo = if wao > 1.0e-7 { atan2f(wy, wx) } else { 0. };
        let aso = rd[0] * w.asv[is[0]] + rd[1] * w.asv[is[1]] + rd[2] * w.asv[is[2]] + rd[3] * w.asv[is[3]];
        let curx = rd[0] * w.cx[is[0]] + rd[1] * w.cx[is[1]] + rd[2] * w.cx[is[2]] + rd[3] * w.cx[is[3]];
        let cury = rd[0] * w.cy[is[0]] + rd[1] * w.cy[is[1]] + rd[2] * w.cy[is[2]] + rd[3] * w.cy[is[3]];
        let cao = sqrtf(curx * curx + cury * cury);
        let cdo = if cao > 1.0e-7 { atan2f(cury, curx) } else { 0. };

        let mut b = vec![0u8; 12]; // W3IOPO zeroes IW, II, IL before it writes them.
        for v in [dpo, wao, wdo, aso, cao, cdo, sum(0.), sum(0.5), sum(1000.)] {
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
