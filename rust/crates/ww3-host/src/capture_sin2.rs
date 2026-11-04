// SPDX-License-Identifier: MPL-2.0
//! Readers of `regtests/capture/sin2.py` and `regtests/capture/inptab.py`. A file of its own, with a
//! small reader of its own, so that this port adds nothing to `capture.rs`.

use ww3_kernels::real::Real;
use ww3_kernels::sin2::{BETATB_LEN, Sin2Params, Sin2Point};

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
    fn reals(&mut self, n: usize) -> Vec<Real> {
        self.u32s(n).into_iter().map(f32::from_bits).collect()
    }
    /// `common.array`: first extent, count, data.
    fn table(&mut self) -> Vec<Real> {
        let h = self.ints(2);
        self.reals(h[1] as usize)
    }
}

/// One `W3SIN2` call: its inputs and the reference's outputs.
pub struct Sin2Call {
    pub point: Sin2Point,
    pub a: Vec<Real>,
    pub cg: Vec<Real>,
    pub k: Vec<Real>,
    pub fpi: Real,
    pub s: Vec<Real>,
    pub d: Vec<Real>,
}

/// A `W3SIN2` capture: the module values as the first call found them, then the calls.
pub struct Sin2Capture {
    pub params: Sin2Params,
    pub sig: Vec<Real>,
    pub sig2: Vec<Real>,
    pub dden: Vec<Real>,
    pub ecos: Vec<Real>,
    pub esin: Vec<Real>,
    /// `BETATB(-400:401,21)`, 802 * 21 values.
    pub betatb: Vec<Real>,
    pub calls: Vec<Sin2Call>,
}

impl Sin2Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"SIN2", "{path}");
        let h = r.ints(3);
        let f = r.reals(10);
        let (sig, sig2, dden) = (r.table(), r.table(), r.table());
        let (ecos, esin) = (r.table(), r.table());
        let betatb = r.reals(BETATB_LEN);
        let (nk, nspec) = (h[0] as usize, h[2] as usize);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let v = r.reals(4);
            let point = Sin2Point { u: v[0], udir: v[1], cd: v[2], z0: v[3] };
            let (a, cg, k) = (r.reals(nspec), r.reals(nk), r.reals(nspec));
            let fpi = r.reals(1)[0];
            let (s, d) = (r.reals(nspec), r.reals(nspec));
            calls.push(Sin2Call { point, a, cg, k, fpi, s, d });
        }
        let params = Sin2Params {
            nk: h[0],
            nth: h[1],
            nspec: h[2],
            dsiga: f[0],
            ddrag: f[1],
            zwind: f[2],
            fte: f[3],
            fttr: f[4],
            xfr: f[5],
            fpimin: f[6],
            facti1: f[7],
            facti2: f[8],
            fswell: f[9],
        };
        Sin2Capture { params, sig, sig2, dden, ecos, esin, betatb, calls }
    }
}

/// What `regtests/capture/inptab.py` records when `INPTAB` returns.
pub struct InptabCapture {
    pub dsiga: Real,
    pub ddrag: Real,
    pub betatb: Vec<Real>,
}

impl InptabCapture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"INPT", "{path}");
        let f = r.reals(2);
        let betatb = r.reals(BETATB_LEN);
        assert_eq!(r.p, r.b.len(), "{path}");
        InptabCapture { dsiga: f[0], ddrag: f[1], betatb }
    }
}
