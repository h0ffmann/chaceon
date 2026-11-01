// SPDX-License-Identifier: MPL-2.0
//! `W3IOBC` (`model/src/w3iobcmd.F90` at `92e2cf66`) for `switch_chaceon_shrd`: the `WRITE` of the boundary data
//! files `nestNN.ww3` (`:186-267`, `:317-335`, `:414-440`). The records are big-endian, as `file_endian` makes them
//! in this build. `READ` and `DUMP` are not translated: see `rust/port_report/W3IOBC.md`.

use crate::moddef::ModDef;
use crate::wtime::Time;
use std::io::Write;
use ww3_kernels::real::Real;

/// `IDSTRBC` and `VERBPTBC`.
const IDSTRBC: &str = "WAVEWATCH III BOUNDARY DATA FILE";
const VERBPTBC: &str = "2018-03-01";

/// The files `nest01.ww3` ... of `INXOUT = 'WRITE'`, one per `NFBPO`.
pub struct NestOut {
    files: Vec<std::io::BufWriter<std::fs::File>>,
    header: bool,
    dir: String,
}

fn rec(f: &mut impl Write, b: &[u8]) {
    let n = (b.len() as i32).to_be_bytes();
    f.write_all(&n).unwrap();
    f.write_all(b).unwrap();
    f.write_all(&n).unwrap();
}

fn text(v: &mut Vec<u8>, s: &str, n: usize) {
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
