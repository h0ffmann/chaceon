// SPDX-License-Identifier: MPL-2.0
//! `W3EXNC` and `W3CRNC` of `model/src/ww3_ounf.F90` at metocean/WW3 `92e2cf66` (`:860-3486`, `:3505-3990`),
//! with `S2GRID` (`:4007-4070`), `UV_TO_MAG_DIR` of `W3SERVMD` and the metadata writer `WRITE_META`
//! (`model/src/w3ounfmetamd.F90:2115-2246`). The order of the NetCDF calls is the Fortran's, because the bytes of
//! the file depend on it. Not implemented, and refused where the program reads its input: SMC and unstructured
//! grids, rotated poles, forecast variables, a calendar other than `standard`, `ounfmeta.inp`, `NC_globatt.inp`.

use crate::nc;
use std::collections::HashMap;
use ww3_host::iogo::UNDEF;
use ww3_host::moddef::ModDef;
use ww3_host::ounf::*;
use ww3_host::ounfmeta::{self, Ctx, Meta, Metas};
use ww3_host::wtime::{dsec21, tick21, Time};
use ww3_kernels::constants::{DWAT, GRAV, RADE, TPIINV};
use ww3_kernels::real::Real;

type Key = (usize, usize, usize);

/// `SINGLEFILE` of `W3OUNF`. The namelist branch never sets it (`ww3_ounf.F90:427` is in the `.inp` branch), and the
/// reference binary reads it as true: `w3exnc` loads it from the stack frame of `MAIN__` (`ldr w0, [x0, #112]` at
/// 0xa944 and 0xb0e4 of the `ww3_ounf` in `chaceon-ref`), and a run with `FIELD%TIMESPLIT = 8` or `10` writes every
/// time into the file named after the first. ⚠ The value is stack contents the program never wrote.
const SINGLEFILE: bool = true;

/// The grid arrays of `W3EXNC`: `(NX+1,NY)`, `IX` fastest.
#[derive(Clone, Copy, PartialEq)]
enum X {
    X1,
    X2,
    Xx,
    Xy,
}

pub struct Ounf {
    m: ModDef,
    q: Request,
    grd: GrdIn,
    nx: usize,
    ny: usize,
    nsea: usize,
    nk: usize,
    // Program state of W3OUNF.
    nctype: i32,
    ncvartypei: i32,
    /// The `NCVARTYPE` of `W3OUNFMETAMD`, which `W3EXNC` changes field by field.
    ncvartype: i32,
    s3: i32,
    tvartype: i32,
    ix1: usize,
    ixn: usize,
    iy1: usize,
    iyn: usize,
    flg2d: Vec<bool>,
    tabipart: Vec<usize>,
    meta: Metas,
    snamep: Vec<String>,
    ncids: HashMap<Key, i32>,
    // SAVE variables of W3EXNC.
    oldtimeid: String,
    timeid: String,
    /// `CUSTOMFRQ`, which has an initial value and so is `SAVE`d.
    customfrq: bool,
    /// `FIRSTTIME`, `SAVE`d.
    firsttime: String,
    /// For the line that says which branches a run reached.
    steps: usize,
    files: usize,
    pub strstopdate: String,
    // Work arrays.
    x1: Vec<Real>,
    x2: Vec<Real>,
    xx: Vec<Real>,
    xy: Vec<Real>,
    xk: Vec<Real>,
    xxk: Vec<Real>,
    xyk: Vec<Real>,
}

/// `NINT` of a `REAL` into a default `INTEGER`, as the reference's build does it on aarch64 (`fcvtas`): halves away from
/// zero, and a value out of range saturates (`P2L` with `FIELD%TYPE = 3` has `valid_max` of 1.E12/0.0004).
fn nint(x: Real) -> i32 {
    x.round() as i32
}

fn fortran_index(hay: &str, needle: &str) -> usize {
    hay.find(needle).map_or(0, |i| i + 1)
}

fn pad16(s: &str) -> String {
    format!("{s:<16}")
}

/// `W3S2XY`: only the sea points are assigned, as in the Fortran.
fn s2xy(m: &ModDef, s: &[Real], x: &mut [Real]) {
    let nsea = m.nsea as usize;
    let nx1 = m.nx as usize + 1;
    for (isea, &v) in s.iter().enumerate().take(nsea) {
        let ix = m.mapsf[isea] as usize - 1;
        let iy = m.mapsf[isea + nsea] as usize - 1;
        x[ix + iy * nx1] = v;
    }
}

/// `S2GRID`, for a grid that is not SMC.
fn s2grid(m: &ModDef, noval: Real, s: &mut [Real], x: &mut [Real], fldirn: bool) {
    if fldirn {
        for v in s.iter_mut() {
            if *v != UNDEF {
                *v = (630. - RADE * *v) % 360.;
            }
        }
    }
    if noval != UNDEF {
        for v in s.iter_mut() {
            if *v == UNDEF {
                *v = noval;
            }
        }
    }
    s2xy(m, s, x);
}

/// `UV_TO_MAG_DIR` of `W3SERVMD` (`:1921-2019`), in place. `conv` is `'N'` or `'O'`.
fn uv_to_mag_dir(u: &mut [Real], v: &mut [Real], tol: Real, conv: char) {
    let offset: Real = if conv == 'N' { 630. } else { 450. };
    for (u, v) in u.iter_mut().zip(v.iter_mut()) {
        let tmp = (*u * *u + *v * *v).sqrt();
        if tmp >= tol {
            *v = (offset + (-RADE * v.atan2(*u))) % 360.;
            *u = tmp;
        } else {
            *u = UNDEF;
            *v = UNDEF;
        }
    }
}

impl Ounf {
    /// Everything `W3OUNF` does before its time loop (`:262-720`), for the namelist input.
    pub fn new(dir: &str) -> Self {
        let m = ModDef::read("mod_def.ww3");
        println!("\n                  *** WAVEWATCH III Field output postp. ***   \n               ===============================================\n");
        println!("  Grid name : {:<30}\n", m.gname);
        let _ = dir;
        let mut grd = GrdIn::open("out_grd.ww3", &m);
        let ok = grd.read(&m);
        assert!(ok, "PREMATURE END OF FILE");
        println!("  Fields in file : \n --------------------------");
        for ifi in 1..=NOGRP {
            for ifj in 1..=NGRPP {
                if grd.flogrd[(ifi - 1) + (ifj - 1) * NOGRP] {
                    println!("      {:<20}", idout(ifi, ifj));
                }
            }
        }
        assert!(
            std::path::Path::new("ww3_ounf.nml").exists(),
            "ww3_ounf.inp is not implemented: only ww3_ounf.nml is read"
        );
        let q = Request::read("ww3_ounf.nml");
        let mut flg2d = flags_from_list(&q.list, &m);
        report_fields(&flg2d);

        // 4.4 checks on the time variable.
        assert!(q.timeunit == 'S' || q.timeunit == 'D', "TIMEUNITS MUST BE ONE OF \"S\" OR \"D\"");
        let tvartype = match q.timevar {
            'D' => nc::NC_DOUBLE,
            'I' => nc::NC_INT64,
            other => panic!("TIMEVAR TYPE MUST BE ONE OF \"I\" OR \"D\", GOT: {other}"),
        };
        assert!(!(q.timevar == 'I' && q.timeunit == 'D'), "CANNOT HAVE TIME UNITS OF DAYS WITH TIME VARYTPE OF INT64");
        assert!(!(tvartype == nc::NC_INT64 && q.nctype < 4), "INT64 TIME ENCODING REQUIRES NETCDF4 FILE FORMAT");
        assert!(!q.fcvars, "FIELD%FCVARS is not implemented (forecast_period, forecast_reference_time)");
        assert!(m.caltype == "standard", "CALTYPE {} is not implemented (standard only)", m.caltype);
        assert!(m.gtype == 1 || m.gtype == 2, "only rectilinear and curvilinear grids are implemented");
        assert!(!std::path::Path::new("ounfmeta.inp").exists(), "ounfmeta.inp is not implemented");
        assert!(!std::path::Path::new("NC_globatt.inp").exists(), "NC_globatt.inp is not implemented");

        // 4.1 Time setup.
        let mut tout = q.timestart;
        let dtreq = q.dtreq.max(0.);
        let mut nout = q.nout;
        if dtreq == 0. {
            nout = 1;
        }
        nout = nout.max(1);
        println!("\n  Output time data : \n --------------------------------------------------\n      First time         : {}", stme21(tout));
        let mut tdum: Time = [0, 0];
        tick21(&mut tdum, dtreq);
        let mut idtime = stme21(tdum);
        let ddday = if dtreq >= 86400. { format!("{:10} ", (dtreq / 86400.) as i32) } else { "           ".to_string() };
        idtime.replace_range(0..11, &ddday);
        idtime.replace_range(20..23, "   ");
        println!("      Interval           : {idtime}\n      Number of requests : {nout:10}");

        // 4.2 Output fields.
        for ifi in 1..=NOGRP {
            for ifj in 1..=NGRPP {
                let k = (ifi - 1) + (ifj - 1) * NOGRP;
                if flg2d[k] {
                    if grd.flogrd[k] {
                        println!("      {:<20}   ", idout(ifi, ifj));
                    } else {
                        println!("      {:<20}  *** NOT AVAILABLE ***", idout(ifi, ifj));
                        flg2d[k] = false;
                    }
                }
            }
        }

        // 4.3 Output type: the partitions requested.
        let noswll = m.noswll as usize;
        let mut tabipart = Vec::new();
        for ch in q.partition.chars().take(30) {
            if ch == ' ' {
                continue;
            }
            let ipart = ch.to_digit(10).unwrap_or_else(|| panic!("FIELD%PARTITION: {ch:?} is not a digit")) as usize;
            if ipart > noswll {
                println!("\n *** WAVEWATCH III WARNING IN W3OUNF : \n     IGNORING REQUEST FOR IPART ={ipart:2}\n     MAX SWELL PARTITIONS (NOSW) ={noswll:2}\n");
                continue;
            }
            tabipart.push(ipart);
            assert!(tabipart.len() <= noswll + 1, "OUT OF RANGE REQUEST FOR NBIPART ={:2}\n     MAX SWELL PARTITIONS (NOSW) ={noswll:2}", tabipart.len());
        }
        assert!(q.nctype == 3 || q.nctype == 4, "ILLEGAL TYPE, NCTYPE ={:4}", q.nctype);
        let (nx, ny, nsea, nk) = (m.nx as usize, m.ny as usize, m.nsea as usize, m.nk as usize);
        let ix1 = q.ix0.max(1) as usize;
        let ixn = q.ixn.min(m.nx) as usize;
        let iy1 = q.iy0.max(1) as usize;
        let iyn = q.iyn.min(m.ny) as usize;
        println!("      X range : {ix1:7}{ixn:7}\n      Y range : {iy1:7}{iyn:7}");

        // 4.4 Initialise meta-data.
        let ctx = meta_ctx(&m, &q);
        let meta = ounfmeta::default_meta(&ctx);
        // SNAMEP is CHARACTER(LEN=15): 'quaternary_swell' keeps its first 15 characters.
        let snamep: Vec<String> = if m.ptmeth <= 3 {
            ["wind", "primary_swell", "secondary_swell", "tertiary_swell", "quaternary_swell", "quinary_swell"]
        } else {
            ["wind", "swell", "", "", "", ""]
        }
        .iter()
        .map(|s| s.chars().take(15).collect())
        .collect();

        println!("\n  Generating files \n --------------------------------------------------");
        let mut o = Ounf {
            nx,
            ny,
            nsea,
            nk,
            nctype: q.nctype,
            ncvartypei: q.ncvartype,
            ncvartype: q.ncvartype,
            s3: q.timesplit,
            tvartype,
            ix1,
            ixn,
            iy1,
            iyn,
            flg2d,
            tabipart,
            meta,
            snamep,
            ncids: HashMap::new(),
            oldtimeid: pad16("0000000000000000"),
            timeid: pad16("0000000000000000"),
            customfrq: false,
            firsttime: pad16("0000000000000000"),
            steps: 0,
            files: 0,
            strstopdate: String::new(),
            x1: Vec::new(),
            x2: Vec::new(),
            xx: Vec::new(),
            xy: Vec::new(),
            xk: Vec::new(),
            xxk: Vec::new(),
            xyk: Vec::new(),
            m,
            q,
            grd,
        };

        // 5.1 Loops on out_grd.ww3 to read the time and data.
        let mut iout = 0;
        loop {
            let dtest = dsec21(o.grd.time, tout);
            if dtest > 0. {
                if !o.grd.read(&o.m) {
                    println!("\n      End of file reached \n");
                    break;
                }
                continue;
            }
            if dtest < 0. {
                tick21(&mut tout, dtreq);
                continue;
            }
            iout += 1;
            o.steps += 1;
            println!("      Files for {}", stme21(tout));
            o.exnc();
            let sd = t2d(tout);
            o.strstopdate = format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", sd[0], sd[1], sd[2], sd[4], sd[5], sd[6]);
            tick21(&mut tout, dtreq);
            if iout >= nout {
                break;
            }
        }
        o
    }

    /// 5.2 and the loop after it: stop dates and close of every file still open.
    pub fn finish(&mut self) {
        let stop = self.strstopdate.clone();
        let close = |id: i32| {
            nc::redef(id);
            nc::att_text(id, nc::NC_GLOBAL, "stop_date", &stop);
            nc::close(id);
        };
        let nid = |o: &Self, k: Key| o.ncids.get(&k).copied().unwrap_or(0);
        if self.q.together && nid(self, (1, 1, 1)) != 0 {
            close(nid(self, (1, 1, 1)));
        }
        let noswll = self.m.noswll as usize;
        for ifi in 1..=NOGRP {
            for ifj in 1..=NGRPP {
                let k = (ifi - 1) + (ifj - 1) * NOGRP;
                if self.flg2d[k] && self.grd.flogrd[k] {
                    if !self.q.together {
                        if nid(self, (ifi, ifj, 1)) != 0 {
                            close(nid(self, (ifi, ifj, 1)));
                        }
                        if ifi == 4 && ifj <= NOGE[ifi - 1] {
                            for ipart in 1..=noswll {
                                if nid(self, (ifi, ifj, ipart + 1)) != 0 {
                                    close(nid(self, (ifi, ifj, ipart + 1)));
                                }
                            }
                        }
                    } else if ((ifi == 6 && ifj == 8) || (ifi == 6 && ifj == 9) || ifi == 3) && nid(self, (ifi, ifj, 1)) != 0 {
                        close(nid(self, (ifi, ifj, 1)));
                    }
                }
            }
        }
        println!("\n  End of program \n =========================================\n         WAVEWATCH III Field output \n");
        let names: Vec<&str> = (1..=NOGRP)
            .flat_map(|i| (1..=NGRPP).map(move |j| (i, j)))
            .filter(|&(i, j)| self.flg2d[(i - 1) + (j - 1) * NOGRP])
            .map(|(i, j)| idout(i, j).trim_end())
            .collect();
        // On stdout, after the Fortran's own lines: two streams of a container reach a file in any order.
        let line = format!(
            "ww3_ounf: reached: NETCDF {}, FIELD%TYPE {}, {}, TIMESPLIT {}, time {} {}, {} {}, {} steps, {} files, {} fields",
            self.nctype,
            self.ncvartypei,
            if self.q.together { "SAMEFILE" } else { "one file per field" },
            self.s3,
            if self.q.timevar == 'I' { "int64" } else { "double" },
            if self.q.timeunit == 'S' { "seconds" } else { "days" },
            if self.q.vector { "vectors" } else { "magnitude and direction" },
            if self.m.gtype == 1 { "RECT" } else { "CURV" },
            self.steps,
            self.files,
            names.len()
        );
        println!("{line}");
    }

    fn rec(&mut self, ifi: usize, ifj: usize, k: usize) -> &mut Vec<Real> {
        self.grd.rec.get_mut(&(ifi, ifj, k)).unwrap_or_else(|| panic!("field ({ifi},{ifj}) record {k} was not read"))
    }

    /// `CALL S2GRID (record(:, col), x [, .TRUE.])`, a column of `NSEA` values of a record.
    fn s2(&mut self, key: Key, col: usize, x: X, dir: bool) {
        let nsea = self.nsea;
        let noval = self.q.noval;
        let s = self.grd.rec.get_mut(&key).unwrap_or_else(|| panic!("field {key:?} was not read"));
        let s = &mut s[col * nsea..(col + 1) * nsea];
        let xg = match x {
            X::X1 => &mut self.x1,
            X::X2 => &mut self.x2,
            X::Xx => &mut self.xx,
            X::Xy => &mut self.xy,
        };
        s2grid(&self.m, noval, s, xg, dir);
    }

    /// `CALL W3S2XY` without `S2GRID`: no `NOVAL`, no direction conversion.
    fn s2_plain(&mut self, key: Key, x: X) {
        let s = self.grd.rec.get(&key).unwrap_or_else(|| panic!("field {key:?} was not read")).clone();
        let m = &self.m;
        let xg = match x {
            X::X1 => &mut self.x1,
            X::X2 => &mut self.x2,
            X::Xx => &mut self.xx,
            X::Xy => &mut self.xy,
        };
        s2xy(m, &s, xg);
    }

    /// `META(I) = GETMETA (IFI, IFJ, ICOMP=I, IPART=IPART)`.
    fn getmeta(&self, ifi: usize, ifj: usize, icomp: usize, ipart: usize) -> Meta {
        assert!(ifi != 10, "user defined output fields (group 10) are not implemented");
        let mut mt = self.meta.get(ifi, ifj)[icomp].clone();
        if ifi == 4 {
            let isn = (ipart + 1).min(if self.m.ptmeth <= 3 { 6 } else { 2 });
            let sn = self.snamep[isn - 1].clone();
            let sub = |s: &mut String, lim: usize| {
                // The first occurrence of each token, in the order of PARTNO_STRING_SUB.
                if let Some(i) = s.find("<IPART>") {
                    *s = format!("{}{}{}", &s[..i], ipart % 10, &s[i + 7..]);
                }
                if let Some(i) = s.find("<SPART>") {
                    *s = format!("{}{}{}", &s[..i], sn, &s[i + 7..]);
                }
                if let Some(i) = s.find("<SPART_>") {
                    *s = format!("{}{}{}", &s[..i], sn.replace(' ', "_"), &s[i + 8..]);
                }
                *s = s.chars().take(lim).collect();
            };
            sub(&mut mt.ename, 50);
            sub(&mut mt.varnm, 80);
            sub(&mut mt.varnl, 80);
            sub(&mut mt.varns, 120);
            sub(&mut mt.varng, 120);
            sub(&mut mt.varnc, 512);
            sub(&mut mt.varnd, 120);
        }
        mt
    }

    /// `WRITE_META`.
    fn write_meta(&self, ncid: i32, varid: i32, mt: &Meta) {
        let has = |s: &str| !s.is_empty() && s != ounfmeta::UNSETC;
        nc::att_text(ncid, varid, "long_name", &mt.varnl);
        if has(&mt.varns) {
            nc::att_text(ncid, varid, "standard_name", &mt.varns);
        }
        if has(&mt.varng) {
            nc::att_text(ncid, varid, "globwave_name", &mt.varng);
        }
        nc::att_text(ncid, varid, "units", &mt.units);
        if self.ncvartype == 2 {
            nc::att_short(ncid, varid, "_FillValue", nc::FILL_SHORT);
        } else {
            nc::att_float(ncid, varid, "_FillValue", nc::FILL_FLOAT);
        }
        nc::att_float(ncid, varid, "scale_factor", mt.fsc);
        nc::att_float(ncid, varid, "add_offset", 0.);
        if self.ncvartype == 2 {
            nc::att_int(ncid, varid, "valid_min", nint(mt.vmin / mt.fsc));
            nc::att_int(ncid, varid, "valid_max", nint(mt.vmax / mt.fsc));
        } else {
            nc::att_float(ncid, varid, "valid_min", mt.vmin);
            nc::att_float(ncid, varid, "valid_max", mt.vmax);
        }
        if has(&mt.varnc) {
            nc::att_text(ncid, varid, "comment", &mt.varnc);
        }
        if has(&mt.varnd) {
            nc::att_text(ncid, varid, "direction_reference", &mt.varnd);
        }
        // CRS_NAME: 'crs' on a rectilinear or curvilinear lat/lon grid (DEFAULT_CRS_META).
        nc::att_text(ncid, varid, "grid_mapping", "crs");
    }

    /// `W3CRNC`.
    fn crnc(&self, name: &str, dimln: &[usize; 7], extradim: usize, dimid: &mut [i32; 8], varid: &mut [i32; 301], epoch: &str) -> i32 {
        let flagll = self.m.flagll;
        let gtype = self.m.gtype;
        let ncid = if self.nctype == 3 { nc::create(name, 0) } else { nc::create(name, nc::NC_NETCDF4) };
        let deflate = |v: i32| {
            if self.nctype == 4 {
                nc::def_var_deflate(ncid, v, 1);
            }
        };
        dimid[1] = nc::def_dim(ncid, "level", dimln[1]);
        if flagll {
            dimid[2] = nc::def_dim(ncid, "longitude", dimln[2]);
            dimid[3] = nc::def_dim(ncid, "latitude", dimln[3]);
        } else {
            dimid[2] = nc::def_dim(ncid, "x", dimln[2]);
            dimid[3] = nc::def_dim(ncid, "y", dimln[3]);
        }
        if extradim == 1 {
            dimid[4] = nc::def_dim(ncid, "f", dimln[4]);
        }
        dimid[4 + extradim] = nc::def_dim(ncid, "time", nc::NC_UNLIMITED);

        let (lonn, latn) = if flagll { ("longitude", "latitude") } else { ("x", "y") };
        if gtype == 1 {
            varid[1] = nc::def_var(ncid, lonn, nc::NC_FLOAT, &[dimid[2]]);
            varid[2] = nc::def_var(ncid, latn, nc::NC_FLOAT, &[dimid[3]]);
        } else {
            varid[1] = nc::def_var(ncid, lonn, nc::NC_FLOAT, &[dimid[3], dimid[2]]);
            varid[2] = nc::def_var(ncid, latn, nc::NC_FLOAT, &[dimid[3], dimid[2]]);
        }
        if flagll {
            nc::att_text(ncid, varid[1], "units", "degree_east");
            nc::att_text(ncid, varid[1], "long_name", "longitude");
            nc::att_text(ncid, varid[1], "standard_name", "longitude");
            nc::att_float(ncid, varid[1], "valid_min", -180.0);
            nc::att_float(ncid, varid[1], "valid_max", 360.);
            nc::att_text(ncid, varid[2], "units", "degree_north");
            nc::att_text(ncid, varid[2], "long_name", "latitude");
            nc::att_text(ncid, varid[2], "standard_name", "latitude");
            nc::att_float(ncid, varid[2], "valid_min", -90.0);
            nc::att_float(ncid, varid[2], "valid_max", 90.);
        } else {
            nc::att_text(ncid, varid[1], "units", "m");
            nc::att_text(ncid, varid[1], "long_name", "x");
            nc::att_text(ncid, varid[2], "units", "m");
            nc::att_text(ncid, varid[2], "long_name", "y");
        }
        nc::att_text(ncid, varid[1], "axis", "X");
        nc::att_text(ncid, varid[2], "axis", "Y");
        deflate(varid[1]);
        deflate(varid[2]);

        if extradim == 1 {
            varid[10] = nc::def_var(ncid, "f", nc::NC_FLOAT, &[dimid[4]]);
            deflate(varid[10]);
            nc::att_text(ncid, varid[10], "long_name", "wave_frequency");
            nc::att_text(ncid, varid[10], "standard_name", "wave_frequency");
            nc::att_text(ncid, varid[10], "units", "s-1");
            nc::att_text(ncid, varid[10], "axis", "Hz");
        }

        varid[3] = nc::def_var(ncid, "time", self.tvartype, &[dimid[4 + extradim]]);
        deflate(varid[3]);
        nc::att_text(ncid, varid[3], "long_name", "time");
        nc::att_text(ncid, varid[3], "standard_name", "time");
        nc::att_text(ncid, varid[3], "units", epoch);
        nc::att_text(ncid, varid[3], "axis", "T");
        nc::att_text(ncid, varid[3], "calendar", "standard");

        if self.q.mapsta {
            varid[20] = nc::def_var(ncid, "MAPSTA", nc::NC_SHORT, &[dimid[3], dimid[2]]);
            deflate(varid[20]);
            nc::att_text(ncid, varid[20], "long_name", "status map");
            nc::att_text(ncid, varid[20], "standard_name", "status map");
            nc::att_text(ncid, varid[20], "units", "1");
            nc::att_int(ncid, varid[20], "valid_min", -32);
            nc::att_int(ncid, varid[20], "valid_max", 32);
        }

        // Global attributes (FL_DEFAULT_GBL_META is true).
        nc::att_text(ncid, nc::NC_GLOBAL, "WAVEWATCH_III_version_number", "7.14");
        nc::att_text(ncid, nc::NC_GLOBAL, "WAVEWATCH_III_switches", SWITCHES);
        // ST4: the namelist parameters that differ from the defaults (REAL comparisons).
        if self.m.zzwnd != 10.0 {
            nc::att_float(ncid, nc::NC_GLOBAL, "SIN4 namelist parameter ZWD", self.m.zzwnd);
        }
        if self.m.aalpha != 0.0095 {
            nc::att_float(ncid, nc::NC_GLOBAL, "SIN4 namelist parameter ALPHA0", self.m.aalpha);
        }
        if self.m.bbeta != 1.43 {
            nc::att_float(ncid, nc::NC_GLOBAL, "SIN4 namelist parameter BETAMAX", self.m.bbeta);
        }
        if self.m.ssdsc[6] != 0.3 {
            nc::att_float(ncid, nc::NC_GLOBAL, "SDS4 namelist parameter WHITECAPWIDTH", self.m.ssdsc[6]);
        }
        nc::att_text(ncid, nc::NC_GLOBAL, "product_name", name);
        nc::att_text(ncid, nc::NC_GLOBAL, "area", &self.m.gname);
        ncid
    }

    /// Defines the variables of one field in the open file, 2.5.4 and 2.6.3.
    fn def_field_vars(&self, ncid: i32, nfield: usize, extradim: usize, dimid: &[i32; 8], varid: &mut [i32; 301], meta: &mut [Meta; 3]) {
        for i in 1..=nfield {
            let ivar = 21 + i;
            let mut dims: Vec<i32> = Vec::new();
            for d in (2..=4 + extradim).rev() {
                dims.push(dimid[d]);
            }
            let xtype = if self.ncvartype == 2 { nc::NC_SHORT } else { nc::NC_FLOAT };
            varid[ivar] = nc::def_var(ncid, &meta[i - 1].varnm, xtype, &dims);
            if self.nctype == 4 {
                nc::def_var_deflate(ncid, varid[ivar], 1);
            }
            if self.ncvartype > 2 {
                meta[i - 1].fsc = 1.0;
            }
            self.write_meta(ncid, varid[ivar], &meta[i - 1]);
        }
    }

    /// `W3EXNC`: one output time of every requested field.
    fn exnc(&mut self) {
        let (nx, ny, nk) = (self.nx, self.ny, self.nk);
        let nx1 = nx + 1;
        self.x1 = vec![UNDEF; nx1 * ny];
        self.x2 = vec![UNDEF; nx1 * ny];
        self.xx = vec![UNDEF; nx1 * ny];
        self.xy = vec![UNDEF; nx1 * ny];
        self.xk = vec![0.; nx1 * ny * nk];
        self.xxk = vec![0.; nx1 * ny * nk];
        self.xyk = vec![0.; nx1 * ny * nk];
        self.ncvartype = self.ncvartypei;

        let epoch_iso = t2iso(self.q.tepoch);
        let epoch = if self.q.timeunit == 'D' { format!("days since {epoch_iso}") } else { format!("seconds since {epoch_iso}") };
        let epochdate = t2d(self.q.tepoch);
        let mfill = nc::FILL_SHORT;
        let mfillr = nc::FILL_FLOAT;
        let time = self.grd.time;

        // 1.2 The date in the file name.
        if self.s3 > 0 && self.s3 < 4 {
            self.s3 = 4;
        }
        if self.s3 > 10 {
            self.s3 = 10;
        }
        let s3 = self.s3;
        assert!(s3 != 9, "FIELD%TIMESPLIT = 9 divides by zero in W3EXNC");
        let mut s4 = s3;
        self.oldtimeid = self.timeid.clone();
        let rdiv = |a: i32, p: i32| -> i32 { ((a as Real) / (nint((10.0f32).powi(p)) as Real)).floor() as i32 };
        let timeid = if s3 == 0 {
            s4 = 5;
            "field".to_string()
        } else if s3 == 10 {
            s4 += 1;
            let s5 = s3 - 8;
            format!("{:08}T{:0w$}", time[0], rdiv(time[1], 6 - s5), w = s5 as usize)
        } else if s3 == 8 {
            format!("{:08}", time[0])
        } else {
            format!("{:0w$}", rdiv(time[0], 8 - s3), w = s3 as usize)
        };
        self.timeid = pad16(&timeid);
        let s4 = s4 as usize;
        let prefix = self.q.prefix.trim_end().to_string();
        if SINGLEFILE && self.firsttime.starts_with("0000000000000000") {
            self.firsttime = self.timeid.clone();
        }
        let stamp = if SINGLEFILE { self.firsttime.clone() } else { self.timeid.clone() };

        let mut i1 = 0;
        let mut j1 = 0;
        // NCID is one variable for the whole call: a later field sees the file an earlier one opened.
        let mut ncid = 0;
        // 2. Loop over output fields.
        for ifi in 1..=NOGRP {
            for ifj in 1..=NGRPP {
                if !self.flg2d[(ifi - 1) + (ifj - 1) * NOGRP] {
                    continue;
                }
                let mut indexipart = 0usize;
                let mut ipart = self.tabipart[0];
                loop {
                    if i1 == 0 {
                        i1 = ifi;
                    }
                    if j1 == 0 {
                        j1 = ifj;
                    }
                    // 2.1 Set output arrays and parameters.
                    let (nfield, flfrq, i1f, i2f) = self.extract(ifi, ifj, ipart);
                    let mut meta: [Meta; 3] = [Meta::default(), Meta::default(), Meta::default()];
                    for i in 1..=nfield {
                        meta[i - 1] = self.getmeta(ifi, ifj, i - 1, ipart);
                    }

                    // 2.2 Make map.
                    let mut mapout = vec![0i16; nx * ny];
                    for ix in 0..nx {
                        for iy in 0..ny {
                            let c = iy + ix * ny;
                            mapout[ix + iy * nx] = (self.grd.mapsta[c] + 8 * self.grd.mapst2[c]) as i16;
                            if self.grd.mapsta[c] == 0 {
                                let g = ix + iy * nx1;
                                self.x1[g] = UNDEF;
                                self.x2[g] = UNDEF;
                                self.xx[g] = UNDEF;
                                self.xy[g] = UNDEF;
                            }
                        }
                    }

                    // 2.3 The file name: the variable name is left out when all fields share one file.
                    let s2 = meta[0].ename.trim_end().len();
                    let together = self.q.together;
                    let name_part: String = if together && !flfrq { String::new() } else { meta[0].ename.chars().skip(1).take(s2.saturating_sub(1)).collect() };
                    let base = format!("{}{}", prefix, &stamp[..s4.min(16)]);
                    let fnamenc_full = if name_part.is_empty() { format!("{base}.nc") } else { format!("{base}_{name_part}.nc") };
                    let extradim = usize::from(flfrq);
                    let mut dimln = [0usize; 7];
                    dimln[4] = if flfrq { i2f - i1f + 1 } else { 0 };
                    dimln[2] = self.ixn - self.ix1 + 1;
                    dimln[3] = self.iyn - self.iy1 + 1;

                    // 2.4.1 Save the id of the previous file.
                    let oldncid = if together && !flfrq { self.ncids.get(&(1, 1, 1)).copied().unwrap_or(0) } else { self.ncids.get(&(ifi, ifj, ipart + 1)).copied().unwrap_or(0) };

                    // 2.4.2 Remove the new file (if not created by the run).
                    let mut fexist = std::path::Path::new(&fnamenc_full).exists();
                    if fexist {
                        let mut fremove = false;
                        if !SINGLEFILE && fortran_index(&self.timeid, &self.oldtimeid) == 0 {
                            if together && !flfrq && ncid == 0 {
                                fremove = true;
                            }
                            if !together || flfrq {
                                fremove = true;
                            }
                        }
                        if fremove {
                            let _ = std::fs::remove_file(&fnamenc_full);
                            fexist = false;
                        } else {
                            ncid = oldncid;
                        }
                    }

                    // 2.4.3 Finalize the previous file (if a new one will be created).
                    if !fexist
                        && fortran_index("0000000000000000", &self.oldtimeid) == 0
                        && fortran_index(&self.timeid, &self.oldtimeid) == 0
                    {
                        nc::redef(oldncid);
                        nc::att_text(oldncid, nc::NC_GLOBAL, "stop_date", &self.strstopdate);
                        nc::close(oldncid);
                    }

                    let mut dimid = [0i32; 8];
                    let mut varid = [0i32; 301];
                    let mut n: usize;
                    let ivar1 = 21;
                    let _ = ivar1;
                    // 2.5 Creates the netcdf file.
                    if !fexist {
                        dimln[1] = 1;
                        n = 1;
                        ncid = self.crnc(&fnamenc_full, &dimln, extradim, &mut dimid, &mut varid, &epoch);
                        if together && !flfrq {
                            self.ncids.insert((1, 1, 1), ncid);
                        } else {
                            self.ncids.insert((ifi, ifj, ipart + 1), ncid);
                        }
                        self.files += 1;
                        println!("NEW NETCDF FILE WAS CREATED {fnamenc_full:<128}");

                        // 2.5.2 Lat-lon arrays and the global attributes that follow them.
                        let (lon2d, lat2d, lon, lat);
                        if self.m.gtype == 2 {
                            nc::att_text(ncid, nc::NC_GLOBAL, "latitude_resolution", "n/a");
                            nc::att_text(ncid, nc::NC_GLOBAL, "longitude_resolution", "n/a");
                            // LON2D = TRANSPOSE(XGRD): (NX,NY) from XGRD(NY,NX).
                            let tr = |a: &Vec<Real>| -> Vec<Real> {
                                let mut o = vec![0.; nx * ny];
                                for ix in 0..nx {
                                    for iy in 0..ny {
                                        o[ix + iy * nx] = a[iy + ix * ny];
                                    }
                                }
                                o
                            };
                            lon2d = tr(&self.m.xgrd);
                            lat2d = tr(&self.m.ygrd);
                            lon = Vec::new();
                            lat = Vec::new();
                        } else {
                            let sxd = 0.000001f64 * (1e6 * f64::from(self.m.sx)).round_ties_away();
                            let syd = 0.000001f64 * (1e6 * f64::from(self.m.sy)).round_ties_away();
                            let x0d = 0.000001f64 * (1e6 * f64::from(self.m.x0)).round_ties_away();
                            let y0d = 0.000001f64 * (1e6 * f64::from(self.m.y0)).round_ties_away();
                            lon = (0..nx).map(|i| (x0d + sxd * i as f64) as Real).collect::<Vec<Real>>();
                            lat = (0..ny).map(|i| (y0d + syd * i as f64) as Real).collect::<Vec<Real>>();
                            nc::att_text(ncid, nc::NC_GLOBAL, "latitude_resolution", &f12_0(self.m.sy));
                            nc::att_text(ncid, nc::NC_GLOBAL, "longitude_resolution", &f12_0(self.m.sx));
                            lon2d = Vec::new();
                            lat2d = Vec::new();
                        }
                        let minmax = |a: &[Real]| -> (Real, Real) { (a.iter().copied().fold(Real::INFINITY, Real::min), a.iter().copied().fold(Real::NEG_INFINITY, Real::max)) };
                        let (ymin, ymax) = minmax(&self.m.ygrd);
                        let (xmin, xmax) = minmax(&self.m.xgrd);
                        nc::att_text(ncid, nc::NC_GLOBAL, "southernmost_latitude", &f12_0(ymin));
                        nc::att_text(ncid, nc::NC_GLOBAL, "northernmost_latitude", &f12_0(ymax));
                        nc::att_text(ncid, nc::NC_GLOBAL, "westernmost_longitude", &f12_0(xmin));
                        nc::att_text(ncid, nc::NC_GLOBAL, "easternmost_longitude", &f12_0(xmax));
                        nc::att_text(ncid, nc::NC_GLOBAL, "minimum_altitude", "-12000 m");
                        nc::att_text(ncid, nc::NC_GLOBAL, "maximum_altitude", "9000 m");
                        nc::att_text(ncid, nc::NC_GLOBAL, "altitude_resolution", "n/a");

                        let sd = t2d(time);
                        let strstartdate = format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", sd[0], sd[1], sd[2], sd[4], sd[5], sd[6]);
                        nc::enddef(ncid);

                        // 2.5.3 Writes longitudes, latitudes, frequencies and the status map.
                        let (cx, cy) = (self.ixn - self.ix1 + 1, self.iyn - self.iy1 + 1);
                        if self.m.gtype == 1 {
                            nc::put_f32(ncid, varid[1], &[0], &[cx], &lon[self.ix1 - 1..self.ixn]);
                            nc::put_f32(ncid, varid[2], &[0], &[cy], &lat[self.iy1 - 1..self.iyn]);
                        } else {
                            let sub = |a: &Vec<Real>| -> Vec<Real> {
                                let mut o = Vec::with_capacity(cx * cy);
                                for iy in self.iy1 - 1..self.iyn {
                                    for ix in self.ix1 - 1..self.ixn {
                                        o.push(a[ix + iy * nx]);
                                    }
                                }
                                o
                            };
                            nc::put_f32(ncid, varid[1], &[0, 0], &[cy, cx], &sub(&lon2d));
                            nc::put_f32(ncid, varid[2], &[0, 0], &[cy, cx], &sub(&lat2d));
                        }
                        if extradim == 1 {
                            let freq: Vec<Real> = if self.customfrq {
                                (0..self.m.usspf[1] as usize).map(|i| (GRAV * self.m.ussp_wn[i]).sqrt() * TPIINV).collect()
                            } else {
                                (1..=i2f - i1f + 1).map(|i| self.m.sig[i1f - 1 + i] * TPIINV).collect()
                            };
                            nc::put_f32(ncid, varid[10], &[0], &[freq.len()], &freq);
                        }
                        if self.q.mapsta {
                            let mut d = Vec::with_capacity(cx * cy);
                            for iy in self.iy1 - 1..self.iyn {
                                for ix in self.ix1 - 1..self.ixn {
                                    d.push(mapout[ix + iy * nx]);
                                }
                            }
                            nc::put_i16(ncid, varid[20], &[0, 0], &[cy, cx], &d);
                        }

                        // 2.5.4 Defines the fields of the variable.
                        nc::redef(ncid);
                        self.def_field_vars(ncid, nfield, extradim, &dimid, &mut varid, &mut meta);
                        nc::att_text(ncid, nc::NC_GLOBAL, "start_date", &strstartdate);
                        nc::enddef(ncid);
                    } else {
                        // 2.6 Append data to the existing file.
                        // 2.6.1 Get the dimensions from the netcdf header.
                        dimid[2] = nc::inq_dimid(ncid, if self.m.flagll { "longitude" } else { "x" });
                        dimid[3] = nc::inq_dimid(ncid, if self.m.flagll { "latitude" } else { "y" });
                        varid[1] = nc::inq_varid(ncid, if self.m.flagll { "longitude" } else { "x" });
                        varid[2] = nc::inq_varid(ncid, if self.m.flagll { "latitude" } else { "y" });
                        dimid[4 + extradim] = nc::inq_dimid(ncid, "time");
                        n = nc::inq_dimlen(ncid, dimid[4 + extradim]);
                        varid[3] = nc::inq_varid(ncid, "time");
                        if extradim == 1 {
                            dimid[4] = nc::inq_dimid(ncid, "f");
                        }
                        // 2.6.2 Increments the time step for existing file.
                        if (together && ifi == i1 && ifj == j1 && ipart == self.tabipart[0]) || !together || flfrq {
                            n += 1;
                        }
                        // 2.6.3 Defines or gets the variables identifiers.
                        nc::redef(ncid);
                        if n == 1 {
                            self.def_field_vars(ncid, nfield, extradim, &dimid, &mut varid, &mut meta);
                        } else {
                            for i in 1..=nfield {
                                varid[21 + i] = nc::inq_varid(ncid, &meta[i - 1].varnm);
                            }
                        }
                        nc::enddef(ncid);
                    }

                    // 2.6.4 Defines the current time step and index.
                    let curdate = t2d(time);
                    println!(
                        "Writing new record {:<9}number {:6} for {:4}:{:02}:{:02}T{:02}:{:02}:{:02} in file {}",
                        meta[0].ename.chars().skip(1).take(9).collect::<String>(),
                        n,
                        curdate[0],
                        curdate[1],
                        curdate[2],
                        curdate[4],
                        curdate[5],
                        curdate[6],
                        fnamenc_full
                    );

                    // Puts time in the NetCDF file.
                    if (ifi == i1 && ifj == j1 && self.q.together) || !self.q.together || flfrq {
                        if self.q.timeunit == 'S' {
                            let outsecs = tsubsec(&epochdate, &curdate);
                            nc::put_i64(ncid, varid[3], &[n - 1], &[1], &[outsecs]);
                        } else {
                            let outjulday = tsub(&epochdate, &curdate);
                            nc::put_f64(ncid, varid[3], &[n - 1], &[1], &[outjulday]);
                        }
                    }

                    // 2.6.5 Puts field(s) in NetCDF file.
                    self.put_fields(ncid, &varid, &meta, nfield, extradim, i1f, i2f, n, mfill, mfillr);

                    // Loops over IPART for partition variables.
                    if ifi == 4 && ifj + 8 <= NOGE[ifi - 1] && indexipart + 1 < self.tabipart.len() {
                        indexipart += 1;
                        ipart = self.tabipart[indexipart];
                        continue;
                    }
                    break;
                }
            }
        }
    }

    /// The `MAPSTA` test of the packers: `MAPSTA(IY,IX) .LE. 0 .OR. value .EQ. UNDEF`.
    fn masked(&self, ix: usize, iy: usize, v: Real) -> bool {
        self.grd.mapsta[iy + ix * self.ny] <= 0 || v == UNDEF
    }

    /// 2.6.5: the packers of `W3EXNC` and the `NF90_PUT_VAR` calls. A cell is a fill value where `MAPSTA` is not
    /// positive or the first array of the group holds `UNDEF`; a short is `NINT(value/FSC)` kept to 16 bits.
    #[allow(clippy::too_many_arguments)]
    fn put_fields(&self, ncid: i32, varid: &[i32; 301], meta: &[Meta; 3], nfield: usize, extradim: usize, i1f: usize, i2f: usize, n: usize, mfill: i16, mfillr: Real) {
        let nx1 = self.nx + 1;
        let (cx, cy) = (self.ixn - self.ix1 + 1, self.iyn - self.iy1 + 1);
        let sz = nx1 * self.ny;
        // The array of component `i` (0-based): X1 X2 XY for three, XX XY for two, X1 for one; the plane of
        // XXK, XYK or XK when the field has a frequency dimension.
        let src = |i: usize, ik: usize| -> &[Real] {
            if extradim == 0 {
                match (nfield, i) {
                    (3, 0) => &self.x1,
                    (3, 1) => &self.x2,
                    (3, _) => &self.xy,
                    (2, 0) => &self.xx,
                    (2, _) => &self.xy,
                    _ => &self.x1,
                }
            } else {
                let base = match (nfield, i) {
                    (2, 0) => &self.xxk,
                    (2, _) => &self.xyk,
                    _ => &self.xk,
                };
                &base[(ik - 1) * sz..ik * sz]
            }
        };
        let iks: Vec<usize> = if extradim == 0 { vec![0] } else { (i1f..=i2f).collect() };
        for (kk, &ik) in iks.iter().enumerate() {
            let probe = src(0, ik);
            for i in 0..nfield {
                let g = src(i, ik);
                let (start, count): (Vec<usize>, Vec<usize>) =
                    if extradim == 0 { (vec![n - 1, 0, 0], vec![1, cy, cx]) } else { (vec![n - 1, kk, 0, 0], vec![1, 1, cy, cx]) };
                let cells = (self.iy1 - 1..self.iyn).flat_map(|iy| (self.ix1 - 1..self.ixn).map(move |ix| (ix, iy)));
                if self.ncvartype == 2 {
                    let d: Vec<i16> = cells
                        .map(|(ix, iy)| {
                            let cell = ix + iy * nx1;
                            if self.masked(ix, iy, probe[cell]) { mfill } else { nint(g[cell] / meta[i].fsc) as i16 }
                        })
                        .collect();
                    nc::put_i16(ncid, varid[22 + i], &start, &count, &d);
                } else {
                    let d: Vec<Real> = cells
                        .map(|(ix, iy)| {
                            let cell = ix + iy * nx1;
                            if self.masked(ix, iy, probe[cell]) { mfillr } else { g[cell] }
                        })
                        .collect();
                    nc::put_f32(ncid, varid[22 + i], &start, &count, &d);
                }
            }
        }
    }

    /// 2.1: fills the grids for one field. Returns `(NFIELD, FLFRQ, I1F, I2F)`.
    fn extract(&mut self, ifi: usize, ifj: usize, ipart: usize) -> (usize, bool, usize, usize) {
        let nsea = self.nsea;
        let nk = self.nk;
        let nx1 = self.nx + 1;
        let vector = self.q.vector;
        let ncvi = self.ncvartypei;
        let mut nfield = 1;
        let mut flfrq = false;
        let (mut i1f, mut i2f) = (0usize, 0usize);
        self.ncvartype = if ncvi == 3 { 2 } else { self.ncvartype };
        // `IF (NCVARTYPEI.EQ.3) NCVARTYPE=2` opens every field; the cases below that say 4 follow it.
        let k0 = |j: usize| (ifi, j, 0usize);
        match (ifi, ifj) {
            (1, 1) => self.s2(k0(1), 0, X::X1, false),
            (1, 2) => {
                if !vector {
                    let mut u = self.rec(1, 2, 0).clone();
                    let mut v = self.rec(1, 2, 1).clone();
                    uv_to_mag_dir(&mut u, &mut v, 0.05, 'O');
                    *self.rec(1, 2, 0) = u;
                    *self.rec(1, 2, 1) = v;
                }
                self.s2((1, 2, 0), 0, X::Xx, false);
                self.s2((1, 2, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (1, 3) => {
                if !vector {
                    let mut u = self.rec(1, 3, 0).clone();
                    let mut v = self.rec(1, 3, 1).clone();
                    uv_to_mag_dir(&mut u, &mut v, 1.0, 'N');
                    *self.rec(1, 3, 0) = u;
                    *self.rec(1, 3, 1) = v;
                }
                self.s2((1, 3, 0), 0, X::Xx, false);
                self.s2((1, 3, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (1, 4..=6) => self.s2(k0(ifj), 0, X::X1, false),
            (1, 7) => {
                self.s2(k0(7), 0, X::X1, false);
                for v in self.x1.iter_mut() {
                    if *v != UNDEF {
                        *v *= 0.1;
                    }
                }
            }
            (1, 8) => {
                self.s2_plain((1, 8, 0), X::Xx);
                self.s2_plain((1, 8, 1), X::Xy);
                nfield = 2;
            }
            (1, 9) => self.s2_plain(k0(9), X::X1),
            (2, 18) => {
                let fp: Vec<Real> = self.rec(2, 6, 0).iter().map(|&f| if f != UNDEF { 1.0 / f } else { UNDEF }).collect();
                let noval = self.q.noval;
                let mut aux = fp;
                s2grid(&self.m, noval, &mut aux, &mut self.x1, false);
            }
            (2, 19) => self.s2_plain(k0(19), X::X1),
            (2, 7 | 9) => self.s2(k0(ifj), 0, X::X1, true),
            (2, j) if j <= 17 => self.s2(k0(j), 0, X::X1, false),
            (3, 1..=5) => {
                flfrq = true;
                i1f = self.m.e3df[(ifj - 1) * 3 + 1] as usize;
                i2f = self.m.e3df[(ifj - 1) * 3 + 2] as usize;
                for ik in i1f..=i2f {
                    self.s2((3, ifj, 0), ik - i1f, X::Xx, false);
                    if ifj == 1 && self.ncvartype == 2 {
                        for v in self.xx.iter_mut() {
                            if *v >= 0. {
                                *v = (*v + 1e-12).log10();
                            }
                        }
                    }
                    self.copy_plane(0, ik);
                }
            }
            (3, 6) => {
                flfrq = true;
                i1f = 1;
                i2f = nk;
                for ik in 1..=nk {
                    // WN is (0:NK+1,0:NSEA) and the file fills (1:NK,1:NSEA). `WN(IK,:)` is NSEA+1 values from
                    // index 0, and W3S2XY takes the first NSEA of them, so sea point J gets WN(IK,J-1): the
                    // field is shifted by one point, and point 1 gets WN(IK,0), which nothing ever sets.
                    // ⚠ That element is taken as 0, the zero page of a fresh allocation.
                    let noval = self.q.noval;
                    let mut row: Vec<Real> = std::iter::once(0.).chain((0..nsea).map(|i| self.rec(3, 6, 0)[ik - 1 + i * nk])).collect();
                    s2grid(&self.m, noval, &mut row, &mut self.xx, false);
                    for i in 0..nsea {
                        self.rec(3, 6, 0)[ik - 1 + i * nk] = row[i + 1];
                    }
                    self.copy_plane(0, ik);
                }
            }
            (4, 1..=15) => self.s2(k0(ifj), ipart, X::X1, ifj == 4 || ifj == 7),
            (4, 16..=23) => self.s2(k0(ifj), 0, X::X1, false),
            (5, 1) => {
                let (n0, n1) = (self.rec(5, 1, 0).clone(), self.rec(5, 1, 1).clone());
                let (mut u, mut v) = (n0, n1);
                for i in 0..nsea {
                    let uabs = (u[i] * u[i] + v[i] * v[i]).sqrt();
                    if uabs >= 10. {
                        u[i] = UNDEF;
                        v[i] = UNDEF;
                    }
                }
                *self.rec(5, 1, 0) = u;
                *self.rec(5, 1, 1) = v;
                self.s2((5, 1, 0), 0, X::Xx, false);
                self.s2((5, 1, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (5, 2) => self.s2(k0(2), 0, X::X1, false),
            (5, 3) => {
                for v in self.rec(5, 3, 0).iter_mut() {
                    *v *= 0.001;
                }
                self.s2(k0(3), 0, X::X1, false);
            }
            (5, 4) => {
                if ncvi == 3 {
                    self.ncvartype = 4;
                }
                self.s2(k0(4), 0, X::X1, false);
            }
            (5, 5 | 6) => {
                self.s2((5, ifj, 0), 0, X::Xx, false);
                self.s2((5, ifj, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (5, 7..=11) => self.s2(k0(ifj), 0, X::X1, false),
            (6, 1) => {
                self.s2((6, 1, 0), 0, X::X1, false);
                self.s2((6, 1, 1), 0, X::X2, false);
                self.s2((6, 1, 2), 0, X::Xy, false);
                nfield = 3;
            }
            (6, 2 | 5 | 10) => {
                self.s2((6, ifj, 0), 0, X::Xx, false);
                self.s2((6, ifj, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (6, 3) => self.s2(k0(3), 0, X::X1, false),
            (6, 4) => {
                if ncvi == 3 {
                    self.ncvartype = 4;
                }
                for v in self.rec(6, 4, 0).iter_mut() {
                    *v = v.min(3000.);
                }
                self.s2(k0(4), 0, X::X1, false);
            }
            (6, 6) => {
                for k in 0..2 {
                    for v in self.rec(6, 6, k).iter_mut() {
                        *v = (-0.9998f32).max(v.min(0.9998));
                    }
                }
                self.s2((6, 6, 0), 0, X::Xx, false);
                self.s2((6, 6, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (6, 7) => {
                nfield = 2;
                self.s2((6, 7, 0), 0, X::Xx, false);
                self.s2((6, 7, 1), 0, X::Xy, false);
            }
            (6, 8 | 12) => {
                flfrq = true;
                nfield = 2;
                if ifj == 8 {
                    i1f = self.m.us3df[1] as usize;
                    i2f = self.m.us3df[2] as usize;
                } else {
                    if self.m.usspf[0] == 1 {
                        self.customfrq = true;
                    }
                    i1f = 1;
                    i2f = self.m.usspf[1] as usize;
                }
                for ik in i1f..=i2f {
                    self.s2((6, ifj, 0), ik - i1f, X::Xx, false);
                    self.s2((6, ifj, 1), ik - i1f, X::Xy, false);
                    self.copy_plane(1, ik);
                    self.copy_plane(2, ik);
                }
            }
            (6, 9) => {
                flfrq = true;
                i1f = self.m.p2msf[1] as usize;
                i2f = self.m.p2msf[2] as usize;
                let kk = DWAT * GRAV;
                let k2 = kk * kk;
                for ik in i1f..=i2f {
                    self.s2((6, 9, 0), ik - i1f, X::Xx, false);
                    let ncv = self.ncvartype;
                    for v in self.xx.iter_mut() {
                        if *v >= 0. {
                            *v = if ncv == 2 { (*v * k2 + 1e-12).log10() } else { *v * k2 };
                        }
                    }
                    self.copy_plane(0, ik);
                }
            }
            (6, 11) => {
                if ncvi == 3 {
                    self.ncvartype = 4;
                }
                self.s2(k0(11), 0, X::X1, false);
            }
            (6, 13) => {
                self.s2_plain((6, 13, 0), X::Xx);
                self.s2_plain((6, 13, 1), X::Xy);
                nfield = 2;
            }
            (7, 1 | 2 | 5) => {
                self.s2((7, ifj, 0), 0, X::Xx, false);
                self.s2((7, ifj, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (7, 3) => {
                self.s2((7, 3, 0), 0, X::X1, false);
                self.s2((7, 3, 1), 0, X::X2, false);
                self.s2((7, 3, 2), 0, X::Xy, false);
                nfield = 3;
            }
            (7, 4) => self.s2(k0(4), 0, X::X1, false),
            (8, 1 | 2) => {
                self.s2((8, ifj, 0), 0, X::Xx, false);
                self.s2((8, ifj, 1), 0, X::Xy, false);
                nfield = 2;
            }
            (8, 3 | 4) => {
                for v in self.rec(8, ifj, 0).iter_mut() {
                    if *v != UNDEF {
                        *v = (630. - RADE * *v) % 180.;
                    }
                }
                self.s2(k0(ifj), 0, X::X1, false);
            }
            (8, 5) => self.s2(k0(5), 0, X::X1, false),
            (9, 1) => {
                for v in self.rec(9, 1, 0).iter_mut() {
                    if *v != UNDEF {
                        *v /= 60.;
                    }
                }
                self.s2(k0(1), 0, X::X1, false);
            }
            (9, 2..=5) => self.s2(k0(ifj), 0, X::X1, false),
            _ => panic!("output field ({ifi},{ifj}) is not implemented"),
        }
        let _ = nx1;
        (nfield, flfrq, i1f, i2f)
    }

    /// `XK(:,:,IK) = XX` (`which` 0), `XXK(:,:,IK) = XX` (1), `XYK(:,:,IK) = XY` (2).
    fn copy_plane(&mut self, which: usize, ik: usize) {
        let sz = (self.nx + 1) * self.ny;
        let (src, dst) = match which {
            0 => (&self.xx, &mut self.xk),
            1 => (&self.xx, &mut self.xxk),
            _ => (&self.xy, &mut self.xyk),
        };
        dst[(ik - 1) * sz..ik * sz].copy_from_slice(src);
    }
}

fn report_fields(flg2d: &[bool]) {
    let mut first = true;
    for ifi in 1..=NOGRP {
        for ifj in 1..=NGRPP {
            if flg2d[(ifi - 1) + (ifj - 1) * NOGRP] {
                if first {
                    println!("            Fields   : {:<20}", idout(ifi, ifj));
                    first = false;
                } else {
                    println!("                       {:<20}", idout(ifi, ifj));
                }
            }
        }
    }
    if first {
        println!("            Fields   : no fields defined");
    }
}

/// `INIT_META`: the context `DEFAULT_META` reads.
fn meta_ctx(m: &ModDef, q: &Request) -> Ctx {
    let partcom = match m.ptmeth {
        1 => "Wind sea and swells defined using topographic partitions and partition wave-age cut-off (WWIII default scheme)".to_string(),
        2 => "Wind sea and swells defined using topographic partitions and spectral wave-age cut-off".to_string(),
        3 => "Wave components defined using topographic partitions only".to_string(),
        4 => "Wind sea and swell defined using spectral wave-age cut-off".to_string(),
        5 => format!("Wave components defined using {:5.3}Hz spectral frequency cutoff", f64::from(m.ptfcut)),
        k => format!("PTM_{}_Unknown", k % 10),
    };
    Ctx {
        vector: q.vector,
        ncvartype: q.ncvartype,
        dircom: String::new(),
        partcom,
        f1: m.sig[1] * TPIINV,
        fnk: m.sig[m.nk as usize] * TPIINV,
    }
}

/// The `WAVEWATCH_III_switches` attribute: the switch file of `switch_chaceon_shrd`.
const SWITCHES: &str = "NOGRB NC4 SHRD PR3 UQ ST4 STAB0 FLX0 LN1 NL1 IC0 IS0 REF0 BT1 DB1 MLIM TR0 BS0 WNX1 WNT1 CRX1 CRT1 O0 O1 O2 O4 O5 O6 O7";

trait RoundAway {
    fn round_ties_away(self) -> f64;
}

impl RoundAway for f64 {
    /// `DNINT`.
    fn round_ties_away(self) -> f64 {
        self.round()
    }
}
