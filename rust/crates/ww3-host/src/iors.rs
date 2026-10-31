// SPDX-License-Identifier: MPL-2.0
//! `W3IORS` (`model/src/w3iorsmd.F90`) for this switch: a stream file of fixed records, big-endian, with
//! no extra fields. The header is longer than a record on small grids, and record 2's zeros then
//! overwrite its tail: writing in WW3's order at WW3's positions keeps that.

use crate::moddef::ModDef;
use crate::wtime::Time;
use ww3_kernels::real::Real;

const IDSTR: &str = "WAVEWATCH III RESTART FILE";
const VERINI: &str = "2021-05-28";
/// `NOGRP*NGRPP` of `FLOGRR`, every entry false here.
const FLOGRR: usize = 10 * 25;

/// `LRECL` in bytes.
fn lrecl(nspec: usize) -> usize {
    (4 * nspec).max(4 * (6 + 25 / 4 + 9 / 4 + 29 / 4 + 3 / 4))
}

/// The model state that `restart.ww3` holds. `va` is `(NSPEC, 0:NSEA)`, and `ust`, `ustdir` are 1-based
/// through index 0 like WW3's.
pub struct State {
    pub va: Vec<Real>,
    pub wlv: Vec<Real>,
    pub ice: Vec<Real>,
    pub ust: Vec<Real>,
    pub ustdir: Vec<Real>,
    pub asf: Vec<Real>,
    pub fpis: Vec<Real>,
    pub tlev: Time,
    pub tice: Time,
    pub trho: Time,
    /// 2 for a full restart, 0 for a cold start, 4 for a calm one.
    pub rstype: i32,
}

struct Stream {
    b: Vec<u8>,
}

impl Stream {
    fn put(&mut self, pos: usize, v: &[u8]) {
        if self.b.len() < pos + v.len() {
            self.b.resize(pos + v.len(), 0);
        }
        self.b[pos..pos + v.len()].copy_from_slice(v);
    }
    /// `WRITE (NDSR,POS=RPOS) WRITEBUFF`, then the data.
    fn record(&mut self, rec: usize, lrecl: usize, v: &[u8]) {
        let pos = lrecl * (rec - 1);
        self.put(pos, &vec![0; lrecl]);
        self.put(pos, v);
    }
}

fn bytes(v: &[Real]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn ibytes(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn pad(s: &str, n: usize) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    b.resize(n, b' ');
    b
}

/// `W3IORS ('READ', ...)`. No file is a calm start, as WW3's label 800 makes it.
pub fn read(path: &str, m: &ModDef, time: Time, mapsta: &mut [i32], mapst2: &mut [i32]) -> State {
    let (nsea, ns) = (m.nsea as usize, (m.nk * m.nth) as usize);
    let (nx, ny) = (m.nx as usize, m.ny as usize);
    let n1 = nsea + 1;
    let mut s = State {
        va: vec![0.; ns * n1],
        wlv: vec![0.; n1],
        ice: vec![0.; n1],
        ust: vec![0.; n1],
        ustdir: vec![0.; n1],
        asf: vec![1.; n1],
        fpis: vec![m.sig[m.nk as usize]; n1],
        tlev: [-1, 0],
        tice: [-1, 0],
        trho: [-1, 0],
        rstype: 4,
    };
    let Ok(b) = std::fs::read(path) else { return s };
    assert_eq!(&b[..26], IDSTR.as_bytes(), "not a restart file");
    let ty = String::from_utf8_lossy(&b[66..70]).to_string();
    let int = |p: usize| i32::from_be_bytes(b[p..p + 4].try_into().unwrap());
    assert!(int(70) == nsea as i32 && int(74) == ns as i32, "restart of another grid");
    let l = lrecl(ns);
    s.rstype = match ty.as_str() {
        "FULL" => 2,
        "COLD" => 0,
        "CALM" => 4,
        _ => panic!("restart type {ty} is not implemented"),
    };
    if s.rstype == 4 {
        return s;
    }
    if s.rstype == 2 {
        assert_eq!([int(l), int(l + 4)], time, "restart time differs");
    }
    let real = |p: usize| f32::from_be_bytes(b[p..p + 4].try_into().unwrap());
    for isea in 1..=nsea {
        let p = l * (isea + 1);
        for i in 0..ns {
            s.va[i + ns * isea] = real(p + 4 * i).max(0.);
        }
    }
    if s.rstype != 2 {
        return s;
    }
    let size = l / 4;
    let mut rec = nsea + 3;
    let p = l * (rec - 1);
    (s.tlev, s.tice, s.trho) = ([int(p), int(p + 4)], [int(p + 8), int(p + 12)], [int(p + 16), int(p + 20)]);
    let field = |v: &mut [Real], rec: &mut usize| {
        for i in 0..nsea {
            if i % size == 0 {
                *rec += 1;
            }
            v[i + 1] = real(l * (*rec - 1) + 4 * (i % size));
        }
    };
    field(&mut s.wlv, &mut rec);
    field(&mut s.ice, &mut rec);
    let mut tmp = vec![0; nx * ny];
    for iy in 0..ny {
        for ix in 0..nx {
            if ix % size == 0 {
                rec += 1;
            }
            tmp[iy + ix * ny] = int(l * (rec - 1) + 4 * (ix % size));
        }
    }
    let (sta, st2) = crate::grid::unpack(&tmp);
    mapsta.copy_from_slice(&sta);
    mapst2.copy_from_slice(&st2);
    field(&mut s.ust, &mut rec);
    field(&mut s.ustdir, &mut rec);
    field(&mut s.asf, &mut rec);
    field(&mut s.fpis, &mut rec);
    s
}

/// `W3IORS ('HOT', ...)`.
pub fn write(path: &str, m: &ModDef, s: &mut State, time: Time, mapsta: &[i32], mapst2: &[i32]) {
    let (nsea, ns) = (m.nsea as usize, (m.nk * m.nth) as usize);
    let (nx, ny) = (m.nx as usize, m.ny as usize);
    let l = lrecl(ns);
    let size = l / 4;
    let mut f = Stream { b: Vec::new() };

    let mut h = pad(IDSTR, 26);
    h.extend(pad(VERINI, 10));
    h.extend(pad(&m.gname, 30));
    h.extend(pad("FULL", 4));
    h.extend(ibytes(&[nsea as i32, ns as i32]));
    h.extend(vec![0; FLOGRR * 4]);
    f.record(1, l, &h);
    f.record(2, l, &ibytes(&time));
    for isea in 1..=nsea {
        f.record(isea + 2, l, &bytes(&s.va[ns * isea..ns * (isea + 1)]));
    }

    let mut rec = nsea + 3;
    f.record(rec, l, &ibytes(&[s.tlev[0], s.tlev[1], s.tice[0], s.tice[1], s.trho[0], s.trho[1]]));
    let parts = |f: &mut Stream, v: &[Real], rec: &mut usize| {
        for c in v[1..=nsea].chunks(size) {
            *rec += 1;
            f.record(*rec, l, &bytes(c));
        }
    };
    parts(&mut f, &s.wlv, &mut rec);
    parts(&mut f, &s.ice, &mut rec);
    for iy in 0..ny {
        let row: Vec<i32> = (0..nx).map(|ix| mapsta[iy + ix * ny] + 8 * mapst2[iy + ix * ny]).collect();
        for c in row.chunks(size) {
            rec += 1;
            f.record(rec, l, &ibytes(c));
        }
    }
    parts(&mut f, &s.ust, &mut rec);
    parts(&mut f, &s.ustdir, &mut rec);
    parts(&mut f, &s.asf, &mut rec);
    parts(&mut f, &s.fpis, &mut rec);
    std::fs::write(path, &f.b).unwrap();
    for v in &mut s.va {
        *v = v.max(0.);
    }
}
