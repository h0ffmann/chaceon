// SPDX-License-Identifier: MPL-2.0
//! `mod_def.ww3`, as `W3IOGR` writes it (`model/src/w3iogrmd.F90:115-1781`, the `WRITE` branches) at
//! metocean/WW3 `92e2cf66`, for `switch_chaceon_shrd` and rectilinear or curvilinear grids. It is the
//! inverse of `ModDef::read`: records are big-endian with big-endian markers (`ENDIANNESS='big_endian'`).

use crate::moddef::ModDef;

/// Fortran unformatted sequential output, one record at a time.
struct Out {
    file: Vec<u8>,
    rec: Vec<u8>,
}

impl Out {
    fn i(&mut self, v: i32) {
        self.rec.extend_from_slice(&v.to_be_bytes());
    }
    fn l(&mut self, v: bool) {
        self.i(v as i32);
    }
    fn f(&mut self, v: f32) {
        self.rec.extend_from_slice(&v.to_bits().to_be_bytes());
    }
    fn d(&mut self, v: f64) {
        self.rec.extend_from_slice(&v.to_bits().to_be_bytes());
    }
    fn fs(&mut self, v: &[f32]) {
        for x in v {
            self.f(*x);
        }
    }
    fn is(&mut self, v: &[i32]) {
        for x in v {
            self.i(*x);
        }
    }
    /// A `CHARACTER(LEN=n)`: blank padded on the right.
    fn text(&mut self, s: &str, n: usize) {
        assert!(s.len() <= n, "{s:?} is longer than {n} characters");
        self.rec.extend_from_slice(s.as_bytes());
        self.rec.resize(self.rec.len() + n - s.len(), b' ');
    }
    fn end(&mut self) {
        let n = (self.rec.len() as i32).to_be_bytes();
        self.file.extend_from_slice(&n);
        self.file.append(&mut self.rec);
        self.file.extend_from_slice(&n);
    }
}

/// `WRITE (NDSM) ...` of every record of `W3IOGR`, in its order.
pub fn write(m: &ModDef, path: &str) {
    assert!(m.gtype == 1 || m.gtype == 2, "only rectilinear and curvilinear grids are implemented");
    let (nx, ny, nsea) = (m.nx as usize, m.ny as usize, m.nsea as usize);
    let mut o = Out { file: Vec::new(), rec: Vec::new() };

    o.text(&m.idstr, 35);
    o.text(&m.vergrd, 10);
    o.is(&[m.nx, m.ny, m.nsea, m.nth, m.nk, m.nbi, m.nfbpo]);
    o.text(&m.gname, 30);
    for n in &m.fnames {
        o.text(n, 30);
    }
    o.end();

    o.is(&m.nbo);
    o.is(&m.nbo2);
    o.end();

    o.i(m.gtype);
    o.l(m.flagll);
    o.i(m.iclose);
    o.end();

    if m.gtype == 1 {
        o.fs(&[m.sx, m.sy, m.x0, m.y0]);
    } else {
        o.fs(&m.xgrd);
        o.fs(&m.ygrd);
    }
    o.end();

    o.fs(&m.zb);
    o.is(&m.maptmp);
    o.is(&m.mapfs);
    // MAPSF(:,3) is zero in the file (W3DIMX) and filled when W3IOGR reads it.
    o.is(&m.mapsf[..2 * nsea]);
    o.is(&vec![0; nsea]);
    o.i(m.trflag);
    o.end();
    if m.trflag != 0 {
        o.fs(&m.trnx);
        o.fs(&m.trny);
        o.end();
    }
    debug_assert_eq!(m.maptmp.len(), nx * ny);

    o.fs(&[m.dtcfl, m.dtcfli, m.dtmax, m.dtmin, m.dmin, m.ctmax]);
    o.fs(&[m.fice0, m.ficen, m.ficel, m.pfmove]);
    for b in [m.fldry, m.flcx, m.flcy, m.flcth, m.flck, m.flsou, m.flbpi, m.flbpo] {
        o.l(b);
    }
    o.f(m.clat0[0]);
    o.fs(&m.clats);
    o.f(m.clat0[1]);
    o.fs(&m.clatis);
    o.f(m.clat0[2]);
    o.fs(&m.cthg0s);
    o.fs(&[m.stexu, m.steyu, m.stedu, m.iicehmin, m.iicehinit]);
    o.l(m.iicedisp);
    o.fs(&m.icescales);
    o.text(&m.caltype, 8);
    o.l(m.cmprtrck);
    o.fs(&[m.iicehfac, m.iicehdisp, m.iiceddisp, m.iicefdisp, m.btbeta, m.aaircmin, m.aairgb]);
    o.end();

    o.d(m.gridshift);
    o.end();

    o.is(&m.mapwn);
    o.is(&m.mapth);
    o.f(m.dth);
    o.fs(&m.th);
    o.fs(&m.esin);
    o.fs(&m.ecos);
    o.fs(&m.es2);
    o.fs(&m.esc);
    o.fs(&m.ec2);
    o.fs(&[m.xfr, m.fr1]);
    o.fs(&m.sig);
    o.fs(&m.sig2);
    o.fs(&m.dsip);
    o.fs(&m.dsii);
    o.fs(&m.dden);
    o.fs(&m.dden2);
    o.fs(&[m.fte, m.ftf, m.ftwn, m.fttr, m.ftwl, m.facti1, m.facti2, m.fachfa, m.fachfe]);
    o.end();

    o.is(&m.e3df);
    o.is(&m.p2msf);
    o.is(&m.us3df);
    o.is(&m.usspf);
    o.fs(&m.ussp_wn);
    o.end();

    o.fs(&m.xbpo);
    o.fs(&m.ybpo);
    o.fs(&m.rdbpo);
    o.is(&m.ipbpo);
    o.is(&m.isbpo);
    o.end();

    o.i(m.ihmax);
    o.fs(&[m.hspmin, m.wsmult, m.wscut]);
    o.l(m.flcomb);
    o.i(m.noswll);
    o.i(m.ptmeth);
    o.f(m.ptfcut);
    o.end();

    o.fs(&[m.facp, m.xrel, m.xflt, m.fxfm, m.fxpm, m.xft, m.xfc, m.facsd, m.fhmax, m.ffacberg, m.delab]);
    o.fs(&m.fwtable);
    o.end();

    o.fs(&[m.slnc1, m.fspm, m.fshf]);
    o.end();

    o.fs(&[m.zzwnd, m.aalpha, m.zz0max, m.bbeta, m.ssinthp, m.zzalp, m.ttauwshelter]);
    o.i(m.sswellfpar);
    o.fs(&m.sswellf);
    o.fs(&[m.ssinbr, m.zz0rat]);
    o.fs(&m.ssdsc);
    o.i(m.ssdsiso);
    o.fs(&[m.ssdsbr, m.ssdsbt]);
    o.fs(&m.ssdsbm);
    o.fs(&[m.ssdsp, m.ssdscos, m.ssdsdth, m.wwnmeanp, m.wwnmeanptail]);
    o.fs(&[m.sstxftf, m.sstxftftail, m.sstxftwn, m.sstxftf, m.sstxftwn, m.ssdsbrf1, m.ssdsbrf2]);
    o.i(m.ssdsbrfdf);
    o.fs(&[m.ssdsbck, m.ssdsabk, m.ssdspbk, m.ssdsbint, m.ffxpm, m.ffxfm, m.ffxfa, m.ssdshck]);
    o.fs(&[m.delust, m.deltail, m.deltauw, m.delu, m.delalp]);
    o.fs(&m.taut);
    o.fs(&m.tauhft);
    o.fs(&m.tauhft2);
    o.is(&m.iktab);
    o.fs(&m.dcki);
    o.fs(&m.qbi);
    o.is(&m.satindices);
    o.fs(&m.satweights);
    o.i(m.dikcumul);
    o.fs(&m.cumulw);
    o.end();

    o.fs(&[m.snlc1, m.lam, m.kdcon, m.kdmn, m.snls1, m.snls2, m.snls3]);
    o.end();

    o.f(m.sbtc1);
    o.end();

    o.fs(&[m.sdbc1, m.sdbc2]);
    o.l(m.fdonly);
    o.end();

    o.fs(&[m.wdcg, m.wdth]);
    o.end();

    std::fs::write(path, &o.file).unwrap_or_else(|e| panic!("{path}: {e}"));
}
