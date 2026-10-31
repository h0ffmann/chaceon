// SPDX-License-Identifier: MPL-2.0
//! `mod_def.ww3`, as `W3IOGR` reads it (`model/src/w3iogrmd.F90`), for `switch_chaceon_shrd`. The record
//! layout changes with the switches, so a file from another build fails the length checks here.
//! WW3's CMake builds with `ENDIANNESS='big_endian'`, so records and data are big-endian.

use crate::insnl1::insnl1;
use crate::insnl1::Snl1Tables;
use ww3_kernels::sds4::Sds4Params;
use ww3_kernels::sin4::{Sin4Params, Spr4Params};
use ww3_kernels::snl1::Snl1Params;
use ww3_kernels::srce::{SrceParams, SrceTables};

/// Fortran unformatted records.
pub struct Records {
    b: Vec<u8>,
    p: usize,
}

impl Records {
    pub fn open(path: &str) -> Self {
        Self { b: std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}")), p: 0 }
    }

    /// The next record's payload.
    pub fn next(&mut self) -> Rec<'_> {
        let n = i32::from_be_bytes(self.b[self.p..self.p + 4].try_into().unwrap());
        assert!(n >= 0, "record of {n} bytes: a split record is not implemented");
        let n = n as usize;
        let start = self.p + 4;
        let tail = i32::from_be_bytes(self.b[start + n..start + n + 4].try_into().unwrap());
        assert_eq!(tail as usize, n, "record markers differ");
        self.p = start + n + 4;
        Rec { b: &self.b[start..start + n], p: 0 }
    }
}

/// A cursor through one record.
pub struct Rec<'a> {
    b: &'a [u8],
    p: usize,
}

impl Rec<'_> {
    pub fn len(&self) -> usize {
        self.b.len()
    }
    pub fn is_empty(&self) -> bool {
        self.b.is_empty()
    }
    pub fn skip(&mut self, n: usize) {
        self.p += n;
    }
    pub fn i(&mut self) -> i32 {
        let v = i32::from_be_bytes(self.b[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        v
    }
    pub fn f(&mut self) -> f32 {
        f32::from_bits(self.i() as u32)
    }
    /// Blanks at the end are Fortran's padding.
    pub fn text(&mut self, n: usize) -> String {
        let s = String::from_utf8_lossy(&self.b[self.p..self.p + n]).trim_end().to_string();
        self.p += n;
        s
    }
    pub fn fs(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.f()).collect()
    }
    pub fn is(&mut self, n: usize) -> Vec<i32> {
        (0..n).map(|_| self.i()).collect()
    }
    /// Every byte used: a wrong layout stops here, not later in the physics.
    pub fn done(&self, what: &str) {
        assert_eq!(self.p, self.b.len(), "{what}: record length differs from this build's layout");
    }
}

/// The grid: every field of `mod_def.ww3` this port reads.
#[derive(Default)]
pub struct ModDef {
    pub nx: i32,
    pub ny: i32,
    pub nsea: i32,
    pub nth: i32,
    pub nk: i32,
    pub gname: String,
    pub flagll: bool,
    pub iclose: i32,
    pub sx: f32,
    pub sy: f32,
    pub x0: f32,
    pub y0: f32,
    pub zb: Vec<f32>,
    /// `MAPSTA + 8*MAPST2`, `(NY,NX)`.
    pub maptmp: Vec<i32>,
    pub mapfs: Vec<i32>,
    pub mapsf: Vec<i32>,
    pub trflag: i32,
    pub trnx: Vec<f32>,
    pub trny: Vec<f32>,
    pub dtcfl: f32,
    pub dtcfli: f32,
    pub ctmax: f32,
    pub clats: Vec<f32>,
    pub cthg0s: Vec<f32>,
    pub fldry: bool,
    pub flbpi: bool,
    pub mapwn: Vec<i32>,
    pub wdcg: f32,
    pub wdth: f32,
    pub dtmax: f32,
    pub dtmin: f32,
    pub dmin: f32,
    pub flcx: bool,
    pub flcy: bool,
    pub flcth: bool,
    pub flck: bool,
    pub flsou: bool,
    pub icescales: Vec<f32>,
    pub dth: f32,
    pub th: Vec<f32>,
    pub esin: Vec<f32>,
    pub ecos: Vec<f32>,
    pub es2: Vec<f32>,
    pub esc: Vec<f32>,
    pub ec2: Vec<f32>,
    pub xfr: f32,
    pub sig: Vec<f32>,
    pub sig2: Vec<f32>,
    pub dsip: Vec<f32>,
    pub dden: Vec<f32>,
    pub dden2: Vec<f32>,
    pub fte: f32,
    pub ftf: f32,
    pub facti1: f32,
    pub facti2: f32,
    pub fachfa: f32,
    pub fachfe: f32,
    pub facp: f32,
    pub xrel: f32,
    pub xflt: f32,
    pub fhmax: f32,
    pub delab: f32,
    pub fwtable: Vec<f32>,
    pub slnc1: f32,
    pub fspm: f32,
    pub fshf: f32,
    pub zzwnd: f32,
    pub aalpha: f32,
    pub bbeta: f32,
    pub ssinthp: f32,
    pub zzalp: f32,
    pub ttauwshelter: f32,
    pub sswellf: Vec<f32>,
    pub zz0rat: f32,
    pub ssdsc: Vec<f32>,
    pub ssdsiso: i32,
    pub ssdsbr: f32,
    pub ssdsbt: f32,
    pub ssdsbm: Vec<f32>,
    pub ssdsp: f32,
    pub ssdsdth: f32,
    pub wwnmeanp: f32,
    pub wwnmeanptail: f32,
    pub sstxftftail: f32,
    pub sstxftwn: f32,
    pub ssdsbrfdf: i32,
    pub ffxpm: f32,
    pub ffxfm: f32,
    pub ffxfa: f32,
    pub delust: f32,
    pub deltail: f32,
    pub deltauw: f32,
    pub delu: f32,
    pub delalp: f32,
    pub taut: Vec<f32>,
    pub tauhft: Vec<f32>,
    pub tauhft2: Vec<f32>,
    pub iktab: Vec<i32>,
    pub dcki: Vec<f32>,
    pub qbi: Vec<f32>,
    pub satindices: Vec<i32>,
    pub satweights: Vec<f32>,
    pub sat_n: i32,
    pub dikcumul: i32,
    pub cumulw: Vec<f32>,
    pub snlc1: f32,
    pub lam: f32,
    pub kdcon: f32,
    pub kdmn: f32,
    pub snls1: f32,
    pub snls2: f32,
    pub snls3: f32,
    pub sbtc1: f32,
    pub sdbc1: f32,
    pub sdbc2: f32,
    pub fdonly: bool,
    pub aaircmin: f32,
    pub aairgb: f32,
}

const NKHS: usize = 2000;
const NKD: usize = 1300;
const NDTAB: usize = 2000;

impl ModDef {
    pub fn read(path: &str) -> Self {
        let mut r = Records::open(path);
        let mut m = ModDef::default();

        let mut c = r.next();
        c.skip(35 + 10);
        (m.nx, m.ny, m.nsea, m.nth, m.nk) = (c.i(), c.i(), c.i(), c.i(), c.i());
        c.skip(2 * 4); // NBI NFBPO
        m.gname = c.text(30);
        let (nx, ny, nsea) = (m.nx as usize, m.ny as usize, m.nsea as usize);
        let (nth, nk) = (m.nth as usize, m.nk as usize);
        let nspec = nth * nk;
        let nfull = nspec + nth;

        r.next(); // NBO, NBO2
        let mut c = r.next();
        let gtype = c.i();
        m.flagll = c.i() != 0;
        m.iclose = c.i();
        assert_eq!(gtype, 1, "only a rectilinear grid is implemented");
        c.done("GTYPE");

        let mut c = r.next();
        (m.sx, m.sy, m.x0, m.y0) = (c.f(), c.f(), c.f(), c.f());
        c.done("SX");

        let mut c = r.next();
        m.zb = c.fs(nsea);
        m.maptmp = c.is(ny * nx);
        m.mapfs = c.is(ny * nx);
        m.mapsf = c.is(nsea * 3);
        m.trflag = c.i();
        c.done("ZB");
        // W3IOGR's read fills the third column.
        for i in 0..nsea {
            m.mapsf[i + 2 * nsea] = m.mapsf[i + nsea] + (m.mapsf[i] - 1) * m.ny;
        }
        if m.trflag != 0 {
            let mut c = r.next();
            m.trnx = c.fs(ny * nx);
            m.trny = c.fs(ny * nx);
            c.done("TRNX");
        }

        let mut c = r.next();
        m.dtcfl = c.f();
        m.dtcfli = c.f();
        m.dtmax = c.f();
        m.dtmin = c.f();
        m.dmin = c.f();
        m.ctmax = c.f();
        c.skip(4 * 4); // FICE0 FICEN FICEL PFMOVE
        m.fldry = c.i() != 0;
        m.flcx = c.i() != 0;
        m.flcy = c.i() != 0;
        m.flcth = c.i() != 0;
        m.flck = c.i() != 0;
        m.flsou = c.i() != 0;
        m.flbpi = c.i() != 0;
        c.i(); // FLBPO
        // CLATS, CLATIS and CTHG0S are (0:NSEA).
        c.f();
        m.clats = c.fs(nsea);
        c.skip((nsea + 1) * 4); // CLATIS
        c.f();
        m.cthg0s = c.fs(nsea);
        // From here the sizes depend on the grid, so the end of the record is the anchor: ICESCALES(4),
        // CALTYPE*8, CMPRTRCK, IICEHFAC, IICEDDISP, IICEHDISP, IICEFDISP, BTBETA, AAIRCMIN, AAIRGB.
        c.skip(c.len() - 56 - c.p);
        m.icescales = c.fs(4);
        c.skip(32);
        m.aaircmin = c.f();
        m.aairgb = c.f();
        c.done("DTCFL");

        r.next(); // GRIDSHIFT

        let mut c = r.next();
        m.mapwn = c.is(nfull);
        c.skip(nfull * 4); // MAPTH
        m.dth = c.f();
        m.th = c.fs(nth);
        m.esin = c.fs(nfull);
        m.ecos = c.fs(nfull);
        m.es2 = c.fs(nfull);
        m.esc = c.fs(nfull);
        m.ec2 = c.fs(nfull);
        m.xfr = c.f();
        c.f(); // FR1
        m.sig = c.fs(nk + 2);
        m.sig2 = c.fs(nspec);
        m.dsip = c.fs(nk + 2);
        c.skip(nk * 4); // DSII
        m.dden = c.fs(nk);
        m.dden2 = c.fs(nspec);
        m.fte = c.f();
        m.ftf = c.f();
        c.f(); // FTWN
        c.f(); // FTTR
        c.f(); // FTWL
        m.facti1 = c.f();
        m.facti2 = c.f();
        m.fachfa = c.f();
        m.fachfe = c.f();
        c.done("MAPWN");

        r.next(); // E3DF P2MSF US3DF USSPF USSP_WN
        r.next(); // XBPO YBPO RDBPO IPBPO ISBPO
        r.next(); // IHMAX HSPMIN WSMULT WSCUT FLCOMB NOSWLL PTMETH PTFCUT

        let mut c = r.next();
        m.facp = c.f();
        m.xrel = c.f();
        m.xflt = c.f();
        c.skip(2 * 4); // FXFM FXPM
        c.skip(3 * 4); // XFT XFC FACSD
        m.fhmax = c.f();
        c.f(); // FFACBERG
        m.delab = c.f();
        m.fwtable = c.fs(301);
        c.done("FACP");

        let mut c = r.next();
        (m.slnc1, m.fspm, m.fshf) = (c.f(), c.f(), c.f());
        c.done("SLNC1");

        let mut c = r.next();
        m.zzwnd = c.f();
        m.aalpha = c.f();
        c.f(); // ZZ0MAX
        m.bbeta = c.f();
        m.ssinthp = c.f();
        m.zzalp = c.f();
        m.ttauwshelter = c.f();
        c.i(); // SSWELLFPAR
        m.sswellf = c.fs(7);
        c.f(); // SSINBR
        m.zz0rat = c.f();
        m.ssdsc = c.fs(21);
        m.ssdsiso = c.i();
        m.ssdsbr = c.f();
        m.ssdsbt = c.f();
        m.ssdsbm = c.fs(5);
        m.ssdsp = c.f();
        c.f(); // SSDSCOS
        m.ssdsdth = c.f();
        m.wwnmeanp = c.f();
        m.wwnmeanptail = c.f();
        c.f(); // SSTXFTF
        m.sstxftftail = c.f();
        m.sstxftwn = c.f();
        c.f(); // SSTXFTF again
        c.f(); // SSTXFTWN again
        c.f(); // SSDSBRF1
        c.f(); // SSDSBRF2
        m.ssdsbrfdf = c.i();
        c.skip(4 * 4); // SSDSBCK SSDSABK SSDSPBK SSDSBINT
        m.ffxpm = c.f();
        m.ffxfm = c.f();
        m.ffxfa = c.f();
        c.f(); // SSDSHCK
        m.delust = c.f();
        m.deltail = c.f();
        m.deltauw = c.f();
        m.delu = c.f();
        m.delalp = c.f();
        m.taut = c.fs(201 * 201);
        m.tauhft = c.fs(101 * 201);
        m.tauhft2 = c.fs(101 * 201 * 51);
        m.iktab = c.is(nk * NDTAB);
        m.dcki = c.fs(NKHS * NKD);
        m.qbi = c.fs(NKHS * NKD);
        // SATINDICES and SATWEIGHTS are (NTH-1, NTH) here, read from what is left of the record.
        let left = c.len() - c.p;
        let sat = left - 4 - nspec * nspec * 4;
        m.sat_n = (sat / 8 / nth) as i32;
        m.satindices = c.is(sat / 8);
        m.satweights = c.fs(sat / 8);
        m.dikcumul = c.i();
        m.cumulw = c.fs(nspec * nspec);
        c.done("ST4");

        let mut c = r.next();
        (m.snlc1, m.lam, m.kdcon, m.kdmn) = (c.f(), c.f(), c.f(), c.f());
        (m.snls1, m.snls2, m.snls3) = (c.f(), c.f(), c.f());
        c.done("SNLC1");

        let mut c = r.next();
        m.sbtc1 = c.f();
        c.done("SBTC1");

        let mut c = r.next();
        (m.sdbc1, m.sdbc2) = (c.f(), c.f());
        m.fdonly = c.i() != 0;
        c.done("SDBC1");
        let mut c = r.next();
        (m.wdcg, m.wdth) = (c.f(), c.f());
        c.done("WDCG");
        m
    }

    pub fn snl1(&self) -> Snl1Tables {
        let given = Snl1Params {
            nk: self.nk,
            nth: self.nth,
            nspec: self.nk * self.nth,
            fachfe: self.fachfe,
            kdcon: self.kdcon,
            kdmn: self.kdmn,
            snlc1: self.snlc1,
            snls1: self.snls1,
            snls2: self.snls2,
            snls3: self.snls3,
            ..Default::default()
        };
        insnl1(given, self.dth, self.xfr, self.lam, &self.sig)
    }

    /// `flogrd` is `(FLOGRD(5,7), FLOGRD(5,8))`, which come from the run, not from this file.
    pub fn sds4(&self, flogrd: (bool, bool)) -> Sds4Params {
        let mut p = Sds4Params {
            nk: self.nk,
            nth: self.nth,
            nspec: self.nk * self.nth,
            dikcumul: self.dikcumul,
            ssdsbrfdf: self.ssdsbrfdf,
            ssdsiso: self.ssdsiso,
            nkhs: NKHS as i32,
            nkd: NKD as i32,
            ndtab: NDTAB as i32,
            sat_n: self.sat_n,
            cumul_n: self.nk * self.nth,
            flogrd57: flogrd.0 as u32,
            flogrd58: flogrd.1 as u32,
            ssdsbr: self.ssdsbr,
            ssdsbt: self.ssdsbt,
            ssdsp: self.ssdsp,
            ssdsdth: self.ssdsdth,
            dth: self.dth,
            aairgb: self.aairgb,
            aaircmin: self.aaircmin,
            ..Default::default()
        };
        p.ssdsc.copy_from_slice(&self.ssdsc);
        p.ssdsbm.copy_from_slice(&self.ssdsbm);
        p
    }

    pub fn sin4(&self) -> Sin4Params {
        let mut p = Sin4Params {
            nk: self.nk,
            nth: self.nth,
            nspec: self.nk * self.nth,
            bbeta: self.bbeta,
            zzwnd: self.zzwnd,
            aalpha: self.aalpha,
            zzalp: self.zzalp,
            ttauwshelter: self.ttauwshelter,
            ssinthp: self.ssinthp,
            zz0rat: self.zz0rat,
            dth: self.dth,
            delust: self.delust,
            deltail: self.deltail,
            delalp: self.delalp,
            delab: self.delab,
            ..Default::default()
        };
        p.sswellf.copy_from_slice(&self.sswellf);
        p
    }

    pub fn spr4(&self) -> Spr4Params {
        Spr4Params {
            nk: self.nk,
            nth: self.nth,
            nspec: self.nk * self.nth,
            wwnmeanp: self.wwnmeanp,
            wwnmeanptail: self.wwnmeanptail,
            fte: self.fte,
            ftf: self.ftf,
            sstxftftail: self.sstxftftail,
            sstxftwn: self.sstxftwn,
            zzwnd: self.zzwnd,
            aalpha: self.aalpha,
            deltauw: self.deltauw,
            delu: self.delu,
        }
    }

    pub fn srce(&self, snl: Snl1Params, flogrd: (bool, bool)) -> SrceParams {
        let mut p = SrceParams {
            nk: self.nk,
            nth: self.nth,
            nspec: self.nk * self.nth,
            fdonly: self.fdonly as u32,
            inflags2_4: 0,
            dmin: self.dmin,
            dtmax: self.dtmax,
            dtmin: self.dtmin,
            facti1: self.facti1,
            facti2: self.facti2,
            fachfa: self.fachfa,
            facp: self.facp,
            xflt: self.xflt,
            xrel: self.xrel,
            fhmax: self.fhmax,
            ffxfm: self.ffxfm,
            ffxpm: self.ffxpm,
            ffxfa: self.ffxfa,
            slnc1: self.slnc1,
            fspm: self.fspm,
            fshf: self.fshf,
            sbtc1: self.sbtc1,
            sdbc1: self.sdbc1,
            sdbc2: self.sdbc2,
            snl,
            sds: self.sds4(flogrd),
            sin: self.sin4(),
            spr: self.spr4(),
            ..Default::default()
        };
        p.icescales.copy_from_slice(&self.icescales);
        p
    }

    pub fn tables<'a>(&'a self, t: &'a Snl1Tables) -> SrceTables<'a> {
        SrceTables {
            sig: &self.sig,
            sig2: &self.sig2,
            dden: &self.dden,
            dden2: &self.dden2,
            ec2: &self.ec2,
            es2: &self.es2,
            esc: &self.esc,
            ecos: &self.ecos,
            esin: &self.esin,
            af11: &t.af11,
            pm: &t.pm,
            ic: &t.ic,
            satindices: &self.satindices,
            satweights: &self.satweights,
            cumulw: &self.cumulw,
            iktab: &self.iktab,
            dcki: &self.dcki,
            qbi: &self.qbi,
            fwtable: &self.fwtable,
            tauhft: &self.tauhft,
            tauhft2: &self.tauhft2,
            taut: &self.taut,
        }
    }
}
