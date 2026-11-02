// SPDX-License-Identifier: MPL-2.0
//! `W3IOBC` (`model/src/w3iobcmd.F90` at `92e2cf66`) for `switch_chaceon_shrd`: the `WRITE` of the boundary data
//! files `nestNN.ww3` (`:186-267`, `:317-335`, `:414-440`). The records are big-endian, as `file_endian` makes them
//! in this build. `READ` and `DUMP` are not translated: see `rust/port_report/W3IOBC.md`.

use crate::cspc::Cspc;
use crate::gridsru::{Gsu, w3gfpt};
use crate::moddef::{ModDef, Records};
use crate::wtime::{Time, dsec21};
use std::io::Write;
use ww3_kernels::pro3::Xyp3Bnd;
use ww3_kernels::real::Real;

/// `IDSTRBC` and `VERBPTBC`.
pub const IDSTRBC: &str = "WAVEWATCH III BOUNDARY DATA FILE";
pub const VERBPTBC: &str = "2018-03-01";

/// The files `nest01.ww3` ... of `INXOUT = 'WRITE'`, one per `NFBPO`.
pub struct NestOut {
    files: Vec<std::io::BufWriter<std::fs::File>>,
    header: bool,
    dir: String,
}

pub fn rec(f: &mut impl Write, b: &[u8]) {
    let n = (b.len() as i32).to_be_bytes();
    f.write_all(&n).unwrap();
    f.write_all(b).unwrap();
    f.write_all(&n).unwrap();
}

pub fn text(v: &mut Vec<u8>, s: &str, n: usize) {
    let mut b = s.as_bytes().to_vec();
    b.resize(n, b' ');
    v.extend(b);
}

impl NestOut {
    pub fn new(dir: &str) -> Self {
        Self { files: Vec::new(), header: false, dir: dir.to_string() }
    }

    /// One call of `W3IOBC ( 'WRITE', ..., TIME, TIME, ... )`. `va` and `cg` follow `shel.rs`: `NSPEC` and `NK + 2`
    /// values per point from `ISEA = 0`.
    pub fn write(&mut self, m: &ModDef, time: Time, va: &[Real], cg: &[Real]) {
        let (nk, nth) = (m.nk as usize, m.nth as usize);
        let (nspec, nk2) = (nk * nth, nk + 2);
        let nfbpo = m.nfbpo as usize;
        let nbot = m.nbo[nfbpo] as usize;
        if !self.header {
            // FILEW: the files open, and each gets its header and its points.
            for ifile in 1..=nfbpo {
                let name = format!("{}/nest{:02}.ww3", self.dir, ifile);
                let mut f = std::io::BufWriter::new(std::fs::File::create(name).unwrap());
                let (lo, hi) = (m.nbo[ifile - 1] as usize, m.nbo[ifile] as usize);
                let mut h = Vec::new();
                text(&mut h, IDSTRBC, 32);
                text(&mut h, VERBPTBC, 10);
                h.extend((nk as i32).to_be_bytes());
                h.extend((nth as i32).to_be_bytes());
                for v in [m.xfr, m.fr1, m.th[0]] {
                    h.extend(v.to_be_bytes());
                }
                h.extend(((hi - lo) as i32).to_be_bytes());
                rec(&mut f, &h);
                let mut p = Vec::new();
                for i in lo + 1..=hi {
                    p.extend(m.xbpo[i - 1].to_be_bytes());
                }
                for i in lo + 1..=hi {
                    p.extend(m.ybpo[i - 1].to_be_bytes());
                }
                for j in 0..4 {
                    for i in lo + 1..=hi {
                        p.extend(m.ipbpo[i - 1 + nbot * j].to_be_bytes());
                    }
                }
                for j in 0..4 {
                    for i in lo + 1..=hi {
                        p.extend(m.rdbpo[i - 1 + nbot * j].to_be_bytes());
                    }
                }
                rec(&mut f, &p);
                self.files.push(f);
            }
            self.header = true;
        }
        for ifile in 1..=nfbpo {
            let (lo, hi) = (m.nbo2[ifile - 1] as usize, m.nbo2[ifile] as usize);
            let f = &mut self.files[ifile - 1];
            let mut t = Vec::new();
            t.extend(time[0].to_be_bytes());
            t.extend(time[1].to_be_bytes());
            t.extend(((hi - lo) as i32).to_be_bytes());
            rec(f, &t);
            for isout in lo + 1..=hi {
                let isea = m.isbpo[isout - 1] as usize;
                let mut b = Vec::with_capacity(4 * nspec);
                for is in 1..=nspec {
                    // SHRD: ABPOS = VA * SIG2 / CG(1+(IS-1)/NTH).
                    let a = va[nspec * isea + is - 1] * m.sig2[is - 1] / cg[1 + (is - 1) / nth + nk2 * isea];
                    b.extend(a.to_be_bytes());
                }
                rec(f, &b);
            }
            f.flush().unwrap();
        }
    }
}

/// `EXTCDE` after one of `W3IOBC`'s messages.
fn stop(code: i32, msg: &str) -> ! {
    eprintln!("\n *** WAVEWATCH III ERROR IN W3IOBC :\n     {msg}\n");
    std::process::exit(code);
}

/// `W3IOBC ( 'READ', ... )` and `W3UBPT`, with what `W3ODATMD` keeps for them. `ABPI0(NSPEC,0:NBI2)` is
/// `abpi0[isp + nspec*ip]`, and `BBPI0(NSPEC,0:NBI)` likewise.
pub struct NestIn {
    path: String,
    r: Option<Records>,
    /// `FILER`.
    filer: bool,
    /// `FLBPI`.
    pub flbpi: bool,
    pub tbpi0: Time,
    pub tbpin: Time,
    nkthi: usize,
    cspc: Option<Cspc>,
    nbi: usize,
    nbi2: usize,
    /// `ISBPI(1:NBI)`.
    pub isbpi: Vec<i32>,
    ipbpi: Vec<i32>,
    rdbpi: Vec<Real>,
    abpi0: Vec<Real>,
    abpin: Vec<Real>,
    pub bbpi0: Vec<Real>,
    pub bbpin: Vec<Real>,
}

impl NestIn {
    pub fn new(dir: &str, flbpi: bool) -> Self {
        Self {
            path: format!("{dir}/nest.ww3"),
            r: None,
            filer: true,
            flbpi,
            tbpi0: [-1, 0],
            tbpin: [-1, 0],
            nkthi: 0,
            cspc: None,
            nbi: 0,
            nbi2: 0,
            isbpi: Vec::new(),
            ipbpi: Vec::new(),
            rdbpi: Vec::new(),
            abpi0: Vec::new(),
            abpin: Vec::new(),
            bbpi0: Vec::new(),
            bbpin: Vec::new(),
        }
    }

    /// The `W3XYP3` argument at `time`.
    pub fn at(&self, nspec: usize, time: Time) -> Xyp3Bnd<'_> {
        Xyp3Bnd {
            t0: dsec21(self.tbpi0, time),
            t0n: dsec21(self.tbpi0, self.tbpin),
            nspec,
            isbpi: &self.isbpi,
            bbpi0: &self.bbpi0,
            bbpin: &self.bbpin,
        }
    }

    /// The header, `w3iobcmd.F90:422-556`. False when the file is not there, which is `IOTST = 1`.
    fn open(&mut self, m: &ModDef, mapsta: &[i32]) -> bool {
        if !std::path::Path::new(&self.path).exists() {
            eprintln!("\n *** WAVEWATCH III WARNING IN W3IOBC : \n     INPUT FILE WITH BOUNDARY CONDITIONS NOT FOUND\n     BOUNDARY CONDITIONS WILL NOT BE UPDATED     1\n");
            self.flbpi = false;
            return false;
        }
        let mut r = Records::open(&self.path);
        let (nki, nthi, xfri, fr1i, th1i);
        {
            let mut c = r.next();
            let idtst = c.text(32);
            if idtst != IDSTRBC {
                stop(10, &format!("ILLEGAL IDSTRBC, READ : {idtst}"));
            }
            let vertst = c.text(10);
            if vertst != VERBPTBC {
                stop(11, &format!("ILLEGAL VEROGR, READ : {vertst}"));
            }
            (nki, nthi) = (c.i(), c.i());
            (xfri, fr1i, th1i) = (c.f(), c.f(), c.f());
            self.nbi = c.i() as usize;
        }
        let (nk, nth) = (m.nk, m.nth);
        let spconv = nki != nk
            || nthi != nth
            || (xfri / m.xfr - 1.).abs() > 0.01
            || (fr1i / m.fr1 - 1.).abs() > 0.01
            || (th1i - m.th[0]).abs() > 0.01 * m.dth;
        self.nkthi = (nki * nthi) as usize;
        if spconv {
            self.cspc = Some(Cspc::new(nki as usize, nthi as usize, xfri, fr1i, th1i, nk as usize, nth as usize, m.xfr, m.fr1, m.th[0], m.fachfe));
        }
        let nb = self.nbi;
        let (xbpi, ybpi);
        {
            let mut c = r.next();
            xbpi = c.fs(nb);
            ybpi = c.fs(nb);
            self.ipbpi = c.is(4 * nb);
            self.rdbpi = c.fs(4 * nb);
        }
        let (nx, ny, nsea) = (m.nx as usize, m.ny as usize, m.nsea as usize);
        let xg: Vec<f64> = m.xgrd.iter().map(|x| *x as f64).collect();
        let yg: Vec<f64> = m.ygrd.iter().map(|y| *y as f64).collect();
        let gsu = Gsu::create(false, m.flagll, m.iclose, nx as i32, ny as i32, xg, yg);
        let mut flok = true;
        self.isbpi = vec![0; nb];
        for i in 0..nb {
            let Some((ix, iy)) = w3gfpt(&gsu, xbpi[i], ybpi[i], 0.1) else {
                stop(12, &format!("POINT {} {} {} NOT LOCATED IN GRID", i + 1, xbpi[i], ybpi[i]));
            };
            let c = (iy - 1) as usize + (ix - 1) as usize * ny;
            if mapsta[c].abs() != 2 {
                eprintln!("\n *** WAVEWATCH III ERROR IN W3IOBC :\n     POINT{}{} NOT ACTIVE SEA POINT ({})", ix, iy, mapsta[c].abs());
                flok = false;
            }
            self.isbpi[i] = m.mapfs[c];
        }
        if !flok {
            std::process::exit(20);
        }
        for isea in 1..=nsea {
            let (ix, iy) = (m.mapsf[isea - 1], m.mapsf[isea - 1 + nsea]);
            let c = (iy - 1) as usize + (ix - 1) as usize * ny;
            if mapsta[c].abs() == 2 && !self.isbpi.contains(&(isea as i32)) {
                eprintln!(" *** WAVEWATCH III WARNING : POINT {ix} {iy} WILL NOT BE UPDATED");
            }
        }
        self.r = Some(r);
        true
    }

    /// One `W3IOBC ( 'READ', NDS(9), TBPI0, TBPIN, ITEST, IMOD )`, returning `ITEST`.
    pub fn read(&mut self, m: &ModDef, mapsta: &[i32]) -> i32 {
        let nspec = (m.nk * m.nth) as usize;
        if self.filer && !self.open(m, mapsta) {
            return 1;
        }
        if !self.filer {
            self.tbpi0 = self.tbpin;
            for i in nspec..nspec * (self.nbi2 + 1) {
                self.abpi0[i] = self.abpin[i];
            }
        }
        let r = self.r.as_mut().unwrap();
        let Some(mut c) = r.try_next() else {
            if self.filer {
                stop(43, "NO DATA IN INPUT FILE");
            }
            // Label 810: TIME2 and the spectra stay as they were read last.
            self.tbpi0 = self.tbpin;
            self.abpi0.copy_from_slice(&self.abpin);
            self.flbpi = false;
            return -1;
        };
        let (t, nbi2) = ([c.i(), c.i()], c.i() as usize);
        self.tbpin = t;
        if self.filer {
            // W3DMO5 ( ..., 3 ) on the first read, from its header.
            self.nbi2 = nbi2;
            self.abpi0 = vec![0.; nspec * (nbi2 + 1)];
            self.abpin = vec![0.; nspec * (nbi2 + 1)];
            self.bbpi0 = vec![-1.; nspec * (self.nbi + 1)];
            self.bbpin = vec![0.; nspec * (self.nbi + 1)];
        }
        assert!(nbi2 <= self.nbi2, "W3IOBC: more spectra than ABPIN holds");
        self.nbi2 = nbi2;
        let mut tmp = vec![0.; self.nkthi * nbi2];
        for ip in 0..nbi2 {
            let v = r.next().fs(self.nkthi);
            match self.cspc {
                None => self.abpin[nspec * (ip + 1)..nspec * (ip + 2)].copy_from_slice(&v),
                Some(_) => tmp[self.nkthi * ip..self.nkthi * (ip + 1)].copy_from_slice(&v),
            }
        }
        if let Some(cs) = &self.cspc {
            cs.apply(&tmp, &mut self.abpin[nspec..nspec * (nbi2 + 1)], nbi2);
        }
        if self.filer {
            self.tbpi0 = self.tbpin;
            for i in nspec..nspec * (nbi2 + 1) {
                self.abpi0[i] = self.abpin[i];
            }
            self.abpi0[..nspec].fill(0.);
            self.abpin[..nspec].fill(0.);
        }
        self.filer = false;
        0
    }

    /// `W3UBPT`. `cg` as in `write`.
    pub fn w3ubpt(&mut self, m: &ModDef, cg: &[Real]) {
        let nspec = (m.nk * m.nth) as usize;
        let nk2 = m.nk as usize + 2;
        let nb = self.nbi;
        let interp = |a: &[Real], isp: usize, ibi: usize| {
            let v = |j: usize| self.rdbpi[ibi + nb * j] * a[isp + nspec * self.ipbpi[ibi + nb * j] as usize];
            v(0) + v(1) + v(2) + v(3)
        };
        let (mut b0, mut bn) = (std::mem::take(&mut self.bbpi0), std::mem::take(&mut self.bbpin));
        if b0[0] == -1. {
            b0[..nspec].fill(0.);
            bn[..nspec].fill(0.);
            for ibi in 0..nb {
                let isea = self.isbpi[ibi] as usize;
                for isp in 0..nspec {
                    b0[isp + nspec * (ibi + 1)] = cg[m.mapwn[isp] as usize + nk2 * isea] / m.sig2[isp] * interp(&self.abpi0, isp, ibi);
                }
            }
        } else {
            b0.copy_from_slice(&bn);
        }
        for ibi in 0..nb {
            let isea = self.isbpi[ibi] as usize;
            for isp in 0..nspec {
                bn[isp + nspec * (ibi + 1)] = cg[m.mapwn[isp] as usize + nk2 * isea] / m.sig2[isp] * interp(&self.abpin, isp, ibi);
            }
        }
        (self.bbpi0, self.bbpin) = (b0, bn);
    }
}
