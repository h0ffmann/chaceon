// SPDX-License-Identifier: MPL-2.0
//! The program `W3OUNP` (`model/src/ww3_ounp.F90:22-1417`) and its internal subroutine `W3EXNC` (`:1439-3045`) for
//! `switches/switch_chaceon_shrd`, with the control flow of the labels 560, 700 and 888 kept as the Fortran has it.
//!
//! Reached: `ww3_ounp.nml` with `TYPE` 1 and `SPECTRA%OUTPUT` 3 (the spectra file, the `SHORT` branch of `W3EXNC`) and
//! `TYPE` 2 with `PARAM%OUTPUT` 1, 2, 5 and 6, classic and NetCDF-4 files, one file or one file per station.
//! Stops with a message and status 64, never silently:
//!
//! - `ww3_ounp.inp` without `ww3_ounp.nml`, and a file `NC_globatt.inp` in the directory;
//! - `TYPE` 1 with `SPECTRA%OUTPUT` 2 and 4, `TYPE` 2 with `PARAM%OUTPUT` 3, 4 and above 6, `TYPE` 3 (they call the source
//!   terms, `W3PART` and `W3FLX`);
//! - a calendar other than `standard`, `FILE%NETCDF` other than 3 and 4, `BUFFER` below 1, `TIMESPLIT` 9, a point list the
//!   Fortran would read out of bounds;
//! - an unstructured or SMC grid (`mod_def.ww3` reader), a namelist key the program does not read.

use crate::crnc::{self, Crnc};
use crate::nc;
use ww3_host::disp::Dispersion;
use ww3_host::fortfmt as ff;
use ww3_host::moddef::ModDef;
use ww3_host::ounf::{t2d, tsub};
use ww3_host::ounp::{self, Pnt, Rd, Request};
use ww3_host::outp::{check_grid, stme21, Sink};
use ww3_host::wtime::{dsec21, tick21, Time};
use ww3_kernels::constants::{RADE, TPI, TPIINV};
use ww3_kernels::real::Real;

const PROG: &str = "ww3_ounp";

/// `SINGLEFILE` is read from the old `.inp` input only (`ww3_ounp.F90:550`); the namelist path leaves it as the
/// frame of the program had it. The reference reads it as true: with `TIMESPLIT` 8 on 20 days of output it wrote one
/// file named after the first day and no "New time step" line (`scripts/ounp_variants.sh`, `split8`). ⚠ This is the
/// content of a stack slot that the program never writes, as in `ww3_ounf`.
const SINGLEFILE: bool = true;

/// `FNMPRE`, `w3odatmd.F90:330`.
const FNMPRE: &str = "./";

/// `HEAD900` and `999` are the banner and the closing lines.
const BANNER: &str = "\n                   *** WAVEWATCH III Point output post.***    \n               ===============================================\n\n";
const END999: &str = "\n  End of program \n =========================================\n         WAVEWATCH III Point output \n\n";

struct Run {
    m: ModDef,
    disp: Dispersion,
    pnt: Pnt,
    sink: Sink,
    m2km: Real,
    itype: i32,
    otype: i32,
    nctype: i32,
    ncvartype: i32,
    together: bool,
    order: bool,
    one: usize,
    two: usize,
    nopts: usize,
    /// `FLREQ(1:NOPTS)`.
    flreq: Vec<bool>,
    ncfile: Vec<String>,
    ncid: Vec<i32>,
    varid: Vec<[i32; 29]>,
    iout: i32,
    refdate: [i32; 8],
}

fn refuse(sink: &mut Sink, what: &str) -> ! {
    sink.refuse(PROG, what)
}

fn parse<T>(sink: &mut Sink, what: &str, x: Result<T, String>) -> T {
    match x {
        Ok(v) => v,
        Err(e) => refuse(sink, &format!("{what}: {e}")),
    }
}

fn trim(s: &str) -> &str {
    s.trim_end_matches(' ')
}

impl Run {
    /// `CHECK_ERR` (`ww3_ounp.F90:5552-5572`): list-directed output, so every record starts with a blank, a default
    /// integer takes 11 columns and `NF90_STRERROR` is 80 characters.
    fn check_err(&mut self, iret: i32, icode: i32) {
        if iret != 0 {
            self.sink.put(&format!("  *** WAVEWATCH III ERROR IN OUNP :\n  NETCDF ERROR MESSAGE: \n {}\n  ICODE: \n {:>11}\n", ff::a(80, &nc::strerror(iret)), icode));
            self.sink.exit(icode)
        }
    }

    /// Formats 951 and 953.
    fn line_point(&mut self, i: usize) {
        let (x, y) = (self.m2km * self.pnt.ptloc[i][0], self.m2km * self.pnt.ptloc[i][1]);
        let name = ff::a(40, &self.pnt.ptnme[i]);
        if self.m.flagll {
            self.sink.put(&format!("      {name}{}{}\n", ff::f(10, 2, x as f64), ff::f(10, 2, y as f64)));
        } else {
            self.sink.put(&format!("      {name}{}E3{}E3\n", ff::f(8, 1, x as f64), ff::f(8, 1, y as f64)));
        }
    }

    /// The start of a variable of two dimensions, `(station, time)` in `DIMORDER` true and `(time, station)` in false.
    fn st(&self, station: usize, time: usize) -> [usize; 2] {
        if self.order { [station, time] } else { [time, station] }
    }

    /// `W3EXNC` (`ww3_ounp.F90:1439-3045`) for file `i` (1-based) and the stations `indreq` (1-based point indices).
    fn w3exnc(&mut self, i: usize, indreq: &[usize]) {
        let ncid = self.ncid[i - 1];
        let vid = self.varid[i - 1];
        let (itype, otype) = (self.itype, self.otype);
        let nreq = indreq.len();
        let iout = self.iout as usize;
        let time = self.pnt.time;
        let m2km = self.m2km;
        let (nk, nth) = (self.m.nk as usize, self.m.nth as usize);
        let outjulday = tsub(&self.refdate, &t2d(time));
        if itype == 1 && otype == 3 {
            // The SHORT version of `ITYPE` 1 and `OTYPE` 3 (`:1763-1888`).
            let mut e3 = vec![0.0 as Real; nth * nk * nreq];
            for (j1, &p) in indreq.iter().enumerate() {
                let sp = &self.pnt.spco[p - 1];
                for ik in 0..nk {
                    for ith in 0..nth {
                        e3[ith + nth * (ik + nk * j1)] = sp[ith + ik * nth];
                    }
                }
            }
            let r = nc::put_double(ncid, vid[1], &[iout], outjulday);
            self.check_err(r, 4);
            if iout == 1 {
                for (j1, &p) in indreq.iter().enumerate() {
                    let r = nc::put_int(ncid, vid[27], &[j1 + 1], None, &[], &[p as i32]);
                    self.check_err(r, 5);
                    let name = trim(&self.pnt.ptnme[p - 1]).to_string();
                    let r = nc::put_text(ncid, vid[2], &[1, j1 + 1], &[name.len(), 1], &name);
                    self.check_err(r, 6);
                }
            }
            // FLWW3 is 0 on the namelist path, so VARID(3) does not exist and the puts of `:1825-1827` do not run.
            let st = self.st(1, iout);
            let lon: Vec<Real> = indreq.iter().map(|&p| m2km * self.pnt.ptloc[p - 1][0]).collect();
            let r = nc::put_real(ncid, vid[4], &st, None, &[nreq], &lon);
            self.check_err(r, 8);
            let lat: Vec<Real> = indreq.iter().map(|&p| m2km * self.pnt.ptloc[p - 1][1]).collect();
            let r = nc::put_real(ncid, vid[5], &st, None, &[nreq], &lat);
            self.check_err(r, 9);
            let pick = |a: &[Real]| -> Vec<Real> { indreq.iter().map(|&p| a[p - 1]).collect() };
            let udir1: Vec<Real> = indreq.iter().map(|&p| (270. - self.pnt.wdo[p - 1] * RADE) % 360.).collect();
            let cdir1: Vec<Real> = indreq.iter().map(|&p| (270. - self.pnt.cdo[p - 1] * RADE) % 360.).collect();
            let (dpo, wao, cao) = (pick(&self.pnt.dpo), pick(&self.pnt.wao), pick(&self.pnt.cao));
            if self.ncvartype <= 3 {
                let q = |v: &[Real], d: Real| -> Vec<i32> { v.iter().map(|x| ounp::nint_sat(x / d)).collect() };
                let r = nc::put_int(ncid, vid[11], &st, None, &[nreq], &q(&dpo, 0.5));
                self.check_err(r, 10);
                let r = nc::put_int(ncid, vid[12], &st, None, &[nreq], &q(&wao, 0.1));
                self.check_err(r, 11);
                let r = nc::put_int(ncid, vid[13], &st, None, &[nreq], &q(&udir1, 0.1));
                self.check_err(r, 12);
                let r = nc::put_int(ncid, vid[14], &st, None, &[nreq], &q(&cao, 0.1));
                self.check_err(r, 13);
                let r = nc::put_int(ncid, vid[15], &st, None, &[nreq], &q(&cdir1, 0.1));
                self.check_err(r, 14);
            } else {
                for (v, x, code) in [(vid[11], &dpo, 10), (vid[12], &wao, 11), (vid[13], &udir1, 12), (vid[14], &cao, 13), (vid[15], &cdir1, 14)] {
                    let r = nc::put_real(ncid, v, &st, None, &[nreq], x);
                    self.check_err(r, code);
                }
            }
            if self.ncvartype <= 3 {
                for e in e3.iter_mut() {
                    if *e >= 0. {
                        *e = ounp::log_spec(*e);
                    }
                }
            }
            let r = if self.order {
                nc::put_real(ncid, vid[10], &[1, 1, 1, iout], Some(&[nth, nk, nreq, 1]), &[nth, nk, nreq], &e3)
            } else {
                nc::put_real(ncid, vid[10], &[1, 1, iout, 1], Some(&[nth, nk, 1, nreq]), &[nth, nk, nreq], &e3)
            };
            self.check_err(r, 15);
            return;
        }
        // The full version, for `ITYPE` 2: the stations of `FLREQ` in the order of their indices, one in a file of its own.
        let nopts = self.nopts;
        let (mut j, mut j1) = (if self.together { 1 } else { i }, 1usize);
        let mut last = false;
        while !last {
            if self.flreq[j - 1] {
                let mn = ounp::mean(&self.m, &self.disp, &self.pnt, j - 1);
                let p = &self.pnt;
                let r = nc::put_double(ncid, vid[1], &[iout], outjulday);
                let _ = r;
                let s = [j1, iout];
                let name = trim(&p.ptnme[j - 1]).to_string();
                let _ = nc::put_int(ncid, vid[27], &[j1], None, &[], &[j as i32]);
                let _ = nc::put_text(ncid, vid[2], &[1, j1], &[name.len(), 1], &name);
                let (lon, lat) = (m2km * p.ptloc[j - 1][0], m2km * p.ptloc[j - 1][1]);
                let put = |v: i32, x: Real| {
                    let _ = nc::put_real(ncid, v, &s, None, &[], &[x]);
                };
                put(vid[4], lon);
                put(vid[5], lat);
                match otype {
                    1 => {
                        put(vid[6], p.dpo[j - 1]);
                        put(vid[7], p.cao[j - 1]);
                        put(vid[8], mn.cdir);
                        put(vid[9], p.wao[j - 1]);
                        put(vid[10], mn.udir);
                    }
                    2 => {
                        put(vid[6], mn.hsig);
                        put(vid[7], mn.wlen);
                        put(vid[8], mn.tmean);
                        put(vid[9], mn.thp);
                        put(vid[10], mn.spp);
                        put(vid[11], mn.fp * TPIINV);
                        put(vid[12], mn.thmean);
                        put(vid[13], mn.thsprd);
                    }
                    5 => {
                        let (hmat, ages) = ounp::validation(&self.disp, &mn, p.dpo[j - 1]);
                        let (age1, age2) = ages.unwrap_or((nc::FILL_FLOAT, nc::FILL_FLOAT));
                        put(vid[6], p.wao[j - 1]);
                        put(vid[7], mn.udir);
                        put(vid[8], mn.hsig);
                        put(vid[9], hmat);
                        put(vid[10], age1);
                        put(vid[11], age2);
                        put(vid[12], p.aso[j - 1]);
                    }
                    6 => {
                        put(vid[6], p.wao[j - 1]);
                        put(vid[7], mn.udir);
                        put(vid[8], mn.hsig);
                        put(vid[9], if mn.hsig >= 0.05 { TPI / mn.fp } else { 0.0 });
                    }
                    _ => unreachable!(),
                }
                if self.together {
                    j1 += 1;
                }
            }
            if self.together {
                j += 1;
                if j > nopts {
                    last = true;
                }
            } else {
                last = true;
            }
        }
    }
}

/// `READ (str,*)` of the list of points into `INDREQTMP`; `None` if the first word is `all`.
fn point_list(sink: &mut Sink, list: &str, nopts: usize) -> Vec<usize> {
    let words: Vec<&str> = list.split_whitespace().collect();
    if words.first() == Some(&"all") {
        return (1..=nopts).collect();
    }
    if words.len() > nopts {
        refuse(sink, "POINT%LIST with more words than the file has points (the Fortran reads past its list)");
    }
    let mut seen = vec![false; nopts];
    let mut idx = Vec::new();
    for w in words {
        let Ok(ip) = w.parse::<i64>() else { refuse(sink, &format!("POINT%LIST word {w:?}, which is not an integer")) };
        if ip < 1 {
            refuse(sink, &format!("POINT%LIST index {ip}, below 1 (the Fortran indexes FLREQ out of bounds)"));
        }
        if (ip as usize) <= nopts && idx.len() < nopts {
            if !seen[ip as usize - 1] {
                idx.push(ip as usize);
            }
            seen[ip as usize - 1] = true;
        }
    }
    idx
}

// `startdate` and `nbfileout_last` are set in the loop of section 5.6 before the Fortran reads them in 7.4.
#[allow(unused_assignments)]
pub fn run() -> ! {
    let mut sink = Sink { log: Vec::new() };
    sink.put(BANNER);
    // 2. Model definition file.
    check_grid(PROG, "mod_def.ww3", &mut sink);
    let m = ModDef::read("mod_def.ww3");
    if m.caltype != "standard" {
        refuse(&mut sink, &format!("calendar {:?} in mod_def.ww3 (only standard)", m.caltype));
    }
    sink.put(&format!("  Grid name : {}\n\n", ff::a(30, &m.gname)));
    let m2km: Real = if m.flagll { 1. } else { 1.0e-3 };
    // 3. First time from out_pnt.ww3.
    let mut pnt = Pnt::open("out_pnt.ww3", &m, &mut sink);
    let _ = pnt.read(&m, &mut sink);
    let nopts = pnt.nopts;
    sink.put("  Points in file : \n ------------------------------------\n");
    let mut r = Run {
        m,
        disp: Dispersion::new(),
        pnt,
        sink,
        m2km,
        itype: 0,
        otype: 0,
        nctype: 0,
        ncvartype: 0,
        together: true,
        order: true,
        one: 1,
        two: 2,
        nopts,
        flreq: vec![false; nopts],
        ncfile: vec![String::new(); nopts.max(1)],
        ncid: vec![0; nopts.max(1)],
        varid: vec![[0; 29]; nopts.max(1)],
        iout: 0,
        refdate: [1990, 1, 1, 0, 0, 0, 0, 0],
    };
    for i in 0..nopts {
        r.line_point(i);
    }
    // 4. Requests: the namelist. The old ww3_ounp.inp is not read.
    if !std::path::Path::new(&format!("{FNMPRE}ww3_ounp.nml")).exists() {
        refuse(&mut r.sink, "ww3_ounp.inp without ww3_ounp.nml (only the namelist is read)");
    }
    let req = match Request::read(&format!("{FNMPRE}ww3_ounp.nml")) {
        Ok(q) => q,
        Err(e) => refuse(&mut r.sink, &format!("ww3_ounp.nml: {e}")),
    };
    std::fs::write(format!("{FNMPRE}ww3_ounp.nml.log"), req.log()).unwrap_or_else(|e| panic!("ww3_ounp.nml.log: {e}"));
    let mut dtreq = parse(&mut r.sink, "POINT%TIMESTRIDE", ounp::one_real(&req.timestride));
    let mut nout = parse(&mut r.sink, "POINT%TIMECOUNT", ounp::one_int(&req.timecount));
    let mut tout: Time = parse(&mut r.sink, "POINT%TIMESTART", ounp::two_ints(&req.timestart));
    let nopts_i = nopts;
    let indreq = point_list(&mut r.sink, &req.list, nopts_i);
    let nreq = indreq.len();
    if req.list.split_whitespace().next() == Some("all") {
        r.flreq.iter_mut().for_each(|x| *x = true);
    } else {
        for &p in &indreq {
            r.flreq[p - 1] = true;
        }
    }
    // 4.3 Output type.
    let fileprefix = req.prefix.clone();
    r.nctype = req.netcdf;
    let mut s3 = req.timesplit;
    r.together = req.samefile;
    let mfl = req.buffer;
    r.itype = req.itype;
    r.order = req.dimorder;
    let (mut scale1, mut scale2) = (0., 0.);
    match r.itype {
        1 => {
            r.otype = req.sp_output;
            scale1 = req.sp_scale;
            scale2 = req.sp_outfac;
            r.ncvartype = req.sp_type;
        }
        2 => r.otype = req.par_output,
        3 => r.otype = req.so_output,
        _ => {}
    }
    let _ = (scale1, scale2);
    if std::path::Path::new("NC_globatt.inp").exists() {
        refuse(&mut r.sink, "NC_globatt.inp in the directory (the global attributes it holds are not read)");
    }
    // Forms the translation does not reach, before any file is created.
    let (itype, otype) = (r.itype, r.otype);
    let ext_known = matches!((itype, otype), (1, 2..=4) | (2, _) | (3, 2..=4));
    let implemented = matches!((itype, otype), (1, 3) | (2, 1 | 2 | 5 | 6));
    if ext_known && !implemented {
        let what = match (itype, otype) {
            (1, 2) => "POINT%TYPE 1 with SPECTRA%OUTPUT 2 (1-D spectra tables: W3SPR4, W3SIN4, W3SDS4)".to_string(),
            (1, 4) => "POINT%TYPE 1 with SPECTRA%OUTPUT 4 (partitions: W3PART)".to_string(),
            (2, 3 | 4) => format!("POINT%TYPE 2 with PARAM%OUTPUT {otype} (nondimensional parameters: W3SPR4, W3SIN4, W3SDS4)"),
            (2, o) => format!("POINT%TYPE 2 with PARAM%OUTPUT {o} (the Fortran creates the file and stops with status 30)"),
            (3, _) => "POINT%TYPE 3 (source terms)".to_string(),
            _ => unreachable!(),
        };
        refuse(&mut r.sink, &what);
    }
    if !matches!(r.nctype, 3 | 4) {
        refuse(&mut r.sink, &format!("FILE%NETCDF {} (only 3 and 4)", r.nctype));
    }
    if mfl < 1 {
        refuse(&mut r.sink, "POINT%BUFFER below 1 (the Fortran divides by it)");
    }

    // 4.1 Time setup.
    dtreq = if dtreq > 0. { dtreq } else { 0. };
    if dtreq == 0. {
        nout = 1;
    }
    nout = nout.max(1);
    let noutl = nout;
    let toutl = tout;
    r.sink.put(&format!("\n  Output time data : \n --------------------------------------------------\n      First time         : {}\n", stme21(tout)));
    let mut tdum: Time = [0, 0];
    tick21(&mut tdum, dtreq);
    let mut idtime: Vec<u8> = stme21(tdum).into_bytes();
    let idday = if dtreq >= 86400. { format!("{} ", ff::i(10, (dtreq / 86400.) as i64)) } else { " ".repeat(11) };
    idtime[..11].copy_from_slice(idday.as_bytes());
    idtime[20..23].copy_from_slice(b"   ");
    r.sink.put(&format!("      Interval           : {}\n      Number of requests : {}\n", String::from_utf8_lossy(&idtime), ff::i(8, nout as i64)));

    // 4.1.2 and 4.1.3: the first time that out_pnt.ww3 and the request share.
    let tm = r.pnt.time;
    let mut date = if tout[0] > tm[0] || (tout[0] == tm[0] && tout[1] > tm[1]) { ounp::date14(tout) } else { ounp::date14(tm) };
    loop {
        let dtest = dsec21(r.pnt.time, tout);
        if dtest > 0. {
            if let Rd::Eof = r.pnt.read(&r.m, &mut r.sink) {
                r.sink.put("\n      End of file reached \n\n");
                finish(r.sink);
            }
            continue;
        }
        if dtest < 0. {
            tick21(&mut tout, dtreq);
            continue;
        }
        break;
    }
    let mut pastdate = ounp::date14(r.pnt.time);

    // 4.3 Output type.
    if s3 > 0 && s3 < 4 {
        s3 = 4;
    }
    if s3 > 10 {
        s3 = 10;
    }
    if s3 == 9 {
        refuse(&mut r.sink, "POINT%TIMESPLIT 9 (the Fortran divides by zero)");
    }
    let s5 = s3 - 8;
    let _ = s5;
    let mut filetime = ounp::filetime(r.pnt.time, s3, false);
    let firsttime = filetime.clone();
    (r.one, r.two) = if r.order { (1, 2) } else { (2, 1) };
    if r.nctype == 4 && nc::libvers().contains("\"3.") {
        r.sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3OUNP : \n     NCTYPE=4 IS INCOMPATIBLE WITH\n     NETCDF LIBRARY USED :{}\n\n", nc::libvers()));
        r.sink.exit(44)
    }
    if r.nctype == 3 && !r.order {
        r.sink.put("\n *** WAVEWATCH III ERROR IN W3OUNP : \n     NCTYPE=3 IS INCOMPATIBLE WITH\n     THE OPTIMIZED DIMENSION ORDER\n\n");
        r.sink.exit(43)
    }
    // 4.4 Direction in degrees and the frequencies, as `W3CRNC` computes them.
    let nk = r.m.nk as usize;
    let nth = r.m.nth as usize;
    let dthd: Real = 360. / nth as Real;
    let rth0: Real = r.m.th[0] / r.m.dth;
    let thd: Vec<Real> = (0..nth).map(|ith| dthd * (rth0 + ith as Real)).collect();
    let freq: Vec<Real> = (1..=nk).map(|ik| r.m.sig[ik] * TPIINV).collect();
    let mut freq1: Vec<Real> = freq.iter().map(|&f| f - 0.5 * (f - (f / r.m.xfr))).collect();
    let mut freq2: Vec<Real> = freq.iter().map(|&f| f + 0.5 * (-f + (f * r.m.xfr))).collect();
    freq1[0] = r.m.sig[1] * TPIINV;
    freq2[nk - 1] = r.m.sig[nk] * TPIINV;
    let dir: Vec<Real> = thd.iter().map(|&t| (450. - t) % 360.).collect();

    // 5.1 Number of files per bunch.
    let nfl = if r.together { 1 } else { 1 + nopts as i32 / mfl };
    // 5.2 File name extension.
    let sep = if s3 == 0 { "" } else { "_" };
    let ext = match (itype, otype) {
        (1, 2) | (1, 4) | (2, _) | (3, 2) | (3, 3) => format!("{sep}tab.nc"),
        (1, 3) => format!("{sep}spec.nc"),
        (3, 4) => format!("{sep}src.nc"),
        _ => String::new(),
    };
    if ext.is_empty() {
        r.sink.put("\n *** WAVEWATCH III ERROR IN W3OUNP : \n     ITYPE AND OTYPE COMBINATION NOT RECOGNIZED\n\n");
        r.sink.exit(46)
    }
    // 5.3 netCDF type for many files.
    if r.nctype == 4 && !r.together && nfl > 300 && nreq > 9000 {
        for l in [
            " WARNING : Files will be generated in netCDF3 with NF90_share mode",
            " WARNING : this is due to NF90_sync memory problem with netCDF4 library",
            " WARNING : to convert in netCDF4, use ncks -h -a -4 -L 9 file.nc3 file.nc4",
            " WARNING : or use option \"Points in same file\" with value TRUE in .inp file",
            " WARNING : or limit the output points list to less than 300",
        ] {
            r.sink.put(&format!("{l}\n"));
        }
        r.nctype = 3;
    }
    // 5.4 Periodic flushing: FLOOR of a REAL, saturating as the target's conversion does.
    let ncflush: i32 = (15.0e7f32 / (nk as Real * nth as Real * nreq as Real / nfl as Real)).floor() as i32;
    if r.nctype == 3 && nreq > 10 && !r.together {
        r.sink.put(&format!("      Buffer will be flushed every {} steps.\n", ff::i(6, ncflush as i64)));
    }
    // 5.5 Duplicates in the one-file-per-station mode.
    let mut ncname = String::new();
    if !r.together {
        let mut set: Vec<Option<String>> = vec![None; nopts];
        for i in 1..=nopts {
            if r.flreq[i - 1] {
                ncname = format!("{}{}_{}{}", trim(&fileprefix), trim(&r.pnt.ptnme[i - 1]), trim(&filetime), ext);
                r.ncfile[i - 1] = format!("{FNMPRE}{ncname}");
                set[i - 1] = Some(r.ncfile[i - 1].clone());
                let n = set.iter().filter(|s| s.as_deref() == Some(r.ncfile[i - 1].as_str())).count();
                if n > 1 {
                    r.flreq[i - 1] = false;
                    let nm = trim(&r.pnt.ptnme[i - 1]).to_string();
                    r.sink.put(&format!("      Point already exists, it will be skipped : {nm}\n"));
                }
            }
        }
    }

    // 5.6 Bunches of stations.
    let mut ifl = 1;
    let mut startdate = String::new();
    let mut nbfileout_last = 0usize;
    'ifl: while ifl <= nfl {
        'file: loop {
            // Label 560: a new file, so the time counter starts again.
            r.iout = 0;
            // 5.6.1 The date in the name.
            if SINGLEFILE {
                filetime = firsttime.clone();
            } else {
                filetime = ounp::filetime(r.pnt.time, s3, true);
            }
            // 5.6.2 The names.
            if r.together {
                ncname = format!("{}{}{}", trim(&fileprefix), trim(&filetime), ext);
                r.ncfile[0] = format!("{FNMPRE}{ncname}");
            } else {
                for i in 1..=nopts {
                    if r.flreq[i - 1] {
                        ncname = format!("{}{}_{}{}", trim(&fileprefix), trim(&r.pnt.ptnme[i - 1]), trim(&filetime), ext);
                        r.ncfile[i - 1] = format!("{FNMPRE}{ncname}");
                    }
                }
            }
            // 5.6.3 Files and stations to create.
            let (nbfileout, nbstation, nreql): (usize, usize, usize);
            let base = (ifl as usize - 1) * mfl as usize;
            if r.together {
                nbfileout = 1;
                nbstation = nreq;
                nreql = nbfileout;
            } else {
                // NBFILEOUT is zero (or negative) in the last bunch when BUFFER divides NOPTS: no station, no file.
                nbfileout = (mfl as i64).min(nopts as i64 - base as i64).max(0) as usize;
                nbstation = 1;
                nreql = (base + 1..=base + nbfileout).filter(|&i| r.flreq[i - 1]).count();
            }
            if nreql == 0 {
                ifl += 1;
                continue 'ifl;
            }
            nbfileout_last = nbfileout;
            // 5.6.4 Creates the files.
            let who = match itype {
                1 => "\n  Output type  1 :\n --------------------------------------------------\n      1-D and/or 2-D spectra, pass #".to_string() + &format!("{}\n\n      Subtype   : Transfer file\n", ff::i(3, ifl as i64)),
                _ => "\n  Output type  2 :\n --------------------------------------------------\n      Table of mean wave parameters\n".to_string(),
            };
            r.sink.put(&who);
            for i in base + 1..=base + nbfileout {
                if r.flreq[i - 1] || r.together {
                    let file = r.ncfile[i - 1].clone();
                    let c = Crnc {
                        itype: r.itype,
                        otype: r.otype,
                        nctype: r.nctype,
                        ncfile: &file,
                        dimln: [0, 0, nbstation, 40, nk, nth],
                        one: r.one,
                        two: r.two,
                                        ncvartype: r.ncvartype,
                                        flww3: 0,
                        flagll: r.m.flagll,
                        caltype: "standard",
                        freq: &freq,
                        freq1: &freq1,
                        freq2: &freq2,
                        dir: &dir,
                    };
                    // CHECK_ERR stops at the first failure, before the next library call.
                    let (id, vids) = crnc::w3crnc(&c, &mut |iret, code| r.check_err(iret, code));
                    r.ncid[i - 1] = id;
                    r.varid[i - 1] = vids;
                }
            }
            if itype == 2 {
                let what = match otype {
                    1 => "depth, current and wind",
                    2 => "Mean wave parameters",
                    5 => "Validation parameters",
                    6 => "WMO standard mean parameters",
                    _ => unreachable!(),
                };
                r.sink.put(&format!("      Table output : {what}\n      File name    : {}\n", ff::a(128, &ncname)));
                r.sink.put("  \n");
            }
            // 5.6.5 Output of the output points.
            r.sink.put(&format!("\n  Requested output for {} points : \n --------------------------------------------------\n", ff::i(6, nreq as i64)));
            if r.together {
                for i in base + 1..=base + nbstation {
                    if i <= nopts && r.flreq[i - 1] {
                        r.line_point(i - 1);
                    }
                }
            } else {
                for i in base + 1..=base + nbfileout {
                    if r.flreq[i - 1] {
                        r.line_point(i - 1);
                    }
                }
            }
            // 6. Time management.
            startdate = ounp::datestr(&t2d(r.pnt.time));
            loop {
                let dtest = dsec21(r.pnt.time, tout);
                if dtest > 0. {
                    if let Rd::Eof = r.pnt.read(&r.m, &mut r.sink) {
                        r.sink.put("\n      End of file reached \n\n");
                        break;
                    }
                    continue;
                }
                if dtest < 0. {
                    tick21(&mut tout, dtreq);
                    continue;
                }
                r.iout += 1;
                date = ounp::date14(tout);
                // 6.1 A new file when the date of the name changes.
                if !SINGLEFILE && r.iout > 1 && ounp::date_differs(&pastdate, &date, s3) {
                    let n = if s3 > 0 { &date[..s3 as usize] } else { "" };
                    r.sink.put(&format!("\n      New time step      : {n}\n"));
                    nout -= r.iout - 1;
                    break;
                }
                // 6.2 Progress.
                if nreq > 10 || nbfileout > 10 {
                    let t = r.pnt.time;
                    r.sink.put(&format!(
                        "  Processing time : {}{} in {}files. Step {}out of {} pass {}\n",
                        ff::i(8, t[0] as i64),
                        ff::i(8, t[1] as i64),
                        ff::i(8, nbfileout as i64),
                        ff::i(10, r.iout as i64),
                        ff::i(10, nout as i64),
                        ff::i(4, ifl as i64)
                    ));
                }
                // 6.3 One call of W3EXNC per file.
                for i in base + 1..=base + nbfileout {
                    if r.flreq[i - 1] || r.together {
                        if r.together {
                            let ind = indreq.clone();
                            r.w3exnc(i, &ind);
                        } else {
                            r.w3exnc(i, &[i]);
                            let flush = if ncflush == 0 { r.iout } else { r.iout % ncflush };
                            if flush == 0 {
                                let _ = nc::sync(r.ncid[i - 1]);
                            }
                        }
                    }
                }
                pastdate = ounp::date14(tout);
                tick21(&mut tout, dtreq);
                if r.iout >= nout {
                    break;
                }
            }
            // 7. Finalizes the files.
            finalize(&mut r, ifl, mfl, nbfileout, &startdate, dtreq);
            // 7.2 Back to the start of the bunch with a new date.
            if r.iout > 1 && ounp::date_differs(&pastdate, &date, s3) {
                continue 'file;
            }
            break 'file;
        }
        // 7.3 Reopens out_pnt.ww3 and starts the times again.
        r.pnt.reopen(&r.m, &mut r.sink);
        let _ = r.pnt.read(&r.m, &mut r.sink);
        tout = toutl;
        nout = noutl;
        // 7.4
        loop {
            let dtest = dsec21(r.pnt.time, tout);
            if dtest > 0. {
                if let Rd::Eof = r.pnt.read(&r.m, &mut r.sink) {
                    r.sink.put("\n      End of file reached \n\n");
                    // GOTO 700: the files are finalized again, which the library refuses.
                    finalize(&mut r, ifl, mfl, nbfileout_last, &startdate, dtreq);
                    break;
                }
                continue;
            }
            if dtest < 0. {
                tick21(&mut tout, dtreq);
                continue;
            }
            break;
        }
        ifl += 1;
    }
    finish(r.sink)
}

/// Label 888.
fn finish(mut sink: Sink) -> ! {
    sink.put(END999);
    sink.exit(0)
}

/// Section 7.1 (`ww3_ounp.F90:1145-1213`): the global attributes, then the file is closed.
fn finalize(r: &mut Run, ifl: i32, mfl: i32, nbfileout: usize, startdate: &str, dtreq: Real) {
    let stopdate = ounp::datestr(&t2d(r.pnt.time));
    let base = (ifl as usize - 1) * mfl as usize;
    for i in base + 1..=base + nbfileout {
        if r.flreq[i - 1] || r.together {
            let ncid = r.ncid[i - 1];
            let iret = nc::redef(ncid);
            r.check_err(iret, 0);
            let g = nc::NC_GLOBAL;
            let file = r.ncfile[i - 1].clone();
            let prod = if file.len() > 2 { &file[2..] } else { "" };
            let _ = nc::put_att_text(ncid, g, "product_name", prod);
            let _ = nc::put_att_text(ncid, g, "area", trim(&r.m.gname));
            for (k, v) in [
                ("data_type", "OCO spectra 2D"),
                ("format_version", "1.1"),
                ("southernmost_latitude", "n/a"),
                ("northernmost_latitude", "n/a"),
                ("latitude_resolution", "n/a"),
                ("westernmost_longitude", "n/a"),
                ("easternmost_longitude", "n/a"),
                ("longitude_resolution", "n/a"),
                ("minimum_altitude", "n/a"),
                ("maximum_altitude", "n/a"),
                ("altitude_resolution", "n/a"),
                ("start_date", startdate),
                ("stop_date", stopdate.as_str()),
            ] {
                let _ = nc::put_att_text(ncid, g, k, v);
            }
            let ft = if dtreq == 3600. {
                "hourly"
            } else if dtreq == 7200. {
                "2-hourly"
            } else if dtreq == 10800. {
                "3-hourly"
            } else if dtreq == 21600. {
                "6-hourly"
            } else if dtreq == 32400. {
                "9-hourly"
            } else if dtreq == 43200. {
                "12-hourly"
            } else if dtreq == 86400. {
                "daily"
            } else {
                "n/a"
            };
            let _ = nc::put_att_text(ncid, g, "field_type", ft);
            let iret = nc::enddef(ncid);
            r.check_err(iret, 2);
            let iret = nc::close(ncid);
            r.check_err(iret, 3);
        }
    }
}
