// SPDX-License-Identifier: MPL-2.0
//! The time loop of `ww3_shel` and `W3WAVE` (`model/src/ww3_shel.F90`, `w3wavemd.F90`), with `W3UWND`,
//! `W3FLDH`, `WAVNU1`, `DISTAB` and the point output `W3IOPE`, `W3IOPO`, as far as the first whole case
//! needs them: rectilinear or curvilinear grids, `PR3 UQ`, winds, currents, water levels and ice
//! concentration (`IC0`), and boundary input from `nest.ww3`.
//! Anything else stops with a message: this is not a general `ww3_shel`.
//!
//! The kernels run on the CPU here, or propagation runs through a `Device`.

use crate::grid::{Maps, Metrics, unpack, w3dzxy, w3map3, w3mapt, w3utrn};
use crate::disp::Dispersion;
use crate::iobc::NestIn;
use crate::iogo::{self, Grd, Input};
use crate::partition::{CprtInput, Neigh, Part, w3cprt};
use crate::iors::{self, State};
use crate::moddef::{ModDef, Records};
use crate::wtime::{Time, dsec21, tick21};
use par::{Layout, Par};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use ww3_kernels::constants::{DAIR, DERA, GRAV, PI, RADIUS, TPI, TPIINV};
use ww3_kernels::nint;
use ww3_kernels::pro3::{
    Ktp3Params, Ktp3Point, Ktp3Tables, Ktp3Work, NoBoundary, Xyp3Params, Xyp3Tables, Xyp3Work, w3ktp3, w3xyp3, w3xyp3_va,
};
use ww3_kernels::real::Real;
use ww3_kernels::sin4::{Sin4Out, Spr4Out};
use ww3_kernels::srce::{SRCE_DIRECT, SrceOut, SrceParams, SrcePoint, SrceTables, SrceWork, w3srce, work_lens, work_small};
use ww3_libm::{atan2f, atanf, sincosf, sqrtf};

pub mod par;

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
    /// `DATE%BOUNDARY`: the nest files of `W3IOBC ( 'WRITE' )`.
    pub boundary: (Time, i32, Time),
    pub wind: Option<Forcing>,
    pub current: Option<Forcing>,
    /// `INPUT%FORCING%WATER_LEVELS`.
    pub level: Option<Scalar>,
    /// `INPUT%FORCING%ICE_CONC`.
    pub ice: Option<Scalar>,
    /// `XPT`, `YPT`, `PNAMES`.
    pub points: Vec<(Real, Real, String)>,
    /// `FLOGRD(5,7)` and `FLOGRD(5,8)`.
    pub flogrd: (bool, bool),
    /// `TYPE%FIELD%LIST`.
    pub fields: String,
    /// `--threads`: how many CPU threads share the per-point and per-component loops. 1 takes the serial path.
    pub threads: usize,
}

/// Hands `items` to `scratch.len()` threads round-robin, each with its own scratch, and joins them.
/// Every item is a sea point or a component whose results land in its own slices, so the result does not
/// depend on the thread count.
fn spread<S: Send, T: Send>(scratch: &mut [S], items: impl Iterator<Item = T>, f: impl Fn(&mut S, T) + Sync) {
    let n = scratch.len();
    let mut bins: Vec<Vec<T>> = (0..n).map(|_| Vec::new()).collect();
    for (i, it) in items.enumerate() {
        bins[i % n].push(it);
    }
    let f = &f;
    std::thread::scope(|sc| {
        for (w, bin) in scratch.iter_mut().zip(bins) {
            sc.spawn(move || {
                for it in bin {
                    f(w, it);
                }
            });
        }
    });
}

/// One thread's `W3KTP3` work arrays.
struct KtpScratch {
    dsdd: Vec<Real>,
    frk: Vec<Real>,
    frg: Vec<Real>,
    fkc: Vec<Real>,
    vq: Vec<Real>,
    db: Vec<Real>,
    dm: Vec<Real>,
    vcflt: Vec<Real>,
    cflk: Vec<Real>,
    fla: Vec<Real>,
}

impl KtpScratch {
    fn new(nk: usize, nth: usize) -> Self {
        let nk2 = nk + 2;
        Self {
            dsdd: vec![0.; nk2],
            frk: vec![0.; nk],
            frg: vec![0.; nk],
            fkc: vec![0.; nth],
            vq: vec![0.; nk2 * (nth + 3)],
            db: vec![0.; nk2 * (nth + 1)],
            dm: vec![0.; nk2 * (nth + 2)],
            vcflt: vec![0.; nk2 * (nth + 1)],
            cflk: vec![0.; nk2 * nth],
            fla: vec![0.; nk2 * (nth + 1)],
        }
    }
}

/// One thread's `W3XYP3` work arrays and its gathered field.
struct XypScratch {
    iixy: [Vec<i32>; 4],
    rdi: [Vec<Real>; 4],
    vlcflx: Vec<Real>,
    vlcfly: Vec<Real>,
    cxtot: Vec<Real>,
    cytot: Vec<Real>,
    field: Vec<Real>,
    aq: Vec<Real>,
    fla: Vec<Real>,
}

impl XypScratch {
    fn new(nx: usize, ny: usize, nsea: usize) -> Self {
        Self {
            iixy: std::array::from_fn(|_| vec![0; nsea]),
            rdi: std::array::from_fn(|_| vec![0.; nsea]),
            vlcflx: vec![0.; (nx + 1) * ny],
            vlcfly: vec![0.; (nx + 1) * ny],
            cxtot: vec![0.; (nx + 1) * ny],
            cytot: vec![0.; nx * ny],
            field: vec![0.; ny * (nx + 3)],
            aq: vec![0.; ny * (nx + 3)],
            fla: vec![0.; ny * (nx + 1)],
        }
    }
}

/// One thread's `W3SRCE` scratch.
struct SrceScratch {
    bufs: Vec<Vec<Real>>,
    llws: Vec<u32>,
    w_snl: Vec<Real>,
    w_sds: Vec<Real>,
    iw_sds: Vec<i32>,
    w_small: Vec<Real>,
    sin_out: [Sin4Out; 1],
    spr_out: [Spr4Out; 1],
    out: [SrceOut; 1],
}

impl SrceScratch {
    fn new(p: &SrceParams, nspec: usize) -> Self {
        let (l_snl, l_sds, l_ids) = work_lens(p);
        Self {
            bufs: (0..17).map(|_| vec![0.; nspec]).collect(),
            llws: vec![0u32; nspec],
            w_snl: vec![0.; l_snl],
            w_sds: vec![0.; l_sds],
            iw_sds: vec![0i32; l_ids],
            w_small: vec![0.; work_small(p)],
            sin_out: [Sin4Out::default()],
            spr_out: [Spr4Out::default()],
            out: [SrceOut::default()],
        }
    }
}

/// One sea point of `W3SRCE` with the slices it writes.
struct SrceTask<'a> {
    isea: usize,
    spec: &'a mut [Real],
    whitecap: &'a mut [Real],
    ust: &'a mut Real,
    ustdir: &'a mut Real,
    dtdyn: &'a mut Real,
    fcut: &'a mut Real,
    src: &'a mut SrceOut,
}

/// Where a forcing field comes from.
enum Source {
    /// `(DATE, VALUE1, VALUE2, VALUE3)`, for `W3FLDH`.
    Homogeneous(VecDeque<(Time, Real, Real, Real)>),
    /// `current.ww3` or `wind.ww3`, for `W3FLDG`; true for the third record of `WNS`.
    File(Records, bool),
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
        let id = field_id(&mut r);
        Self::new(Source::File(r, id == "WNS"), n)
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
                    (t, vec![ha * c], vec![ha * s], vec![hs])
                }),
                Source::File(r, wns) => {
                    let t = r.try_next().map(|mut c| [c.i(), c.i()]);
                    let n = self.xn.len();
                    let sn = &self.sn;
                    t.map(|t| {
                        let (x, y) = (r.next().fs(n), r.next().fs(n));
                        // Without the third record W3FLDG leaves FAN as it was.
                        (t, x, y, if *wns { r.next().fs(n) } else { sn.clone() })
                    })
                }
            };
            let Some((t, x, y, hs)) = got else {
                self.tfn = tn;
                tick21(&mut self.tfn, 1.);
                if matches!(self.src, Source::File(..)) {
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
            if hs.len() == 1 {
                self.sn.fill(hs[0]);
            } else {
                self.sn.copy_from_slice(&hs);
            }
            if dsec21(t0, self.tfn) > 0. {
                break;
            }
        }
        if matches!(self.src, Source::File(..)) && self.tf0[0] == -1 {
            self.tf0 = t0;
            self.x0.copy_from_slice(&self.xn);
            self.y0.copy_from_slice(&self.yn);
            self.s0.copy_from_slice(&self.sn);
        }
    }
}

/// The header `W3FLDO ('READ', ...)` checks, for `TSFLD`: `WNS` and `ISI` are read in place of `WND` and `ICE`.
fn field_id(r: &mut Records) -> String {
    let mut c = r.next();
    c.skip(13);
    c.text(3)
}

/// Where a field without interpolation comes from.
enum ScalarSource {
    /// `(DATE, VALUE1)`, for `W3FLDH`.
    Homogeneous(VecDeque<(Time, Real)>),
    /// `level.ww3` or `ice.ww3`, for `W3FLDG`; true for the concentration and iceberg records of `ISI`.
    File(Records, bool),
}

/// `TLN, WLEV` or `TIN, ICEI, BERGI`, with the grids `(NX,NY)`. These fields are used as they arrive, with
/// no `T0` to interpolate from.
pub struct Scalar {
    src: ScalarSource,
    tfn: Time,
    fan: Vec<Real>,
    fbn: Vec<Real>,
    /// `FLLSTL` or `FLLSTI`: the input has ended, and `TFN` no longer sets the end of the next `W3WAVE` call.
    last: bool,
    /// `.NOT. INFLAGS1`: `ww3_shel.F90:2582` turns the field off only after that call, so `W3ULEV` can still run in it
    /// with `TFN` at `TN` plus a second.
    off: bool,
    /// The end of homogeneous input sets `FLLSTL`; `FLLSTI` is set only in `W3FLDG`'s branch.
    homog_ends: bool,
}

impl Scalar {
    pub fn homogeneous(q: VecDeque<(Time, Real)>, n: usize, ends: bool) -> Self {
        let mut f = Self::new(ScalarSource::Homogeneous(q), n);
        f.homog_ends = ends;
        f
    }

    /// `W3FLDO ('READ', ...)`.
    pub fn file(path: &str, n: usize) -> Self {
        let mut r = Records::open(path);
        let id = field_id(&mut r);
        Self::new(ScalarSource::File(r, id == "ISI"), n)
    }

    fn new(src: ScalarSource, n: usize) -> Self {
        Self { src, tfn: [-1, 0], fan: vec![0.; n], fbn: vec![0.; n], last: false, off: false, homog_ends: false }
    }

    /// `W3FLDH` for `J` of 1 or 4, or `W3FLDG` with `FLINTERP` false. At the first call, a field at `T0`
    /// itself ends the search, unlike the interpolated fields.
    fn next(&mut self, t0: Time, tn: Time) {
        let flfrst = self.tfn[0] == -1;
        loop {
            match &mut self.src {
                ScalarSource::Homogeneous(q) => match q.pop_front() {
                    Some((t, ha)) => {
                        self.tfn = t;
                        self.fan.fill(ha);
                    }
                    None => {
                        self.tfn = tn;
                        tick21(&mut self.tfn, 1.);
                        self.last = self.homog_ends;
                    }
                },
                ScalarSource::File(r, isi) => {
                    let Some(t) = r.try_next().map(|mut c| [c.i(), c.i()]) else {
                        // W3FLDG's label 800: without FLINTERP, TFN and the field stay as they were.
                        self.last = true;
                        return;
                    };
                    let n = self.fan.len();
                    self.tfn = t;
                    self.fan = r.next().fs(n);
                    if *isi {
                        self.fbn = r.next().fs(n);
                    }
                }
            }
            let d = dsec21(t0, self.tfn);
            if d > 0. || (flfrst && d == 0.) {
                break;
            }
        }
    }
}

/// A forcing field is `FAN(IX,IY)`, unlike `MAPSTA(IY,IX)`, which `MAPSF(:,3)` indexes.
fn field_cell(m: &ModDef, isea: usize) -> usize {
    let nsea = m.nsea as usize;
    (m.mapsf[isea - 1] - 1 + (m.mapsf[isea - 1 + nsea] - 1) * m.nx) as usize
}

/// `W3UICE` for `IC0`. `VA` is by `JSEA`, which is `ISEA` here.
#[allow(clippy::too_many_arguments)]
fn w3uice(
    m: &ModDef,
    f: &Scalar,
    s: &mut State,
    berg: &mut [Real],
    mapsta: &mut [i32],
    mapst2: &mut [i32],
    src: &mut [SrceOut],
    nspec: usize,
) {
    let nsea = m.nsea as usize;
    s.tice = f.tfn;
    let mut mapice: Vec<i32> = mapst2.iter().map(|v| v % 2).collect();
    for (st2, mi) in mapst2.iter_mut().zip(&mapice) {
        *st2 -= mi;
    }
    for isea in 1..=nsea {
        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
        let fc = field_cell(m, isea);
        let icei = f.fan[fc];
        s.ice[isea] = icei;
        berg[isea] = f.fbn[fc];
        if icei >= m.ficen && mapice[c] == 0 {
            mapsta[c] = -mapsta[c].abs();
            mapice[c] = 1;
            s.va[nspec * isea..nspec * (isea + 1)].fill(0.);
            src[isea].charn = m.aalpha;
        }
        if icei < m.ficen && mapice[c] == 1 {
            mapice[c] = 0;
            s.ust[isea] = 0.05;
            if mapst2[c] == 0 {
                mapsta[c] = mapsta[c].abs();
                s.va[nspec * isea..nspec * (isea + 1)].fill(0.);
                src[isea].charn = m.aalpha;
            }
        }
    }
    for (st2, mi) in mapst2.iter_mut().zip(&mapice) {
        *st2 += mi;
    }
}

/// `W3ULEV ( VA, VA )`: the two arguments are one array, `A(ITH,IK,JSEA)` is `VA(ITH+(IK-1)*NTH,JSEA)`.
/// `RD2` is a stack variable the Fortran reads unset when no old wavenumber reaches the new first one:
/// here it keeps the value of the point before, which is not proved to be what that slot holds ⚠.
#[allow(clippy::too_many_arguments)]
fn w3ulev(
    m: &ModDef,
    disp: &Dispersion,
    f: &Scalar,
    s: &mut State,
    dw: &mut [Real],
    wn: &mut [Real],
    cg: &mut [Real],
    mapsta: &mut [i32],
    mapst2: &mut [i32],
) {
    const KDMAX: Real = 4.;
    const RDKMIN: Real = 0.05;
    let (nth, nk, nsea) = (m.nth as usize, m.nk as usize, m.nsea as usize);
    let (nspec, nk2) = (nth * nk, nk + 2);
    assert!(nk >= 2, "W3ULEV requires NK > 1");
    s.tlev = f.tfn;
    let mut mapdry: Vec<i32> = mapst2.iter().map(|v| v / 2 % 2).collect();
    for (st2, d) in mapst2.iter_mut().zip(&mapdry) {
        *st2 -= 2 * d;
    }
    let dwo = dw.to_vec();
    for isea in 1..=nsea {
        s.wlv[isea] = f.fan[field_cell(m, isea)];
        dw[isea] = (0f32).max(s.wlv[isea] - m.zb[isea - 1]);
    }
    let (mut wno, mut cgo, mut own, mut dwn) = (vec![0.; nk2], vec![0.; nk2], vec![0.; nk + 1], vec![0.; nk + 1]);
    let mut ta = vec![0.; nspec];
    let mut rd2: Real = 0.;
    for isea in 1..=nsea {
        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
        let w = nk2 * isea;
        let kdchck = wn[1 + w] * dwo[isea].min(dw[isea]);
        if kdchck >= KDMAX {
            continue;
        }
        let depth = m.dmin.max(dw[isea]);
        for ik in 0..nk2 {
            wno[ik] = wn[ik + w];
            cgo[ik] = cg[ik + w];
            (wn[ik + w], cg[ik + w]) = disp.wavnu1(m.sig[ik], depth);
        }
        for ik in 1..=nk {
            own[ik] = m.dsip[ik] / cgo[ik];
            dwn[ik] = m.dsip[ik] / cg[ik + w];
        }
        let sp = nspec * isea;
        if s.wlv[isea] - m.zb[isea - 1] <= 0. {
            if mapdry[c] == 0 {
                s.va[sp..sp + nspec].fill(0.);
                mapdry[c] = 1;
                mapsta[c] = -mapsta[c].abs();
            }
            continue;
        }
        if mapdry[c] == 1 {
            mapdry[c] = 0;
            s.va[sp..sp + nspec].fill(0.);
            s.ust[isea] = 0.05;
            if mapst2[c] == 0 {
                mapsta[c] = mapsta[c].abs();
            }
            continue;
        }
        let rdk = (wno[1] - wn[1 + w]).abs() / dwn[1];
        if rdk < RDKMIN || mapsta[c] < 0 {
            continue;
        }
        for ik in 1..=nk {
            for ith in 1..=nth {
                let is = (ith - 1) + (ik - 1) * nth;
                ta[is] = s.va[is + sp] * own[ik];
            }
        }
        s.va[sp..sp + nspec].fill(0.);
        let (mut ik0, mut i1, mut i2) = (1, 1, 2);
        let mut reached = true;
        if wno[1] < wn[1 + w] {
            (ik0, i1, i2) = (1, 0, 1);
            loop {
                ik0 += 1;
                if ik0 > nk + 1 {
                    reached = false;
                    break;
                }
                if wno[ik0] >= wn[1 + w] {
                    ik0 -= 1;
                    break;
                }
            }
        }
        if reached {
            'ik: for ik in ik0..=nk {
                while wno[ik] > wn[i2 + w] {
                    i1 += 1;
                    if i1 > nk {
                        continue 'ik;
                    }
                    i2 = i1 + 1;
                }
                let rd1 = if i1 == 0 {
                    (wn[1 + w] - wno[ik]) / dwn[1]
                } else {
                    (wn[i2 + w] - wno[ik]) / (wn[i2 + w] - wn[i1 + w])
                };
                rd2 = 1. - rd1;
                if i1 >= 1 {
                    for ith in 1..=nth {
                        let (to, from) = ((ith - 1) + (i1 - 1) * nth, (ith - 1) + (ik - 1) * nth);
                        s.va[to + sp] = s.va[to + sp] + rd1 * ta[from];
                    }
                }
                if i2 <= nk {
                    for ith in 1..=nth {
                        let (to, from) = ((ith - 1) + (i2 - 1) * nth, (ith - 1) + (ik - 1) * nth);
                        s.va[to + sp] = s.va[to + sp] + rd2 * ta[from];
                    }
                }
            }
        }
        for is in 0..nspec {
            s.va[is + sp] = s.va[is + sp] / dwn[m.mapwn[is] as usize];
        }
        if i2 <= nk && rd2 <= 0.95 {
            for ik in i2.max(2)..=nk {
                for ith in 1..=nth {
                    let (to, from) = ((ith - 1) + (ik - 1) * nth, (ith - 1) + (ik - 2) * nth);
                    s.va[to + sp] = m.fachfa * s.va[from + sp];
                }
            }
        }
    }
    for (st2, d) in mapst2.iter_mut().zip(&mapdry) {
        *st2 += 2 * d;
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

/// Which sea points `W3KTP3` and `W3SRCE` act on.
fn active_of(m: &ModDef, mapsta: &[i32]) -> Vec<u32> {
    let nsea = m.nsea as usize;
    (0..nsea).map(|i| (mapsta[(m.mapsf[i + 2 * nsea] - 1) as usize] == 1) as u32).collect()
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

/// Runs the spectrum's kernels somewhere other than the CPU, with `VA` resident there. `va` is
/// `(NSPEC,1:NSEA)`, and `cg`, `wn`, `cg1`, `wn1` and every point list have one block per sea point; an
/// entry of `active` is 0 where `MAPSTA` is not 1.
pub trait Device {
    fn va_write(&mut self, va: &[Real]);
    fn va_read(&mut self, va: &mut [Real]);
    fn whitecap_read(&mut self, w: &mut [Real]);
    fn xyp3(&mut self, p: &Xyp3Params, t: &Xyp3Tables, dtg: Real, yfirst: bool, nspec: usize);
    #[allow(clippy::too_many_arguments)]
    fn ktp3(&mut self, p: &Ktp3Params, t: &Ktp3Tables, pts: &[Ktp3Point], active: &[u32], cg: &[Real], wn: &[Real]);
    #[allow(clippy::too_many_arguments)]
    fn srce(
        &mut self,
        p: &SrceParams,
        t: &SrceTables,
        pts: &[SrcePoint],
        active: &[u32],
        wn1: &[Real],
        cg1: &[Real],
        out: &mut [SrceOut],
    );
}

/// Runs the case on one rank. Writes `out_grd.ww3`, `out_pnt.ww3` and `restartNNN.ww3` into `dir`.
pub fn run(m: &ModDef, r: Run, dir: &str, device: Option<&mut dyn Device>) {
    run_impl(m, r, dir, device, &Par::serial());
}

/// Runs the case on the ranks of `comm`, the CPU kernels only. Rank 0 writes the files.
pub fn run_ranks(m: &ModDef, r: Run, dir: &str, comm: &Par) {
    run_impl(m, r, dir, None, comm);
}

/// One sea point's values of everything an output reads that `W3SRCE` writes: `VA`, `UST`, `USTDIR`,
/// `DTDYN`, `FCUT`, `SRCE` and `WHITECAP`.
const SRCE_N: usize = std::mem::size_of::<SrceOut>() / std::mem::size_of::<Real>();
const _: () = assert!(SRCE_N * std::mem::size_of::<Real>() == std::mem::size_of::<SrceOut>());

/// `W3IOGO`'s gather: rank 0 receives the sea points the other ranks own. Every other rank keeps stale
/// values at points it does not own, which nothing reads before the point is written again.
#[allow(clippy::too_many_arguments)]
fn gather_out(
    comm: &Par,
    lay: &Layout,
    nspec: usize,
    s: &mut State,
    dtdyn: &mut [Real],
    fcut: &mut [Real],
    src: &mut [SrceOut],
    whitecap: &mut [Real],
) {
    let width = nspec + 4 + SRCE_N + 4;
    let me = comm.rank();
    let mut send = Vec::with_capacity(lay.pts[me].len() * width);
    for &i in &lay.pts[me] {
        send.extend_from_slice(&s.va[nspec * i..nspec * (i + 1)]);
        send.extend_from_slice(&[s.ust[i], s.ustdir[i], dtdyn[i], fcut[i]]);
        // SAFETY: SrceOut is repr(C) and made only of Real, as the assert above checks.
        send.extend_from_slice(unsafe { std::slice::from_raw_parts(&src[i] as *const SrceOut as *const Real, SRCE_N) });
        send.extend_from_slice(&whitecap[4 * i..4 * (i + 1)]);
    }
    let scnt: Vec<usize> = (0..comm.size()).map(|q| if q == 0 { send.len() } else { 0 }).collect();
    let rcnt: Vec<usize> = lay.pts.iter().map(|p| if me == 0 { p.len() * width } else { 0 }).collect();
    let mut recv = vec![0.; rcnt.iter().sum()];
    comm.alltoallv(&send, &scnt, &mut recv, &rcnt);
    if me != 0 {
        return;
    }
    let mut blocks = recv.chunks_exact(width);
    for p in &lay.pts {
        for &i in p {
            let b = blocks.next().unwrap();
            s.va[nspec * i..nspec * (i + 1)].copy_from_slice(&b[..nspec]);
            let b = &b[nspec..];
            (s.ust[i], s.ustdir[i], dtdyn[i], fcut[i]) = (b[0], b[1], b[2], b[3]);
            // SAFETY: as above; the destination is a whole SrceOut.
            unsafe { std::slice::from_raw_parts_mut(&mut src[i] as *mut SrceOut as *mut Real, SRCE_N) }
                .copy_from_slice(&b[4..4 + SRCE_N]);
            whitecap[4 * i..4 * (i + 1)].copy_from_slice(&b[4 + SRCE_N..]);
        }
    }
}

fn run_impl(m: &ModDef, mut r: Run, dir: &str, mut device: Option<&mut dyn Device>, comm: &Par) {
    assert!(!m.fldry, "FLDRY is not implemented");
    let mut nbc = NestIn::new(dir, m.flbpi);
    // W3IOBC clears FLBPI when there is no nest file, so such a run is a device's too.
    assert!(device.is_none() || !m.flbpi || !std::path::Path::new(&format!("{dir}/nest.ww3")).exists(), "input boundaries are not implemented on a device");
    let (nth, nk, nsea) = (m.nth as usize, m.nk as usize, m.nsea as usize);
    let (nx, ny) = (m.nx as usize, m.ny as usize);
    let (nspec, nk2) = (nth * nk, nk + 2);
    let nxy = nx * ny;
    let t = m.snl1();
    let mut p = m.srce(t.params, r.flogrd);
    // INFLAGS2(4): ice concentration is in this run, so W3SRCE scales the source terms by it.
    p.inflags2_4 = r.ice.is_some() as u32;
    let tables = m.tables(&t);
    // W3ULEV changes WN, CG and VA, and W3UICE changes VA, between the kernels: a device would need them written back.
    assert!(device.is_none() || (r.level.is_none() && r.ice.is_none()), "water levels and ice are not implemented on a device");

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
    // W3DIMA starts these at UNDEF, and a point W3SRCE has not reached is written so: a field that arrives at the
    // start time gives an FLZERO pass, with output and no W3SRCE.
    let mut whitecap = vec![UNDEF; 4 * (nsea + 1)];
    let mut src = vec![
        SrceOut {
            tauox: UNDEF,
            tauoy: UNDEF,
            tauwix: UNDEF,
            tauwiy: UNDEF,
            tauwnx: UNDEF,
            tauwny: UNDEF,
            phiaw: UNDEF,
            charn: UNDEF,
            tws: UNDEF,
            phioc: UNDEF,
            tauocx: UNDEF,
            tauocy: UNDEF,
            wnmean: UNDEF,
            ..Default::default()
        };
        nsea + 1
    ];
    // W3WAVE, FLCOLD.
    let cold = s.rstype != 2;
    let mut fliwnd = s.rstype == 1;
    let mut dtdyn = vec![0.; nsea + 1];
    let flcur = r.current.is_some();
    let z = || vec![0.; nsea + 1];
    let (mut ua, mut ud, mut asv, mut u10, mut u10d) = (z(), z(), z(), z(), z());
    asv[1..].fill(UNDEF);
    let (mut cx, mut cy) = (z(), z());
    let mut berg = z();
    let (mut dcxdx, mut dcxdy, mut dcydx, mut dcydy) = (vec![0.; nxy], vec![0.; nxy], vec![0.; nxy], vec![0.; nxy]);
    let mut pwind: Option<Polar> = None;
    let mut pcur: Option<Polar> = None;
    let mut fcut = vec![if cold { m.sig[nk] * TPIINV } else { 0. }; nsea + 1];
    let mut cflxy = vec![UNDEF; nsea + 1];
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

    // `--threads`: one scratch set per thread; the serial path above stays the one `--threads 1` takes.
    let par = r.threads > 1 && device.is_none();
    // `DIST`: the ranks. One rank takes none of the paths below.
    let dist = comm.size() > 1;
    assert!(!dist || device.is_none(), "ranks and a device are not combined");
    let (rank, size) = (comm.rank(), comm.size());
    let root = rank == 0;
    let mine = |isea: usize| (isea - 1) % size == rank;
    let lay = dist.then(|| Layout::new(size, nsea, nspec, nk, nth, m.dtmax, m.dtcfl, &m.sig));
    let nown = lay.as_ref().map_or(0, |l| l.specs[rank].len());
    let nthr = if par { r.threads } else { dist as usize };
    let mut ktp_s: Vec<KtpScratch> = (0..if par { nthr } else { 0 }).map(|_| KtpScratch::new(nk, nth)).collect();
    let mut xyp_s: Vec<XypScratch> = (0..nthr).map(|_| XypScratch::new(nx, ny, nsea)).collect();
    let mut srce_s: Vec<SrceScratch> = (0..if par { nthr } else { 0 }).map(|_| SrceScratch::new(&p, nspec)).collect();
    // The components of `W3XYP3`, one column of `NSEA` values each, between its gather and its scatter.
    let mut xyp_cols: Vec<Real> = vec![0.; if dist { nown * nsea } else if par { nsea * nspec } else { 0 }];
    // W3GATH and W3SCAT's buffers: this rank's points by all components, and this rank's components by all points.
    let nmine = lay.as_ref().map_or(0, |l| l.pts[rank].len());
    let mut buf_pts: Vec<Real> = vec![0.; nmine * nspec];
    let mut buf_cols: Vec<Real> = vec![0.; nown * nsea];

    let mut mp = Maps::default();
    let (mut atrnx, mut atrny) = (vec![1.; 3 * nxy], vec![1.; 3 * nxy]);
    let mut maptrn = vec![0u32; nxy];
    let (mut dddx, mut dddy) = (vec![0.; nxy], vec![0.; nxy]);

    // W3INIT's output timing, for J = 1, 2, 4. J = 1 writes nothing here, but it splits the time loop.
    if let Some(d) = device.as_deref_mut() {
        d.va_write(&s.va[nspec..]);
    }
    let (mut wn1, mut cg1) = (Vec::with_capacity(nk * nsea), Vec::with_capacity(nk * nsea));
    for isea in 1..=nsea {
        wn1.extend_from_slice(&wn[nk2 * isea + 1..nk2 * isea + nk + 1]);
        cg1.extend_from_slice(&cg[nk2 * isea + 1..nk2 * isea + nk + 1]);
    }
    let mut time = r.time0;
    const JS: [usize; 4] = [1, 2, 4, 5];
    let mut tonext = [r.field.0, r.point.0, r.restart.0, r.boundary.0];
    // W3INIT: FLOUT(5) = FLBPO, and DTOUT(5) = 0 without it.
    let dtout = [r.field.1 as Real, r.point.1 as Real, r.restart.1 as Real, if m.flbpo { r.boundary.1 as Real } else { 0. }];
    let tolast = [r.field.2, r.point.2, r.restart.2, r.boundary.2];
    let mut flout = [false; 4];
    let mut tofrst: Time = [-1, 0];
    for j in 0..4 {
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
    // W3WAVE: FLPFLD = FLOGRD(4,J) for any J.
    let flags_part = (1..=25).any(|j| flags.on(4, j));
    let mut nest = crate::iobc::NestOut::new(dir);
    let mut grd = (flout[0] && root).then(|| Grd::new(&format!("{dir}/out_grd.ww3"), m, flags.clone()));
    let mut pnt = (flout[1] && !pts.is_empty() && root).then(|| {
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
        for f in [r.level.as_mut(), r.ice.as_mut()].into_iter().flatten().filter(|f| !f.off) {
            let dttst = if f.tfn[0] == -1 { 0. } else { dsec21(time0, f.tfn) };
            if dttst <= 0. {
                f.next(time0, timen);
            }
            if dsec21(f.tfn, ttime) > 0. && !f.last {
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
        // W3WAVE, 1.b and 1.e: what is left to apply of the level and ice fields.
        let mut dtl0: Real = 0.;
        if let Some(f) = r.level.as_ref().filter(|f| !f.off) {
            dtl0 = if s.tlev[0] >= 0 { dsec21(s.tlev, f.tfn) } else { 1. };
            assert!(dtl0 >= 0., "water level input time before the last one applied");
        }
        let mut dti0: Real = 0.;
        if let Some(f) = r.ice.as_ref().filter(|f| !f.off) {
            dti0 = if s.tice[0] >= 0 { dsec21(s.tice, f.tfn) } else { 1. };
            assert!(dti0 >= 0., "ice input time before the last one applied");
        }
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
                    if let Some(d) = device.as_deref_mut() {
                        d.va_write(&s.va[nspec..]);
                    }
                }
                // W3WAVE 3.2, before W3UICE and W3ULEV: W3UBPT reads CG as it was before this step's level.
                while nbc.flbpi {
                    let readbc = nbc.tbpin[0] == -1 || dsec21(time, nbc.tbpin) < 0.;
                    if !readbc {
                        break;
                    }
                    if nbc.read(m, &mapsta) != 1 {
                        nbc.w3ubpt(m, &cg);
                    }
                }
                if let Some(f) = r.ice.as_ref().filter(|_| dti0 != 0.) {
                    if s.tice[0] < 0 || dsec21(time, f.tfn) <= 0.5 * dti0 {
                        w3uice(m, f, &mut s, &mut berg, &mut mapsta, &mut mapst2, &mut src, nspec);
                        dti0 = 0.;
                        flmap = true;
                    }
                }
                if let Some(f) = r.level.as_ref().filter(|_| dtl0 != 0.) {
                    if s.tlev[0] < 0 || dsec21(time, f.tfn) <= 0.5 * dtl0 {
                        w3ulev(m, &disp, f, &mut s, &mut dw, &mut wn, &mut cg, &mut mapsta, &mut mapst2);
                        ugdtupdate = true;
                        cflxy.fill(0.);
                        dtl0 = 0.;
                        flmap = true;
                        flddir = flddir || flcth || flck;
                    }
                }
                if flmap {
                    mapstx_fill(m, &mapsta, &mut mapstx);
                    w3map3(m, &mapsta, &mut mp);
                    w3utrn(m, &metrics, &mapsta, &s.ice, &berg, &mut atrnx, &mut atrny);
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
                                let pts: Vec<_> = (1..=nsea)
                                    .map(|isea| {
                                        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                        Ktp3Point {
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
                                        }
                                    })
                                    .collect();
                                let act = active_of(m, &mapsta);
                                d.ktp3(&kp, &kt, &pts, &act, &cg[nk2..], &wn[nk2..]);
                                continue;
                            }
                            if par {
                                let tasks = va.chunks_mut(nspec).enumerate().skip(1).take(nsea).filter(|(i, _)| mine(*i));
                                spread(&mut ktp_s, tasks, |sc, (isea, spec)| {
                                    let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                    if mapsta[c] != 1 {
                                        return;
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
                                        dsdd: &mut sc.dsdd,
                                        frk: &mut sc.frk,
                                        frg: &mut sc.frg,
                                        fkc: &mut sc.fkc,
                                        vq: &mut sc.vq,
                                        db: &mut sc.db,
                                        dm: &mut sc.dm,
                                        vcflt: &mut sc.vcflt,
                                        cflk: &mut sc.cflk,
                                        fla: &mut sc.fla,
                                    };
                                    w3ktp3(
                                        &kp,
                                        &kt,
                                        &q,
                                        0,
                                        &cg[nk2 * isea..nk2 * (isea + 1)],
                                        &wn[nk2 * isea..nk2 * (isea + 1)],
                                        spec,
                                        &mut w,
                                    );
                                });
                                continue;
                            }
                            for isea in 1..=nsea {
                                let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                if !mine(isea) || mapsta[c] != 1 {
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
                        // FLBPI as it stands after this step's read: the end of the file clears it.
                        let bnd = nbc.flbpi.then(|| nbc.at(nspec, time));
                        if let Some(d) = device.as_deref_mut() {
                            d.xyp3(&px, &tb, dtg, yfirst, nspec);
                        } else if let Some(l) = lay.as_ref() {
                            // `DIST`: W3GATH moves each component's values from the ranks that own the sea
                            // points to the rank that owns the component, which runs W3XYP3 on the whole
                            // field as the serial run does; W3SCAT moves the result back.
                            let ny_i = m.ny as usize;
                            let specs_me = &l.specs[rank];
                            let scnt: Vec<usize> = l.specs.iter().map(|q| l.pts[rank].len() * q.len()).collect();
                            let rcnt: Vec<usize> = l.pts.iter().map(|p| p.len() * specs_me.len()).collect();
                            let mut k = 0;
                            for q in &l.specs {
                                for &isea in &l.pts[rank] {
                                    for &ispec in q {
                                        buf_pts[k] = s.va[ispec + nspec * isea];
                                        k += 1;
                                    }
                                }
                            }
                            comm.alltoallv(&buf_pts, &scnt, &mut buf_cols, &rcnt);
                            let offs: Vec<usize> = rcnt.iter().scan(0, |a, &n| Some(std::mem::replace(a, *a + n))).collect();
                            let (recv, nme) = (&buf_cols, specs_me.len());
                            let cols = xyp_cols.chunks_mut(nsea).enumerate();
                            spread(&mut xyp_s, cols, |sc, (lspec, col)| {
                                sc.field.fill(0.);
                                for (p, pts) in l.pts.iter().enumerate() {
                                    for (i, &isea) in pts.iter().enumerate() {
                                        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                        sc.field[c + ny_i] = recv[offs[p] + i * nme + lspec];
                                    }
                                }
                                let [iixy1, iixy2, iixy3, iixy4] = sc.iixy.each_mut().map(|v| v.as_mut_slice());
                                let [rdi1, rdi2, rdi3, rdi4] = sc.rdi.each_mut().map(|v| v.as_mut_slice());
                                let mut w = Xyp3Work {
                                    iixy1,
                                    iixy2,
                                    iixy3,
                                    iixy4,
                                    rdi1,
                                    rdi2,
                                    rdi3,
                                    rdi4,
                                    vlcflx: &mut sc.vlcflx,
                                    vlcfly: &mut sc.vlcfly,
                                    cxtot: &mut sc.cxtot,
                                    cytot: &mut sc.cytot,
                                    aq: &mut sc.aq,
                                    fla: &mut sc.fla,
                                };
                                match bnd {
                                    Some(b) => w3xyp3(&px, &tb, specs_me[lspec] as i32 + 1, dtg, yfirst, 0, &mut sc.field, &mut w, b),
                                    None => w3xyp3(&px, &tb, specs_me[lspec] as i32 + 1, dtg, yfirst, 0, &mut sc.field, &mut w, NoBoundary),
                                }
                                for isea in 1..=nsea {
                                    let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                    col[isea - 1] = sc.field[c + ny_i];
                                }
                            });
                            let mut k = 0;
                            for pts in &l.pts {
                                for &isea in pts {
                                    for lspec in 0..nme {
                                        buf_cols[k] = xyp_cols[lspec * nsea + isea - 1];
                                        k += 1;
                                    }
                                }
                            }
                            comm.alltoallv(&buf_cols, &rcnt, &mut buf_pts, &scnt);
                            let mut k = 0;
                            for q in &l.specs {
                                for &isea in &l.pts[rank] {
                                    let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                    for &ispec in q {
                                        if mapsta[c] != 0 {
                                            s.va[ispec + nspec * isea] = buf_pts[k];
                                        }
                                        k += 1;
                                    }
                                }
                            }
                        } else if par {
                            // W3GATH, W3XYP3 and W3SCAT of `w3xyp3_va`, split so that no thread writes
                            // `va` by component: each thread gathers its components from `va` (read only)
                            // and leaves the result in its own column; then each sea point takes its
                            // values from the columns.
                            let ny_i = m.ny as usize;
                            let va_r: &[Real] = &s.va;
                            let cols = xyp_cols.chunks_mut(nsea).enumerate();
                            spread(&mut xyp_s, cols, |sc, (ispec, col)| {
                                sc.field.fill(0.);
                                for isea in 1..=nsea {
                                    let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                    sc.field[c + ny_i] = va_r[ispec + nspec * isea];
                                }
                                let [iixy1, iixy2, iixy3, iixy4] = sc.iixy.each_mut().map(|v| v.as_mut_slice());
                                let [rdi1, rdi2, rdi3, rdi4] = sc.rdi.each_mut().map(|v| v.as_mut_slice());
                                let mut w = Xyp3Work {
                                    iixy1,
                                    iixy2,
                                    iixy3,
                                    iixy4,
                                    rdi1,
                                    rdi2,
                                    rdi3,
                                    rdi4,
                                    vlcflx: &mut sc.vlcflx,
                                    vlcfly: &mut sc.vlcfly,
                                    cxtot: &mut sc.cxtot,
                                    cytot: &mut sc.cytot,
                                    aq: &mut sc.aq,
                                    fla: &mut sc.fla,
                                };
                                match bnd {
                                    Some(b) => w3xyp3(&px, &tb, ispec as i32 + 1, dtg, yfirst, 0, &mut sc.field, &mut w, b),
                                    None => w3xyp3(&px, &tb, ispec as i32 + 1, dtg, yfirst, 0, &mut sc.field, &mut w, NoBoundary),
                                }
                                for isea in 1..=nsea {
                                    let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                    col[isea - 1] = sc.field[c + ny_i];
                                }
                            });
                            let cols = &xyp_cols;
                            let tasks = s.va.chunks_mut(nspec).enumerate().skip(1).take(nsea);
                            spread(&mut xyp_s, tasks, |_, (isea, spec)| {
                                let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                                if mapsta[c] != 0 {
                                    for (ispec, v) in spec.iter_mut().enumerate() {
                                        *v = cols[ispec * nsea + isea - 1];
                                    }
                                }
                            });
                        } else {
                            for ispec in 0..nspec {
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
                                let isp = ispec as i32 + 1;
                                match bnd {
                                    Some(b) => w3xyp3_va(&px, &tb, isp, dtg, yfirst, 0, nspec, nspec, &mut s.va, &mut field, &mut w, b),
                                    None => w3xyp3_va(&px, &tb, isp, dtg, yfirst, 0, nspec, nspec, &mut s.va, &mut field, &mut w, NoBoundary),
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
                    let point = |isea: usize, s: &State| SrcePoint {
                        srce_call: SRCE_DIRECT,
                        it,
                        d_inp: dw[isea],
                        u10abs: u10[isea],
                        u10dir: u10d[isea],
                        ustar: s.ust[isea],
                        ustdir: s.ustdir[isea],
                        ice: s.ice[isea],
                        dtg,
                        dair: DAIR,
                        coef: 1.,
                    };
                    if let Some(d) = device.as_deref_mut() {
                        let pts: Vec<_> = (1..=nsea).map(|i| point(i, &s)).collect();
                        let act = active_of(m, &mapsta);
                        d.srce(&p, &tables, &pts, &act, &wn1, &cg1, &mut src[1..]);
                        for isea in 1..=nsea {
                            if act[isea - 1] != 0 {
                                (s.ust[isea], s.ustdir[isea]) = (src[isea].ustar, src[isea].ustdir);
                                (dtdyn[isea], fcut[isea]) = (src[isea].dtdyn, src[isea].fcut);
                            } else {
                                (s.ust[isea], s.ustdir[isea], dtdyn[isea], fcut[isea]) = (UNDEF, UNDEF, UNDEF, UNDEF);
                                src[isea] = SrceOut::default();
                            }
                        }
                    }
                    if par {
                        let tasks = s
                            .va
                            .chunks_mut(nspec)
                            .zip(whitecap.chunks_mut(4))
                            .zip(s.ust.iter_mut())
                            .zip(s.ustdir.iter_mut())
                            .zip(dtdyn.iter_mut())
                            .zip(fcut.iter_mut())
                            .zip(src.iter_mut())
                            .enumerate()
                            .skip(1)
                            .take(nsea)
                            .filter(|(isea, _)| mine(*isea))
                            .map(|(isea, ((((((spec, whitecap), ust), ustdir), dtdyn), fcut), src))| SrceTask {
                                isea,
                                spec,
                                whitecap,
                                ust,
                                ustdir,
                                dtdyn,
                                fcut,
                                src,
                            });
                        spread(&mut srce_s, tasks, |sc, t| {
                            let isea = t.isea;
                            let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                            if mapsta[c] != 1 {
                                *t.ust = UNDEF;
                                *t.ustdir = UNDEF;
                                *t.dtdyn = UNDEF;
                                *t.fcut = UNDEF;
                                return;
                            }
                            let q = SrcePoint {
                                srce_call: SRCE_DIRECT,
                                it,
                                d_inp: dw[isea],
                                u10abs: u10[isea],
                                u10dir: u10d[isea],
                                ustar: *t.ust,
                                ustdir: *t.ustdir,
                                ice: s.ice[isea],
                                dtg,
                                dair: DAIR,
                                coef: 1.,
                            };
                            let [specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda] =
                                sc.bufs.iter_mut().map(|b| b.as_mut_slice()).collect::<Vec<_>>().try_into().ok().unwrap();
                            let mut w = SrceWork {
                                specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda,
                                llws: &mut sc.llws,
                                w_snl: &mut sc.w_snl,
                                w_sds: &mut sc.w_sds,
                                iw_sds: &mut sc.iw_sds,
                                w_small: &mut sc.w_small,
                                whitecap: t.whitecap,
                                sin_out: &mut sc.sin_out,
                                spr_out: &mut sc.spr_out,
                            };
                            let (a, b) = (1 + nk2 * isea, nk2 * isea + nk + 1);
                            w3srce(&p, &tables, &q, t.spec, &wn[a..b], &cg[a..b], &mut w, &mut sc.out, 0);
                            *t.ust = sc.out[0].ustar;
                            *t.ustdir = sc.out[0].ustdir;
                            *t.dtdyn = sc.out[0].dtdyn;
                            *t.fcut = sc.out[0].fcut;
                            *t.src = sc.out[0];
                        });
                    }
                    for isea in 1..=if device.is_some() || par { 0 } else { nsea } {
                        if !mine(isea) {
                            continue;
                        }
                        let c = (m.mapsf[isea - 1 + 2 * nsea] - 1) as usize;
                        if mapsta[c] != 1 {
                            s.ust[isea] = UNDEF;
                            s.ustdir[isea] = UNDEF;
                            dtdyn[isea] = UNDEF;
                            fcut[isea] = UNDEF;
                            continue;
                        }
                        let q = point(isea, &s);
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
                if let Some(l) = lay.as_ref() {
                    if (0..4).any(|j| flout[j] && dsec21(time, tonext[j]) == 0.) {
                        gather_out(comm, l, nspec, &mut s, &mut dtdyn, &mut fcut, &mut src, &mut whitecap);
                    }
                }
                // The only reads of the resident spectrum: what an output due now needs.
                if let Some(d) = device.as_deref_mut() {
                    if (0..4).any(|j| flout[j] && dsec21(time, tonext[j]) == 0.) {
                        d.va_read(&mut s.va[nspec..]);
                        if flout[0] && (5..=10).any(|j| flags.on(5, j)) {
                            d.whitecap_read(&mut whitecap[4..]);
                        }
                    }
                }
                tofrst = [-1, 0];
                for j in 0..4 {
                    if !flout[j] {
                        continue;
                    }
                    if dsec21(time, tonext[j]) == 0. {
                        if JS[j] == 1 {
                            if let Some(f) = grd.as_mut() {
                                // W3WAVE: FLPART when a group 4 field is requested and the field output is due.
                                let prt = flags_part.then(|| {
                                    let wn1 = |si, h| disp.wavnu1(si, h);
                                    let p = Part { m, neigh: Neigh::new(nk, nth), wavnu: &wn1 };
                                    w3cprt(&p, &CprtInput { va: &s.va, cg: &cg, wn: &wn, dw: &dw, u10: &u10, u10d: &u10d, asf: &s.asf, mapsta: &mapsta })
                                });
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
                                    prt: prt.as_ref(),
                                };
                                f.write(m, &mut g);
                            }
                            // W3IOGO's blank is in place, and CFLXYMAX is computed only at the start of a W3WAVE
                            // call: a point that ice left in between is still UNDEF at the next output.
                            if flags.on(9, 3) {
                                for isea in 1..=nsea {
                                    if mapsta[(m.mapsf[isea - 1 + 2 * nsea] - 1) as usize] < 0 {
                                        cflxy[isea] = UNDEF;
                                    }
                                }
                            }
                        } else if JS[j] == 2 {
                            if let Some(f) = pnt.as_mut() {
                                let w = Winds { ua: &ua, ud: &ud, asv: &asv, cx: &cx, cy: &cy };
                                write_points(f, time, &pts, m, &mapsta, &s.va, &dw, &cg, &s.ice, &w);
                            }
                        } else if JS[j] == 5 {
                            if root {
                                nest.write(m, time, &s.va, &cg);
                            }
                        } else if JS[j] == 4 {
                            let path = format!("{dir}/restart{ifile:03}.ww3");
                            ifile += 1;
                            if root {
                                let before = s.va.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
                                iors::write(&path, m, &mut s, time, &mapsta, &mapst2);
                                // W3IORS ends with VA = MAX(0.,VA).
                                if s.va.iter().zip(&before).any(|(x, b)| x.to_bits() != *b) {
                                    if let Some(d) = device.as_deref_mut() {
                                        d.va_write(&s.va[nspec..]);
                                    }
                                }
                            } else {
                                // The same clamp on the points this rank owns; the file is rank 0's.
                                for v in &mut s.va {
                                    *v = v.max(0.);
                                }
                            }
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
        for f in [r.level.as_mut(), r.ice.as_mut()].into_iter().flatten() {
            f.off = f.last;
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
    ice: &[Real],
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
        let iceo = rd[0] * ice[is[0]] + rd[1] * ice[is[1]] + rd[2] * ice[is[2]] + rd[3] * ice[is[3]];
        for v in [dpo, wao, wdo, aso, cao, cdo, iceo, sum(0.5), sum(1000.)] {
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
