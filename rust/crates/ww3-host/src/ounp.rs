// SPDX-License-Identifier: MPL-2.0
//! What `ww3_ounp` (`model/src/ww3_ounp.F90`, 5580 lines at `92e2cf66`) computes before it touches NetCDF, for
//! `switches/switch_chaceon_shrd`: the namelist `ww3_ounp.nml` (`W3NMLOUNP`, `w3nmlounpmd.F90:75-1300`) and its log
//! `ww3_ounp.nml.log`, the point output file `out_pnt.ww3` (`W3IOPO`, the `READ` branch, `w3iopomd.F90:1160-1395`), the
//! names of the files, and the mean wave parameters of `W3EXNC` (`ww3_ounp.F90:1960-2070`) with the quantities that
//! `ITYPE` 2 writes. The program that writes the files is `rust/tools/ww3_ounp`. What is not implemented stops the
//! run with a message that names it, status 64.

// The loops keep the Fortran's 1-based indices, so a line can be read against `ww3_ounp.F90`.
#![allow(clippy::needless_range_loop)]

use crate::disp::Dispersion;
use crate::fortfmt as ff;
use crate::gridio;
use crate::moddef::{ModDef, Records};
use crate::outp::Sink;
use crate::wtime::Time;
use ww3_kernels::constants::{GRAV, RADE, TPI, TPIINV};
use ww3_kernels::real::Real;
use ww3_libm::{atan2f, log10f, sqrtf};

const IDSTR: &str = "WAVEWATCH III POINT OUTPUT FILE";
const VEROPT: &str = "2021-04-06";

/// The namelist groups of `W3NMLOUNPMD`, with their defaults (`:303-311`, `:414-415`, `:519-522`, `:625`,
/// `:726-736`). The strings keep the Fortran's lengths: `CHARACTER(15)` for the times, `(1024)` for the list and
/// `(30)` for the prefix.
pub struct Request {
    pub timestart: String,
    pub timestride: String,
    pub timecount: String,
    pub timesplit: i32,
    pub list: String,
    pub samefile: bool,
    pub buffer: i32,
    pub itype: i32,
    pub dimorder: bool,
    pub prefix: String,
    pub netcdf: i32,
    pub sp_output: i32,
    pub sp_scale: Real,
    pub sp_outfac: Real,
    pub sp_type: i32,
    pub par_output: i32,
    pub so_output: i32,
    pub so_scale: Real,
    pub so_outfac: Real,
    pub so_table: i32,
    /// SPECTRUM INPUT INTERACTIONS DISSIPATION BOTTOM ICE TOTAL
    pub so_flags: [bool; 7],
}

fn clip(s: String, n: usize) -> String {
    s.chars().take(n).collect()
}

impl Request {
    /// Reads the file. A key or group the program does not know is an error, as the Fortran namelist read would be.
    pub fn read(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let groups = gridio::groups(&text);
        let mut r = Request {
            timestart: "19000101 000000".into(),
            timestride: "0".into(),
            timecount: "1000000000".into(),
            timesplit: 6,
            list: "all".into(),
            samefile: true,
            buffer: 150,
            itype: 1,
            dimorder: true,
            prefix: "ww3.".into(),
            netcdf: 3,
            sp_output: 3,
            sp_scale: 1.,
            sp_outfac: 0.,
            sp_type: 4,
            par_output: 3,
            so_output: 4,
            so_scale: 0.,
            so_outfac: 0.,
            so_table: 0,
            so_flags: [true; 7],
        };
        let num = |v: &str| -> Result<Real, String> {
            let t = v.trim().replace(['d', 'D'], "e");
            t.parse().map_err(|_| format!("{v:?} is not a real"))
        };
        let int = |v: &str| -> Result<i32, String> { v.trim().parse().map_err(|_| format!("{v:?} is not an integer")) };
        let log = |v: &str| -> Result<bool, String> {
            match v.trim().trim_start_matches('.').to_ascii_uppercase().chars().next() {
                Some('T') => Ok(true),
                Some('F') => Ok(false),
                _ => Err(format!("{v:?} is not a logical")),
            }
        };
        for name in ["POINT_NML", "FILE_NML", "SPECTRA_NML", "PARAM_NML", "SOURCE_NML"] {
            if !groups.iter().any(|g| g.name == name) {
                return Err(format!("the group &{name} is missing (the Fortran namelist read would fail)"));
            }
        }
        for g in &groups {
            let known = ["POINT_NML", "FILE_NML", "SPECTRA_NML", "PARAM_NML", "SOURCE_NML"];
            if !known.contains(&g.name.as_str()) {
                return Err(format!("the group &{} is not read by ww3_ounp", g.name));
            }
            for (k, v) in &g.items {
                let t = gridio::text(v);
                match (g.name.as_str(), k.as_str()) {
                    ("POINT_NML", "POINT%TIMESTART") => r.timestart = clip(t, 15),
                    ("POINT_NML", "POINT%TIMESTRIDE") => r.timestride = clip(t, 15),
                    ("POINT_NML", "POINT%TIMECOUNT") => r.timecount = clip(t, 15),
                    ("POINT_NML", "POINT%TIMESPLIT") => r.timesplit = int(v)?,
                    ("POINT_NML", "POINT%LIST") => r.list = clip(t, 1024),
                    ("POINT_NML", "POINT%SAMEFILE") => r.samefile = log(v)?,
                    ("POINT_NML", "POINT%BUFFER") => r.buffer = int(v)?,
                    ("POINT_NML", "POINT%TYPE") => r.itype = int(v)?,
                    ("POINT_NML", "POINT%DIMORDER") => r.dimorder = log(v)?,
                    ("FILE_NML", "FILE%PREFIX") => r.prefix = clip(t, 30),
                    ("FILE_NML", "FILE%NETCDF") => r.netcdf = int(v)?,
                    ("SPECTRA_NML", "SPECTRA%OUTPUT") => r.sp_output = int(v)?,
                    ("SPECTRA_NML", "SPECTRA%SCALE_FAC") => r.sp_scale = num(v)?,
                    ("SPECTRA_NML", "SPECTRA%OUTPUT_FAC") => r.sp_outfac = num(v)?,
                    ("SPECTRA_NML", "SPECTRA%TYPE") => r.sp_type = int(v)?,
                    ("PARAM_NML", "PARAM%OUTPUT") => r.par_output = int(v)?,
                    ("SOURCE_NML", "SOURCE%OUTPUT") => r.so_output = int(v)?,
                    ("SOURCE_NML", "SOURCE%SCALE_FAC") => r.so_scale = num(v)?,
                    ("SOURCE_NML", "SOURCE%OUTPUT_FAC") => r.so_outfac = num(v)?,
                    ("SOURCE_NML", "SOURCE%TABLE_FAC") => r.so_table = int(v)?,
                    ("SOURCE_NML", "SOURCE%SPECTRUM") => r.so_flags[0] = log(v)?,
                    ("SOURCE_NML", "SOURCE%INPUT") => r.so_flags[1] = log(v)?,
                    ("SOURCE_NML", "SOURCE%INTERACTIONS") => r.so_flags[2] = log(v)?,
                    ("SOURCE_NML", "SOURCE%DISSIPATION") => r.so_flags[3] = log(v)?,
                    ("SOURCE_NML", "SOURCE%BOTTOM") => r.so_flags[4] = log(v)?,
                    ("SOURCE_NML", "SOURCE%ICE") => r.so_flags[5] = log(v)?,
                    ("SOURCE_NML", "SOURCE%TOTAL") => r.so_flags[6] = log(v)?,
                    (grp, key) => return Err(format!("the key {key} of &{grp} is not read by ww3_ounp")),
                }
            }
        }
        Ok(r)
    }

    /// `ww3_ounp.nml.log`: the `REPORT_*_NML` routines (`w3nmlounpmd.F90:763-1200`). `REPORT_SOURCE_NML` does not
    /// report `BOTTOM`.
    pub fn log(&self) -> String {
        let l = |b: bool| if b { "T" } else { "F" };
        let i8 = |n: i32| ff::i(8, n as i64);
        let f82 = |x: Real| ff::f(8, 2, x as f64);
        let mut s = String::new();
        let mut row = |m: &str, k: &str, v: &str| {
            if m.is_empty() {
                s.push('\n');
            } else {
                s.push_str(&format!("{m}  {k}{v}\n"));
            }
        };
        let t = |x: &str| x.trim_end().to_string();
        row("", "", "");
        row("POINT %", "TIMESTART  = ", &t(&self.timestart));
        row("POINT %", "TIMESTRIDE = ", &t(&self.timestride));
        row("POINT %", "TIMECOUNT  = ", &t(&self.timecount));
        row("POINT %", "TIMESPLIT  = ", &i8(self.timesplit));
        row("POINT %", "LIST       = ", &t(&self.list));
        row("POINT %", "SAMEFILE   = ", l(self.samefile));
        row("POINT %", "BUFFER     = ", &i8(self.buffer));
        row("POINT %", "TYPE       = ", &i8(self.itype));
        row("POINT %", "DIMORDER   = ", l(self.dimorder));
        row("", "", "");
        row("FILE %", "PREFIX    = ", &t(&self.prefix));
        row("FILE %", "NETCDF    = ", &i8(self.netcdf));
        row("", "", "");
        row("SPECTRA %", "OUTPUT     = ", &i8(self.sp_output));
        row("SPECTRA %", "SCALE_FAC  = ", &f82(self.sp_scale));
        row("SPECTRA %", "OUTPUT_FAC = ", &f82(self.sp_outfac));
        row("SPECTRA %", "TYPE       = ", &i8(self.sp_type));
        row("", "", "");
        row("PARAM %", "OUTPUT     = ", &i8(self.par_output));
        row("", "", "");
        row("SOURCE %", "OUTPUT       = ", &i8(self.so_output));
        row("SOURCE %", "SCALE_FAC    = ", &f82(self.so_scale));
        row("SOURCE %", "OUTPUT_FAC   = ", &f82(self.so_outfac));
        row("SOURCE %", "TABLE_FAC    = ", &i8(self.so_table));
        row("SOURCE %", "SPECTRUM     = ", l(self.so_flags[0]));
        row("SOURCE %", "INPUT        = ", l(self.so_flags[1]));
        row("SOURCE %", "INTERACTIONS = ", l(self.so_flags[2]));
        row("SOURCE %", "DISSIPATION  = ", l(self.so_flags[3]));
        row("SOURCE %", "ICE          = ", l(self.so_flags[5]));
        row("SOURCE %", "TOTAL        = ", l(self.so_flags[6]));
        s
    }
}

/// `READ (str,*) a, b` of two integers.
pub fn two_ints(s: &str) -> Result<Time, String> {
    let mut w = s.split(|c: char| c.is_whitespace() || c == ',').filter(|x| !x.is_empty());
    let mut one = || -> Result<i32, String> { w.next().ok_or_else(|| format!("{s:?}: two integers expected"))?.parse().map_err(|_| format!("{s:?}: not integers")) };
    Ok([one()?, one()?])
}

/// `READ (str,*) x` of a real.
pub fn one_real(s: &str) -> Result<Real, String> {
    let t = s.split(|c: char| c.is_whitespace() || c == ',').find(|x| !x.is_empty()).ok_or_else(|| format!("{s:?}: a real expected"))?;
    t.replace(['d', 'D'], "e").parse().map_err(|_| format!("{s:?}: not a real"))
}

/// `READ (str,*) n` of an integer.
pub fn one_int(s: &str) -> Result<i32, String> {
    let t = s.split(|c: char| c.is_whitespace() || c == ',').find(|x| !x.is_empty()).ok_or_else(|| format!("{s:?}: an integer expected"))?;
    t.parse().map_err(|_| format!("{s:?}: not an integer"))
}

/// `FILETIME` of `ww3_ounp.F90:638-656` (`nint` false, FLOOR) and `:762-777` (`nint` true, NINT): the part of the
/// date that `S3` selects. `S3` is already clamped; 9 divides by zero in the Fortran and the caller refuses it.
pub fn filetime(time: Time, s3: i32, nint: bool) -> String {
    let div = |x: i32, k: i32| -> i32 {
        let q = x as Real / (10f32.powi(k).round() as Real);
        (if nint { q.round() } else { q.floor() }) as i32
    };
    match s3 {
        0 => String::new(),
        10 => format!("{}T{}", ff::im(8, 8, time[0] as i64), ff::im(2, 2, div(time[1], 4) as i64)),
        8 => ff::im(8, 8, time[0] as i64),
        s if s < 0 => String::new(),
        s => ff::im(s as usize, s as usize, div(time[0], 8 - s) as i64),
    }
}

/// `WRITE (DATE,'(I8.8,I6.6)') t`.
pub fn date14(t: Time) -> String {
    format!("{}{}", ff::im(8, 8, t[0] as i64), ff::im(6, 6, t[1] as i64))
}

/// `WRITE (STR,'(I4.4,A,4(I2.2,A),I2.2)') d(1),'-',d(2),'-',d(3),' ',d(5),':',d(6),':',d(7)`.
pub fn datestr(d: &[i32; 8]) -> String {
    let i = |n: i32, w: usize| ff::im(w, w, n as i64);
    format!("{}-{}-{} {}:{}:{}", i(d[0], 4), i(d[1], 2), i(d[2], 2), i(d[4], 2), i(d[5], 2), i(d[6], 2))
}

/// `INDEX (a(1:n), b(1:n)) .EQ. 0` for two strings of one length: they differ. A zero length matches.
pub fn date_differs(past: &str, date: &str, n: i32) -> bool {
    if n <= 0 {
        return false;
    }
    let n = n as usize;
    past.get(..n) != date.get(..n)
}

/// One output time of `out_pnt.ww3`: the arrays `W3IOPO` fills.
pub struct Pnt {
    path: String,
    rec: Records,
    pub time: Time,
    pub nopts: usize,
    pub ptloc: Vec<[Real; 2]>,
    pub ptnme: Vec<String>,
    pub dpo: Vec<Real>,
    pub wao: Vec<Real>,
    pub wdo: Vec<Real>,
    pub aso: Vec<Real>,
    pub cao: Vec<Real>,
    pub cdo: Vec<Real>,
    pub spco: Vec<Vec<Real>>,
}

/// What `W3IOPO ('READ')` reports.
pub enum Rd {
    Ok,
    /// `IOTEST = -1`.
    Eof,
}

impl Pnt {
    /// The first `W3IOPO` call: the header and the points. The caller then reads the first time with `read`.
    pub fn open(path: &str, m: &ModDef, log: &mut Sink) -> Self {
        let mut rec = Records::open(path);
        let (nopts, ptloc, ptnme) = Self::header(&mut rec, m, log);
        let z = vec![0.; nopts];
        Self {
            path: path.to_string(),
            rec,
            time: [0, 0],
            nopts,
            ptloc,
            ptnme,
            dpo: z.clone(),
            wao: z.clone(),
            wdo: z.clone(),
            aso: z.clone(),
            cao: z.clone(),
            cdo: z,
            spco: vec![Vec::new(); nopts],
        }
    }

    /// `CLOSE (NDSOP)` and `IPASS = 0`: the next `W3IOPO` opens the file again and reads the header again.
    pub fn reopen(&mut self, m: &ModDef, log: &mut Sink) {
        self.rec = Records::open(&self.path);
        let (nopts, ptloc, ptnme) = Self::header(&mut self.rec, m, log);
        self.nopts = nopts;
        self.ptloc = ptloc;
        self.ptnme = ptnme;
    }

    fn header(rec: &mut Records, m: &ModDef, log: &mut Sink) -> (usize, Vec<[Real; 2]>, Vec<String>) {
        let nopts;
        {
            let Some(mut c) = rec.try_next() else {
                log.put("\n *** WAVEWATCH III ERROR IN W3IOPO : \n     PREMATURE END OF FILE\n\n");
                log.exit(21)
            };
            let idtst = c.text(31);
            let vertst = c.text(10);
            let (mk, mth, n) = (c.i(), c.i(), c.i());
            c.done("W3IOPO header");
            if idtst != IDSTR {
                log.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOPO :\n     ILLEGAL IDSTR, READ : {}\n                  CHECK : {}\n\n", ff::a(31, &idtst), IDSTR));
                log.exit(10)
            }
            if vertst != VEROPT {
                log.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOPO :\n     ILLEGAL VEROPT, READ : {}\n                   CHECK : {}\n\n", ff::a(10, &vertst), VEROPT));
                log.exit(11)
            }
            if m.nk != mk || m.nth != mth {
                log.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOPO :\n     ERROR IN SPECTRA, MK, MTH : {:>8}{:>8}\n              ARRAY DIMENSIONS : {:>8}{:>8}\n\n", mk, mth, m.nk, m.nth));
                log.exit(12)
            }
            nopts = n as usize;
        }
        let Some(mut c) = rec.try_next() else {
            log.put("\n *** WAVEWATCH III ERROR IN W3IOPO : \n     PREMATURE END OF FILE\n\n");
            log.exit(21)
        };
        let mut loc = Vec::new();
        for _ in 0..nopts {
            loc.push([c.f(), c.f()]);
        }
        let mut nm = Vec::new();
        for _ in 0..nopts {
            nm.push(c.text(40));
        }
        c.done("W3IOPO points");
        (nopts, loc, nm)
    }

    /// The time and the points' records of the next output time. At the end of the file `TIME` keeps its value.
    pub fn read(&mut self, m: &ModDef, log: &mut Sink) -> Rd {
        let Some(mut c) = self.rec.try_next() else { return Rd::Eof };
        self.time = [c.i(), c.i()];
        c.done("W3IOPO time");
        let nspec = (m.nk * m.nth) as usize;
        for i in 0..self.nopts {
            let Some(mut c) = self.rec.try_next() else {
                log.put("\n *** WAVEWATCH III ERROR IN W3IOPO : \n     PREMATURE END OF FILE\n\n");
                log.exit(21)
            };
            c.skip(12); // IW, II, IL
            self.dpo[i] = c.f();
            self.wao[i] = c.f();
            self.wdo[i] = c.f();
            self.aso[i] = c.f();
            self.cao[i] = c.f();
            self.cdo[i] = c.f();
            c.skip(12); // ICEO, ICEHO, ICEFO
            c.skip(13); // GRDID
            self.spco[i] = c.fs(nspec);
            c.done("W3IOPO point");
        }
        Rd::Ok
    }
}

/// `MOD (a, p)` of reals.
fn modr(a: Real, p: Real) -> Real {
    a % p
}

/// `SIGN (a, b)`.
fn sign(a: Real, b: Real) -> Real {
    f32::from_bits((a.to_bits() & 0x7fff_ffff) | (b.to_bits() & 0x8000_0000))
}

/// The mean wave parameters of one point and one time (`ww3_ounp.F90:1960-2070`), `HSMIN` 0.05.
pub struct Mean {
    pub depth: Real,
    pub udir: Real,
    pub uabs: Real,
    pub cdir: Real,
    pub hsig: Real,
    pub wlen: Real,
    pub tmean: Real,
    pub thmean: Real,
    pub thsprd: Real,
    pub fp: Real,
    pub thp: Real,
    pub spp: Real,
}

const HSMIN: Real = 0.05;

/// Section 2 (the inlined `WAVNU1`) and 3.a to 3.b of `W3EXNC` for point `j`.
pub fn mean(m: &ModDef, disp: &Dispersion, p: &Pnt, j: usize) -> Mean {
    let (nk, nth) = (m.nk as usize, m.nth as usize);
    let sig = |ik: usize| m.sig[ik];
    let xl: Real = 1. / m.xfr - 1.;
    let xh: Real = m.xfr - 1.;
    let xl2 = xl * xl;
    let xh2 = xh * xh;
    let depth: Real = if p.dpo[j] > m.dmin { p.dpo[j] } else { m.dmin };
    let sqrth = sqrtf(depth);
    let udir = modr(270. - p.wdo[j] * RADE, 360.);
    let uabs: Real = if p.wao[j] > 0.001 { p.wao[j] } else { 0.001 };
    let cdir = modr(270. - p.cdo[j] * RADE, 360.);
    let (dsie, n1max, ewn1, _) = disp.tables();
    let mut wn = vec![0.; nk + 1];
    for ik in 1..=nk {
        let six = sig(ik) * sqrth;
        let i1 = (six / dsie) as i32;
        if i1 <= n1max {
            let i2 = i1 + 1;
            let r1 = six / dsie - i1 as Real;
            let r2 = 1. - r1;
            wn[ik] = (r2 * ewn1[i1 as usize] + r1 * ewn1[i2 as usize]) / depth;
        } else {
            wn[ik] = sig(ik) * sig(ik) / GRAV;
        }
    }
    let spco = &p.spco[j];
    let (mut et, mut ewn, mut etr, mut etx, mut ety): (Real, Real, Real, Real, Real) = (0., 0., 0., 0., 0.);
    let mut e1 = vec![0.; nk + 1];
    let mut thbnd = vec![0.; nk + 1];
    let mut spbnd = vec![0.; nk + 1];
    let (mut ebx, mut eby): (Real, Real) = (0., 0.);
    for ik in 1..=nk {
        let mut ebnd: Real = 0.;
        ebx = 0.;
        eby = 0.;
        for ith in 1..=nth {
            let v = spco[ith + (ik - 1) * nth - 1];
            ebnd += v;
            ebx += v * m.ecos[ith - 1];
            eby += v * m.esin[ith - 1];
        }
        e1[ik] = ebnd * m.dth;
        if e1[ik] > 1.0e-5 {
            thbnd[ik] = modr(630. - RADE * atan2f(eby, ebx), 360.);
            let t = (ebx * ebx + eby * eby) / (ebnd * ebnd);
            let t = if t > 0. { t } else { 0. };
            let t = 2. * (1. - sqrtf(t));
            spbnd[ik] = RADE * sqrtf(if t > 0. { t } else { 0. });
        } else {
            thbnd[ik] = -999.9;
            spbnd[ik] = -999.9;
        }
        let ebnd2 = e1[ik] * m.dsii[ik - 1] * TPIINV;
        et += ebnd2;
        ewn += ebnd2 / wn[ik];
        etr += ebnd2 / sig(ik);
        etx += ebx * m.dsii[ik - 1];
        ety += eby * m.dsii[ik - 1];
    }
    let ebnd: Real = e1[nk] * TPIINV / (sig(nk) * m.dth);
    et += m.fte * ebnd;
    ewn += m.ftwl * ebnd;
    etr += m.fttr * ebnd;
    etx = m.dth * etx * TPIINV + m.fte * ebx * TPIINV / sig(nk);
    ety = m.dth * ety * TPIINV + m.fte * eby * TPIINV / sig(nk);
    let hsig: Real = 4. * sqrtf(et);
    let (wlen, tmean, thmean, mut thsprd);
    if hsig > HSMIN {
        wlen = ewn / et * TPI;
        tmean = etr / et * TPI;
        thmean = modr(630. - RADE * atan2f(ety, etx), 360.);
        let t = (etx * etx + ety * ety) / (et * et);
        let t = if t > 0. { t } else { 0. };
        let t = 2. * (1. - sqrtf(t));
        thsprd = RADE * sqrtf(if t > 0. { t } else { 0. });
        if thsprd < 0.01 * RADE * m.dth {
            thsprd = 0.;
        }
    } else {
        wlen = 0.;
        tmean = 0.;
        thmean = 0.;
        thsprd = 0.;
        for v in e1.iter_mut() {
            *v = 0.;
        }
    }
    let mut emax = e1[nk];
    let mut ikm = nk;
    for ik in (1..nk).rev() {
        if e1[ik] > emax {
            emax = e1[ik];
            ikm = ik;
        }
    }
    let (fp, thp, mut spp);
    if hsig >= HSMIN && ikm != nk {
        let el = if ikm == 1 { -e1[ikm] } else { e1[ikm - 1] - e1[ikm] };
        let eh = e1[ikm + 1] - e1[ikm];
        let denom = xl * eh - xh * el;
        let dd = denom.abs();
        let dd = if dd > 1.0e-15 { dd } else { 1.0e-15 };
        fp = sig(ikm) * (1. + 0.5 * (xl2 * eh - xh2 * el) / sign(dd, denom));
        thp = thbnd[ikm];
        spp = spbnd[ikm];
        if spp < 0.01 * RADE * m.dth {
            spp = 0.;
        }
    } else {
        fp = 0.;
        thp = 0.;
        spp = 0.;
    }
    Mean { depth, udir, uabs, cdir, hsig, wlen, tmean, thmean, thsprd, fp, thp, spp }
}

/// `NINT (ALOG10 (e + 1E-12) / 0.0004)` of `W3EXNC`'s transfer-file branch, kept as the `REAL` the `WHERE` stores.
pub fn log_spec(e: Real) -> Real {
    let q = log10f(e + 1.0e-12) / 0.0004;
    nint_sat(q) as Real
}

/// Fortran `NINT` of a `REAL`: halves away from zero, saturating (aarch64 `fcvtas`).
pub fn nint_sat(x: Real) -> i32 {
    x.round() as i32
}

/// `HMAT`, `AGE1` and `AGE2` of `ITYPE` 2, `OTYPE` 5 (`ww3_ounp.F90:2787-2797`); ages are `None` below `HSMIN`, where
/// the Fortran stores `NF90_FILL_FLOAT`.
pub fn validation(disp: &Dispersion, mn: &Mean, dpo: Real) -> (Real, Option<(Real, Real)>) {
    let h = 3.33 * GRAV * mn.hsig / (mn.uabs * mn.uabs);
    let hmat = if h < 100. { h } else { 100. };
    if mn.hsig >= HSMIN {
        let (wna, _) = disp.wavnu1(mn.fp, dpo);
        let a1 = mn.fp / wna / mn.uabs;
        let age1 = if a1 < 100. { a1 } else { 100. };
        let afr = TPI / mn.tmean;
        let (wna, _) = disp.wavnu1(afr, dpo);
        let a2 = afr / wna / mn.uabs;
        let age2 = if a2 < 100. { a2 } else { 100. };
        (hmat, Some((age1, age2)))
    } else {
        (hmat, None)
    }
}
