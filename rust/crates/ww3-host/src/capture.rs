// SPDX-License-Identifier: MPL-2.0
//! Reads what `regtests/capture/snl1.py` recorded from the reference.

use crate::insnl1::{Snl1Tables, insnl1};
use ww3_kernels::snl1::Snl1Params;

/// One `W3SNL1` call: its inputs and the reference's outputs.
pub struct Snl1Call {
    pub kdmean: f32,
    pub a: Vec<f32>,
    pub cg: Vec<f32>,
    pub s: Vec<f32>,
    pub d: Vec<f32>,
}

/// `ints`: NK NTH NSPEC NFR NFRHGH NFRCHG NSPECX NSPECY. `reals`: LAM XFR DTH FACHFE KDCON KDMN SNLC1
/// SNLS1 SNLS2 SNLS3 DAL1-3 AWG1-8 SWG1-8. `pm` and `ic` are interleaved per `ISP`.
pub struct Snl1Capture {
    pub ints: Vec<i32>,
    pub reals: Vec<f32>,
    pub sig: Vec<f32>,
    pub af11: Vec<f32>,
    pub pm: Vec<i32>,
    pub ic: Vec<i32>,
    pub calls: Vec<Snl1Call>,
}

struct Reader {
    b: Vec<u8>,
    p: usize,
}

impl Reader {
    fn u32s(&mut self, n: usize) -> Vec<u32> {
        let v = self.b[self.p..self.p + 4 * n]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        self.p += 4 * n;
        v
    }
    fn ints(&mut self, n: usize) -> Vec<i32> {
        self.u32s(n).into_iter().map(|u| u as i32).collect()
    }
    fn reals(&mut self, n: usize) -> Vec<f32> {
        self.u32s(n).into_iter().map(f32::from_bits).collect()
    }
    fn columns(&mut self, count: usize, rows: usize) -> Vec<i32> {
        let c: Vec<Vec<i32>> = (0..count).map(|_| self.ints(rows)).collect();
        (0..rows).flat_map(|i| c.iter().map(move |col| col[i])).collect()
    }
}

impl Snl1Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"SNL1");
        let ints = r.ints(8);
        let (nk, nspec, nspecx) = (ints[0] as usize, ints[2] as usize, ints[6] as usize);
        let reals = r.reals(29);
        let sig = r.reals(nk + 2);
        let af11 = r.reals(nspecx);
        let pm = r.columns(16, nspecx);
        let ic = r.columns(16, nspec);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let kdmean = r.reals(1)[0];
            let (a, cg) = (r.reals(nspec), r.reals(nk));
            let (s, d) = (r.reals(nspec), r.reals(nspec));
            calls.push(Snl1Call { kdmean, a, cg, s, d });
        }
        Self { ints, reals, sig, af11, pm, ic, calls }
    }

    /// `INSNL1` from the namelist values the capture holds.
    pub fn rebuild(&self) -> Snl1Tables {
        let (i, f) = (&self.ints, &self.reals);
        let given = Snl1Params {
            nk: i[0],
            nth: i[1],
            nspec: i[2],
            fachfe: f[3],
            kdcon: f[4],
            kdmn: f[5],
            snlc1: f[6],
            snls1: f[7],
            snls2: f[8],
            snls3: f[9],
            ..Default::default()
        };
        insnl1(given, f[2], f[1], f[0], &self.sig)
    }
}
