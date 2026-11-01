// SPDX-License-Identifier: MPL-2.0
//! `ww3_outf` in Rust (`model/src/ww3_outf.F90`, 2670 lines): the field output postprocessor, for
//! `switches/switch_chaceon_shrd` at `92e2cf66`. It reads `ww3_outf.inp`, `mod_def.ww3` and `out_grd.ww3` from a
//! directory, writes the log to standard output and `tabNN.ww3` or `ww3.YYMMDDHH.<field>` into the directory.
//!
//! Reached: `ITYPE` 0 to 4 (contents, print plots, statistics, transfer files in free, fixed and unformatted
//! format with layouts 1 to 5, sea-point text) for every field `W3EXGO` lists under this switch. A field `W3EXGO`
//! does not list stops the run with the Fortran's own message and exit 1, as the Fortran does. Refused, with a
//! message and exit 64: a `mod_def.ww3` that is not rectilinear or curvilinear, and the user-defined group 10.

use crate::finp::{self, Inp, ReadErr};
use crate::fortfmt as ff;
use crate::moddef::{ModDef, Records};
use crate::outp::{check_grid, stme21, Sink};
use crate::wtime::{dsec21, tick21, Time};
use std::collections::HashMap;
use std::io::Write;
use ww3_kernels::constants::RADE;
use ww3_kernels::nint;
use ww3_kernels::real::Real;
use ww3_libm::{atan2f, sqrtf};

const NOGRP: usize = 10;
const NGRPP: usize = 25;
const IDSTR: &str = "WAVEWATCH III GRID OUTPUT FILE";
const VEROGR: &str = "2019-10-04";
const NCOL: i32 = 132;

/// `W3FLDTOIJ` under this switch (`w3iogomd.F90:759-1189`): names are matched on their first six characters.
const NAMES: [(&str, usize, usize); 96] = [
    ("DPT", 1, 1), ("CUR", 1, 2), ("WND", 1, 3), ("AST", 1, 4), ("WLV", 1, 5), ("ICE", 1, 6), ("IBG", 1, 7),
    ("TAU", 1, 8), ("RHO", 1, 9), ("HS", 2, 1), ("LM", 2, 2), ("T02", 2, 3), ("T0M1", 2, 4), ("T01", 2, 5),
    ("FP", 2, 6), ("DIR", 2, 7), ("SPR", 2, 8), ("DP", 2, 9), ("HIG", 2, 10), ("MXE", 2, 11), ("MXES", 2, 12),
    ("MXH", 2, 13), ("MXHC", 2, 14), ("SDMH", 2, 15), ("SDMHC", 2, 16), ("WBT", 2, 17), ("TP", 2, 18),
    ("WNM", 2, 19), ("EF", 3, 1), ("TH1M", 3, 2), ("STH1M", 3, 3), ("TH2M", 3, 4), ("STH2M", 3, 5), ("WN", 3, 6),
    ("PHS", 4, 1), ("PTP", 4, 2), ("PLP", 4, 3), ("PDIR", 4, 4), ("PSPR", 4, 5), ("PWS", 4, 6), ("PDP", 4, 7),
    ("PQP", 4, 8), ("PPE", 4, 9), ("PGW", 4, 10), ("PSW", 4, 11), ("PTM10", 4, 12), ("PT01", 4, 13),
    ("PT02", 4, 14), ("PEP", 4, 15), ("TWS", 4, 16), ("PNR", 4, 17), ("SEA8HS", 4, 18), ("SW8HS", 4, 19),
    ("SEA8TP", 4, 20), ("SW8TP", 4, 21), ("SEA8DP", 4, 22), ("SW8DP", 4, 23), ("UST", 5, 1), ("CHA", 5, 2),
    ("CGE", 5, 3), ("FAW", 5, 4), ("TAW", 5, 5), ("TWA", 5, 6), ("WCC", 5, 7), ("WCF", 5, 8), ("WCH", 5, 9),
    ("WCM", 5, 10), ("FWS", 5, 11), ("SXY", 6, 1), ("TWO", 6, 2), ("BHD", 6, 3), ("FOC", 6, 4), ("TUS", 6, 5),
    ("USS", 6, 6), ("P2S", 6, 7), ("USF", 6, 8), ("P2L", 6, 9), ("TWI", 6, 10), ("FIC", 6, 11), ("USP", 6, 12),
    ("TOC", 6, 13), ("ABR", 7, 1), ("UBR", 7, 2), ("BED", 7, 3), ("FBB", 7, 4), ("TBB", 7, 5), ("MSS", 8, 1),
    ("MSC", 8, 2), ("MSD", 8, 3), ("MCD", 8, 4), ("QP", 8, 5), ("DTD", 9, 1), ("FC", 9, 2), ("CFX", 9, 3),
    ("CFD", 9, 4), ("CFK", 9, 5),
];

/// `IDOUT` (`w3odatmd.F90:751-925`), 20 characters.
fn idout(i: usize, j: usize) -> String {
    let s = match (i, j) {
        (1, 1) => "Water depth         ",
        (1, 2) => "Current vel.        ",
        (1, 3) => "Wind speed          ",
        (1, 4) => "Air-sea temp. dif.  ",
        (1, 5) => "Water level         ",
        (1, 6) => "Ice concentration   ",
        (1, 7) => "Iceberg damp coeffic",
        (1, 8) => "Atmospheric momentum",
        (1, 9) => "Air density         ",
        (2, 1) => "Wave height         ",
        (2, 2) => "Mean wave length    ",
        (2, 3) => "Mean wave period(+2)",
        (2, 4) => "Mean wave period(-1)",
        (2, 5) => "Mean wave period(+1)",
        (2, 6) => "Peak frequency      ",
        (2, 7) => "Mean wave dir. a1b1 ",
        (2, 8) => "Mean dir. spr. a1b1 ",
        (2, 9) => "Peak direction      ",
        (2, 10) => "Infragravity height",
        (2, 11) => "Space-Time Max E   ",
        (2, 12) => "Space-Time Max Std ",
        (2, 13) => "Space-Time Hmax    ",
        (2, 14) => "Spc-Time Hmax^crest",
        (2, 15) => "STD Space-Time Hmax",
        (2, 16) => "STD ST Hmax^crest  ",
        (2, 17) => "Dominant wave bT   ",
        (2, 18) => "Peak prd. (from fp)",
        (2, 19) => "Mean wave number   ",
        (3, 1) => "1D Freq. Spectrum   ",
        (3, 2) => "Mean wave dir. a1b1 ",
        (3, 3) => "Mean dir. spr. a1b1 ",
        (3, 4) => "Mean wave dir. a2b2 ",
        (3, 5) => "Mean dir. spr. a2b2 ",
        (3, 6) => "Wavenumber array    ",
        (4, 1) => "Part. wave height   ",
        (4, 2) => "Part. peak period   ",
        (4, 3) => "Part. peak wave len.",
        (4, 4) => "Part. mean direction",
        (4, 5) => "Part. dir. spread   ",
        (4, 6) => "Part. wind sea frac.",
        (4, 7) => "Part. peak direction",
        (4, 8) => "Part. peakedness    ",
        (4, 9) => "Part. peak enh. fac.",
        (4, 10) => "Part. gaussian width",
        (4, 11) => "Part. spectral width",
        (4, 12) => "Part. mean per. (-1)",
        (4, 13) => "Part. mean per. (+1)",
        (4, 14) => "Part. mean per. (+2)",
        (4, 15) => "Part. peak density  ",
        (4, 16) => "Total wind sea frac.",
        (4, 17) => "Number of partitions",
        (4, 18) => "8s sea wave height  ",
        (4, 19) => "8s swell wave height",
        (4, 20) => "8s sea peak period  ",
        (4, 21) => "8s swell peak period",
        (4, 22) => "8s sea peak dir     ",
        (4, 23) => "8s swell peak dir   ",
        (5, 1) => "Friction velocity   ",
        (5, 2) => "Charnock parameter  ",
        (5, 3) => "Energy flux         ",
        (5, 4) => "Wind-wave enrgy flux",
        (5, 5) => "Wind-wave net mom. f",
        (5, 6) => "Wind-wave neg.mom.f.",
        (5, 7) => "Whitecap coverage   ",
        (5, 8) => "Whitecap mean thick.",
        (5, 9) => "Mean breaking height",
        (5, 10) => "Dominant break prob ",
        (5, 11) => "Wind sea period",
        (6, 1) => "Radiation stresses  ",
        (6, 2) => "Wave-ocean mom. flux",
        (6, 3) => "wave ind p Bern Head",
        (6, 4) => "Wave-ocean TKE  flux",
        (6, 5) => "Stokes transport    ",
        (6, 6) => "Stokes drift at z=0 ",
        (6, 7) => "2nd order pressure  ",
        (6, 8) => "Stokes drft spectrum",
        (6, 9) => "2nd ord press spectr",
        (6, 10) => "Wave-ice mom. flux  ",
        (6, 11) => "Wave-ice energy flux",
        (6, 12) => "Split Surface Stokes",
        (6, 13) => "Tot wav-ocn mom flux",
        (7, 1) => "Bottom rms ampl.    ",
        (7, 2) => "Bottom rms velocity ",
        (7, 3) => "Bedform parameters  ",
        (7, 4) => "Energy diss. in WBBL",
        (7, 5) => "Moment. loss in WBBL",
        (8, 1) => "Mean square slopes  ",
        (8, 2) => "Phillips tail const",
        (8, 3) => "Slope direction     ",
        (8, 4) => "Tail slope direction",
        (8, 5) => "Goda peakedness parm",
        (9, 1) => "Avg. time step.     ",
        (9, 2) => "Cut-off freq.       ",
        (9, 3) => "Maximum spatial CFL ",
        (9, 4) => "Maximum angular CFL ",
        (9, 5) => "Maximum k advect CFL",
        _ => "Undefined / Not Used",
    };
    format!("{s:<20}")
}


/// How many records `W3IOGO` reads for a field of the file (`w3iogomd.F90:3563-3896`).
fn nrec(i: usize, j: usize) -> usize {
    match (i, j) {
        (6, 1) | (7, 3) => 3,
        (1, 2) | (1, 3) | (1, 8) | (5, 1) | (5, 5) | (5, 6) | (6, 2) | (6, 5) | (6, 6) | (6, 7) | (6, 8)
        | (6, 10) | (6, 12) | (6, 13) | (7, 1) | (7, 2) | (7, 5) | (8, 1) | (8, 2) => 2,
        _ => 1,
    }
}

fn modr(a: Real, p: Real) -> Real {
    a % p
}

fn f(w: usize, d: usize, x: Real) -> String {
    ff::f(w, d, x as f64)
}
fn e(w: usize, d: usize, x: Real) -> String {
    ff::e(w, d, x as f64)
}

fn premature(sink: &mut Sink) -> ! {
    sink.put("\n *** WAVEWATCH III ERROR IN W3OUTF : \n     PREMATURE END OF INPUT FILE\n\n");
    sink.exit(11)
}

fn read_err(sink: &mut Sink, e: ReadErr) -> ! {
    match e {
        ReadErr::End => premature(sink),
        ReadErr::Bad(s) => sink.refuse("ww3_outf", &format!("an input item libgfortran would reject with an IOSTAT this port does not reproduce: {s:?}")),
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

/// `NEXTLN`.
fn nextln(inp: &mut Inp, sink: &mut Sink) {
    if inp.nextln().is_err() {
        sink.put("\n *** WAVEWATCH III ERROR IN NEXTLN : \n     PREMATURE END OF INPUT FILE\n\n");
        sink.exit(1)
    }
}

/// `out_grd.ww3`, read as `W3IOGO ('READ')`.
struct Grd {
    rec: Records,
    undef: Real,
    time: Time,
    flogrd: Vec<bool>,
    mapsta: Vec<i32>,
    /// Records of a field of the file, kept from the last time that held it.
    data: HashMap<(usize, usize), Vec<Vec<Real>>>,
}

impl Grd {
    fn at(i: usize, j: usize) -> usize {
        (i - 1) + (j - 1) * NOGRP
    }

    fn open(path: &str, m: &ModDef, sink: &mut Sink) -> Self {
        let mut rec = Records::open(path);
        let Some(mut c) = rec.try_next() else {
            sink.put("\n *** WAVEWATCH III ERROR IN W3IOGO : \n     PREMATURE END OF FILE\n\n");
            sink.exit(42)
        };
        let idtst = c.text(30);
        let vertst = c.text(10);
        let tname = c.text(30);
        let (mogrp, mgrpp, nsea, nx, ny) = (c.i(), c.i(), c.i(), c.i(), c.i());
        let undef = c.f();
        let moswll = c.i();
        c.done("W3IOGO header");
        if idtst != IDSTR {
            sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOGO :\n     ILLEGAL IDSTR, READ : {}\n                  CHECK : {}\n\n", ff::a(30, &idtst), IDSTR));
            sink.exit(20)
        }
        if vertst != VEROGR {
            sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOGO :\n     ILLEGAL VEROGR, READ : {}\n                   CHECK : {}\n\n", ff::a(10, &vertst), VEROGR));
            sink.exit(21)
        }
        if mogrp as usize != NOGRP || mgrpp as usize != NGRPP {
            sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOGO :\n     DIFFERENT NUMBER OF FIELDS, FILE :{:>8}{:>8}\n                              PROGRAM :{:>8}{:>8}\n\n", mogrp, mgrpp, NOGRP, NGRPP));
            sink.exit(22)
        }
        if tname != m.gname {
            sink.put(&format!("\n *** WAVEWATCH III WARNING IN W3IOGO :\n     ILLEGAL GNAME, READ : {}\n                  CHECK : {}\n\n", ff::a(30, &tname), ff::a(30, &m.gname)));
        }
        if moswll != m.noswll {
            sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3IOGO :\n     ILLEGAL NOSWLL, READ : {:>4}\n                   CHECK : {:>4}\n\n", moswll, m.noswll));
            sink.exit(24)
        }
        assert!(nsea == m.nsea && nx == m.nx && ny == m.ny, "out_grd.ww3 and mod_def.ww3 differ in size");
        Self { rec, undef, time: [0, 0], flogrd: vec![false; NOGRP * NGRPP], mapsta: Vec::new(), data: HashMap::new() }
    }

    /// The next output time. `false` at the end of the file (`IOTEST = -1`).
    fn read(&mut self, m: &ModDef, sink: &mut Sink) -> bool {
        let Some(mut c) = self.rec.try_next() else { return false };
        self.time = [c.i(), c.i()];
        self.flogrd = (0..NOGRP * NGRPP).map(|_| c.i() != 0).collect();
        c.done("W3IOGO time");
        let (nx, ny, nsea) = (m.nx as usize, m.ny as usize, m.nsea as usize);
        let mut c = self.rec.next();
        let tmp = c.is(nx * ny);
        c.done("W3IOGO map");
        // The file runs IX fastest, MAPSTA(IY,IX) runs IY fastest.
        self.mapsta = vec![0; nx * ny];
        for iy in 0..ny {
            for ix in 0..nx {
                let t = tmp[ix + nx * iy];
                self.mapsta[iy + ny * ix] = (t + 2) % 8 - 2;
            }
        }
        for i in 1..=NOGRP {
            for j in 1..=NGRPP {
                if !self.flogrd[Self::at(i, j)] {
                    continue;
                }
                let mut v = Vec::new();
                for _ in 0..nrec(i, j) {
                    let Some(mut c) = self.rec.try_next() else {
                        sink.put("\n *** WAVEWATCH III ERROR IN W3IOGO : \n     PREMATURE END OF FILE\n\n");
                        sink.exit(42)
                    };
                    let n = c.len() / 4;
                    assert!(n >= nsea);
                    v.push(c.fs(n));
                    c.done("W3IOGO field");
                }
                self.data.insert((i, j), v);
            }
        }
        true
    }
}

/// `NOGE`.
fn noge(i: usize) -> usize {
    [9, 19, 6, 23, 11, 13, 5, 5, 5, 0][i - 1]
}

/// `W3READFLGRD` (`w3iogomd.F90:333-565`): the group flags and field names of `ww3_outf.inp`.
fn readflgrd(inp: &mut Inp, sink: &mut Sink, flog: &mut [bool], flreq: &mut [bool]) {
    let mut names = false;
    for ifi in 1..=NOGRP {
        nextln(inp, sink);
        let l = inp.list(1).unwrap_or_else(|e| read_err(sink, e));
        let a = l[0].clone().unwrap_or_default();
        match a.chars().next() {
            Some('T') => flog[ifi - 1] = true,
            Some('F') => flog[ifi - 1] = false,
            Some('N') => {
                names = true;
                break;
            }
            _ => {
                sink.put(&format!("\n *** WAVEWATCH III ERROR    : \n     WAS EXPECTING \"T\" \"F\" or \"N\", but found \"{}\".\n\n", a.chars().next().unwrap_or(' ')));
                // The caller sees IERR = 1 and reports it as a failure to open the input file.
                sink.put("\n *** WAVEWATCH III ERROR IN W3OUTF : \n     ERROR IN OPENING INPUT FILE\n     IOSTAT =    1\n\n");
                sink.exit(10)
            }
        }
        if flog[ifi - 1] {
            nextln(inp, sink);
            let line = inp.raw_line().unwrap_or_else(|e| read_err(sink, e));
            let words: Vec<&str> = line.split_whitespace().collect();
            for (k, w) in words.iter().enumerate() {
                if *w == "T" && k < NGRPP {
                    flreq[Grd::at(ifi, k + 1)] = true;
                }
            }
            if words.len() < noge(ifi) {
                sink.put(&format!("\n *** WAVEWATCH III WARNING  : \n     NUMBER OF REQUESTED OUTPUT FIELD FLAGS IN GROUP {}\n LESS THAN AVAILABLE, CHECK DOCS FOR MORE OPTIONS\n", ff::i(2, ifi as i64)));
            }
        }
    }
    if names {
        nextln(inp, sink);
        let line = inp.raw_line().unwrap_or_else(|e| read_err(sink, e));
        for w in line.split_whitespace() {
            let up = w.to_uppercase();
            let key: String = up.chars().take(6).collect();
            match NAMES.iter().find(|n| n.0 == key) {
                Some(n) => flreq[Grd::at(n.1, n.2)] = true,
                None => sink.put(&format!("\n *** WAVEWATCH III WARNING  : \n     REQUESTED OUTPUT FIELD {up} WAS NOT RECOGNIZED.\n\n")),
            }
        }
    }
    let mut first = true;
    for ifi in 1..=NOGRP {
        for ifj in 1..=NGRPP {
            if flreq[Grd::at(ifi, ifj)] {
                if first {
                    sink.put(&format!("            Fields   : {}\n", idout(ifi, ifj)));
                    first = false;
                } else {
                    sink.put(&format!("                       {}\n", idout(ifi, ifj)));
                }
            }
        }
        if (1..=NGRPP).any(|j| flreq[Grd::at(ifi, j)]) {
            flog[ifi - 1] = true;
        }
    }
    if first {
        sink.put("            Fields   : no fields defined\n");
    }
}

struct Out {
    sink: Sink,
    files: HashMap<i32, std::io::BufWriter<std::fs::File>>,
}

struct Params {
    itype: i32,
    ix1: i32,
    ixn: i32,
    ixs: i32,
    iy1: i32,
    iyn: i32,
    iys: i32,
    idla: i32,
    idfm: i32,
    scale: bool,
    vector: bool,
    ipart: usize,
}

/// `W3EXGO`'s `SAVE` variables.
struct Stat {
    timeid: String,
    ipass: i32,
}

/// The program, section by section as `ww3_outf.F90:179-526`.
pub fn run(dir: &str) -> i32 {
    let mut sink = Sink { log: Vec::new() };
    sink.put("\n                  *** WAVEWATCH III Field output postp. ***   \n               ===============================================\n\n");
    let mut inp = match Inp::open(&format!("{dir}/ww3_outf.inp")) {
        Ok(i) => i,
        Err(_) => {
            sink.put("\n *** WAVEWATCH III ERROR IN W3OUTF : \n     ERROR IN OPENING INPUT FILE\n     IOSTAT =    2\n\n");
            sink.exit(10)
        }
    };
    let comstr = match inp.comment_char() {
        Ok(c) => c,
        Err(_) => premature(&mut sink),
    };
    sink.put(&format!("  Comment character is '{comstr}'\n\n"));
    check_grid("ww3_outf", &format!("{dir}/mod_def.ww3"), &mut sink);
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    sink.put(&format!("  Grid name : {}\n\n", ff::a(30, &m.gname)));
    let mut grd = Grd::open(&format!("{dir}/out_grd.ww3"), &m, &mut sink);
    grd.read(&m, &mut sink);
    sink.put("  Fields in file : \n --------------------------\n");
    for ifi in 1..=NOGRP {
        for ifj in 1..=NGRPP {
            if grd.flogrd[Grd::at(ifi, ifj)] {
                sink.put(&format!("      {}\n", idout(ifi, ifj)));
            }
        }
    }

    nextln(&mut inp, &mut sink);
    let l = inp.list(4).unwrap_or_else(|e| read_err(&mut sink, e));
    let mut tout: Time = [rd_i(&mut sink, &l[0], 0), rd_i(&mut sink, &l[1], 0)];
    let mut dtreq = rd_r(&mut sink, &l[2], 0.);
    let mut nout = rd_i(&mut sink, &l[3], 0);
    dtreq = if dtreq > 0. { dtreq } else { 0. };
    if dtreq == 0. {
        nout = 1;
    }
    nout = nout.max(1);
    sink.put(&format!("\n  Output time data : \n --------------------------------------------------\n      First time         : {}\n", stme21(tout)));
    let mut tdum: Time = [0, 0];
    tick21(&mut tdum, dtreq);
    let mut idtime: Vec<u8> = stme21(tdum).into_bytes();
    let idday = if dtreq >= 86400. { format!("{} ", ff::i(10, (dtreq / 86400.) as i64)) } else { " ".repeat(11) };
    idtime[..11].copy_from_slice(idday.as_bytes());
    idtime[20..23].copy_from_slice(b"   ");
    sink.put(&format!("      Interval           : {}\n      Number of requests : {}\n", String::from_utf8_lossy(&idtime), ff::i(6, nout as i64)));

    let mut flog = vec![false; NOGRP];
    let mut flreq = vec![false; NOGRP * NGRPP];
    readflgrd(&mut inp, &mut sink, &mut flog, &mut flreq);

    nextln(&mut inp, &mut sink);
    let l = inp.list(2).unwrap_or_else(|e| read_err(&mut sink, e));
    let itype = rd_i(&mut sink, &l[0], 0);
    let ipart = rd_i(&mut sink, &l[1], 0);
    if !(0..=4).contains(&itype) {
        sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3OUTF : \n     ILLEGAL TYPE, ITYPE ={}\n\n", ff::i(4, itype as i64)));
        sink.exit(1)
    }
    let ipart = ipart.min(m.noswll).max(0);
    let head = |n: i32, s: &str| format!("\n  Output type {} :\n --------------------------------------------------\n      {s}\n\n", ff::i(2, n as i64));
    let (nx, ny) = (m.nx, m.ny);
    let mut p = Params { itype, ix1: 1, ixn: nx, ixs: 1, iy1: 1, iyn: ny, iys: 1, idla: 1, idfm: 1, scale: false, vector: false, ipart: ipart as usize };
    match itype {
        0 => {
            sink.put(&head(itype, "Checking contents of file"));
            loop {
                sink.put(&format!("      Data for {}\n", stme21(grd.time)));
                if !grd.read(&m, &mut sink) {
                    sink.put("\n      End of file reached \n\n");
                    break;
                }
            }
            return finish(sink, HashMap::new());
        }
        1 => {
            sink.put(&head(itype, "Print plots"));
            nextln(&mut inp, &mut sink);
            let l = inp.list(8).unwrap_or_else(|e| read_err(&mut sink, e));
            p.ix1 = rd_i(&mut sink, &l[0], 0).max(1);
            p.ixn = rd_i(&mut sink, &l[1], 0).min(nx);
            p.ixs = rd_i(&mut sink, &l[2], 0).max(1);
            p.iy1 = rd_i(&mut sink, &l[3], 0).max(1);
            p.iyn = rd_i(&mut sink, &l[4], 0).min(ny);
            p.iys = rd_i(&mut sink, &l[5], 0).max(1);
            p.scale = rd_l(&mut sink, &l[6], false);
            p.vector = rd_l(&mut sink, &l[7], false);
            sink.put(&format!(
                "      X range and interval : {}{}{}\n      Y range and interval : {}{}{}\n",
                ff::i(5, p.ix1 as i64), ff::i(5, p.ixn as i64), ff::i(5, p.ixs as i64), ff::i(5, p.iy1 as i64), ff::i(5, p.iyn as i64), ff::i(5, p.iys as i64)
            ));
            if p.scale {
                sink.put("      Data is normalized \n");
            }
        }
        2 => {
            sink.put(&head(itype, "Field statistics"));
            nextln(&mut inp, &mut sink);
            let l = inp.list(4).unwrap_or_else(|e| read_err(&mut sink, e));
            p.ix1 = rd_i(&mut sink, &l[0], 0).max(1);
            p.ixn = rd_i(&mut sink, &l[1], 0).min(nx);
            p.iy1 = rd_i(&mut sink, &l[2], 0).max(1);
            p.iyn = rd_i(&mut sink, &l[3], 0).min(ny);
            sink.put(&format!("      X range : {}{}\n      Y range : {}{}\n", ff::i(5, p.ix1 as i64), ff::i(5, p.ixn as i64), ff::i(5, p.iy1 as i64), ff::i(5, p.iyn as i64)));
        }
        _ => {
            sink.put(&head(itype, if itype == 3 { "Transfer files" } else { "Full sea-point output." }));
            nextln(&mut inp, &mut sink);
            let l = inp.list(6).unwrap_or_else(|e| read_err(&mut sink, e));
            p.ix1 = rd_i(&mut sink, &l[0], 0);
            p.ixn = rd_i(&mut sink, &l[1], 0);
            p.iy1 = rd_i(&mut sink, &l[2], 0);
            p.iyn = rd_i(&mut sink, &l[3], 0);
            p.idla = rd_i(&mut sink, &l[4], 0);
            p.idfm = rd_i(&mut sink, &l[5], 0);
            if itype == 3 {
                p.ix1 = p.ix1.max(1);
                p.ixn = p.ixn.min(nx);
                p.iy1 = p.iy1.max(1);
                p.iyn = p.iyn.min(ny);
                if !(1..=5).contains(&p.idla) {
                    p.idla = 1;
                }
                if !(1..=3).contains(&p.idfm) {
                    p.idfm = 1;
                }
                p.vector = true;
                sink.put(&format!(
                    "      X range          : {}{}\n      Y range          : {}{}\n      Layout indicator : {}\n      Format indicator : {}\n",
                    ff::i(5, p.ix1 as i64), ff::i(5, p.ixn as i64), ff::i(5, p.iy1 as i64), ff::i(5, p.iyn as i64), ff::i(5, p.idla as i64), ff::i(5, p.idfm as i64)
                ));
            }
        }
    }

    sink.put(if itype != 2 { "\n  Requested output fields : \n --------------------------------------------------\n" } else { "\n  Output files and fields : \n --------------------------------------------------\n" });
    let mut o = Out { sink, files: HashMap::new() };
    let mut ndsdt = 49;
    for ifi in 1..=NOGRP {
        for ifj in 1..=NGRPP {
            let k = Grd::at(ifi, ifj);
            if !flreq[k] {
                continue;
            }
            if ifi == 10 {
                o.sink.refuse("ww3_outf", "output group 10 (user-defined fields)");
            }
            if grd.flogrd[k] {
                if itype != 2 {
                    o.sink.put(&format!("      {}  {}\n", idout(ifi, ifj), " "));
                } else {
                    ndsdt += 1;
                    let tabnme = format!("tab{}.ww3", ff::im(2, 2, ndsdt as i64));
                    o.sink.put(&format!("      {tabnme} : {}\n", idout(ifi, ifj)));
                    let mut w = std::io::BufWriter::new(std::fs::File::create(format!("{dir}/{tabnme}")).unwrap());
                    w.write_all(format!(" Statitics of {}\n   (time, min, max, avg, std)\n\n", idout(ifi, ifj)).as_bytes()).unwrap();
                    o.files.insert(ndsdt, w);
                }
            } else {
                o.sink.put(&format!("      {}  {}\n", idout(ifi, ifj), "*** NOT AVAILABLE ***"));
                flreq[k] = false;
            }
        }
    }
    if flog[3] {
        if p.ipart == 0 {
            o.sink.put("\n         Partitioned field data for wind seas\n");
        } else {
            o.sink.put(&format!("\n         Partitioned field data for swell field{}\n", ff::i(2, p.ipart as i64)));
        }
    }

    // 5. Time management.
    let mut iout = 0;
    if itype == 3 {
        o.sink.put("\n\n  Generating files \n --------------------------------------------------\n");
    }
    let mut st = Stat { timeid: "00000000".into(), ipass: 0 };
    loop {
        let dtest = dsec21(grd.time, tout);
        if dtest > 0. {
            if !grd.read(&m, &mut o.sink) {
                o.sink.put("\n      End of file reached \n\n");
                return finish(o.sink, o.files);
            }
            continue;
        }
        if dtest < 0. {
            tick21(&mut tout, dtreq);
            continue;
        }
        iout += 1;
        if itype == 1 {
            o.sink.put(&format!("\n\n  Output for {}\n --------------------------------------------------\n", stme21(tout)));
        } else if itype == 3 {
            o.sink.put(&format!("      Files for {}\n", stme21(tout)));
        }
        exgo(&m, &grd, &p, &flreq, &mut o, &mut st, dir);
        tick21(&mut tout, dtreq);
        if iout >= nout {
            break;
        }
    }
    if itype == 3 {
        o.sink.put(" \n");
    }
    finish(o.sink, o.files)
}

fn finish(mut sink: Sink, mut files: HashMap<i32, std::io::BufWriter<std::fs::File>>) -> i32 {
    for w in files.values_mut() {
        w.flush().unwrap();
    }
    sink.put("\n  End of program \n =========================================\n         WAVEWATCH III Field output \n\n");
    sink.flush();
    0
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    One,
    Two,
    Dir,
    Tri,
}

/// The four grid arrays and the four sea-point arrays of `W3EXGO`; they keep their values from one field to the
/// next within a call, as the Fortran's locals do.
struct Work {
    x1: Vec<Real>,
    x2: Vec<Real>,
    xx: Vec<Real>,
    xy: Vec<Real>,
    xs: [Vec<Real>; 4],
}

#[derive(Clone, Copy)]
enum G {
    X1,
    X2,
    Xx,
    Xy,
}

/// The text of one field: scale, units, name, format.
struct Spec {
    kind: Kind,
    fsc: Real,
    units: &'static str,
    ename: String,
    formf: &'static str,
}

/// `W3EXGO` (`ww3_outf.F90:546-2665`).
fn exgo(m: &ModDef, g: &Grd, p: &Params, flreq: &[bool], o: &mut Out, st: &mut Stat, dir: &str) {
    let (nx, ny, nsea) = (m.nx as usize, m.ny as usize, m.nsea as usize);
    let mx = nx + 1;
    let undef = g.undef;
    let itype = p.itype;
    let mut w = Work {
        x1: vec![undef; mx * ny],
        x2: vec![undef; mx * ny],
        xx: vec![undef; mx * ny],
        xy: vec![undef; mx * ny],
        xs: std::array::from_fn(|_| vec![undef; nsea]),
    };
    let time = g.time;
    // 1. Preparations.
    let (mut nxmax, mut nblok) = (0, 0);
    if itype == 1 {
        nxmax = if p.scale { (NCOL - 10) / 2 } else { (NCOL - 10) / 5 };
        let nxtot = 1 + (p.ixn - p.ix1) / p.ixs;
        nblok = 1 + (nxtot - 1) / nxmax;
    }
    let (mut ih, mut im, mut is) = (0, 0, 0);
    let mut ndsdt = 49;
    if itype == 2 {
        ih = time[1] / 10000;
        im = time[1] % 10000 / 100;
        is = time[1] % 100;
    }
    let mfill: i32 = -999;
    let mut fbase = String::new();
    if itype == 3 || itype == 4 {
        let oldtid = st.timeid.clone();
        st.timeid = format!("{}{}", ff::im(6, 6, (time[0] % 1000000) as i64), ff::im(2, 2, (time[1] / 10000) as i64));
        let pre = if st.timeid != oldtid {
            st.ipass = 1;
            "ww3.".to_string()
        } else {
            let s = format!("e{}.", ff::im(2, 2, st.ipass as i64));
            st.ipass += 1;
            s
        };
        fbase = format!("{pre}{}", st.timeid);
    }
    let ix_of = |ix: usize, iy: usize| ix + mx * iy;

    // 2. Loop over output fields.
    for ifi in 1..=NOGRP {
        for ifj in 1..=NGRPP {
            if !flreq[Grd::at(ifi, ifj)] {
                continue;
            }
            // 2.a Set output arrays and parameters.
            let d = |k: usize| -> Vec<Real> { g.data[&(ifi, ifj)][k][..nsea].to_vec() };
            let dp = || -> Vec<Real> { g.data[&(ifi, ifj)][0][p.ipart * nsea..(p.ipart + 1) * nsea].to_vec() };
            let mut spec = Spec { kind: Kind::One, fsc: 1., units: "", ename: String::new(), formf: "(1X,32I4)" };
            let vec_kind = if p.vector { Kind::Two } else { Kind::Dir };
            let put = |w: &mut Work, tg: G, sea: usize, v: &[Real]| {
                if itype == 4 {
                    w.xs[sea] = v.to_vec();
                } else {
                    let arr = match tg {
                        G::X1 => &mut w.x1,
                        G::X2 => &mut w.x2,
                        G::Xx => &mut w.xx,
                        G::Xy => &mut w.xy,
                    };
                    // W3S2XY.
                    for i in 0..nsea {
                        let (ix, iy) = (m.mapsf[i] as usize, m.mapsf[i + nsea] as usize);
                        arr[ix_of(ix - 1, iy - 1)] = v[i];
                    }
                }
            };
            macro_rules! one {
                ($fsc:expr, $units:expr, $en:expr, $v:expr) => {{
                    spec.kind = Kind::One;
                    spec.fsc = $fsc;
                    spec.units = $units;
                    spec.ename = $en.to_string();
                    let v: Vec<Real> = $v;
                    put(&mut w, G::X1, 0, &v);
                }};
            }
            // MOD(630.-RADE*X,360.) where X is set.
            let dir_of = |v: Vec<Real>, md: Real| -> Vec<Real> { v.into_iter().map(|x| if x != undef { modr(630. - RADE * x, md) } else { x }).collect() };
            // A vector field: raw components to XX/XY (XS1, XS2), then magnitude and direction to X1/X2 (XS3, XS4).
            macro_rules! vector {
                ($fsc:expr, $units:expr, $en:expr, $a:expr, $b:expr, $rule:expr) => {{
                    spec.kind = vec_kind;
                    spec.fsc = $fsc;
                    spec.units = $units;
                    spec.ename = $en.to_string();
                    let (mut a, mut b): (Vec<Real>, Vec<Real>) = ($a, $b);
                    put(&mut w, G::Xx, 0, &a);
                    put(&mut w, G::Xy, 1, &b);
                    for i in 0..nsea {
                        let (na, nb) = ($rule)(a[i], b[i]);
                        a[i] = na;
                        b[i] = nb;
                    }
                    put(&mut w, G::X1, 2, &a);
                    put(&mut w, G::X2, 3, &b);
                }};
            }
            let cdir = |a: Real, b: Real| -> Real { modr(630. - RADE * atan2f(b, a), 360.) };
            let norm = |a: Real, b: Real| -> Real { sqrtf(a * a + b * b) };
            match (ifi, ifj) {
                (1, 1) => {
                    let dw = d(0);
                    let mut dmax = dw[0];
                    for &v in &dw {
                        dmax = if v > dmax { v } else { dmax };
                    }
                    let mut fsc: Real = 1.;
                    if dmax > 999. {
                        fsc = 0.1;
                    } else if dmax > 99.9 {
                        fsc = 0.1;
                    } else if dmax > 9.99 {
                        fsc = 0.01;
                    }
                    if itype == 3 {
                        fsc = 0.01;
                    }
                    spec.formf = "(1X,17I7)";
                    one!(fsc, "m", ".dpt", dw);
                }
                (1, 2) => {
                    spec.formf = "(1X,17I7)";
                    vector!(0.01, "m s-1", ".cur", d(0), d(1), |a: Real, b: Real| {
                        let c = norm(a, b);
                        if c > 0.05 { (c, cdir(a, b)) } else { (c, undef) }
                    });
                }
                (1, 3) => {
                    vector!(0.1, "m s-1", ".wnd", d(0), d(1), |a: Real, b: Real| {
                        let u = norm(a, b);
                        if u > 1.0 { (u, cdir(a, b)) } else { (u, undef) }
                    });
                }
                (1, 4) => one!(0.1, "K", ".ast", d(0)),
                (1, 5) => one!(0.01, "m", ".wlv", d(0)),
                (1, 6) => one!(0.001, "1", ".ice", d(0)),
                (1, 7) => one!(0.0002, "km-1", ".ibg", d(0).into_iter().map(|x| if x != undef { x * 0.1 } else { x }).collect()),
                (1, 8) => {
                    // The Fortran passes (TAUA, TAUADIR) to ATAN2 and keeps TAUA in X1.
                    spec.kind = vec_kind;
                    spec.fsc = 0.01;
                    spec.units = "Pa";
                    spec.ename = ".taua".into();
                    let (a, mut b) = (d(0), d(1));
                    put(&mut w, G::Xx, 0, &a);
                    put(&mut w, G::Xy, 1, &b);
                    for i in 0..nsea {
                        let u = norm(a[i], b[i]);
                        b[i] = if u > 0.01 { modr(630. - RADE * atan2f(a[i], b[i]), 360.) } else { undef };
                    }
                    put(&mut w, G::X1, 2, &a);
                    put(&mut w, G::X2, 3, &b);
                }
                (1, 9) => one!(0.0001, "kg m-3", ".rhoa", d(0)),
                (2, 1) => one!(0.01, "m", ".hs", d(0)),
                (2, 2) => one!(1., "m", ".lm", d(0)),
                (2, 3) => one!(0.01, "s", ".t02", d(0)),
                (2, 4) => one!(0.01, "s", ".t0m1", d(0)),
                (2, 5) => one!(0.01, "s", ".t01", d(0)),
                (2, 6) => one!(0.001, "s-1", ".fp", d(0)),
                (2, 7) => one!(1., "degree", ".dir", dir_of(d(0), 360.)),
                (2, 8) => one!(0.1, "degree", ".spr", d(0)),
                (2, 9) => one!(1., "degree", ".dp", dir_of(d(0), 360.)),
                (2, 10) => one!(0.001, "m", ".hig", d(0)),
                (2, 11) => one!(0.002, "m", ".emc", d(0)),
                (2, 12) => one!(0.002, "m", ".smc", d(0)),
                (2, 13) => one!(0.002, "m", ".emh", d(0)),
                (2, 14) => one!(0.002, "m", ".eml", d(0)),
                (2, 15) => one!(0.002, "m", ".smh", d(0)),
                (2, 16) => one!(0.002, "m", ".sml", d(0)),
                (2, 17) => one!(0.001, "1", ".wbt", d(0)),
                (2, 18) => one!(0.01, "s", ".tp", d(0).into_iter().map(|x| if x != undef { 1.0 / x } else { undef }).collect()),
                (2, 19) => one!(0.001, "m-1", ".wnm", d(0)),
                (4, 1) => one!(0.01, "m", ".phs", dp()),
                (4, 2) => one!(0.01, "s", ".ptp", dp()),
                (4, 3) => one!(1., "m", ".plp", dp()),
                (4, 4) => one!(1., "degree", ".pdir", dir_of(dp(), 360.)),
                (4, 5) => one!(0.1, "degree", ".pspr", dp()),
                (4, 6) => one!(0.001, "1", ".pws", dp()),
                (4, 7) => one!(1.0, "degree", ".pdp", dir_of(dp(), 360.)),
                (4, 8) => one!(0.01, "1", ".pqp", dp()),
                (4, 9) => one!(0.01, "1", ".ppe", dp()),
                (4, 10) => one!(0.0001, "s-1", ".pgw", dp()),
                (4, 11) => one!(0.0001, "1", ".psw", dp()),
                (4, 12) => one!(0.01, "s", ".ptm10", dp()),
                (4, 13) => one!(0.01, "s", ".pt01", dp()),
                (4, 14) => one!(0.01, "s", ".pt02", dp()),
                (4, 15) => one!(0.02, "m2 s rad-1", ".pep", dp()),
                (4, 16) => one!(0.001, "1", ".tws", d(0)),
                (4, 17) => one!(1., "1", ".pnr", d(0)),
                (5, 1) => {
                    spec.formf = "(1X,20I6)";
                    spec.kind = vec_kind;
                    spec.fsc = 0.001;
                    spec.units = "m s-1";
                    spec.ename = ".ust".into();
                    let (mut a, mut b) = (d(0), d(1));
                    put(&mut w, G::Xx, 0, &a);
                    put(&mut w, G::Xy, 1, &b);
                    for i in 0..nsea {
                        let mut u = norm(a[i], b[i]);
                        if a[i] == undef {
                            b[i] = undef;
                            u = undef;
                        } else if u > 0.05 {
                            b[i] = cdir(a[i], b[i]);
                        } else {
                            b[i] = undef;
                        }
                        a[i] = u;
                    }
                    put(&mut w, G::X1, 2, &a);
                    put(&mut w, G::X2, 3, &b);
                }
                (5, 2) => one!(1.0e-6, "1", ".cha", d(0)),
                (5, 3) => one!(0.1, "kW m-1", ".cge", d(0).into_iter().map(|x| if x != undef { 0.001 * x } else { x }).collect()),
                (5, 4) => one!(0.01, "W m-2", ".faw", d(0).into_iter().map(|x| if x < 99.98 { x } else { 99.98 }).collect()),
                (5, 5) | (5, 6) | (6, 2) | (6, 13) => {
                    let (en, thr): (&str, Option<Real>) = match (ifi, ifj) {
                        (5, 5) => (".taw", None),
                        (5, 6) => (".twa", None),
                        (6, 2) => (".two", Some(1.0e-8)),
                        _ => (".toc", Some(1.0e-8)),
                    };
                    vector!(1.0e-6, "m2 s-2", en, d(0), d(1), |a: Real, b: Real| {
                        let c = norm(a, b);
                        match thr {
                            None => {
                                if a == undef {
                                    (undef, undef)
                                } else if a == 0. && b == 0. {
                                    (c, undef)
                                } else {
                                    (c, cdir(a, b))
                                }
                            }
                            Some(t) => {
                                if a == undef {
                                    (undef, undef)
                                } else if c > t {
                                    (c, cdir(a, b))
                                } else {
                                    (c, undef)
                                }
                            }
                        }
                    });
                }
                (5, 7) => one!(0.001, "1", ".wcc", d(0)),
                (5, 8) => one!(0.1, "m", ".wcf", d(0)),
                (5, 9) => one!(0.1, "m", ".wch", d(0)),
                (5, 10) => one!(0.1, "1", ".wcm", d(0)),
                (6, 1) => {
                    spec.kind = Kind::Tri;
                    spec.fsc = 10.;
                    spec.units = "N m-1";
                    spec.ename = ".sxy".into();
                    put(&mut w, G::X1, 0, &d(0));
                    put(&mut w, G::X2, 1, &d(1));
                    put(&mut w, G::Xy, 2, &d(2));
                }
                (6, 3) => one!(0.001, "N m-1", ".bhd", d(0)),
                (6, 4) => one!(0.1, "W m-2", ".foc", d(0).into_iter().map(|x| if x < 99.98 { x } else { 99.98 }).collect()),
                (6, 5) => vector!(0.001, "m2 s-1", ".tus", d(0), d(1), |a: Real, b: Real| {
                    let c = norm(a, b);
                    if a != undef { (c, cdir(a, b)) } else { (undef, undef) }
                }),
                (6, 6) => {
                    spec.kind = vec_kind;
                    spec.fsc = 0.001;
                    spec.units = "m s-1";
                    spec.ename = ".uss".into();
                    let (mut a, mut b) = (d(0), d(1));
                    for i in 0..nsea {
                        if a[i] != undef {
                            let cl = |v: Real| {
                                let t = if v < 0.9998 { v } else { 0.9998 };
                                if t > -0.9998 { t } else { -0.9998 }
                            };
                            a[i] = cl(a[i]);
                            b[i] = cl(b[i]);
                        }
                    }
                    put(&mut w, G::Xx, 0, &a);
                    put(&mut w, G::Xy, 1, &b);
                    for i in 0..nsea {
                        let c = norm(a[i], b[i]);
                        if a[i] != undef {
                            b[i] = cdir(a[i], b[i]);
                            a[i] = c;
                        } else {
                            b[i] = undef;
                            a[i] = undef;
                        }
                    }
                    put(&mut w, G::X1, 2, &a);
                    put(&mut w, G::X2, 3, &b);
                }
                (6, 7) => {
                    spec.kind = Kind::Two;
                    spec.fsc = 0.01;
                    spec.units = "m4";
                    spec.ename = ".p2s".into();
                    put(&mut w, G::X1, 0, &d(0));
                    put(&mut w, G::X2, 1, &d(1));
                }
                (7, 1) | (7, 2) => {
                    let (en, units) = if ifj == 1 { (".abr", "m") } else { (".ubr", "m s-1") };
                    vector!(0.01, units, en, d(0), d(1), |a: Real, b: Real| {
                        if a != undef {
                            let u = norm(a, b);
                            if u > 0.005 { (u, cdir(a, b)) } else { (u, undef) }
                        } else {
                            (a, b)
                        }
                    });
                }
                (7, 3) => {
                    spec.kind = Kind::Tri;
                    spec.fsc = 1.0e-2;
                    spec.units = "m";
                    spec.ename = ".bed".into();
                    put(&mut w, G::X1, 0, &d(0));
                    put(&mut w, G::X2, 1, &d(1));
                    put(&mut w, G::Xy, 2, &d(2));
                }
                (7, 4) => one!(0.1, "W m-2", ".fbb", d(0)),
                (7, 5) => {
                    spec.kind = Kind::Two;
                    spec.fsc = 1.0e-6;
                    spec.units = "m2 s-2";
                    spec.ename = ".tbb".into();
                    put(&mut w, G::Xx, 0, &d(0));
                    put(&mut w, G::Xy, 1, &d(1));
                }
                (8, 1) => {
                    spec.kind = vec_kind;
                    spec.fsc = 1.0e-6;
                    spec.ename = ".mss".into();
                    spec.formf = "(1X,20I6)";
                    spec.units = "1";
                    put(&mut w, G::Xx, 0, &d(0));
                    put(&mut w, G::Xy, 1, &d(1));
                }
                (8, 2) => {
                    spec.kind = vec_kind;
                    spec.fsc = 0.00001;
                    spec.units = "1";
                    spec.ename = ".msc".into();
                    let (mut a, mut b) = (d(0), d(1));
                    put(&mut w, G::Xx, 0, &a);
                    put(&mut w, G::Xy, 1, &b);
                    for i in 0..nsea {
                        let c = norm(a[i], b[i]);
                        let (na, nb) = if a[i] == undef {
                            (undef, undef)
                        } else if a[i] == 0. && b[i] == 0. {
                            (c, undef)
                        } else {
                            (c, cdir(a[i], b[i]))
                        };
                        a[i] = na;
                        b[i] = nb;
                    }
                    put(&mut w, G::X1, 2, &a);
                    put(&mut w, G::X2, 3, &b);
                }
                (8, 3) => one!(0.1, "degree", ".msd", dir_of(d(0), 180.)),
                (8, 4) => one!(0.1, "degree", ".mcd", dir_of(d(0), 180.)),
                (8, 5) => one!(0.01, "1", ".qp", d(0)),
                (9, 1) => one!(0.1, "min.", ".dtd", d(0).into_iter().map(|x| if x != undef { x / 60. } else { x }).collect()),
                (9, 2) => one!(0.001, "s-1", ".fc", d(0)),
                (9, 3) => one!(1., "1", ".cfx", d(0)),
                (9, 4) => one!(0.001, "1", ".cfd", d(0)),
                (9, 5) => one!(0.001, "1", ".cfk", d(0)),
                _ => {
                    o.sink.put(&format!("\n *** WAVEWATCH III ERROR IN W3EXGO :\n     GROUP{} PARAMETER{} NOT LISTED \n", ff::i(2, ifi as i64), ff::i(3, ifj as i64)));
                    o.sink.put("\n *** WAVEWATCH III ERROR IN W3EXGO :\n     PLEASE UPDATE FIELDS !!! \n\n");
                    o.sink.exit(1)
                }
            }
            let (flone, fltwo, fltri) = (spec.kind == Kind::One, spec.kind == Kind::Two, spec.kind == Kind::Tri);

            // 2.b Make map.
            let mut map = vec![0i32; mx * ny];
            let mut mp2 = vec![0i32; mx * ny];
            for ix in 0..nx {
                for iy in 0..ny {
                    let k = ix_of(ix, iy);
                    if g.mapsta[iy + ny * ix] == 0 {
                        w.x1[k] = undef;
                        w.x2[k] = undef;
                        w.xx[k] = undef;
                        w.xy[k] = undef;
                    }
                    map[k] = if w.x1[k] == undef { 0 } else { 1 };
                    mp2[k] = if w.x2[k] == undef { 0 } else { 1 };
                }
            }
            let ename = spec.ename.as_str();
            let fsc = spec.fsc;
            let mapsta = |ix: usize, iy: usize| g.mapsta[iy + ny * ix];
            let units10 = ff::a(10, spec.units);

            if itype == 1 {
                let (fsc1, fsca) = if p.scale { (0., 0.) } else { (fsc, 1.) };
                let mut ixb = p.ix1 - p.ixs;
                for _ in 0..nblok {
                    let ixa = ixb + p.ixs;
                    ixb = ixa + (nxmax - 1) * p.ixs;
                    ixb = ixb.min(p.ixn);
                    let args = |f: &[Real], mp: &[i32], fs: Real, units: &str| {
                        prtblk(nx, ny, mx, f, mp, 0, fs, (ixa, ixb, p.ixs), (p.iy1, p.iyn, p.iys), &idout(ifi, ifj), units)
                    };
                    let s = if fltri {
                        args(&w.x1, &map, fsc1, &units10) + &args(&w.x2, &map, fsc1, &units10) + &args(&w.xy, &map, fsc1, &units10)
                    } else if flone {
                        args(&w.x1, &map, fsc1, &units10)
                    } else if fltwo {
                        args(&w.xx, &map, fsc1, &units10) + &args(&w.xy, &map, fsc1, &units10)
                    } else {
                        args(&w.x1, &map, fsc1, &units10) + &args(&w.x2, &mp2, fsca, "Deg.")
                    };
                    o.sink.put(&s);
                }
            } else if itype == 2 {
                let (mut xmin, mut xmax): (Real, Real) = (1.0e20, -1.0e20);
                let (mut xds, mut xdsq) = (0f64, 0f64);
                let mut ningrd = 0;
                for ix in p.ix1..=p.ixn {
                    for iy in p.iy1..=p.iyn {
                        let (jx, jy) = (ix as usize - 1, iy as usize - 1);
                        let v = w.x1[ix_of(jx, jy)];
                        if mapsta(jx, jy) > 0 && v != undef {
                            ningrd += 1;
                            xmin = if v < xmin { v } else { xmin };
                            xmax = if v > xmax { v } else { xmax };
                            xds += v as f64;
                            xdsq += (v as f64) * (v as f64);
                        }
                    }
                }
                ndsdt += 1;
                let head = format!(" {}{}{}{}", ff::i(8, time[0] as i64), ff::im(3, 2, ih as i64), ff::im(3, 2, im as i64), ff::im(3, 2, is as i64));
                let line = if ningrd == 0 {
                    head
                } else if ningrd <= 2 {
                    format!("{head}  {}{}", e(12, 4, xmin), e(12, 4, xmax))
                } else {
                    let xavg = (xds / ningrd as f64) as Real;
                    let xstd = ((xdsq - xds * xds / ningrd as f64) / (ningrd - 1) as f64) as Real;
                    let xstd = sqrtf(if xstd > 0. { xstd } else { 0. });
                    format!("{head}  {}{}{}{}", e(12, 4, xmin), e(12, 4, xmax), e(12, 4, xavg), e(12, 4, xstd))
                };
                let wr = o.files.get_mut(&ndsdt).expect("statistics file");
                wr.write_all(format!("{line}\n").as_bytes()).unwrap();
            } else if itype == 3 {
                let path = format!("{dir}/{fbase}{ename}");
                let mut tf = Tf::new(&path);
                let (iy1, iyn, ix1, ixn) = (p.iy1 as usize, p.iyn as usize, p.ix1 as usize, p.ixn as usize);
                let range = |a: &[Real]| -> (Real, Real) {
                    let (mut lo, mut hi) = (a[iy1 - 1 + ny * (ix1 - 1)], a[iy1 - 1 + ny * (ix1 - 1)]);
                    for ix in ix1..=ixn {
                        for iy in iy1..=iyn {
                            let v = a[iy - 1 + ny * (ix - 1)];
                            lo = if v < lo { v } else { lo };
                            hi = if v > hi { v } else { hi };
                        }
                    }
                    (lo, hi)
                };
                let (xlo, xhi) = range(&m.xgrd);
                let (ylo, yhi) = range(&m.ygrd);
                let fileid = "WAVEWATCH III";
                let nxs = p.ixn - p.ix1 + 1;
                let nys = p.iyn - p.iy1 + 1;
                if p.idfm == 3 {
                    let mut b = fileid.as_bytes().to_vec();
                    for v in time {
                        b.extend_from_slice(&v.to_be_bytes());
                    }
                    // XGRD and YGRD are DOUBLE PRECISION.
                    b.extend_from_slice(&(xlo as f64).to_be_bytes());
                    b.extend_from_slice(&(xhi as f64).to_be_bytes());
                    b.extend_from_slice(&nxs.to_be_bytes());
                    b.extend_from_slice(&(ylo as f64).to_be_bytes());
                    b.extend_from_slice(&(yhi as f64).to_be_bytes());
                    b.extend_from_slice(&nys.to_be_bytes());
                    b.extend_from_slice(ff::a(7, ename).as_bytes());
                    b.extend_from_slice(&fsc.to_be_bytes());
                    b.extend_from_slice(units10.as_bytes());
                    b.extend_from_slice(&p.idla.to_be_bytes());
                    b.extend_from_slice(&p.idfm.to_be_bytes());
                    b.extend_from_slice(ff::a(11, spec.formf).as_bytes());
                    b.extend_from_slice(&mfill.to_be_bytes());
                    tf.rec(&b);
                } else {
                    let fscs = if fsc < 1.0e-4 { ff::g(7, 1, fsc as f64) } else { f(7, 4, fsc) };
                    let tail = format!(
                        "{} {} {}",
                        ff::a(4, ename),
                        ff::a(7, &fscs),
                        units10
                    );
                    let dla = format!("{}{}", ff::i(2, p.idla as i64), ff::i(2, p.idfm as i64));
                    tf.line(&header(m.flagll, fileid, time, (xlo, xhi, nxs), (ylo, yhi, nys), &tail, &dla, spec.formf, mfill));
                }
                let fmt = FormF::parse(spec.formf);
                let ints = |srcs: &[&Vec<Real>], gate: &Vec<Real>| -> Vec<Vec<i32>> {
                    let mut outs = vec![vec![0i32; nx * ny]; srcs.len()];
                    for ix in ix1..=ixn {
                        for iy in iy1..=iyn {
                            let (jx, jy) = (ix - 1, iy - 1);
                            let bad = mapsta(jx, jy) <= 0 || gate[ix_of(jx, jy)] == undef;
                            for (k, s) in srcs.iter().enumerate() {
                                outs[k][jy + ny * jx] = if bad { mfill } else { nint(s[ix_of(jx, jy)] / fsc) };
                            }
                        }
                    }
                    outs
                };
                let at = |a: &[i32], ix: usize, iy: usize| a[iy - 1 + ny * (ix - 1)];
                let xy_g = |ix: usize, iy: usize| (m.xgrd[iy - 1 + ny * (ix - 1)], m.ygrd[iy - 1 + ny * (ix - 1)]);
                if fltri {
                    let outs = ints(&[&w.x1, &w.x2, &w.xy], &w.xx);
                    if p.idla != 5 {
                        for a in &outs {
                            tf.outa2i(a, ny, p, &fmt);
                        }
                    } else {
                        for iy in iy1..=iyn {
                            let ygbx = m.y0 + (iy - 1) as Real * m.sy;
                            for ix in ix1..=ixn {
                                let xgbx = m.x0 + (ix - 1) as Real * m.sx;
                                if at(&outs[0], ix, iy) != mfill {
                                    tf.pair(p.idfm == 3, xgbx, ygbx, &[at(&outs[0], ix, iy), at(&outs[1], ix, iy)], Pf::F951);
                                }
                            }
                        }
                    }
                } else if fltwo || spec.kind == Kind::Dir {
                    let outs = ints(&[&w.xx, &w.xy], &w.xx);
                    if p.idla != 5 {
                        for a in &outs {
                            tf.outa2i(a, ny, p, &fmt);
                        }
                    } else {
                        for iy in iy1..=iyn {
                            for ix in ix1..=ixn {
                                let (xg, yg) = xy_g(ix, iy);
                                if at(&outs[0], ix, iy) != mfill {
                                    tf.pair(p.idfm == 3, xg, yg, &[at(&outs[0], ix, iy), at(&outs[1], ix, iy)], if m.flagll { Pf::F951 } else { Pf::E961 });
                                }
                            }
                        }
                    }
                } else {
                    let outs = ints(&[&w.x1], &w.x1);
                    if p.idla != 5 {
                        tf.outa2i(&outs[0], ny, p, &fmt);
                    } else {
                        for iy in iy1..=iyn {
                            for ix in ix1..=ixn {
                                let (xg, yg) = xy_g(ix, iy);
                                if at(&outs[0], ix, iy) != mfill {
                                    tf.pair(p.idfm == 3, xg, yg, &[at(&outs[0], ix, iy)], if m.flagll { Pf::F951 } else { Pf::E961 });
                                }
                            }
                        }
                    }
                }
                tf.close();
            } else if itype == 4 {
                let fname16 = format!("{fbase}{}", ff::a(7, ename));
                o.sink.put(&format!(" {}\n", &fname16[..16]));
                let path = format!("{dir}/{fbase}{ename}");
                let mut tf = Tf::new(&path);
                let n = match spec.kind {
                    Kind::Tri => 3,
                    Kind::Two | Kind::Dir => 2,
                    Kind::One => 1,
                };
                let head = format!(
                    " {}{}{}{}{}{} {} {} {}",
                    "WAVEWATCH III",
                    ff::im(9, 8, time[0] as i64),
                    ff::im(7, 6, time[1] as i64),
                    ff::i(9, nsea as i64),
                    ff::i(3, n as i64),
                    ff::es(10, 2, fsc as f64),
                    ff::a(4, ename),
                    ff::a(10, spec.units),
                    ff::a(30, &m.gname)
                );
                tf.line(&head);
                for k in 0..n as usize {
                    let it: Vec<String> = w.xs[k].iter().map(|&x| ff::es(11, 3, x as f64)).collect();
                    tf.line(&it.chunks(10).map(|c| c.concat()).collect::<Vec<_>>().join("\n"));
                }
                tf.close();
            }
        }
    }
}

/// Formats 950 and 960 of `ww3_outf.F90:2629-2633`.
#[allow(clippy::too_many_arguments)]
fn header(ll: bool, fileid: &str, time: Time, xr: (Real, Real, i32), yr: (Real, Real, i32), tail: &str, dla: &str, formf: &str, mfill: i32) -> String {
    let t9 = ff::im(9, 8, time[0] as i64);
    let t7 = ff::im(7, 6, time[1] as i64);
    if ll {
        // 950: 1X,A13,1X,I9.8,1X,I7.6,2(1X,2F8.2,1X,I4),1X,A4,1X,A7,1X,A10,1X,2I2,1X,A11,1X,I4
        format!(
            " {fileid} {t9} {t7} {}{} {} {}{} {} {tail} {dla} {} {}",
            f(8, 2, xr.0), f(8, 2, xr.1), ff::i(4, xr.2 as i64),
            f(8, 2, yr.0), f(8, 2, yr.1), ff::i(4, yr.2 as i64),
            ff::a(11, formf), ff::i(4, mfill as i64)
        )
    } else {
        // 960: 1X,A13,I9.8,I7.6,2(2E11.3,I4),1X,A4,1X,A7,1X,A10,2I2,1X,A11,I4
        format!(
            " {fileid}{t9}{t7}{}{}{}{}{}{} {tail}{dla} {}{}",
            e(11, 3, xr.0), e(11, 3, xr.1), ff::i(4, xr.2 as i64),
            e(11, 3, yr.0), e(11, 3, yr.1), ff::i(4, yr.2 as i64),
            ff::a(11, formf), ff::i(4, mfill as i64)
        )
    }
}

/// `(1X,nnIww)`, the three formats `W3EXGO` sets.
struct FormF {
    n: usize,
    w: usize,
}

impl FormF {
    fn parse(s: &str) -> Self {
        let b = s.trim_start_matches("(1X,").trim_end_matches(')');
        let (n, w) = b.split_once('I').expect("FORMF");
        Self { n: n.parse().unwrap(), w: w.parse().unwrap() }
    }
}

/// How `IDLA = 5` writes a position: `(1X,2F10.5,2I8)` or `(1X,2E12.4,2I8)`.
enum Pf {
    F951,
    E961,
}

/// A transfer file: formatted lines or unformatted records.
struct Tf {
    w: std::io::BufWriter<std::fs::File>,
}

impl Tf {
    fn new(path: &str) -> Self {
        Self { w: std::io::BufWriter::new(std::fs::File::create(path).unwrap_or_else(|e| panic!("{path}: {e}"))) }
    }
    fn line(&mut self, s: &str) {
        self.w.write_all(s.as_bytes()).unwrap();
        self.w.write_all(b"\n").unwrap();
    }
    fn rec(&mut self, b: &[u8]) {
        let n = (b.len() as i32).to_be_bytes();
        self.w.write_all(&n).unwrap();
        self.w.write_all(b).unwrap();
        self.w.write_all(&n).unwrap();
    }
    fn close(&mut self) {
        self.w.flush().unwrap();
    }

    /// `OUTA2I` (`w3arrymd.F90:625-778`) with `VSC = 1`, `VOF = 0`; `arr` is `(IY,IX)` with `ny` rows.
    fn outa2i(&mut self, arr: &[i32], ny: usize, p: &Params, fmt: &FormF) {
        let (lx, hx, ly, hy) = (p.ix1 as usize, p.ixn as usize, p.iy1 as usize, p.iyn as usize);
        let idfm = if (1..=3).contains(&p.idfm) { p.idfm } else { 1 };
        let idla = if (1..=4).contains(&p.idla) { p.idla } else { 1 };
        let row = |iy: usize| -> Vec<i32> { (lx..=hx).map(|ix| arr[iy - 1 + ny * (ix - 1)]).collect() };
        let ys: Vec<usize> = if idla == 1 || idla == 2 { (ly..=hy).collect() } else { (ly..=hy).rev().collect() };
        let groups: Vec<Vec<i32>> = if idla == 2 || idla == 4 { vec![ys.iter().flat_map(|&y| row(y)).collect()] } else { ys.iter().map(|&y| row(y)).collect() };
        for gv in groups {
            match idfm {
                1 => self.line(&gv.iter().map(|&v| ff::i(12, v as i64)).collect::<String>()),
                2 => {
                    for c in gv.chunks(fmt.n) {
                        self.line(&format!(" {}", c.iter().map(|&v| ff::i(fmt.w, v as i64)).collect::<String>()));
                    }
                }
                _ => {
                    let b: Vec<u8> = gv.iter().flat_map(|v| v.to_be_bytes()).collect();
                    self.rec(&b);
                }
            }
        }
    }

    /// The `IDLA = 5` line: a position and one or two integers.
    fn pair(&mut self, unf: bool, x: Real, y: Real, v: &[i32], pf: Pf) {
        if unf {
            let mut b = Vec::new();
            b.extend_from_slice(&x.to_be_bytes());
            b.extend_from_slice(&y.to_be_bytes());
            for i in v {
                b.extend_from_slice(&i.to_be_bytes());
            }
            self.rec(&b);
        } else {
            let ints: String = v.iter().map(|&i| ff::i(8, i as i64)).collect();
            match pf {
                Pf::F951 => self.line(&format!(" {}{}{ints}", f(10, 5, x), f(10, 5, y))),
                Pf::E961 => self.line(&format!(" {}{}{ints}", e(12, 4, x), e(12, 4, y))),
            }
        }
    }
}

/// `PRTBLK` (`w3arrymd.F90:1110-1362`); `f_` and `map` are `(MX,NY)`, `ix` runs fastest.
#[allow(clippy::too_many_arguments)]
fn prtblk(nx: usize, ny: usize, mx: usize, f_: &[Real], map: &[i32], map0: i32, fsc: Real, (ix1, ix2, ix3): (i32, i32, i32), (iy1, iy2, iy3): (i32, i32, i32), prvar: &str, prunit: &str) -> String {
    let mut out = String::new();
    let at = |ix: i32, iy: i32| (ix - 1) as usize + mx * (iy - 1) as usize;
    let xs: Vec<i32> = (0..).map(|k| ix1 + k * ix3).take_while(|&x| x <= ix2).collect();
    let lx0 = xs.len();
    if fsc <= 0. {
        let mut fmax: Real = 1.0e-15;
        for ix in 1..=nx {
            for iy in 1..=ny {
                let k = ix - 1 + mx * (iy - 1);
                if map[k] != map0 {
                    let v = f_[k].abs();
                    fmax = if v > fmax { v } else { fmax };
                }
            }
        }
        out.push_str(&format!("\n Variable: {prvar} Max.: {} {prunit}\n", e(10, 3, fmax)));
        out.push_str(" \n");
        let h: String = xs.iter().step_by(2).map(|&x| ff::i(8, x as i64)).collect();
        out.push_str(&format!("      {h}\n"));
        out.push_str(&format!("      {}{}{}\n", " +", "--".repeat(lx0), "-+"));
        let mut jm = 0;
        let mut iy = iy2;
        let mut last_lx = lx0;
        while iy >= iy1 {
            let mut cells = String::new();
            for &ix in &xs {
                if map[at(ix, iy)] == map0 {
                    cells.push_str("  ");
                } else {
                    let rr = 10. * f_[at(ix, iy)] / fmax;
                    let mut c = ff::i(2, (rr * 1.000001) as i64);
                    if c == "10" || c == "**" || f_[at(ix, iy)] == fmax {
                        c = if rr < 0. { "-*".into() } else { " *".into() };
                    }
                    cells.push_str(&c);
                }
            }
            last_lx = xs.len();
            let stra = if jm == 0 {
                jm = 2;
                ff::i(5, iy as i64)
            } else {
                jm -= 1;
                "     ".into()
            };
            out.push_str(&format!(" {stra} |{cells} |\n"));
            iy -= iy3;
        }
        out.push_str(&format!("      {}{}{}\n", " +", "--".repeat(last_lx), "-+"));
        out.push_str(&format!("      {h}\n"));
        out.push_str(" \n");
    } else {
        out.push_str(&format!("\n Variable: {prvar} Units: {} {prunit}\n", e(10, 3, fsc)));
        out.push_str(" \n");
        let h: String = xs.iter().map(|&x| ff::i(5, x as i64)).collect();
        out.push_str(&format!("      {h}\n"));
        out.push_str(&format!(" {}{}{}{}\n", "   ", " +", "-----".repeat(lx0), "-+   "));
        let mut jm = 0;
        let mut iy = iy2;
        while iy >= iy1 {
            let stra3 = if jm == 0 {
                jm = 2;
                ff::i(3, iy as i64)
            } else {
                jm -= 1;
                "   ".into()
            };
            let mut cells = String::new();
            for &ix in &xs {
                if map[at(ix, iy)] == map0 {
                    cells.push_str("     ");
                } else {
                    cells.push_str(&ff::i(5, nint(f_[at(ix, iy)] / fsc) as i64));
                }
            }
            out.push_str(&format!(" {stra3} |{cells} |   \n"));
            iy -= iy3;
        }
        out.push_str(&format!(" {}{}{}{}\n", "   ", " +", "-----".repeat(lx0), "-+   "));
        out.push_str(&format!("      {h}\n"));
        out.push_str(" \n");
    }
    out
}
