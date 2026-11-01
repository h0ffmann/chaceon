// SPDX-License-Identifier: MPL-2.0
//! What `ww3_ounf` (`model/src/ww3_ounf.F90`) reads before it writes a NetCDF byte: `ww3_ounf.nml`, the field
//! names (`W3FLDTOIJ`, `model/src/w3iogomd.F90:759-1189`), `out_grd.ww3` (`W3IOGO`, the `READ` branch,
//! `:2654-4016`) and the calendar helpers `T2D`, `TSUB`, `TSUBSEC` (`model/src/w3timemd.F90`). Nothing here
//! touches NetCDF. What it does not implement stops the run with a message that names it.

use crate::gridio::{self, Group};
use crate::iogo::UNDEF;
use crate::moddef::{ModDef, Records};
use crate::wtime::Time;
use std::collections::HashMap;
use ww3_kernels::real::Real;

pub const NOGRP: usize = 10;
pub const NGRPP: usize = 25;

/// `NOGE`: the elements of each group, for `switch_chaceon_shrd` (`model/src/w3odatmd.F90:740-840`).
pub const NOGE: [usize; NOGRP] = [9, 19, 6, 23, 11, 13, 5, 5, 5, 1];

/// `W3FLDTOIJ` for `switch_chaceon_shrd`: `D50` (BT4), `IC1` and `IC5` (IS2), `AHS` (OASACM), `OHS` and `THM`
/// (OASOCM), `ACHA` and `OCHA` are not names of this build. `None` is `I = -1`: an unknown name (a warning is
/// written, to unit 6 as `NDSE` is) or `UNSET`; `USF` and `USP` need `US3DF(1)` and `USSPF(1)` of the grid.
pub fn fld_to_ij(fld: &str, m: &ModDef) -> Option<(usize, usize)> {
    let key: String = fld.chars().take(6).collect();
    let key = key.trim_end();
    let e = match key {
        "DPT" => (1, 1),
        "CUR" => (1, 2),
        "WND" => (1, 3),
        "AST" => (1, 4),
        "WLV" => (1, 5),
        "ICE" => (1, 6),
        "IBG" => (1, 7),
        "TAU" => (1, 8),
        "RHO" => (1, 9),
        "HS" => (2, 1),
        "LM" => (2, 2),
        "T02" => (2, 3),
        "T0M1" => (2, 4),
        "T01" => (2, 5),
        "FP" => (2, 6),
        "DIR" => (2, 7),
        "SPR" => (2, 8),
        "DP" => (2, 9),
        "HIG" => (2, 10),
        "MXE" => (2, 11),
        "MXES" => (2, 12),
        "MXH" => (2, 13),
        "MXHC" => (2, 14),
        "SDMH" => (2, 15),
        "SDMHC" => (2, 16),
        "WBT" => (2, 17),
        "TP" => (2, 18),
        "WNM" => (2, 19),
        "EF" => (3, 1),
        "TH1M" => (3, 2),
        "STH1M" => (3, 3),
        "TH2M" => (3, 4),
        "STH2M" => (3, 5),
        "WN" => (3, 6),
        "PHS" => (4, 1),
        "PTP" => (4, 2),
        "PLP" => (4, 3),
        "PDIR" => (4, 4),
        "PSPR" => (4, 5),
        "PWS" => (4, 6),
        "PDP" => (4, 7),
        "PQP" => (4, 8),
        "PPE" => (4, 9),
        "PGW" => (4, 10),
        "PSW" => (4, 11),
        "PTM10" => (4, 12),
        "PT01" => (4, 13),
        "PT02" => (4, 14),
        "PEP" => (4, 15),
        "TWS" => (4, 16),
        "PNR" => (4, 17),
        "SEA8HS" => (4, 18),
        "SW8HS" => (4, 19),
        "SEA8TP" => (4, 20),
        "SW8TP" => (4, 21),
        "SEA8DP" => (4, 22),
        "SW8DP" => (4, 23),
        "UST" => (5, 1),
        "CHA" => (5, 2),
        "CGE" => (5, 3),
        "FAW" => (5, 4),
        "TAW" => (5, 5),
        "TWA" => (5, 6),
        "WCC" => (5, 7),
        "WCF" => (5, 8),
        "WCH" => (5, 9),
        "WCM" => (5, 10),
        "FWS" => (5, 11),
        "SXY" => (6, 1),
        "TWO" => (6, 2),
        "BHD" => (6, 3),
        "FOC" => (6, 4),
        "TUS" => (6, 5),
        "USS" => (6, 6),
        "P2S" => (6, 7),
        "USF" if m.us3df[0] >= 1 => (6, 8),
        "USF" => {
            println!("\n *** WAVEWATCH III WARNING  : \n     PARAMETER USF not allowed: need to set parameter US3D in OUTS namelist (in ww3_grid.inp)");
            return None;
        }
        "P2L" => (6, 9),
        "TWI" => (6, 10),
        "FIC" => (6, 11),
        "USP" if m.usspf[0] >= 1 => (6, 12),
        "USP" => {
            println!("\n *** WAVEWATCH III WARNING  : \n     PARAMETER USP not allowed: need to set parameter USSP in OUTS namelist (in ww3_grid.inp)");
            return None;
        }
        "TOC" => (6, 13),
        "ABR" => (7, 1),
        "UBR" => (7, 2),
        "BED" => (7, 3),
        "FBB" => (7, 4),
        "TBB" => (7, 5),
        "MSS" => (8, 1),
        "MSC" => (8, 2),
        "MSD" => (8, 3),
        "MCD" => (8, 4),
        "QP" => (8, 5),
        "DTD" => (9, 1),
        "FC" => (9, 2),
        "CFX" => (9, 3),
        "CFD" => (9, 4),
        "CFK" => (9, 5),
        "U1" | "U2" => (10, 1),
        "UNSET" => return None,
        _ => {
            println!("\n *** WAVEWATCH III WARNING  : \n     REQUESTED OUTPUT FIELD {} WAS NOT RECOGNIZED.\n", fld.trim_end());
            return None;
        }
    };
    Some(e)
}

/// `IDOUT` (`model/src/w3odatmd.F90`), the 20-character name `ww3_ounf` prints.
pub fn idout(i: usize, j: usize) -> &'static str {
    const G1: [&str; 9] = [
        "Water depth", "Current vel.", "Wind speed", "Air-sea temp. dif.", "Water level", "Ice concentration",
        "Iceberg damp coeffic", "Atmospheric momentum", "Air density",
    ];
    const G2: [&str; 19] = [
        "Wave height", "Mean wave length", "Mean wave period(+2)", "Mean wave period(-1)", "Mean wave period(+1)",
        "Peak frequency", "Mean wave dir. a1b1", "Mean dir. spr. a1b1", "Peak direction", "Infragravity height",
        "Space-Time Max E", "Space-Time Max Std", "Space-Time Hmax", "Spc-Time Hmax^crest", "STD Space-Time Hmax",
        "STD ST Hmax^crest", "Dominant wave bT", "Peak prd. (from fp)", "Mean wave number",
    ];
    const G3: [&str; 6] = [
        "1D Freq. Spectrum", "Mean wave dir. a1b1", "Mean dir. spr. a1b1", "Mean wave dir. a2b2", "Mean dir. spr. a2b2",
        "Wavenumber array",
    ];
    const G4: [&str; 23] = [
        "Part. wave height", "Part. peak period", "Part. peak wave len.", "Part. mean direction", "Part. dir. spread",
        "Part. wind sea frac.", "Part. peak direction", "Part. peakedness", "Part. peak enh. fac.",
        "Part. gaussian width", "Part. spectral width", "Part. mean per. (-1)", "Part. mean per. (+1)",
        "Part. mean per. (+2)", "Part. peak density", "Total wind sea frac.", "Number of partitions",
        "8s sea wave height", "8s swell wave height", "8s sea peak period", "8s swell peak period", "8s sea peak dir",
        "8s swell peak dir",
    ];
    const G5: [&str; 11] = [
        "Friction velocity", "Charnock parameter", "Energy flux", "Wind-wave enrgy flux", "Wind-wave net mom. f",
        "Wind-wave neg.mom.f.", "Whitecap coverage", "Whitecap mean thick.", "Mean breaking height",
        "Dominant break prob", "Wind sea period",
    ];
    const G6: [&str; 13] = [
        "Radiation stresses", "Wave-ocean mom. flux", "wave ind p Bern Head", "Wave-ocean TKE  flux",
        "Stokes transport", "Stokes drift at z=0", "2nd order pressure", "Stokes drft spectrum",
        "2nd ord press spectr", "Wave-ice mom. flux", "Wave-ice energy flux", "Split Surface Stokes",
        "Tot wav-ocn mom flux",
    ];
    const G7: [&str; 5] =
        ["Bottom rms ampl.", "Bottom rms velocity", "Bedform parameters", "Energy diss. in WBBL", "Moment. loss in WBBL"];
    const G8: [&str; 5] =
        ["Mean square slopes", "Phillips tail const", "Slope direction", "Tail slope direction", "Goda peakedness parm"];
    const G9: [&str; 5] =
        ["Avg. time step.", "Cut-off freq.", "Maximum spatial CFL", "Maximum angular CFL", "Maximum k advect CFL"];
    let g: &[&str] = match i {
        1 => &G1,
        2 => &G2,
        3 => &G3,
        4 => &G4,
        5 => &G5,
        6 => &G6,
        7 => &G7,
        8 => &G8,
        9 => &G9,
        _ => &[],
    };
    g.get(j - 1).copied().unwrap_or("Undefined / Not Used")
}

/// `W3FLGRDFLAG`: the names of `FIELD%LIST`, split at blanks, to `FLG2D(IFI,IFJ)`.
pub fn flags_from_list(list: &str, m: &ModDef) -> Vec<bool> {
    let mut f = vec![false; NOGRP * NGRPP];
    for name in list.split(' ').filter(|s| !s.is_empty()) {
        if let Some((i, j)) = fld_to_ij(&name.to_ascii_uppercase(), m) {
            f[(i - 1) + (j - 1) * NOGRP] = true;
        }
    }
    f
}

/// What the namelist `ww3_ounf.nml` holds: `NML_FIELD_T` and `NML_FILE_T`, with the defaults of
/// `model/src/w3nmlounfmd.F90:283-296` and `:404-409`.
pub struct Request {
    pub timestart: Time,
    pub dtreq: Real,
    pub nout: i32,
    pub tref: Time,
    pub tepoch: Time,
    pub list: String,
    pub partition: String,
    pub together: bool,
    pub vector: bool,
    pub nctype: i32,
    pub ncvartype: i32,
    pub timesplit: i32,
    pub fcvars: bool,
    pub timevar: char,
    pub timeunit: char,
    pub noval: Real,
    pub mapsta: bool,
    pub prefix: String,
    pub ix0: i32,
    pub ixn: i32,
    pub iy0: i32,
    pub iyn: i32,
}

fn two_ints(s: &str) -> Time {
    let mut w = s.split_whitespace();
    let a = w.next().unwrap_or_else(|| panic!("{s:?}: two integers expected"));
    let b = w.next().unwrap_or_else(|| panic!("{s:?}: two integers expected"));
    [gridio::int(a), gridio::int(b)]
}

impl Request {
    /// Reads the file. A key the program does not know is refused, as the Fortran namelist read would stop.
    pub fn read(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let groups = gridio::groups(&text);
        let find = |n: &str| -> Option<&Group> { groups.iter().find(|g| g.name == n) };
        let mut r = Request {
            timestart: [19000101, 0],
            dtreq: 0.,
            nout: 1_000_000_000,
            tref: [0, 0],
            tepoch: [19900101, 0],
            list: "unset".to_string(),
            partition: "0 1 2 3".to_string(),
            together: true,
            vector: true,
            nctype: 3,
            ncvartype: 3,
            timesplit: 6,
            fcvars: false,
            timevar: 'D',
            timeunit: 'D',
            noval: UNDEF,
            mapsta: true,
            prefix: "ww3.".to_string(),
            ix0: 1,
            ixn: 1_000_000_000,
            iy0: 1,
            iyn: 1_000_000_000,
        };
        let mut timeref: Option<Time> = None;
        if let Some(g) = find("FIELD_NML") {
            for (k, v) in &g.items {
                let t = gridio::text(v);
                match k.as_str() {
                    "FIELD%TIMESTART" => r.timestart = two_ints(&t),
                    "FIELD%TIMESTRIDE" => r.dtreq = gridio::real(gridio::first(&t)),
                    "FIELD%TIMECOUNT" => r.nout = gridio::int(gridio::first(&t)),
                    "FIELD%TIMESPLIT" => r.timesplit = gridio::int(v),
                    "FIELD%LIST" => r.list = t,
                    "FIELD%PARTITION" => r.partition = t,
                    "FIELD%SAMEFILE" => r.together = gridio::logical(v),
                    "FIELD%VECTOR" => r.vector = gridio::logical(v),
                    "FIELD%TYPE" => r.ncvartype = gridio::int(v),
                    "FIELD%TIMEREF" => timeref = Some(two_ints(&t)),
                    "FIELD%FCVARS" => r.fcvars = gridio::logical(v),
                    "FIELD%TIMEVAR" => r.timevar = t.chars().next().unwrap_or(' ').to_ascii_uppercase(),
                    "FIELD%TIMEUNIT" => r.timeunit = t.chars().next().unwrap_or(' ').to_ascii_uppercase(),
                    "FIELD%TIMEEPOCH" => r.tepoch = two_ints(&t),
                    "FIELD%NOVAL" => r.noval = gridio::real(v),
                    "FIELD%MAPSTA" => r.mapsta = gridio::logical(v),
                    other => panic!("ww3_ounf.nml: {other} is not implemented"),
                }
            }
        }
        r.tref = timeref.unwrap_or(r.timestart);
        if let Some(g) = find("FILE_NML") {
            for (k, v) in &g.items {
                match k.as_str() {
                    "FILE%PREFIX" => r.prefix = gridio::text(v),
                    "FILE%NETCDF" => r.nctype = gridio::int(v),
                    "FILE%IX0" => r.ix0 = gridio::int(v),
                    "FILE%IXN" => r.ixn = gridio::int(v),
                    "FILE%IY0" => r.iy0 = gridio::int(v),
                    "FILE%IYN" => r.iyn = gridio::int(v),
                    other => panic!("ww3_ounf.nml: {other} is not implemented"),
                }
            }
        }
        for g in &groups {
            assert!(g.name == "FIELD_NML" || g.name == "FILE_NML" || g.name == "SMC_NML", "ww3_ounf.nml: group {} is not implemented", g.name);
        }
        assert!(
            find("SMC_NML").is_none_or(|g| g.items.is_empty()),
            "ww3_ounf.nml: SMC_NML is not implemented (no SMC grid)"
        );
        r
    }
}

/// The records of one field in `out_grd.ww3`, as `(IFI, IFJ)` and the length of each, in elements. `None` is a
/// field this port does not read (group 10).
pub fn layout(m: &ModDef, ifi: usize, ifj: usize) -> Option<Vec<usize>> {
    let nsea = m.nsea as usize;
    let band = |lo: i32, hi: i32| (hi - lo + 1).max(0) as usize;
    let one = |n: usize| Some(vec![n]);
    match (ifi, ifj) {
        (3, 1..=5) => one(nsea * band(m.e3df[(ifj - 1) * 3 + 1], m.e3df[(ifj - 1) * 3 + 2])),
        (3, 6) => one(nsea * m.nk as usize),
        (4, 1..=15) => one(nsea * (m.noswll as usize + 1)),
        (4, 16..=23) => one(nsea),
        (6, 8) => {
            let n = nsea * band(m.us3df[1], m.us3df[2]);
            Some(vec![n, n])
        }
        (6, 9) => one(nsea * band(m.p2msf[1], m.p2msf[2])),
        (6, 12) => {
            let n = nsea * m.usspf[1].max(0) as usize;
            Some(vec![n, n])
        }
        (1, 2 | 3 | 8) | (5, 1 | 5 | 6) | (6, 2 | 5 | 6 | 7 | 10 | 13) | (7, 1 | 2 | 5) | (8, 1 | 2) => Some(vec![nsea; 2]),
        (6, 1) | (7, 3) => Some(vec![nsea; 3]),
        (10, _) => None,
        _ if ifi <= 9 => one(nsea),
        _ => None,
    }
}

/// `out_grd.ww3`, read the way `W3IOGO` reads it, one time step at a time.
pub struct GrdIn {
    recs: Records,
    /// `TIME`.
    pub time: Time,
    /// `FLOGRD(IFI,IFJ)` at `(IFI-1) + (IFJ-1)*NOGRP`.
    pub flogrd: Vec<bool>,
    /// `MAPSTA(IY,IX)` and `MAPST2`, at `IY + IX*NY`.
    pub mapsta: Vec<i32>,
    pub mapst2: Vec<i32>,
    /// The arrays `W3IOGO` fills, which stay as they are until a later step reads the same field again.
    pub rec: HashMap<(usize, usize, usize), Vec<Real>>,
}

impl GrdIn {
    /// Opens the file and checks the first record as `W3IOGO` does.
    pub fn open(path: &str, m: &ModDef) -> Self {
        let mut recs = Records::open(path);
        let mut c = recs.next();
        let idstr = c.text(30);
        let verogr = c.text(10);
        let gname = c.text(30);
        let (nogrp, ngrpp, nsea, nx, ny) = (c.i(), c.i(), c.i(), c.i(), c.i());
        let undef = c.f();
        let noswll = c.i();
        c.done("out_grd header");
        assert_eq!(idstr, "WAVEWATCH III GRID OUTPUT FILE", "ILLEGAL IDSTR, READ : {idstr}");
        assert_eq!(verogr, "2019-10-04", "ILLEGAL VEROGR, READ : {verogr}");
        assert!(nogrp as usize == NOGRP && ngrpp as usize == NGRPP, "DIFFERENT NUMBER OF FIELDS, FILE : {nogrp} {ngrpp}");
        if gname != m.gname {
            println!("\n *** WAVEWATCH III WARNING IN W3IOGO :\n     ILLEGAL GNAME, READ : {gname}\n                  CHECK : {}\n", m.gname);
        }
        assert_eq!(noswll, m.noswll, "ILLEGAL NOSWLL, READ : {noswll}");
        assert!(nsea == m.nsea && nx == m.nx && ny == m.ny, "out_grd.ww3 is for another grid");
        assert_eq!(undef, UNDEF, "UNDEF of out_grd.ww3");
        Self { recs, time: [0, 0], flogrd: vec![false; NOGRP * NGRPP], mapsta: Vec::new(), mapst2: Vec::new(), rec: HashMap::new() }
    }

    /// `CALL W3IOGO ('READ', ...)`. `false` is `IOTEST = -1`, the end of the file at the time record.
    pub fn read(&mut self, m: &ModDef) -> bool {
        let Some(mut c) = self.recs.try_next() else { return false };
        self.time = [c.i(), c.i()];
        for f in self.flogrd.iter_mut() {
            *f = c.i() != 0;
        }
        c.done("TIME and FLOGRD");
        let n = (m.nx * m.ny) as usize;
        let mut c = self.recs.next();
        // The file holds ((MAPTMP(IY,IX), IX=1,NX), IY=1,NY): IX fastest.
        let (nx, ny) = (m.nx as usize, m.ny as usize);
        let mut maptmp = vec![0i32; n];
        for iy in 0..ny {
            for ix in 0..nx {
                maptmp[iy + ix * ny] = c.i();
            }
        }
        c.done("MAPSTA");
        self.mapsta = maptmp.iter().map(|t| (t + 2) % 8 - 2).collect();
        self.mapst2 = maptmp.iter().zip(&self.mapsta).map(|(t, s)| (t - s) / 8).collect();
        for ifi in 1..=NOGRP {
            for ifj in 1..=NGRPP {
                if !self.flogrd[(ifi - 1) + (ifj - 1) * NOGRP] {
                    continue;
                }
                let lay = layout(m, ifi, ifj).unwrap_or_else(|| panic!("output field ({ifi},{ifj}) is not implemented"));
                // TP (2,18) is derived from FP0 and read into it, so the two share one array.
                let key = if (ifi, ifj) == (2, 18) { 6 } else { ifj };
                for (k, len) in lay.into_iter().enumerate() {
                    let mut c = self.recs.next();
                    assert_eq!(c.len(), 4 * len, "field ({ifi},{ifj}) record {k}: {} bytes, {} expected", c.len(), 4 * len);
                    self.rec.insert((ifi, key, k), c.fs(len));
                }
            }
        }
        true
    }
}

/// `T2D`.
pub fn t2d(t: Time) -> [i32; 8] {
    let y = t[0] / 10000;
    let mo = (t[0] - y * 10000) / 100;
    let d = t[0] - y * 10000 - 100 * mo;
    let h = t[1] / 10000;
    let mi = (t[1] - h * 10000) / 100;
    let s = t[1] - h * 10000 - 100 * mi;
    [y, mo, d, 0, h, mi, s, 0]
}

/// The day number and seconds of `TSUB` and `TSUBSEC` for CALTYPE `standard`.
fn day(t: &[i32; 8]) -> i32 {
    let a = (14 - t[1]) / 12;
    let b = t[0] + 4800 - a;
    let c = t[1] + 12 * a - 3;
    t[2] + (153 * c + 2) / 5 + 365 * b + b / 4 - b / 100 + b / 400
}

/// `TSUB`: the days from `t1` to `t2`. The seconds of the day are `REAL` sums before they widen, as in the Fortran.
pub fn tsub(t1: &[i32; 8], t2: &[i32; 8]) -> f64 {
    let secs = |t: &[i32; 8]| -> f64 {
        let e: Real = 3600.0 * t[4] as Real + 60.0 * (t[5] - t[3]) as Real + t[6] as Real + t[7] as Real / 1000.0;
        f64::from(e)
    };
    f64::from(day(t2) - day(t1)) + (secs(t2) - secs(t1)) / 86400.0
}

/// `TSUBSEC`: whole seconds from `t1` to `t2`, as an `INTEGER(KIND=8)` the caller stores.
pub fn tsubsec(t1: &[i32; 8], t2: &[i32; 8]) -> i64 {
    let secs = |t: &[i32; 8]| -> i64 {
        // REAL expression, then truncated to INTEGER(8). The millisecond term uses T1(8) in the Fortran for both.
        let e: Real = 3600.0 * t[4] as Real + 60.0 * (t[5] - t[3]) as Real + t[6] as Real + (t1[7] as Real / 1000.0).round();
        e as i64
    };
    i64::from(day(t2) - day(t1)) * 86400 + (secs(t2) - secs(t1))
}

/// `T2ISO`.
pub fn t2iso(t: Time) -> String {
    format!("{:4}-{:02}-{:02}T{:02}:{:02}:{:02}", t[0] / 10000, t[0] % 10000 / 100, t[0] % 100, t[1] / 10000, t[1] % 10000 / 100, t[1] % 100)
}

/// `STME21`.
pub fn stme21(t: Time) -> String {
    if t[0] < 0 {
        return " date and time not set.".to_string();
    }
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02} UTC",
        t[0] / 10000,
        t[0] % 10000 / 100,
        t[0] % 100,
        t[1] / 10000,
        t[1] % 10000 / 100,
        t[1] % 100
    )
}

/// `WRITE (STR,'(F12.0)') x` and `ADJUSTL`: the value rounded to a whole number, with the decimal point.
pub fn f12_0(v: Real) -> String {
    let s = format!("{:.0}", f64::from(v));
    assert!(s.len() < 12, "F12.0 would overflow for {v}");
    format!("{s}.")
}
