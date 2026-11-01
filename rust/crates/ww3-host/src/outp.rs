// SPDX-License-Identifier: MPL-2.0
//! `ww3_outp` in Rust (`model/src/ww3_outp.F90`, 2848 lines): the point output postprocessor, for
//! `switches/switch_chaceon_shrd` at `92e2cf66`. It reads `ww3_outp.inp`, `mod_def.ww3` and `out_pnt.ww3` from
//! a directory and writes the log to standard output and `tabNN.ww3` or `ww3.YYMMDDHH.spc` into the directory.
//!
//! Reached: `ITYPE` 0 (contents), 1 with `OTYPE` 1 (print plots) and 3 (spectra transfer file, formatted and
//! unformatted), 2 with `OTYPE` 1, 2, 5, 6 (tables). Refused with a message and exit 64, never skipped:
//! `ITYPE` 1 `OTYPE` 2 and `ITYPE` 2 `OTYPE` 3, 4 (they call `W3SPR4`, `W3SIN4`), `ITYPE` 3 (source terms) and
//! `ITYPE` 4 (partitions and bulletins, `W3PART`, `W3BULL`).

use crate::disp::Dispersion;
use crate::finp::{self, Inp, ReadErr};
use crate::fortfmt as ff;
use crate::moddef::{ModDef, Records};
use crate::wtime::{dsec21, tick21, Time};
use std::io::Write;
use ww3_kernels::constants::{DERA, GRAV, PI, RADE, RADIUS, TPI, TPIINV};
use ww3_kernels::nint;
use ww3_kernels::real::Real;
use ww3_libm::{atan2f, sqrtf};

const UNDEF: Real = -999.9;
/// `IDSTR` and `VEROPT` of `out_pnt.ww3` (`w3iopomd.F90`, as `shel.rs` writes them).
const IDSTR: &str = "WAVEWATCH III POINT OUTPUT FILE";
const VEROPT: &str = "2021-04-06";

fn f(w: usize, d: usize, x: Real) -> String {
    ff::f(w, d, x as f64)
}
fn e(w: usize, d: usize, x: Real) -> String {
    ff::e(w, d, x as f64)
}

/// `STME21`.
pub fn stme21(t: Time) -> String {
    if t[0] < 0 {
        return " date and time not set.".to_string();
    }
    format!(
        "{}/{}/{} {}:{}:{} UTC",
        ff::im(4, 4, (t[0] / 10000) as i64),
        ff::im(2, 2, (t[0] % 10000 / 100) as i64),
        ff::im(2, 2, (t[0] % 100) as i64),
        ff::im(2, 2, (t[1] / 10000) as i64),
        ff::im(2, 2, (t[1] % 10000 / 100) as i64),
        ff::im(2, 2, (t[1] % 100) as i64),
    )
}

/// `GTYPE` of `mod_def.ww3`: `ModDef` reads rectilinear and curvilinear grids only, and a program says so
/// instead of stopping in the reader.
pub fn check_grid(prog: &str, path: &str, sink: &mut Sink) {
    let mut r = Records::open(path);
    r.next();
    r.next();
    let gtype = r.next().i();
    if gtype != 1 && gtype != 2 {
        sink.refuse(prog, &format!("GTYPE {gtype} in mod_def.ww3 (only rectilinear and curvilinear grids are read)"));
    }
}

/// The standard output and the files a program opens. `exit` flushes what was written first.
pub struct Sink {
    pub log: Vec<u8>,
}

impl Sink {
    pub fn put(&mut self, s: &str) {
        self.log.extend_from_slice(s.as_bytes());
    }
    pub fn flush(&mut self) {
        let mut o = std::io::stdout().lock();
        o.write_all(&self.log).unwrap();
        o.flush().unwrap();
        self.log.clear();
    }
    /// A Fortran `EXTCDE`: the program ends with this code.
    pub fn exit(&mut self, code: i32) -> ! {
        self.flush();
        std::process::exit(code)
    }
    /// An option this port does not reach: the log so far, a message on standard error, exit 64.
    pub fn refuse(&mut self, prog: &str, what: &str) -> ! {
        self.flush();
        eprintln!("{prog}: not implemented: {what}");
        std::process::exit(64)
    }
}

/// One `W3IOPO` read: the points' state at `time`.
struct Pnt {
    rec: Records,
    time: Time,
    nopts: usize,
    ptloc: Vec<[Real; 2]>,
    ptnme: Vec<String>,
    dpo: Vec<Real>,
    wao: Vec<Real>,
    wdo: Vec<Real>,
    aso: Vec<Real>,
    cao: Vec<Real>,
    cdo: Vec<Real>,
    spco: Vec<Vec<Real>>,
}

/// What `W3IOPO ('READ')` reports.
enum Rd {
    Ok,
    /// `IOTEST = -1`.
    Eof,
}

impl Pnt {
    fn open(path: &str, m: &ModDef, log: &mut Sink) -> Self {
        let mut rec = Records::open(path);
        let (nopts, ptloc, ptnme);
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
        {
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
            ptloc = loc;
            ptnme = nm;
        }
        let z = |_| vec![0.; nopts];
        Self {
            rec,
            time: [0, 0],
            nopts,
            ptloc,
            ptnme,
            dpo: z(0),
            wao: z(0),
            wdo: z(0),
            aso: z(0),
            cao: z(0),
            cdo: z(0),
            spco: vec![Vec::new(); nopts],
        }
    }

    /// The time and the points' records of the next output time (`w3iopomd.F90:1346-1394`).
    fn read(&mut self, m: &ModDef, log: &mut Sink) -> Rd {
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

/// A formatted table file or a transfer file.
struct Tab {
    w: std::io::BufWriter<std::fs::File>,
}

impl Tab {
    fn create(path: &str) -> Option<Self> {
        std::fs::File::create(path).ok().map(|f| Self { w: std::io::BufWriter::new(f) })
    }
    fn put(&mut self, s: &str) {
        self.w.write_all(s.as_bytes()).unwrap();
    }
    /// A formatted record: the text and its line end.
    fn line(&mut self, s: &str) {
        self.put(s);
        self.put("\n");
    }
    /// An unformatted sequential record, big-endian markers as the reference's build writes them.
    fn rec(&mut self, b: &[u8]) {
        let n = (b.len() as i32).to_be_bytes();
        self.w.write_all(&n).unwrap();
        self.w.write_all(b).unwrap();
        self.w.write_all(&n).unwrap();
    }
}

/// `n` items per record, each already formatted: a Fortran format `(nEw.d)` with more items than `n` starts a
/// new record at each reversion.
fn rows(items: &[String], per: usize) -> String {
    items.chunks(per).map(|c| c.concat()).collect::<Vec<_>>().join("\n")
}

fn be_f(v: &mut Vec<u8>, x: Real) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// `x**5` as GCC's power tree expands it (the same tree `ww3_kernels` uses).
fn pow5(x: Real) -> Real {
    let x2 = x * x;
    let x3 = x2 * x;
    x3 * x2
}

/// Fortran `SIGN(A, B)`.
fn sign(a: Real, b: Real) -> Real {
    f32::from_bits((a.to_bits() & 0x7fffffff) | (b.to_bits() & 0x80000000))
}

fn modr(a: Real, p: Real) -> Real {
    a % p
}

// Format statements 1901-1916 of `ww3_outp.F90:2664-2746`, generated from the source text.
const HDR1901: &str = "    Date     Time        d     Uc     Dir.  U10    Dir. \n            h  m  s     (m)   (m/s)  (d.N) (m/s)  (d.N) \n ---------------------------------------------------------";
const HDR1902: &str = "    Date     Time        Hs     L      Tr    Dir.  Spr.     fp    p_dir  p_spr\n            h  m  s     (m)    (m)    (s)   (d.N)  (deg)   (Hz)   (d.N)  (deg)\n ------------------------------------------------------------------------------";
const HDR1905: &str = "   Date     Time     U10    Dir.    Hs      H*    cp/U    cm/U     Dt\n                    (m/s)  (d.N)   (m)     (-)    (-)      (-)   (deg)\n -----------------------------------------------------------------------";
const HDR1906: &str = "     Time      U10    Dir. Hs  Tp  \n  yr mth dy h  (m/s) (d.N) (m) (s) \n ----------------------------------";
const HDR1911: &str = " Time : {dt}\n\n    Long.    Lat.       d     Uc     Dir.  U10    Dir. \n                       (m)   (m/s)  (d.N) (m/s)  (d.N) \n --------------------------------------------------------";
const HDR1912: &str = " Time : {dt}\n\n    Long.    Lat.       Hs     L      Tr    Dir.  Spr.     fp    p_dir  p_spr\n                       (m)    (m)    (s)   (d.N)  (deg)   (Hz)   (d.N)  (deg)\n -----------------------------------------------------------------------------";
const HDR1711: &str = " Time : {dt}\n\n       X        Y          d     Uc     Dir.  U10    Dir. \n      (m)      (m)        (m)   (m/s)  (d.N) (m/s)  (d.N) \n ----------------------------------------------------------";
const HDR1712: &str = " Time : {dt}\n\n       X        Y          Hs     L      Tr    Dir.  Spr.     fp    p_dir  p_spr\n      (m)      (m))       (m)    (m)    (s)   (d.N)  (deg)   (Hz)   (d.N)  (deg)\n -------------------------------------------------------------------------------";
const HDR1915: &str = " Time : {dt}\n\n     Long.    Lat.   U10    Dir.    Hs      H*    cp/U    cm/U     Dt\n                    (m/s)  (d.N)   (m)     (-)    (-)      (-)   (deg)\n ----------------------------------------------------------------------";
const HDR1715: &str = " Time : {dt}\n\n       X        Y      U10    Dir.    Hs      H*    cp/U    cm/U     Dt\n      (m)      (m)    (m/s)  (d.N)   (m)     (-)    (-)      (-)   (deg)\n ------------------------------------------------------------------------";
const HDR1916: &str = " Time : {dt}\n\n   Long.     Lat.   U10   Dir.     Hs      Tp  \n                 (m/s)   (d.N)    (m)     (s)  \n-----------------------------------------------";
const HDR1716: &str = " Time : {dt}\n\n     X      Y      U10      Dir.      Hs       Tp  \n    (m)    (m)    (m/s)    (d.N)     (m)      (s)  \n---------------------------------------------------";

struct Prog<'a> {
    m: &'a ModDef,
    disp: Dispersion,
    pnt: Pnt,
    sink: Sink,
    m2km: Real,
    itype: i32,
    otype: i32,
    scale1: Real,
    scale2: Real,
    flform: bool,
    nreq: i32,
    flreq: Vec<bool>,
    tab: Option<Tab>,
    ipass: i32,
}

/// The Fortran of `ww3_outp.F90:249-1011`.
pub fn run(dir: &str) -> i32 {
    let mut sink = Sink { log: Vec::new() };
    sink.put("\n                   *** WAVEWATCH III Point output post.***    \n               ===============================================\n\n");
    let mut inp = match Inp::open(&format!("{dir}/ww3_outp.inp")) {
        Ok(i) => i,
        Err(_) => {
            sink.put("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ERROR IN OPENING INPUT FILE\n     IOSTAT =    2\n\n");
            sink.exit(40)
        }
    };
    let comstr = match inp.comment_char() {
        Ok(c) => c,
        Err(_) => premature(&mut sink),
    };
    sink.put(&format!("  Comment character is '{comstr}'\n\n"));

    check_grid("ww3_outp", &format!("{dir}/mod_def.ww3"), &mut sink);
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    sink.put(&format!("  Grid name : {}\n\n", ff::a(30, &m.gname)));
    let m2km: Real = if m.flagll { 1. } else { 1.0e-3 };

    let pnt = Pnt::open(&format!("{dir}/out_pnt.ww3"), &m, &mut sink);
    let mut pr = Prog {
        m: &m,
        disp: Dispersion::new(),
        pnt,
        sink,
        m2km,
        itype: 0,
        otype: 0,
        scale1: 0.,
        scale2: 0.,
        flform: false,
        nreq: 0,
        flreq: Vec::new(),
        tab: None,
        ipass: 0,
    };
    // W3IOPO ('READ') of section 3 reads the header and the first output time.
    let first = pr.pnt.read(&m, &mut pr.sink);
    let nopts = pr.pnt.nopts;
    pr.sink.put("  Points in file : \n ------------------------------------\n");
    for i in 0..nopts {
        let (x, y) = (m2km * pr.pnt.ptloc[i][0], m2km * pr.pnt.ptloc[i][1]);
        let name = ff::a(40, &pr.pnt.ptnme[i]);
        if m.flagll {
            pr.sink.put(&format!("      {name}{}{}\n", f(10, 2, x), f(10, 2, y)));
        } else {
            pr.sink.put(&format!("      {name}{}E3{}E3\n", f(8, 1, x), f(8, 1, y)));
        }
    }

    // 4. Requests.
    if inp.nextln().is_err() {
        premature(&mut pr.sink)
    }
    let l = inp.list(4).unwrap_or_else(|e| read_err(&mut pr.sink, e));
    let mut tout: Time = [rd_i(&mut pr.sink, &l[0], 0), rd_i(&mut pr.sink, &l[1], 0)];
    let mut dtreq = rd_r(&mut pr.sink, &l[2], 0.);
    let mut nout = rd_i(&mut pr.sink, &l[3], 0);
    dtreq = if dtreq > 0. { dtreq } else { 0. };
    if dtreq == 0. {
        nout = 1;
    }
    nout = nout.max(1);
    pr.sink.put(&format!("\n  Output time data : \n --------------------------------------------------\n      First time         : {}\n", stme21(tout)));
    let mut tdum: Time = [0, 0];
    tick21(&mut tdum, dtreq);
    let mut idtime: Vec<u8> = stme21(tdum).into_bytes();
    let idday = if dtreq >= 86400. { format!("{}{}", ff::i(10, (dtreq / 86400.) as i64), " ") } else { " ".repeat(11) };
    idtime[..11].copy_from_slice(idday.as_bytes());
    idtime[20..23].copy_from_slice(b"   ");
    pr.sink.put(&format!(
        "      Interval           : {}\n      Number of requests : {}\n",
        String::from_utf8_lossy(&idtime),
        ff::i(6, nout as i64)
    ));

    // Output points.
    pr.flreq = vec![false; nopts];
    pr.nreq = 0;
    let mut ipoint = 0;
    for i in 1..=nopts {
        if inp.nextln().is_err() {
            premature(&mut pr.sink)
        }
        let l = inp.list(1).unwrap_or_else(|e| read_err(&mut pr.sink, e));
        ipoint = rd_i(&mut pr.sink, &l[0], ipoint);
        if ipoint < 0 {
            if i == 1 {
                pr.flreq.iter_mut().for_each(|x| *x = true);
                pr.nreq = nopts as i32;
            }
            break;
        }
        if ipoint > 0 && (ipoint as usize) <= nopts {
            if !pr.flreq[ipoint as usize - 1] {
                pr.nreq += 1;
            }
            pr.flreq[ipoint as usize - 1] = true;
        }
        if ipoint > 0 && pr.nreq as usize == nopts {
            if inp.nextln().is_err() {
                premature(&mut pr.sink)
            }
            let l = inp.list(1).unwrap_or_else(|e| read_err(&mut pr.sink, e));
            ipoint = rd_i(&mut pr.sink, &l[0], ipoint);
        }
    }
    if ipoint != -1 {
        pr.sink.put("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ERROR IN READING FROM INPUT FILE\n     LAST POINT INDEX IS NOT -1\n     OR TOO MANY POINT INDEXES DEFINED\n\n");
        pr.sink.exit(47)
    }

    // Output type.
    if inp.nextln().is_err() {
        premature(&mut pr.sink)
    }
    let l = inp.list(1).unwrap_or_else(|e| read_err(&mut pr.sink, e));
    pr.itype = rd_i(&mut pr.sink, &l[0], 0);
    let itype = pr.itype;
    let head = |n: i32, s: &str| format!("\n  Output type {} :\n --------------------------------------------------\n      {s}\n\n", ff::i(2, n as i64));
    let tabnme = |n: i32| -> (String, i32) {
        let n = if !(1..=99).contains(&n) { 51 } else { n };
        (format!("tab{}.ww3", ff::im(2, 2, n as i64)), n)
    };
    let mut ndstab;
    let open_tab = |pr: &mut Prog, name: &str, n: i32| -> Tab {
        // A unit number the program holds for its own files would be closed by the OPEN.
        if matches!(n, 5 | 6 | 10 | 20) {
            pr.sink.refuse("ww3_outp", &format!("table unit {n}, which the program already holds open (OPEN would close it)"));
        }
        match Tab::create(&format!("{dir}/{name}")) {
            Some(t) => t,
            None => {
                pr.sink.put("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ERROR IN OPENING TABLE FILE\n     IOSTAT =    2\n\n");
                pr.sink.exit(43)
            }
        }
    };
    match itype {
        0 => {
            pr.sink.put(&head(itype, "Checking contents of file"));
            loop {
                pr.sink.put(&format!("      Data for {}\n", stme21(pr.pnt.time)));
                if let Rd::Eof = pr.pnt.read(&m, &mut pr.sink) {
                    pr.sink.put("\n      End of file reached \n\n");
                    break;
                }
            }
            return finish(pr);
        }
        1 => {
            pr.sink.put(&head(itype, "1-D and/or 2-D spectra"));
            if inp.nextln().is_err() {
                premature(&mut pr.sink)
            }
            let l = inp.list(5).unwrap_or_else(|e| read_err(&mut pr.sink, e));
            pr.otype = rd_i(&mut pr.sink, &l[0], 0);
            pr.scale1 = rd_r(&mut pr.sink, &l[1], 0.);
            pr.scale2 = rd_r(&mut pr.sink, &l[2], 0.);
            ndstab = rd_i(&mut pr.sink, &l[3], 0);
            pr.flform = rd_l(&mut pr.sink, &l[4], false);
            match pr.otype {
                1 => {
                    pr.sink.put("      Subtype   : print plots\n");
                    for (s, n) in [(pr.scale1, "1-D"), (pr.scale2, "2-D")] {
                        if s < 0. {
                            pr.sink.put(&format!("      {n} print plots not requested.\n"));
                        } else if s == 0. {
                            pr.sink.put(&format!("      {n} print plots normalized.\n"));
                        } else {
                            pr.sink.put(&format!("      Scale factor {n} spectrum : {}\n", e(10, 3, s)));
                        }
                    }
                }
                2 => pr.sink.refuse("ww3_outp", "ITYPE 1 OTYPE 2 (nondimensional spectral table: W3SPR4, W3SIN4, W3FLX of the ST4 switch)"),
                3 => {
                    let mut tf = *b"ww3.--------.spc";
                    let name = format!("{}{}", ff::im(6, 6, (tout[0] % 1000000) as i64), ff::im(2, 2, (tout[1] / 10000) as i64));
                    tf[4..12].copy_from_slice(name.as_bytes());
                    let tf = String::from_utf8_lossy(&tf).into_owned();
                    pr.sink.put("      Subtype   : Transfer file\n");
                    let kind = if pr.flform { "UNFORMATTED" } else { "FORMATTED" };
                    pr.sink.put(&format!("      File name : {tf} ({kind})\n"));
                    let (ndstab_, _) = (ndstab, 0);
                    if matches!(ndstab_, 5 | 6 | 10 | 20) {
                        pr.sink.refuse("ww3_outp", &format!("transfer unit {ndstab_}, which the program already holds open"));
                    }
                    let mut t = match Tab::create(&format!("{dir}/{tf}")) {
                        Some(t) => t,
                        None => {
                            pr.sink.put("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ERROR IN OPENING IDL FILE\n     IOSTAT =    2\n\n");
                            pr.sink.exit(44)
                        }
                    };
                    let gname = ff::a(30, &m.gname);
                    let (nk, nth) = (m.nk as usize, m.nth as usize);
                    let freqs: Vec<Real> = (1..=nk).map(|ik| m.sig[ik] * TPIINV).collect();
                    let dirs: Vec<Real> = (1..=nth).map(|ith| modr(2.5 * PI - m.th[ith - 1], TPI)).collect();
                    if pr.flform {
                        let mut b = b"WAVEWATCH III SPECTRA".to_vec();
                        for v in [m.nk, m.nth, pr.nreq] {
                            b.extend_from_slice(&v.to_be_bytes());
                        }
                        b.extend_from_slice(gname.as_bytes());
                        t.rec(&b);
                        let mut b = Vec::new();
                        freqs.iter().for_each(|&x| be_f(&mut b, x));
                        t.rec(&b);
                        let mut b = Vec::new();
                        dirs.iter().for_each(|&x| be_f(&mut b, x));
                        t.rec(&b);
                    } else {
                        t.line(&format!("'WAVEWATCH III SPECTRA' {}{}{} '{gname}'", ff::i(6, m.nk as i64), ff::i(6, m.nth as i64), ff::i(6, pr.nreq as i64)));
                        let it: Vec<String> = freqs.iter().map(|&x| e(10, 3, x)).collect();
                        t.line(&rows(&it, 8));
                        let it: Vec<String> = dirs.iter().map(|&x| e(11, 3, x)).collect();
                        t.line(&rows(&it, 7));
                    }
                    pr.tab = Some(t);
                }
                o => {
                    pr.sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ILLEGAL TYPE, OTYPE ={}\n\n", ff::i(4, o as i64)));
                    pr.sink.exit(10)
                }
            }
        }
        2 => {
            pr.sink.put(&head(itype, "Table of mean wave parameters"));
            if inp.nextln().is_err() {
                premature(&mut pr.sink)
            }
            let l = inp.list(2).unwrap_or_else(|e| read_err(&mut pr.sink, e));
            pr.otype = rd_i(&mut pr.sink, &l[0], 0);
            ndstab = rd_i(&mut pr.sink, &l[1], 0);
            let (name, n) = tabnme(ndstab);
            ndstab = n;
            let t = open_tab(&mut pr, &name, ndstab);
            pr.tab = Some(t);
            let what = match pr.otype {
                1 => "Depth, current and wind",
                2 => "Mean wave parameters",
                3 => "Nondimensional parameters (U*)",
                4 => "Nondimensional parameters (U10)",
                5 => "Validation parameters",
                6 => "WMO standard mean parameters",
                o => {
                    pr.sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ILLEGAL TYPE, OTYPE ={}\n\n", ff::i(4, o as i64)));
                    pr.sink.exit(20)
                }
            };
            if pr.otype == 3 || pr.otype == 4 {
                pr.sink.refuse("ww3_outp", "ITYPE 2 OTYPE 3, 4 (nondimensional table: W3SPR4, W3SIN4, W3FLX of the ST4 switch)");
            }
            pr.sink.put(&format!("      Table output : {what}\n      File name    : {name}\n"));
        }
        3 => {
            pr.sink.put(&head(itype, "Source terms"));
            pr.sink.refuse("ww3_outp", "ITYPE 3 (source terms: W3SPR4, W3SIN4, W3SDS4, W3SNL1, W3SBT1, W3SDB1 on a point; the Rust kernels are written for W3SRCE's arrays)");
        }
        4 => {
            pr.sink.put(&head(itype, "Spectral partitions or bulletins"));
            pr.sink.refuse("ww3_outp", "ITYPE 4 (W3PART partitions and W3BULL bulletins)");
        }
        n => {
            pr.sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     ILLEGAL TYPE, ITYPE ={}\n\n", ff::i(4, n as i64)));
            pr.sink.exit(1)
        }
    }
    let _ = ndstab;

    // Output of output points.
    pr.sink.put(&format!(
        "\n  Requested output for{} points : \n --------------------------------------------------\n",
        ff::i(3, pr.nreq as i64)
    ));
    for i in 0..nopts {
        if pr.flreq[i] {
            let (x, y) = (m2km * pr.pnt.ptloc[i][0], m2km * pr.pnt.ptloc[i][1]);
            let name = ff::a(40, &pr.pnt.ptnme[i]);
            if m.flagll {
                pr.sink.put(&format!("      {name}{}{}\n", f(10, 2, x), f(10, 2, y)));
            } else {
                pr.sink.put(&format!("      {name}{}E3{}E3\n", f(8, 1, x), f(8, 1, y)));
            }
        }
    }

    // 5. Time management.
    let mut iout = 0;
    if let Rd::Eof = first {
        // The first W3IOPO read already met the end of the file: DSEC21 sees the TIME the module holds.
    }
    loop {
        let dtest = dsec21(pr.pnt.time, tout);
        if dtest > 0. {
            match pr.pnt.read(&m, &mut pr.sink) {
                Rd::Eof => {
                    pr.sink.put("\n      End of file reached \n\n");
                    break;
                }
                Rd::Ok => {}
            }
            continue;
        }
        if dtest < 0. {
            tick21(&mut tout, dtreq);
            continue;
        }
        iout += 1;
        if itype == 1 && pr.otype == 1 {
            pr.sink.put(&format!("\n\n  Output for {}\n --------------------------------------------------\n", stme21(tout)));
        }
        pr.expo();
        tick21(&mut tout, dtreq);
        if iout >= nout {
            break;
        }
    }
    finish(pr)
}

fn finish(mut pr: Prog) -> i32 {
    if let Some(t) = pr.tab.as_mut() {
        t.w.flush().unwrap();
    }
    pr.sink.put("\n  End of program \n =========================================\n         WAVEWATCH III Point output \n\n");
    pr.sink.flush();
    0
}

fn premature(sink: &mut Sink) -> ! {
    sink.put("\n *** WAVEWATCH III ERROR IN W3OUTP : \n     PREMATURE END OF INPUT FILE\n\n");
    sink.exit(41)
}

fn read_err(sink: &mut Sink, e: ReadErr) -> ! {
    match e {
        ReadErr::End => premature(sink),
        ReadErr::Bad(s) => sink.refuse("ww3_outp", &format!("an input item libgfortran would reject with an IOSTAT this port does not reproduce: {s:?}")),
    }
}

fn rd_i(sink: &mut Sink, t: &Option<String>, old: i32) -> i32 {
    finp::int(t, old).unwrap_or_else(|e| read_err(sink, e))
}
fn rd_r(sink: &mut Sink, t: &Option<String>, old: Real) -> Real {
    finp::real(t, old).unwrap_or_else(|e| read_err(sink, e))
}
fn rd_l(sink: &mut Sink, t: &Option<String>, old: bool) -> bool {
    finp::logical(t, old).unwrap_or_else(|e| read_err(sink, e))
}

impl Prog<'_> {
    /// `W3EXPO` (`ww3_outp.F90:1029-2844`), the branches listed at the top of this file.
    fn expo(&mut self) {
        const HSMIN: Real = 0.05;
        let m = self.m;
        let (nk, nth) = (m.nk as usize, m.nth as usize);
        let (itype, otype) = (self.itype, self.otype);
        let sig = |ik: usize| m.sig[ik];
        let xl: Real = 1. / m.xfr - 1.;
        let xh: Real = m.xfr - 1.;
        let xl2 = xl * xl;
        let xh2 = xh * xh;
        self.ipass += 1;
        let time = self.pnt.time;
        let m2km = self.m2km;

        // Output of time.
        if itype == 1 && otype == 3 {
            let t = self.tab.as_mut().unwrap();
            if self.flform {
                let mut b = Vec::new();
                b.extend_from_slice(&time[0].to_be_bytes());
                b.extend_from_slice(&time[1].to_be_bytes());
                t.rec(&b);
            } else {
                t.line(&format!("{}{}", ff::im(8, 8, time[0] as i64), ff::im(7, 6, time[1] as i64)));
            }
        }
        if itype == 2 {
            let t = self.tab.as_mut().unwrap();
            if self.nreq == 1 && self.ipass == 1 {
                t.line(match otype {
                    1 => HDR1901,
                    2 => HDR1902,
                    5 => HDR1905,
                    6 => HDR1906,
                    _ => unreachable!(),
                });
            }
            if self.nreq != 1 {
                let dt = stme21(time);
                if self.ipass != 1 {
                    t.put(" \n \n");
                }
                let ll = m.flagll;
                let s = match (otype, ll) {
                    (1, true) => HDR1911,
                    (1, false) => HDR1711,
                    (2, true) => HDR1912,
                    (2, false) => HDR1712,
                    (5, true) => HDR1915,
                    (5, false) => HDR1715,
                    (6, true) => HDR1916,
                    (6, false) => HDR1716,
                    _ => unreachable!(),
                }
                .replace("{dt}", &dt);
                t.line(&s);
            }
        }

        // Loop over output points.
        for j in 0..self.pnt.nopts {
            if !self.flreq[j] {
                continue;
            }
            let depth: Real = if self.pnt.dpo[j] > m.dmin { self.pnt.dpo[j] } else { m.dmin };
            let sqrth = sqrtf(depth);
            let wdo = self.pnt.wdo[j];
            let udir = modr(270. - wdo * RADE, 360.);
            let uabs: Real = if self.pnt.wao[j] > 0.001 { self.pnt.wao[j] } else { 0.001 };
            let cdir = modr(270. - self.pnt.cdo[j] * RADE, 360.);
            let (wao, cao, dpo, aso) = (self.pnt.wao[j], self.pnt.cao[j], self.pnt.dpo[j], self.pnt.aso[j]);
            let ptloc = self.pnt.ptloc[j];
            let name = ff::a(40, &self.pnt.ptnme[j]);

            // 2. Grid parameters, an inlined WAVNU1.
            let (dsie, n1max, ewn1, ecg1) = self.disp.tables();
            let mut wn = vec![0.; nk + 1];
            let mut cg = vec![0.; nk + 1];
            for ik in 1..=nk {
                let six = sig(ik) * sqrth;
                let i1 = (six / dsie) as i32;
                if i1 <= n1max {
                    let i2 = i1 + 1;
                    let r1 = six / dsie - i1 as Real;
                    let r2 = 1. - r1;
                    wn[ik] = (r2 * ewn1[i1 as usize] + r1 * ewn1[i2 as usize]) / depth;
                    cg[ik] = (r2 * ecg1[i1 as usize] + r1 * ecg1[i2 as usize]) * sqrth;
                } else {
                    wn[ik] = sig(ik) * sig(ik) / GRAV;
                    cg[ik] = 0.5 * GRAV / sig(ik);
                }
            }

            // 3.a Mean wave parameters.
            let spco = &self.pnt.spco[j];
            let mut et: Real = 0.;
            let mut ewn: Real = 0.;
            let mut etr: Real = 0.;
            let mut etx: Real = 0.;
            let mut ety: Real = 0.;
            let mut e2 = vec![0.; nk * nth]; // E(IK,ITH)
            let mut e1 = vec![0.; nk + 1];
            let mut apm = vec![0.; nk + 1];
            let mut thbnd = vec![0.; nk + 1];
            let mut spbnd = vec![0.; nk + 1];
            let (mut ebx, mut eby): (Real, Real) = (0., 0.);
            let g2: Real = GRAV * GRAV;
            for ik in 1..=nk {
                let mut ebnd: Real = 0.;
                ebx = 0.;
                eby = 0.;
                for ith in 1..=nth {
                    let isp = ith + (ik - 1) * nth;
                    let v = spco[isp - 1];
                    e2[(ik - 1) + nk * (ith - 1)] = v;
                    ebnd += v;
                    ebx += v * m.ecos[ith - 1];
                    eby += v * m.esin[ith - 1];
                }
                e1[ik] = ebnd * m.dth;
                apm[ik] = e1[ik] / (TPI * g2 / pow5(sig(ik)));
                if e1[ik] > 1.0e-5 {
                    thbnd[ik] = modr(630. - RADE * atan2f(eby, ebx), 360.);
                    let inner: Real = {
                        let t = (ebx * ebx + eby * eby) / (ebnd * ebnd);
                        let t = if t > 0. { t } else { 0. };
                        let t = 2. * (1. - sqrtf(t));
                        if t > 0. { t } else { 0. }
                    };
                    spbnd[ik] = RADE * sqrtf(inner);
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
            let hsig: Real = 4. * sqrtf(if et > 0. { et } else { 0. });
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
                for ik in 1..=nk {
                    e1[ik] = 0.;
                    for ith in 0..nth {
                        e2[(ik - 1) + nk * ith] = 0.;
                    }
                }
            }

            // Peak frequency.
            let mut emax = e1[nk];
            let mut ikm = nk;
            for ik in (1..nk).rev() {
                if e1[ik] > emax {
                    emax = e1[ik];
                    ikm = ik;
                }
            }
            let (mut fp, thp, mut spp);
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

            // 4.a Output type 1.
            if itype == 1 {
                match otype {
                    1 => {
                        if self.scale1 >= 0. {
                            let sg: Vec<Real> = (1..=nk).map(sig).collect();
                            let s = prt1ds(nk, &e1[1..], &sg, 17, self.scale1, "E(f)", "m^2s", &name);
                            self.sink.put(&s);
                        }
                        if self.scale2 >= 0. {
                            let sg: Vec<Real> = (1..=nk).map(sig).collect();
                            let s = prt2ds(nk, nth, &e2, &sg, 1., self.scale2, 0.0001, "E(f,th)", "m^2s", &name);
                            self.sink.put(&s);
                        }
                        let ind = |t: &str| format!("{}{t}", " ".repeat(15));
                        self.sink.put(&format!("\n{}{}  (m)\n{}{} (m/s)\n", ind(" Water depth       :"), f(7, 1, dpo), ind(" Wind speed        :"), f(8, 2, uabs)));
                        if wao > 0. {
                            self.sink.put(&format!("{}{}  (degr)\n", ind(" Wind direction    :"), f(7, 1, udir)));
                        }
                        self.sink.put(&format!("{}{}  (degr)\n{}{} (m/s)\n", ind(" Air-sea temp. dif.:"), f(7, 1, aso), ind(" Current speed     :"), f(8, 2, cao)));
                        if cao > 0. {
                            self.sink.put(&format!("{}{}  (degr)\n", ind(" Current direction :"), f(7, 1, cdir)));
                        }
                        self.sink.put(&format!(
                            "{}{} (m)\n{}{}   (m)\n{}{}  (s)\n{}{}  (degr)\n{}{}  (degr)\n\n",
                            ind(" Wave height       :"), f(8, 2, hsig),
                            ind(" Mean wave length  :"), f(6, 0, wlen),
                            ind(" Mean wave period  :"), f(7, 1, tmean),
                            ind(" Mean wave direct. :"), f(7, 1, thmean),
                            ind(" Direct. spread    :"), f(7, 1, thsprd)
                        ));
                    }
                    3 => {
                        let (ni, nj) = (m2km * ptloc[1], m2km * ptloc[0]);
                        let t = self.tab.as_mut().unwrap();
                        if self.flform {
                            let mut b = name.as_bytes().to_vec();
                            for v in [ptloc[1], ptloc[0], dpo, wao, udir, cao, cdir] {
                                be_f(&mut b, v);
                            }
                            t.rec(&b);
                            let mut b = Vec::new();
                            // ((E(IK,ITH),IK=1,NK),ITH=1,NTH)
                            e2.iter().for_each(|&x| be_f(&mut b, x));
                            t.rec(&b);
                        } else {
                            t.line(&format!(
                                "'{}'{}{}{}{}{}{}{}",
                                ff::a(10, &name),
                                f(7, 2, ni), f(7, 2, nj), f(10, 1, dpo), f(7, 2, wao), f(6, 1, udir), f(7, 2, cao), f(6, 1, cdir)
                            ));
                            let it: Vec<String> = e2.iter().map(|&x| e(11, 3, x)).collect();
                            t.line(&rows(&it, 7));
                        }
                    }
                    _ => unreachable!(),
                }
            } else if itype == 2 {
                let (x, y) = (m2km * ptloc[0], m2km * ptloc[1]);
                let t = self.tab.as_mut().unwrap();
                let ll = m.flagll;
                let (iyr, imth, idy) = (time[0] / 10000, time[0] % 10000 / 100, time[0] % 100);
                let (ih, im, is) = (time[1] / 10000, time[1] % 10000 / 100, time[1] % 100);
                let head = format!("  {}{}{}{}", ff::im(8, 8, time[0] as i64), ff::i(3, ih as i64), format!(" {}", ff::im(2, 2, im as i64)), format!(" {}", ff::im(2, 2, is as i64)));
                if self.nreq == 1 {
                    match otype {
                        1 => t.line(&format!("{head}{}{}{}{}{}", f(10, 1, dpo), f(6, 2, cao), f(7, 1, cdir), f(6, 2, wao), f(7, 1, udir))),
                        2 => t.line(&format!("{head}{}{}{}{}{}{}{}{}", f(9, 3, hsig), f(7, 1, wlen), f(7, 2, tmean), f(7, 1, thmean), f(7, 2, thsprd), f(8, 4, fp * TPIINV), f(7, 1, thp), f(7, 2, spp))),
                        5 => {
                            let hmat = {
                                let v = 3.33 * GRAV * hsig / (uabs * uabs);
                                if v < 100. { v } else { 100. }
                            };
                            let (age1, age2);
                            if hsig >= HSMIN {
                                let (wna, _) = self.disp.wavnu1(fp, dpo);
                                let a1 = fp / wna / uabs;
                                age1 = if a1 < 100. { a1 } else { 100. };
                                let afr = TPI / tmean;
                                let (wna, _) = self.disp.wavnu1(afr, dpo);
                                let a2 = afr / wna / uabs;
                                age2 = if a2 < 100. { a2 } else { 100. };
                            } else {
                                age1 = -9.99;
                                age2 = -9.99;
                            }
                            t.line(&format!("{head}{}{}{}{}{}{}{}", f(7, 2, wao), f(7, 1, udir), f(7, 2, hsig), f(7, 2, hmat), f(8, 2, age1), f(8, 2, age2), f(7, 1, aso)));
                        }
                        6 => {
                            let tp = if hsig >= HSMIN { TPI / fp } else { 0.0 };
                            t.line(&format!("  {}{}{}{}{}{}{}{}", ff::i(4, iyr as i64), format!(" {}", ff::i(2, imth as i64)), format!(" {}", ff::i(2, idy as i64)), format!(" {}", ff::i(2, ih as i64)), f(6, 2, wao), format!(" {}", ff::i(3, nint(udir) as i64)), f(6, 2, hsig), f(6, 2, tp)));
                        }
                        _ => unreachable!(),
                    }
                } else {
                    match otype {
                        1 => {
                            if ll {
                                t.line(&format!("  {}{}{}{}{}{}{}", f(8, 3, x), f(8, 3, y), f(10, 1, dpo), f(6, 2, cao), f(7, 1, cdir), f(6, 2, wao), f(7, 1, udir)));
                            } else {
                                t.line(&format!("  {}E3{}E3{}{}{}{}{}", f(7, 1, x), f(7, 1, y), f(10, 1, dpo), f(6, 2, cao), f(7, 1, cdir), f(6, 2, wao), f(7, 1, udir)));
                            }
                        }
                        2 => {
                            if ll {
                                t.line(&format!("  {}{}{}{}{}{}{}{}{}{}", f(8, 3, x), f(8, 3, y), f(9, 3, hsig), f(7, 1, wlen), f(7, 2, tmean), f(7, 1, thmean), f(7, 2, thsprd), f(8, 4, fp * TPIINV), f(7, 1, thp), f(7, 2, spp)));
                            } else {
                                t.line(&format!("  {}E3{}E3{}{}{}{}{}{}{}{}", f(7, 1, x), f(7, 1, y), f(9, 3, hsig), f(7, 1, wlen), f(7, 2, tmean), f(7, 1, thmean), f(7, 2, thsprd), f(8, 4, fp * TPIINV), f(7, 1, thp), f(7, 2, spp)));
                            }
                        }
                        5 => {
                            let hmat = {
                                let v = 3.33 * GRAV * hsig / (uabs * uabs);
                                if v < 100. { v } else { 100. }
                            };
                            let (wna, _) = self.disp.wavnu1(fp, dpo);
                            let a1 = fp / wna / uabs;
                            let age1 = if a1 < 100. { a1 } else { 100. };
                            let afr = TPI / tmean;
                            let (wna, _) = self.disp.wavnu1(afr, dpo);
                            let a2 = afr / wna / uabs;
                            let age2 = if a2 < 100. { a2 } else { 100. };
                            if ll {
                                t.line(&format!("  {}{}{}{}{}{}{}{}{}", f(8, 3, x), f(8, 3, y), f(7, 2, wao), f(7, 1, udir), f(7, 2, hsig), f(7, 2, hmat), f(8, 2, age1), f(8, 2, age2), f(7, 1, aso)));
                            } else {
                                t.line(&format!("  {}E3{}E3{}{}{}{}{}{}{}", f(7, 1, x), f(7, 1, y), f(7, 2, wao), f(7, 1, udir), f(7, 2, hsig), f(7, 2, hmat), f(8, 2, age1), f(8, 2, age2), f(7, 1, aso)));
                            }
                        }
                        6 => {
                            let tp = if hsig >= HSMIN { TPI / fp } else { 0.0 };
                            if ll {
                                t.line(&format!("  {}{}{}{}{}{}", f(8, 3, x), f(8, 3, y), f(6, 2, wao), format!(" {}", ff::i(3, nint(udir) as i64)), f(6, 2, hsig), f(6, 2, tp)));
                            } else {
                                t.line(&format!("  {}E3{}E3{}{}{}{}", f(8, 2, x), f(8, 2, y), f(6, 2, wao), format!(" {}", ff::i(3, nint(udir) as i64)), f(6, 2, hsig), f(6, 2, tp)));
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
            let _ = (&mut fp, DERA, RADIUS, UNDEF);
        }
    }
}

/// `PRT1DS` (`w3arrymd.F90:1364-1623`) with `UFR = 'RAD/S'`.
#[allow(clippy::too_many_arguments)]
fn prt1ds(nfr: usize, e: &[Real], fr: &[Real], nlines: i32, ftopi: Real, prvar: &str, prunit: &str, pntnme: &str) -> String {
    const TOPFAC: Real = 1.1;
    let mut out = String::new();
    let ftop = ftopi;
    let nfrb = nfr.min(50);
    let rlines = nlines as Real;
    let flscle = ftop <= 0.;
    let facfr: Real = 0.159155;
    let (mut emax, mut emin): (Real, Real) = (0., 0.);
    for &v in e.iter().take(nfr) {
        emax = if v > emax { v } else { emax };
        emin = if v < emin { v } else { emin };
    }
    if emax == 0. && emin == 0. {
        emax = 1.0e-20;
        emin = -1.0e-20;
    }
    let extr = if emax > emin.abs() { emax } else { emin };
    let (floc, fsc);
    if flscle {
        if emax > emin.abs() {
            floc = emax * TOPFAC;
            fsc = floc / nint(emax / (emax - emin) * rlines) as Real;
        } else {
            let fl = emin * TOPFAC;
            fsc = fl / nint(emin / (emax - emin) * rlines) as Real;
            floc = ftop + rlines * fsc;
        }
    } else {
        let mut fl = ftop;
        let mut fs = fl / rlines;
        if emax * emin < 0. {
            fs = 2. * fs;
        }
        if emax < 0.01 * fs {
            fl = 0.;
        }
        floc = fl;
        fsc = fs;
    }
    let il0 = nint(floc / fsc) % 2 + 1;
    out.push_str(&format!("\n  Location : {pntnme}\n  Spectrum : {prvar}  Extreme value : {} {prunit}\n\n", e_(10, 3, extr)));
    let blank10 = " ".repeat(10);
    let row = |stra: &str, stra2: &str, p: &[&str]| format!("{}{}{}\n", ff::a(10, stra), stra2, p.concat());
    let mut fline = floc;
    let stra = if (nlines + il0) % 2 == 0 { e_(10, 3, fline) } else { blank10.clone() };
    let mut pnum2: Vec<&str> = Vec::new();
    for ifr in 0..nfrb {
        pnum2.push(if nint((e[ifr] - fline) / fsc) == 0 { "-*" } else { "--" });
    }
    pnum2.push("-+");
    out.push_str(&row(&stra, " +", &pnum2));
    for il in 1..nlines {
        fline = floc - fsc * il as Real;
        if fline.abs() < 0.01 * fsc {
            fline = 0.;
        }
        let (stra, stra2) = if (nlines + il0 - il) % 2 == 0 { (e_(10, 3, fline), " +") } else { (blank10.clone(), " |") };
        let mut p: Vec<&str> = Vec::new();
        let mut last = " |";
        for ifr in 0..nfrb {
            if fline.abs() < 0.1 * fsc {
                last = "-|";
                p.push(if nint((e[ifr] - fline) / fsc) == 0 { "-*" } else { "--" });
            } else {
                last = " |";
                p.push(if nint((e[ifr] - fline) / fsc) == 0 { " *" } else { "  " });
            }
        }
        p.push(last);
        out.push_str(&row(&stra, stra2, &p));
    }
    // The ending uses IL = NLINES, the value the loop leaves.
    let il = nlines;
    fline = floc - fsc * il as Real;
    if fline.abs() < 0.01 * fsc {
        fline = 0.;
    }
    let stra = if il0 % 2 == 0 { e_(10, 3, fline) } else { blank10 };
    let mut p: Vec<&str> = Vec::new();
    for ifr in 1..=nfrb {
        p.push(if nint((e[ifr - 1] - fline) / fsc) == 0 {
            "-*"
        } else if (ifr as i32 - 2) % 4 == 0 {
            "-|"
        } else {
            "--"
        });
    }
    p.push("-+");
    out.push_str(&row(&stra, " +", &p));
    let fl: Vec<String> = (2..=nfrb).step_by(4).map(|ifr| f(8, 3, fr[ifr - 1] * facfr)).collect();
    let mut s = String::new();
    for c in fl.chunks(15) {
        s.push_str(&format!("{}{}\n", " ".repeat(10), c.concat()));
    }
    out.push_str(&s);
    out.push_str(" \n");
    out
}

fn e_(w: usize, d: usize, x: Real) -> String {
    ff::e(w, d, x as f64)
}

/// `ANGSTR (IANG, SANG, 4, 2)`: the four characters of `SANG` that are used.
fn angstr(iang: i32) -> &'static str {
    match iang {
        0 => "   N",
        90 => "   E",
        180 => "   S",
        270 => "   W",
        45 => "  NE",
        135 => "  SE",
        225 => "  SW",
        315 => "  NW",
        _ => "    ",
    }
}

/// `PRT2DS` (`w3arrymd.F90:1941-2379`) with `UFR = 'RAD/S'`, `E(NFR,NTH)` stored `(IFR,ITH)` column-major.
/// `NFR <= 50` holds, so the frequency header is one record in both branches.
#[allow(clippy::too_many_arguments)]
fn prt2ds(nfr: usize, nth: usize, e: &[Real], fr: &[Real], facsp: Real, fsc: Real, rrcut: Real, prvar: &str, prunit: &str, pntnme: &str) -> String {
    assert!(nfr <= 50, "PRT2DS with more than 50 frequencies");
    let mut out = String::new();
    let flscle = fsc == 0.;
    let rrc = if flscle { rrcut * 10. } else { 0. };
    let facfr: Real = 0.159155;
    let at = |ifr: usize, ith: usize| e[ifr + nfr * ith];
    let mut emax: Real = 1.0e-20;
    let mut emin: Real = 0.;
    for ifr in 0..nfr {
        for ith in 0..nth {
            emax = if at(ifr, ith) > emax { at(ifr, ith) } else { emax };
            emin = if at(ifr, ith) < emin { at(ifr, ith) } else { emin };
        }
    }
    emax = if emin.abs() > emax { emin.abs() } else { emax };
    let dthdeg: Real = 360. / nth as Real;
    // The angles in the order the loops visit them: (ITH, INTANG).
    let mut ithsec = nth + 1;
    let mut rows_: Vec<(usize, i32)> = Vec::new();
    for ith in (1..=nth).rev() {
        let intang = 270 - nint(dthdeg * (ith - 1) as Real);
        if intang < 0 {
            ithsec = ith;
            continue;
        }
        rows_.push((ith, intang));
    }
    for ith in (ithsec..=nth).rev() {
        rows_.push((ith, 630 - nint(dthdeg * (ith - 1) as Real)));
    }
    if flscle {
        out.push_str(&format!("\n  Location : {pntnme}\n  Spectrum : {prvar} (Normalized)   Maximum value : {} {prunit}\n\n", e_(8, 3, emax * facsp)));
        let nfrb = nfr.min(50);
        let vals: Vec<Real> = (2..=nfrb).step_by(4).map(|ifr| fr[ifr - 1] * facfr).collect();
        assert!(vals.len() <= 16);
        out.push_str("       ang.|  frequencies (Hz) \n");
        out.push_str("       deg.|");
        for (k, v) in vals.iter().enumerate() {
            out.push_str(&if k == 0 { f(6, 3, *v) } else { f(8, 3, *v) });
        }
        out.push('\n');
        let mut pnum2: Vec<&str> = (1..=nfrb).map(|ifr| if (ifr as i32 - 2) % 4 == 0 { "-|" } else { "--" }).collect();
        pnum2.push("-+");
        out.push_str(&format!("       ----+{}\n", pnum2.concat()));
        for &(ith, intang) in &rows_ {
            let mut line = format!("      {} |", angstr(intang));
            for ifr in 0..nfrb {
                let v = at(ifr, ith - 1);
                let rr = v / emax;
                line.push_str(&if v == emax || rr >= 1. {
                    " *".to_string()
                } else if -v == emax || rr <= -1. {
                    " #".to_string()
                } else if rr.abs() < rrc {
                    "  ".to_string()
                } else if rr * 10. < 0. && rr * 10. > -1. {
                    "-0".to_string()
                } else {
                    ff::i(2, (rr * 10.) as i64)
                });
            }
            line.push_str(" |");
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str(&format!("       ----+{}-+\n", "--".repeat(nfrb)));
        out.push_str(" \n");
    } else {
        out.push_str(&format!(
            "\n  Location : {pntnme}\n  Spectrum : {prvar}  Units : {} {prunit}  Maximum value : {} {prunit}\n\n",
            e_(8, 3, fsc),
            e_(8, 3, emax * facsp)
        ));
        let nfrb = nfr.min(25);
        let vals: Vec<Real> = (2..=nfrb).step_by(2).map(|ifr| fr[ifr - 1] * facfr).collect();
        assert!(vals.len() <= 12);
        out.push_str("  ang.|  frequencies (Hz) \n");
        out.push_str(&format!("  deg.|{}\n", vals.iter().map(|&v| f(10, 3, v)).collect::<String>()));
        let rule = format!("  ----|{}{}\n", "-----".repeat(nfrb), if nfrb < 25 { "--   " } else { "" });
        out.push_str(&rule);
        for &(ith, intang) in &rows_ {
            let mut line = format!(" {} |", angstr(intang));
            for ifr in 0..nfrb {
                let rr = at(ifr, ith - 1);
                if (rr / emax).abs() < rrcut {
                    line.push_str("     ");
                } else {
                    line.push_str(&ff::i(5, nint(rr * facsp / fsc) as i64));
                }
            }
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str(&rule);
        out.push_str(" \n");
    }
    out
}
