// SPDX-License-Identifier: MPL-2.0
//! `W3IOTR` (`model/src/w3iotrmd.F90` at `92e2cf66`) for `switch_chaceon_shrd`: spectra along tracks, from `track_i.ww3`
//! to `track_o.ww3`, for rectilinear and curvilinear grids. Records are big-endian. A line with more than five items
//! makes `STRSPLIT` write past `LIST(5)` in the Fortran; here the rest are dropped ⚠.

use crate::gridsru::{Gsu, w3gfcl_r4};
use crate::iobc::{rec, text};
use crate::moddef::{ModDef, Records};
use crate::wtime::{Time, dsec21, tick21};
use std::io::{BufWriter, Write};
use ww3_kernels::constants::{PI, TPI, TPIINV};
use ww3_kernels::real::Real;
use ww3_libm::sincosf;

const IDSTRI: &str = "WAVEWATCH III TRACK LOCATIONS DATA";
const IDSTRO: &str = "WAVEWATCH III TRACK OUTPUT SPECTRA";
const RDCHCK: Real = 0.05;
const RTCHCK: Real = 0.05;

/// `track_i.ww3`. `BACKSPACE` is a line or record left in `peek`.
enum Input {
    Lines(std::vec::IntoIter<String>),
    Binary(Records),
}

struct Entry {
    time: Time,
    x: Real,
    y: Real,
    id: String,
}

fn parse_real(t: &str) -> Real {
    t.trim().replace(['d', 'D'], "e").parse().unwrap_or_else(|_| {
        eprintln!("Fortran runtime error: Bad real number in item 1 of list input");
        std::process::exit(2);
    })
}

/// `READ (LIST(n),'(Iw)')`: the first `w` columns.
fn parse_int(t: &str, w: usize) -> i32 {
    let c: String = t.chars().take(w).collect();
    c.trim().parse().unwrap_or_else(|_| {
        eprintln!("Fortran runtime error: Bad value during integer read");
        std::process::exit(2);
    })
}

impl Input {
    fn next(&mut self) -> Option<Entry> {
        match self {
            Input::Lines(l) => {
                let line = l.next()?;
                let line: String = line.chars().take(1024).collect();
                let mut t = line.split(' ').filter(|s| !s.is_empty()).map(|s| s.chars().take(100).collect::<String>());
                let (a, b, c, d) = (t.next().unwrap_or_default(), t.next().unwrap_or_default(), t.next().unwrap_or_default(), t.next().unwrap_or_default());
                Some(Entry { time: [parse_int(&a, 8), parse_int(&b, 6)], x: parse_real(&c), y: parse_real(&d), id: t.next().unwrap_or_default().chars().take(32).collect() })
            }
            Input::Binary(r) => {
                let mut c = r.try_next()?;
                let time = [c.i(), c.i()];
                let (x, y) = (c.f(), c.f());
                Some(Entry { time, x, y, id: c.text(32).trim_end().to_string() })
            }
        }
    }
}

/// What `W3IOTR` reads of the model, by `ISEA` from 0 and `IXY` from 0.
pub struct Fields<'a> {
    pub time: Time,
    pub mapsta: &'a [i32],
    pub mapst2: &'a [i32],
    pub va: &'a [Real],
    pub cg: &'a [Real],
    pub dw: &'a [Real],
    pub cx: &'a [Real],
    pub cy: &'a [Real],
    pub ua: &'a [Real],
    pub ud: &'a [Real],
    pub ust: &'a [Real],
    pub asv: &'a [Real],
    pub dpdx: &'a [Real],
    pub dpdy: &'a [Real],
    pub dqdx: &'a [Real],
    pub dqdy: &'a [Real],
}

pub struct Track {
    dir: String,
    /// `NDSINP .GT. 0`.
    formi: bool,
    root: bool,
    ipass: i32,
    /// `STOP`: the input has ended.
    stop: bool,
    input: Option<Input>,
    peek: Option<Entry>,
    out: Option<BufWriter<std::fs::File>>,
    gsu: Option<Gsu>,
    mask2: Vec<bool>,
    trckid: Vec<String>,
}

fn state(sta: i32, st2: i32) -> &'static str {
    match sta {
        0 if st2 == 0 => "LND",
        0 => "XCL",
        s if s < 0 && st2 == 1 => "ICE",
        s if s < 0 && st2 == 2 => "DRY",
        s if s < 0 => "DIS",
        _ => "SEA",
    }
}

impl Track {
    pub fn new(dir: &str, formi: bool, root: bool) -> Self {
        Self { dir: dir.to_string(), formi, root, ipass: 0, stop: false, input: None, peek: None, out: None, gsu: None, mask2: Vec::new(), trckid: Vec::new() }
    }

    /// `IPASS` 1. False on labels 800 to 803, which end the output.
    fn open(&mut self, m: &ModDef) -> bool {
        let path = format!("{}/track_i.ww3", self.dir);
        let msg = |what: &str| eprintln!("\n *** WAVEWATCH III ERROR IN W3IOTR : \n     {what}\n");
        if !std::path::Path::new(&path).exists() {
            msg("ERROR IN OPENING INPUT FILE");
            return false;
        }
        if self.formi {
            let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
            let mut lines: Vec<String> = text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
            if lines.last().is_some_and(|l| l.is_empty()) {
                lines.pop();
            }
            if lines.is_empty() {
                msg("PREMATURE END OF INPUT FILE");
                return false;
            }
            let idtst: String = format!("{:<34}", lines.remove(0)).chars().take(34).collect();
            if idtst != IDSTRI {
                msg("UNEXPECTED FILE ID");
                return false;
            }
            self.input = Some(Input::Lines(lines.into_iter()));
        } else {
            let mut r = Records::open(&path);
            let idtst = match r.try_next() {
                Some(mut c) => c.text(34),
                None => {
                    msg("PREMATURE END OF INPUT FILE");
                    return false;
                }
            };
            if idtst != IDSTRI {
                msg("UNEXPECTED FILE ID");
                return false;
            }
            self.input = Some(Input::Binary(r));
        }
        if self.root {
            let nk = m.nk as usize;
            let mut f = BufWriter::new(std::fs::File::create(format!("{}/track_o.ww3", self.dir)).unwrap());
            let mut h = Vec::new();
            text(&mut h, IDSTRO, 34);
            h.extend((m.flagll as i32).to_be_bytes());
            h.extend(m.nk.to_be_bytes());
            h.extend(m.nth.to_be_bytes());
            h.extend(m.xfr.to_be_bytes());
            rec(&mut f, &h);
            let mut p = Vec::new();
            p.extend((0.5 * PI - m.th[0]).to_be_bytes());
            p.extend((-m.dth).to_be_bytes());
            for ik in 1..=nk {
                p.extend((m.sig[ik] * TPIINV).to_be_bytes());
            }
            for ik in 1..=nk {
                p.extend((m.dsip[ik] * TPIINV).to_be_bytes());
            }
            rec(&mut f, &p);
            self.out = Some(f);
        }
        let n = (m.nx * m.ny) as usize;
        self.mask2 = vec![false; n];
        self.trckid = vec![String::new(); n];
        let xg: Vec<f64> = m.xgrd.iter().map(|x| *x as f64).collect();
        let yg: Vec<f64> = m.ygrd.iter().map(|y| *y as f64).collect();
        self.gsu = Some(Gsu::create(false, m.flagll, m.iclose, m.nx, m.ny, xg, yg));
        true
    }

    /// One `W3IOTR`, with `TOLAST` and `DTOUT` of type 3. False on an error, when `ATOLAST` becomes `TIME`.
    pub fn write(&mut self, m: &ModDef, f: &Fields, tolast: Time, dtout: Real) -> bool {
        let (ny, nk, nth) = (m.ny as usize, m.nk as usize, m.nth as usize);
        let (nspec, nk2) = (nk * nth, nk + 2);
        self.ipass += 1;
        if self.ipass == 1 && !self.open(m) {
            return false;
        }
        let mut mask1 = self.mask2.clone();
        self.mask2.fill(false);
        let (timeb, mut timee) = (f.time, f.time);
        tick21(&mut timee, dtout);
        if dsec21(timee, tolast) < 0. {
            timee = tolast;
        }
        let at = |ix: i32, iy: i32| (iy - 1) as usize + (ix - 1) as usize * ny;
        if !self.stop {
            loop {
                let e = match self.peek.take() {
                    Some(e) => Some(e),
                    None => self.input.as_mut().unwrap().next(),
                };
                let Some(e) = e else {
                    self.stop = true;
                    break;
                };
                if dsec21(timeb, e.time) < 0. {
                    continue;
                }
                if dsec21(timee, e.time) > 0. {
                    self.peek = Some(e);
                    break;
                }
                let flag1 = dsec21(e.time, timee) > RTCHCK * dtout;
                let flag2 = dsec21(timeb, e.time) > RTCHCK * dtout;
                let (mut xt, yt) = (e.x, e.y);
                let Some((mut ixx, mut iyy, xx, yy)) = w3gfcl_r4(self.gsu.as_ref().unwrap(), &mut xt, yt) else { continue };
                // The corners in the order the masks take them.
                ixx.swap(2, 3);
                iyy.swap(2, 3);
                if m.cmprtrck {
                    let c = at(ixx[0], iyy[0]);
                    let rd = f.dpdx[c] * (xt - xx[0]) + f.dpdy[c] * (yt - yy[0]);
                    if rd < RDCHCK {
                        ixx[1] = ixx[0];
                        ixx[3] = ixx[2];
                    } else if rd > 1. - RDCHCK {
                        ixx[0] = ixx[1];
                        ixx[2] = ixx[3];
                    }
                    let c = at(ixx[0], iyy[0]);
                    let rd = f.dqdx[c] * (xt - xx[0]) + f.dqdy[c] * (yt - yy[0]);
                    if rd < RDCHCK {
                        iyy[2] = iyy[0];
                        iyy[3] = iyy[1];
                    } else if rd > 1. - RDCHCK {
                        iyy[0] = iyy[2];
                        iyy[1] = iyy[3];
                    }
                }
                for j in 0..4 {
                    let c = at(ixx[j], iyy[j]);
                    mask1[c] = mask1[c] || flag1;
                    self.mask2[c] = self.mask2[c] || flag2;
                    self.trckid[c] = e.id.clone();
                }
            }
        }
        let Some(out) = self.out.as_mut() else { return true };
        for iy in 1..=m.ny {
            for ix in 1..=m.nx {
                let c = at(ix, iy);
                if !mask1[c] {
                    continue;
                }
                let tst = state(f.mapsta[c], f.mapst2[c]);
                let mut h = Vec::new();
                for v in f.time {
                    h.extend(v.to_be_bytes());
                }
                h.extend(m.xgrd[c].to_be_bytes());
                h.extend(m.ygrd[c].to_be_bytes());
                text(&mut h, tst, 3);
                text(&mut h, &self.trckid[c], 32);
                rec(out, &h);
                if tst == "SEA" {
                    let isea = m.mapfs[c] as usize;
                    let (s, co) = sincosf(f.ud[isea]);
                    let (wx, wy) = (f.ua[isea] * co, f.ua[isea] * s);
                    let mut b = Vec::with_capacity(4 * (7 + nspec));
                    for v in [f.dw[isea], f.cx[isea], f.cy[isea], wx, wy, f.ust[isea], f.asv[isea]] {
                        b.extend(v.to_be_bytes());
                    }
                    // SPEC(IK,ITH) leaves the record in storage order, frequency first.
                    for ith in 0..nth {
                        for ik in 0..nk {
                            let v = TPI * f.va[ith + ik * nth + nspec * isea] * m.sig[ik + 1] / f.cg[ik + 1 + nk2 * isea];
                            b.extend(v.to_be_bytes());
                        }
                    }
                    rec(out, &b);
                }
            }
        }
        true
    }

    pub fn flush(&mut self) {
        if let Some(f) = self.out.as_mut() {
            f.flush().unwrap();
        }
    }
}
