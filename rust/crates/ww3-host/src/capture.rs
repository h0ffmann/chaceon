// SPDX-License-Identifier: MPL-2.0
//! Reads what `regtests/capture/snl1.py` recorded from the reference.

use crate::insnl1::{Snl1Tables, insnl1};
use ww3_kernels::sds4::{Sds4Params, Sds4Point};
use ww3_kernels::sin4::{Sin4Out, Sin4Params, Sin4Point, Spr4Params, Spr4Point};
use ww3_kernels::snl1::Snl1Params;
use ww3_kernels::srce::{SrceParams, SrcePoint};

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

/// `first extent, count, data`, as `regtests/capture/common.py` writes an array.
pub struct Table<T> {
    pub extent: i32,
    pub data: Vec<T>,
}

/// One `W3SDS4` call: its inputs and the reference's outputs.
pub struct Sds4Call {
    pub point: Sds4Point,
    pub whitecap_in: Vec<f32>,
    pub a: Vec<f32>,
    pub k: Vec<f32>,
    pub cg: Vec<f32>,
    pub srhs: Vec<f32>,
    pub ddiag: Vec<f32>,
    pub brlambda: Vec<f32>,
    pub whitecap: Vec<f32>,
}

pub struct Sds4Capture {
    pub params: Sds4Params,
    pub sig: Vec<f32>,
    pub dden: Vec<f32>,
    pub ec2: Vec<f32>,
    pub es2: Vec<f32>,
    pub esc: Vec<f32>,
    pub ecos: Vec<f32>,
    pub esin: Vec<f32>,
    pub satindices: Vec<i32>,
    pub satweights: Vec<f32>,
    pub cumulw: Vec<f32>,
    pub iktab: Vec<i32>,
    pub dcki: Vec<f32>,
    pub qbi: Vec<f32>,
    pub calls: Vec<Sds4Call>,
}

impl Reader {
    fn table_i(&mut self) -> Table<i32> {
        let h = self.ints(2);
        Table { extent: h[0], data: self.ints(h[1] as usize) }
    }
    fn table_r(&mut self) -> Table<f32> {
        let h = self.ints(2);
        Table { extent: h[0], data: self.reals(h[1] as usize) }
    }
}

impl Sds4Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"SDS4");
        let i = r.ints(11);
        let ssdsc = r.table_r().data;
        let ssdsbm = r.table_r().data;
        let f = r.reals(7);
        let sig = r.table_r().data;
        let dden = r.table_r().data;
        let (ec2, es2, esc) = (r.table_r().data, r.table_r().data, r.table_r().data);
        let (ecos, esin) = (r.table_r().data, r.table_r().data);
        let satindices = r.table_i();
        let satweights = r.table_r();
        let cumulw = r.table_r();
        let iktab = r.table_i();
        let dcki = r.table_r();
        let qbi = r.table_r();
        assert_eq!(dden.len(), i[0] as usize);
        let mut params = Sds4Params {
            nk: i[0],
            nth: i[1],
            nspec: i[2],
            dikcumul: i[3],
            ssdsbrfdf: i[4],
            ssdsiso: i[5],
            nkhs: i[6],
            nkd: i[7],
            ndtab: i[8],
            flogrd57: i[9] as u32,
            flogrd58: i[10] as u32,
            sat_n: satindices.extent,
            cumul_n: cumulw.extent,
            ssdsbr: f[0],
            ssdsbt: f[1],
            ssdsp: f[2],
            ssdsdth: f[3],
            dth: f[4],
            aairgb: f[5],
            aaircmin: f[6],
            ..Default::default()
        };
        params.ssdsc.copy_from_slice(&ssdsc);
        params.ssdsbm.copy_from_slice(&ssdsbm);
        let (nk, nspec) = (i[0] as usize, i[2] as usize);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let s = r.reals(5);
            let point = Sds4Point { ustar: s[0], usdir: s[1], depth: s[2], dair: s[3], dlwmean: s[4] };
            calls.push(Sds4Call {
                point,
                whitecap_in: r.reals(4),
                a: r.reals(nspec),
                k: r.reals(nk),
                cg: r.reals(nk),
                srhs: r.reals(nspec),
                ddiag: r.reals(nspec),
                brlambda: r.reals(nspec),
                whitecap: r.reals(4),
            });
        }
        Self {
            params,
            sig,
            dden,
            ec2,
            es2,
            esc,
            ecos,
            esin,
            satindices: satindices.data,
            satweights: satweights.data,
            cumulw: cumulw.data,
            iktab: iktab.data,
            dcki: dcki.data,
            qbi: qbi.data,
            calls,
        }
    }
}

/// One `W3SIN4` call: its inputs and the reference's outputs.
pub struct Sin4Call {
    pub point: Sin4Point,
    pub a: Vec<f32>,
    pub cg: Vec<f32>,
    pub k: Vec<f32>,
    pub out: Sin4Out,
    pub s: Vec<f32>,
    pub d: Vec<f32>,
    pub llws: Vec<u32>,
}

/// What `regtests/capture/sin4.py` records: the kernel's tables, `INSIN4`'s inputs and outputs, calls.
pub struct Sin4Capture {
    pub ints: Vec<i32>,
    /// BBETA ZZWND AALPHA ZZALP TTAUWSHELTER SSINTHP ZZ0RAT DTH ZZ0MAX FACHFE XFR SSDSCOS SSDSBRF1
    /// SSDSBINT SSDSPBK SSDSABK SSDSHCK SSDSBCK SSDSDTH, then DELUST DELTAIL DELTAUW DELU DELALP DELAB.
    pub reals: Vec<f32>,
    pub sswellf: Vec<f32>,
    pub ssdsc: Vec<f32>,
    pub sig: Vec<f32>,
    pub sig2: Vec<f32>,
    pub dden: Vec<f32>,
    pub dden2: Vec<f32>,
    pub ecos: Vec<f32>,
    pub esin: Vec<f32>,
    pub th: Vec<f32>,
    pub dsip: Vec<f32>,
    pub fwtable: Vec<f32>,
    pub taut: Vec<f32>,
    pub tauhft: Vec<f32>,
    pub tauhft2: Vec<f32>,
    pub satindices: Table<i32>,
    pub satweights: Table<f32>,
    pub cumulw: Table<f32>,
    pub iktab: Table<i32>,
    pub dcki: Table<f32>,
    pub qbi: Table<f32>,
    pub calls: Vec<Sin4Call>,
}

impl Sin4Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"SIN4");
        let ints = r.ints(8);
        let reals = r.reals(25);
        let sswellf = r.table_r().data;
        let ssdsc = r.table_r().data;
        let (sig, sig2, dden, dden2) = (r.table_r().data, r.table_r().data, r.table_r().data, r.table_r().data);
        let (ecos, esin, th, dsip) = (r.table_r().data, r.table_r().data, r.table_r().data, r.table_r().data);
        let (fwtable, taut, tauhft, tauhft2) =
            (r.table_r().data, r.table_r().data, r.table_r().data, r.table_r().data);
        let satindices = r.table_i();
        let satweights = r.table_r();
        let cumulw = r.table_r();
        let iktab = r.table_i();
        let dcki = r.table_r();
        let qbi = r.table_r();
        let (nk, nspec) = (ints[0] as usize, ints[2] as usize);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let f = r.reals(7);
            let point = Sin4Point { ustar: f[1], drat: f[2], usdir: f[4], z0: f[5] };
            let (a, cg, k, _brlambda) = (r.reals(nspec), r.reals(nk), r.reals(nspec), r.reals(nspec));
            let o = r.reals(4);
            let out = Sin4Out { tauwx: o[0], tauwy: o[1], tauwnx: o[2], tauwny: o[3] };
            let (s, d) = (r.reals(nspec), r.reals(nspec));
            let llws = r.u32s(nspec);
            calls.push(Sin4Call { point, a, cg, k, out, s, d, llws });
        }
        Self {
            ints, reals, sswellf, ssdsc, sig, sig2, dden, dden2, ecos, esin, th, dsip, fwtable, taut,
            tauhft, tauhft2, satindices, satweights, cumulw, iktab, dcki, qbi, calls,
        }
    }

    pub fn params(&self) -> Sin4Params {
        let (i, f) = (&self.ints, &self.reals);
        let mut p = Sin4Params {
            nk: i[0],
            nth: i[1],
            nspec: i[2],
            bbeta: f[0],
            zzwnd: f[1],
            aalpha: f[2],
            zzalp: f[3],
            ttauwshelter: f[4],
            ssinthp: f[5],
            zz0rat: f[6],
            dth: f[7],
            delust: f[19],
            deltail: f[20],
            delalp: f[23],
            delab: f[24],
            ..Default::default()
        };
        p.sswellf.copy_from_slice(&self.sswellf);
        p
    }
}

/// One `W3SPR4` call: its inputs and the reference's outputs.
pub struct Spr4Call {
    pub point: Spr4Point,
    pub a: Vec<f32>,
    pub cg: Vec<f32>,
    pub wn: Vec<f32>,
    pub llws: Vec<u32>,
    pub out: Vec<f32>,
}

pub struct Spr4Capture {
    pub params: Spr4Params,
    pub sig: Vec<f32>,
    pub dden: Vec<f32>,
    pub ecos: Vec<f32>,
    pub esin: Vec<f32>,
    pub taut: Vec<f32>,
    pub calls: Vec<Spr4Call>,
}

impl Spr4Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"SPR4");
        let i = r.ints(3);
        let f = r.reals(10);
        let params = Spr4Params {
            nk: i[0],
            nth: i[1],
            nspec: i[2],
            wwnmeanp: f[0],
            wwnmeanptail: f[1],
            fte: f[2],
            ftf: f[3],
            sstxftftail: f[4],
            sstxftwn: f[5],
            zzwnd: f[6],
            aalpha: f[7],
            deltauw: f[8],
            delu: f[9],
        };
        let (sig, dden, ecos, esin) = (r.table_r().data, r.table_r().data, r.table_r().data, r.table_r().data);
        let taut = r.table_r().data;
        let (nk, nspec) = (i[0] as usize, i[2] as usize);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let s = r.reals(6);
            let point =
                Spr4Point { u: s[0], udir: s[1], ustar: s[2], usdir: s[3], tauwx: s[4], tauwy: s[5] };
            let (a, cg, wn) = (r.reals(nspec), r.reals(nk), r.reals(nk));
            let llws = r.u32s(nspec);
            calls.push(Spr4Call { point, a, cg, wn, llws, out: r.reals(12) });
        }
        Self { params, sig, dden, ecos, esin, taut, calls }
    }
}

/// One `W3SRCE` call: its inputs and the reference's outputs. `out` holds USTAR USTDIR DTDYN FCUT TAUWX
/// TAUWY TAUOX TAUOY TAUWIX TAUWIY TAUWNX TAUWNY PHIAW CHARN TWS PHIOC PHIBBL TAUBBL(2) TAUICE(2) PHICE
/// TAUOCX TAUOCY WNMEAN, 25 in all. A "SRCB" file also holds `BEDFORM` in and out; an older "SRCE"
/// file leaves them, `D50` and `PSIC` at zero.
pub struct SrceCall {
    pub point: SrcePoint,
    pub bedform_in: [f32; 3],
    pub bedform: [f32; 3],
    pub spec: Vec<f32>,
    pub wn1: Vec<f32>,
    pub cg1: Vec<f32>,
    pub spec_out: Vec<f32>,
    pub out: Vec<f32>,
    pub whitecap: Vec<f32>,
}

pub struct SrceCapture {
    pub ints: Vec<i32>,
    pub reals: Vec<f32>,
    pub icescales: Vec<f32>,
    pub calls: Vec<SrceCall>,
}

impl SrceCapture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        let srcb = &r.b[..4] == b"SRCB";
        assert!(srcb || &r.b[..4] == b"SRCE", "{path}");
        // "SRCB" adds the build's BT switch, 1 or 4; an "SRCE" file is from a BT1 build.
        let mut ints = r.ints(if srcb { 6 } else { 5 });
        if !srcb {
            ints.push(1);
        }
        let reals = r.reals(19);
        let icescales = r.table_r().data;
        let (nk, nspec) = (ints[0] as usize, ints[2] as usize);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let i = r.ints(2);
            let f = r.reals(if srcb { 11 } else { 9 });
            let bedform_in = if srcb { r.reals(3) } else { vec![0.; 3] };
            let point = SrcePoint {
                srce_call: i[0],
                it: i[1],
                d_inp: f[0],
                u10abs: f[1],
                u10dir: f[2],
                ustar: f[3],
                ustdir: f[4],
                ice: f[5],
                dtg: f[6],
                dair: f[7],
                coef: f[8],
                d50: f.get(9).copied().unwrap_or(0.),
                psic: f.get(10).copied().unwrap_or(0.),
            };
            let (spec, wn1, cg1) = (r.reals(nspec), r.reals(nk), r.reals(nk));
            let spec_out = r.reals(nspec);
            let out = r.reals(25);
            let whitecap = r.reals(4);
            let bedform = if srcb { r.reals(3) } else { vec![0.; 3] };
            calls.push(SrceCall {
                point,
                bedform_in: [bedform_in[0], bedform_in[1], bedform_in[2]],
                bedform: [bedform[0], bedform[1], bedform[2]],
                spec,
                wn1,
                cg1,
                spec_out,
                out,
                whitecap,
            });
        }
        Self { ints, reals, icescales, calls }
    }

    pub fn params(&self, snl: Snl1Params, sds: Sds4Params, sin: Sin4Params, spr: Spr4Params) -> SrceParams {
        let (i, f) = (&self.ints, &self.reals);
        let mut p = SrceParams {
            nk: i[0],
            nth: i[1],
            nspec: i[2],
            fdonly: i[3] as u32,
            inflags2_4: i[4] as u32,
            btopt: i[5],
            dmin: f[0],
            dtmax: f[1],
            dtmin: f[2],
            facti1: f[3],
            facti2: f[4],
            fachfa: f[5],
            facp: f[6],
            xflt: f[7],
            xrel: f[8],
            fhmax: f[9],
            ffxfm: f[10],
            ffxpm: f[11],
            ffxfa: f[12],
            slnc1: f[13],
            fspm: f[14],
            fshf: f[15],
            sbtc1: f[16],
            sdbc1: f[17],
            sdbc2: f[18],
            snl,
            sds,
            sin,
            spr,
            ..Default::default()
        };
        p.icescales.copy_from_slice(&self.icescales);
        p
    }
}

/// One `W3SBT4` call of `regtests/capture/sbt4.py`: its inputs and the reference's outputs.
pub struct Sbt4Call {
    pub point: ww3_kernels::sbt4::Sbt4Point,
    pub bedform_in: [f32; 3],
    pub a: Vec<f32>,
    pub cg: Vec<f32>,
    pub wn: Vec<f32>,
    pub s: Vec<f32>,
    pub d: Vec<f32>,
    pub taubbl: [f32; 2],
    pub bedform: [f32; 3],
    /// `DELXERF` and `ERFTABLE(0:SIZEERFTABLE)` as this call left them.
    pub delxerf: f32,
    pub erftable: Vec<f32>,
}

/// A `W3SBT4` capture. `params.delxerf` is the reference's; `erftable` is the first call's.
pub struct Sbt4Capture {
    pub params: ww3_kernels::sbt4::Sbt4Params,
    pub sig: Vec<f32>,
    pub dden: Vec<f32>,
    pub ecos: Vec<f32>,
    pub esin: Vec<f32>,
    pub fwtable: Vec<f32>,
    pub calls: Vec<Sbt4Call>,
}

impl Sbt4Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"SBT4", "{path}");
        let h = r.ints(3);
        let (nk, nth, nspec) = (h[0], h[1], h[2]);
        let delab = r.reals(1)[0];
        let sbtcx = r.table_r().data;
        let (sig, dden, ecos, esin) = (r.table_r().data, r.table_r().data, r.table_r().data, r.table_r().data);
        let fwtable = r.reals(301);
        let (nk_, ns) = (nk as usize, nspec as usize);
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let v = r.reals(6);
            let (a, cg, wn) = (r.reals(ns), r.reals(nk_), r.reals(nk_));
            let (s, d) = (r.reals(ns), r.reals(ns));
            let o = r.reals(5);
            let delxerf = r.reals(1)[0];
            let erftable = r.reals(301);
            calls.push(Sbt4Call {
                point: ww3_kernels::sbt4::Sbt4Point { depth: v[0], d50: v[1], psic: v[2] },
                bedform_in: [v[3], v[4], v[5]],
                a,
                cg,
                wn,
                s,
                d,
                taubbl: [o[0], o[1]],
                bedform: [o[2], o[3], o[4]],
                delxerf,
                erftable,
            });
        }
        let mut sx = [0.0f32; 10];
        sx.copy_from_slice(&sbtcx[..10]);
        let delxerf = calls.first().map_or(0., |c| c.delxerf);
        let params = ww3_kernels::sbt4::Sbt4Params { nk, nth, sbtcx: sx, delab, delxerf };
        Sbt4Capture { params, sig, dden, ecos, esin, fwtable, calls }
    }
}

/// One `W3FLX1` call of `regtests/capture/flx1.py`: its inputs and the reference's results.
pub struct Flx1Call {
    pub input: ww3_kernels::flx1::Flx1In,
    /// `UST`, `USTD`, `Z0`, `CD`.
    pub out: [f32; 4],
}

/// A `W3FLX1` capture.
pub struct Flx1Capture {
    pub calls: Vec<Flx1Call>,
}

impl Flx1Capture {
    pub fn read(path: &str) -> Self {
        let mut r = Reader { b: std::fs::read(path).unwrap(), p: 4 };
        assert_eq!(&r.b[..4], b"FLX1", "{path}");
        let mut calls = Vec::new();
        while r.p < r.b.len() {
            let v = r.reals(7);
            calls.push(Flx1Call {
                input: ww3_kernels::flx1::Flx1In { zwnd: v[0], u10: v[1], u10d: v[2] },
                out: [v[3], v[4], v[5], v[6]],
            });
        }
        Flx1Capture { calls }
    }
}
