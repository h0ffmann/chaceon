// SPDX-License-Identifier: MPL-2.0
//! `ww3_prnc` (`model/src/ww3_prnc.F90` at `92e2cf66`) for `switch_chaceon_shrd`: NetCDF forcing to `wind.ww3`,
//! `current.ww3`, `ice.ww3`, `level.ww3`, `momentum.ww3`, `density.ww3`, `muddens.ww3`, `mudthk.ww3`,
//! `mudvisc.ww3` and `ice1.ww3` to `ice5.ww3`. Input type `AI` (the field is already on the model grid, `ITYPE` 1)
//! and `LL` (a longitude-latitude grid, regular or curvilinear, interpolated with `W3GSUC` and `W3GRMP`,
//! `ITYPE` 2) are translated, from the namelist `ww3_prnc.nml` and from the old `ww3_prnc.inp`. What the Fortran
//! accepts and this does not stops with status 64 and a message that names it; `rust/port_report/WW3_PRNC.md`
//! lists each form. The log the Fortran writes on unit 6 (`NDSO` and `NDSE` are both 6) is reproduced and written
//! to standard output, error messages included, because the comparison is `cmp` of that log too.

use crate::finp::{Inp, ReadErr};
use crate::fortfmt;
use crate::gridio;
use crate::gridsru::{self, Gsu};
use crate::moddef::ModDef;
use crate::ncread as nc;
use crate::ounf::stme21;
use crate::wtime::{Time, dsec21, tick21};
use std::io::Write;
use ww3_kernels::real::Real;

/// Standard output as the Fortran's unit 6: the text is kept until the run ends or stops, so a refusal prints
/// what the Fortran would have printed up to that point before the refusal itself goes to standard error.
struct P {
    out: String,
}

impl P {
    fn w(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn flush(&mut self) {
        let mut so = std::io::stdout().lock();
        let _ = so.write_all(self.out.as_bytes());
        let _ = so.flush();
        self.out.clear();
    }

    /// `CALL EXTCDE ( code )`.
    fn exit(&mut self, code: i32) -> ! {
        self.flush();
        std::process::exit(code)
    }

    /// A form the Fortran runs and this program does not.
    fn refuse(&mut self, what: &str) -> ! {
        self.flush();
        eprintln!("ww3_prnc: {what} is not implemented");
        std::process::exit(64)
    }

    /// `CHECK_ERROR` (`ww3_prnc.F90:2694-2713`): list-directed `WRITE (NDSE,*)`, one blank in front of each record
    /// and an `I11` after the text of the integer item.
    fn chk<T>(&mut self, r: Result<T, i32>, line: u32) -> T {
        match r {
            Ok(v) => v,
            Err(iret) => {
                let msg = nc::strerror(iret);
                self.w("  *** WAVEWATCH III ERROR IN PRNC :\n");
                self.w(&format!("  LINE NUMBER  {line:>11}\n"));
                self.w("  NETCDF ERROR MESSAGE: \n");
                self.w(&format!(" {}\n", fortfmt::a(80, &msg)));
                self.exit(59)
            }
        }
    }
}

/// `INDEX ( s, sub )`, 1-based, 0 when absent.
fn index(s: &str, sub: &str) -> usize {
    s.find(sub).map_or(0, |i| i + 1)
}

/// `INDEX ( s, c, .TRUE. )`.
fn rindex(s: &str, c: char) -> usize {
    s.rfind(c).map_or(0, |i| i + 1)
}

/// `s(a:b)`, 1-based, blank where the string has ended.
fn sub(s: &str, a: usize, b: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    (a..=b).map(|i| c.get(i - 1).copied().unwrap_or(' ')).collect()
}

/// `READ ( str, '(Iw.m)' )`: blanks ignored (`BLANK='NULL'`), an empty field is zero, anything but digits after an
/// optional sign is an error.
fn read_i(s: &str) -> Option<i32> {
    let t: String = s.chars().filter(|c| *c != ' ').collect();
    if t.is_empty() {
        return Some(0);
    }
    let d = t.strip_prefix(['+', '-']).unwrap_or(&t);
    if d.is_empty() || !d.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// `U2D` (`w3timemd.F90:1696-1939`): the date of a `units` attribute, `DAT(1:8)`. A field the `I` edit descriptor
/// cannot read ends the Fortran program with `IOSTAT` and a format that prints an integer through `A`, which is
/// not translated.
fn u2d(p: &mut P, units: &str) -> [i32; 8] {
    let mut dat = [0i32; 8];
    // (word, position of the last '-' for the long form, short form, then the first field positions)
    struct Form {
        word: &'static str,
        // long form: last '-' at `l`; year at `y`; months, days, hours, minutes, seconds follow at +5 steps
        l: usize,
        y: usize,
    }
    let forms = [Form { word: "seconds", l: 22, y: 15 }, Form { word: "days", l: 19, y: 12 }, Form { word: "hours", l: 20, y: 13 }, Form { word: "minutes", l: 22, y: 15 }];
    let Some(f) = forms.iter().find(|f| index(units, f.word) != 0) else { u2d_err(p, units) };
    let last = rindex(units, '-');
    let rd = |p: &mut P, a: usize, b: usize| -> i32 {
        match read_i(&sub(units, a, b)) {
            Some(v) => v,
            None => p.refuse("a time units attribute with a field the I edit descriptor cannot read (U2D, status 45)"),
        }
    };
    let y = f.y;
    if last == f.l {
        // XXX since YYYY-MM-DD hh:mm:ss
        dat[0] = rd(p, y, y + 3);
        dat[1] = rd(p, y + 5, y + 6);
        dat[2] = rd(p, y + 8, y + 9);
        dat[4] = rd(p, y + 11, y + 12);
        dat[5] = rd(p, y + 14, y + 15);
        dat[6] = rd(p, y + 17, y + 18);
    } else if last == f.l - 1 {
        // XXX since YYYY-M-D ...
        dat[0] = rd(p, y, y + 3);
        dat[1] = rd(p, y + 5, y + 5);
        dat[2] = rd(p, y + 7, y + 7);
        let colon = index(units, ":");
        if colon == y + 10 {
            dat[4] = rd(p, y + 9, y + 9);
            dat[5] = rd(p, y + 11, y + 11);
            dat[6] = rd(p, y + 13, y + 13);
        } else if colon == y + 11 {
            dat[4] = rd(p, y + 9, y + 10);
            dat[5] = rd(p, y + 12, y + 13);
            dat[6] = rd(p, y + 15, y + 16);
        } else {
            u2d_err(p, units);
        }
    } else {
        u2d_err(p, units);
    }
    dat
}

/// Label 804: `EXTCDE ( 44 )`.
fn u2d_err(p: &mut P, units: &str) -> ! {
    p.w("\n *** WAVEWATCH III ERROR IN W3TIMEMD : \n");
    p.w("     PREMATURE END OF TIME ATTRIBUTE \n");
    p.w(&format!("     {}\n", units.trim_end()));
    p.w("     DIFFERS FROM CONVENTIONS ISO8601 \n     XXX since YYYY-MM-DD hh:mm:ss\n     XXX since YYYY-M-D h:m:s\n     XXX since YYYY-M-D hh:mm:ss\n\n");
    p.exit(44)
}

/// `T2D`.
fn t2d(t: Time) -> [i32; 8] {
    let y = t[0] / 10000;
    let mo = (t[0] - y * 10000) / 100;
    let d = t[0] - y * 10000 - 100 * mo;
    let h = t[1] / 10000;
    let mi = (t[1] - h * 10000) / 100;
    let s = t[1] - h * 10000 - 100 * mi;
    [y, mo, d, 0, h, mi, s, 0]
}

/// `D2T`.
fn d2t(d: &[i32; 8]) -> Time {
    [d[0] * 10000 + d[1] * 100 + d[2], d[4] * 10000 + d[5] * 100 + d[6]]
}

/// `D2J` (`w3timemd.F90:1226-1345`). `SECOND` is `REAL`.
fn d2j(d: &[i32; 8]) -> f64 {
    let (year, month, day) = (d[0], d[1], d[2]);
    let utc = d[3] * 60;
    let (hour, minute) = (d[4], d[5]);
    let second: f32 = ((d[6] - utc) as f64 + d[7] as f64 / 1000.0f64) as f32;
    if year == 0 || year < -4713 {
        return -f64::from(i32::MAX);
    }
    let a = (14 - month) / 12;
    let y = year + 4800 - a;
    let m = month + 12 * a - 3;
    let jdn = day + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    f64::from(jdn) + f64::from(hour - 12) / 24.0f64 + f64::from(minute) / 1440.0f64 + f64::from(second) / 86400.0f64
}

/// `J2D` (`w3timemd.F90:1349-1492`) with `TZ = 0`. `SECOND` and `SECDAY` are `REAL`, the constants `D0`.
fn j2d(julian: f64) -> [i32; 8] {
    let secday: f32 = 86400.0;
    let mut ijul = julian as i32;
    let mut second: f32 = ((julian - f64::from(ijul)) * f64::from(secday)) as f32;
    second += 0.0;
    if f64::from(second) >= f64::from(secday) / 2.0f64 {
        ijul += 1;
        second = (f64::from(second) - f64::from(secday) / 2.0f64) as f32;
    } else {
        second = (f64::from(second) + f64::from(secday) / 2.0f64) as f32;
    }
    if second >= secday {
        ijul += 1;
        second -= secday;
    }
    let mut minute = (second / 60.0f32) as i32;
    second -= (minute * 60) as f32;
    let hour = minute / 60;
    minute -= hour * 60;
    let jalpha = ((f64::from(ijul - 1867216) - 0.25f64) / 36524.25f64) as i32;
    let ja = ijul + 1 + jalpha - (0.25f64 * f64::from(jalpha)) as i32;
    let jb = ja + 1524;
    let jc = (6680.0f64 + (f64::from(jb - 2439870) - 122.1f64) / 365.25f64) as i32;
    let jd = 365 * jc + (0.25f64 * f64::from(jc)) as i32;
    let je = (f64::from(jb - jd) / 30.6001f64) as i32;
    let day = jb - jd - (30.6001f64 * f64::from(je)) as i32;
    let mut month = je - 1;
    if month > 12 {
        month -= 12;
    }
    let mut year = jc - 4715;
    if month > 2 {
        year -= 1;
    }
    if year <= 0 {
        year -= 1;
    }
    let isec = second as i32;
    [year, month, day, 0, hour, minute, isec, ((second - isec as f32) * 1000.0f32) as i32]
}

/// A namelist group as `ww3_prnc.nml` has it, the keys already upper case.
struct Nml {
    forcing: Vec<(String, String)>,
    file: Vec<(String, String)>,
}

/// The values of `FORCING_NML` and `FILE_NML` with the defaults of `w3nmlprncmd.F90:283-306` and `:421-427`.
struct Req {
    timestart: String,
    timestop: String,
    /// The `FIELD%` flags in the order of the `IF` chain of `ww3_prnc.F90:485-559`.
    field: [bool; 17],
    asis: bool,
    latlon: bool,
    tidal: String,
    filename: String,
    longitude: String,
    latitude: String,
    var: [String; 3],
    timeshift: String,
}

const FIELD_KEYS: [&str; 17] = [
    "ICE_PARAM1", "ICE_PARAM2", "ICE_PARAM3", "ICE_PARAM4", "ICE_PARAM5", "MUD_DENSITY", "MUD_THICKNESS", "MUD_VISCOSITY", "ICE_CONC", "ICE_BERG",
    "WATER_LEVELS", "WINDS", "WINDS_AST", "CURRENTS", "DATA_ASSIM", "ATM_MOMENTUM", "AIR_DENSITY",
];

/// A quoted string, cut to the length of the Fortran variable.
fn nml_text(p: &mut P, key: &str, v: &str, len: usize) -> String {
    let t = v.trim();
    let q = t.chars().next();
    if !matches!(q, Some('\'' | '"')) {
        p.refuse(&format!("the unquoted value of {key} in ww3_prnc.nml"));
    }
    let s = gridio::text(t);
    s.chars().take(len).collect::<String>().trim_end().to_string()
}

fn nml_logical(p: &mut P, key: &str, v: &str) -> bool {
    let t = v.trim().to_ascii_uppercase();
    match t.as_str() {
        "T" | ".T." | ".TRUE." | "TRUE" => true,
        "F" | ".F." | ".FALSE." | "FALSE" => false,
        _ => p.refuse(&format!("the value {v} of {key} in ww3_prnc.nml")),
    }
}

fn read_req(p: &mut P, nml: &Nml) -> Req {
    let mut r = Req {
        timestart: "19000101 000000".into(),
        timestop: "29001231 000000".into(),
        field: [false; 17],
        asis: false,
        latlon: false,
        tidal: "unset".into(),
        filename: "unset".into(),
        longitude: "unset".into(),
        latitude: "unset".into(),
        var: ["unset".into(), "unset".into(), "unset".into()],
        timeshift: "00000000 000000".into(),
    };
    for (k, v) in &nml.forcing {
        match k.as_str() {
            "FORCING%TIMESTART" => r.timestart = nml_text(p, k, v, 15),
            "FORCING%TIMESTOP" => r.timestop = nml_text(p, k, v, 15),
            "FORCING%GRID%ASIS" => r.asis = nml_logical(p, k, v),
            "FORCING%GRID%LATLON" => r.latlon = nml_logical(p, k, v),
            "FORCING%TIDAL" => r.tidal = nml_text(p, k, v, 256),
            other => match other.strip_prefix("FORCING%FIELD%").and_then(|f| FIELD_KEYS.iter().position(|x| *x == f)) {
                Some(i) => r.field[i] = nml_logical(p, k, v),
                None => p.refuse(&format!("the key {k} of FORCING_NML")),
            },
        }
    }
    for (k, v) in &nml.file {
        match k.as_str() {
            "FILE%FILENAME" => r.filename = nml_text(p, k, v, 256),
            "FILE%LONGITUDE" => r.longitude = nml_text(p, k, v, 100),
            "FILE%LATITUDE" => r.latitude = nml_text(p, k, v, 100),
            "FILE%TIMESHIFT" => r.timeshift = nml_text(p, k, v, 15),
            "FILE%VAR(1)" => r.var[0] = nml_text(p, k, v, 100),
            "FILE%VAR(2)" => r.var[1] = nml_text(p, k, v, 100),
            "FILE%VAR(3)" => r.var[2] = nml_text(p, k, v, 100),
            "FILE%VAR" => {
                // A list: 'a', 'b', 'c'.
                let names: Vec<&str> = v.split('\'').skip(1).step_by(2).collect();
                if names.is_empty() || names.len() > 3 {
                    p.refuse("FILE%VAR with other than one to three quoted names");
                }
                for (i, n) in names.iter().enumerate() {
                    r.var[i] = n.chars().take(100).collect::<String>().trim_end().to_string();
                }
            }
            other => p.refuse(&format!("the key {other} of FILE_NML")),
        }
    }
    r
}

/// `READ ( str, * ) a, b` of two default integers.
fn two_ints(p: &mut P, s: &str, what: &str) -> Time {
    let mut it = s.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty());
    let mut one = || it.next().and_then(|t| t.parse::<i32>().ok());
    match (one(), one()) {
        (Some(a), Some(b)) => [a, b],
        _ => p.refuse(&format!("{what} that is not two integers")),
    }
}

/// `STRSPLIT`: the blank-separated words of a line (`w3servmd.F90:1439-1495`).
fn strsplit(s: &str) -> Vec<String> {
    s.split(' ').filter(|t| !t.is_empty()).map(|t| t.chars().take(100).collect()).collect()
}

fn fld_name(idfld: &str) -> &'static str {
    match idfld {
        "LEV" => "level.ww3",
        "CUR" => "current.ww3",
        "WND" | "WNS" => "wind.ww3",
        "ICE" | "ISI" => "ice.ww3",
        "TAU" => "momentum.ww3",
        "RHO" => "density.ww3",
        "MDN" => "muddens.ww3",
        "MTH" => "mudthk.ww3",
        "MVS" => "mudvisc.ww3",
        "IC1" => "ice1.ww3",
        "IC2" => "ice2.ww3",
        "IC3" => "ice3.ww3",
        "IC4" => "ice4.ww3",
        "IC5" => "ice5.ww3",
        _ => unreachable!(),
    }
}

const IDSTR1: [&str; 15] = [
    "ice thickness ", "ice viscosity", "ice density   ", "ice modulus  ", "ice flow diam.", "mud density  ", "mud thickness ", "mud viscosity ", "ice conc.     ",
    "water levels ", "winds         ", "currents     ", "data          ", "momentum     ", "air density   ",
];
const IDSTR2: [&str; 6] = ["pre-processed file  ", "long.-lat. grid     ", "grid from file (1)  ", "grid from file (2)  ", "data (assimilation) ", "pre-pro. file + tide"];

/// A Fortran unformatted sequential record, big-endian.
fn put(out: &mut Vec<u8>, body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
}

fn field_record(f: &[Real]) -> Vec<u8> {
    f.iter().flat_map(|x| x.to_be_bytes()).collect()
}

/// `INTERP` (`ww3_prnc.F90:2551-2686`): `xc` is `(mxm, mym)`, the weights and indices `(nx, ny)`; every array is
/// column-major with the first Fortran index fastest.
#[allow(clippy::too_many_arguments)]
fn interp(nx: usize, ny: usize, mxm: usize, xc: &[Real], ix21: &[i32], ix22: &[i32], iy21: &[i32], iy22: &[i32], rd11: &[Real], rd12: &[Real], rd21: &[Real], rd22: &[Real], fill: Real, fa: &mut [Real]) {
    let at = |i: i32, j: i32| xc[(i - 1) as usize + (j - 1) as usize * mxm];
    for iy in 0..ny {
        for ix in 0..nx {
            let k = ix + iy * nx;
            let (a, b, c, d) = (ix21[k], ix22[k], iy21[k], iy22[k]);
            let mut factor: Real = 0.0;
            fa[k] = 0.0;
            if at(a, c) != fill {
                factor += rd11[k];
                fa[k] = rd11[k] * at(a, c);
            }
            if at(b, c) != fill {
                factor += rd21[k];
                fa[k] += rd21[k] * at(b, c);
            }
            if at(a, d) != fill {
                factor += rd12[k];
                fa[k] += rd12[k] * at(a, d);
            }
            if at(b, d) != fill {
                factor += rd22[k];
                fa[k] += rd22[k] * at(b, d);
            }
            if factor > 0.0 {
                fa[k] /= factor;
            } else if at(a, c) != fill {
                fa[k] = at(a, c);
            } else if at(b, c) != fill {
                fa[k] = at(b, c);
            } else if at(a, d) != fill {
                fa[k] = at(a, d);
            } else if at(b, d) != fill {
                fa[k] = at(b, d);
            } else {
                fa[k] = 0.0;
            }
        }
    }
}

/// `IRET = NF90_INQ_VARID ( ... )` then `IF ( IRET/=NF90_NOERR ) IRET = NF90_INQ_VARID ( ... )` for each name in turn: the
/// first that exists, or the status of the last try.
fn first_var(ncid: i32, names: &[&str]) -> Result<i32, i32> {
    let mut r = Err(-1);
    for n in names {
        r = nc::inq_varid(ncid, n);
        if r.is_ok() {
            break;
        }
    }
    r
}

/// `NF90_GET_VAR` with Fortran-order `start` (1-based) and `count`: padded to the variable's rank with 1, then
/// reversed to the C order of the library. The result is the Fortran array, first index fastest.
fn get_f32(p: &mut P, ncid: i32, varid: i32, rank: usize, fstart: &[usize], fcount: &[usize], line: u32) -> Vec<f32> {
    let (cs, cc) = c_order(p, rank, fstart, fcount);
    let r = nc::get_vara_float(ncid, varid, &cs, &cc);
    p.chk(r, line)
}

fn get_f64(p: &mut P, ncid: i32, varid: i32, rank: usize, fstart: &[usize], fcount: &[usize], line: u32) -> Vec<f64> {
    let (cs, cc) = c_order(p, rank, fstart, fcount);
    let r = nc::get_vara_double(ncid, varid, &cs, &cc);
    p.chk(r, line)
}

fn c_order(p: &mut P, rank: usize, fstart: &[usize], fcount: &[usize]) -> (Vec<usize>, Vec<usize>) {
    if fstart.len() > rank || fcount.len() > rank {
        p.refuse("a start or count longer than the rank of the variable (NF90_GET_VAR)");
    }
    let mut s = vec![1usize; rank];
    let mut c = vec![1usize; rank];
    s[..fstart.len()].copy_from_slice(fstart);
    c[..fcount.len()].copy_from_slice(fcount);
    s.reverse();
    c.reverse();
    (s.iter().map(|x| x - 1).collect(), c)
}

/// Reads `mod_def.ww3` and `ww3_prnc.nml` (or `ww3_prnc.inp`) from `dir`, writes the forcing file there. The
/// NetCDF name in the input is joined to `dir`, as the Fortran's `FNMPRE` (`./`) joins it to the working directory.
pub fn run(dir: &str) -> ! {
    let mut p = P { out: String::new() };
    p.w("\n               ");
    p.w("  *** WAVEWATCH III  Input pre-processing ***  \n               ===============================================\n\n");
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    p.w(&format!("  Grid name : {}\n\n", fortfmt::a(30, &m.gname)));
    let (nx, ny) = (m.nx as usize, m.ny as usize);

    // 3. Read types and variables from the input file.
    let flgnml = std::path::Path::new(&format!("{dir}/ww3_prnc.nml")).exists();
    let (flstab, flberg);
    let idfld: String;
    let ifld: i32;
    let itype: usize;
    let nfields: usize;
    let mut dimsname: Vec<String> = vec![String::new(); 2];
    let mut fieldsname: Vec<String> = vec![String::new(); 4];
    let (mut timestart, mut timestop): (Time, Time) = ([19000101, 0], [29001231, 0]);
    let (mut startjulday, mut stpjulday) = (0.0f64, 100000000.0f64);
    let (fltime, flhdr);
    let mut timeshift: Time = [0, 0];
    let namef: String;
    let strdimsname: String;
    if flgnml {
        let text = std::fs::read_to_string(format!("{dir}/ww3_prnc.nml")).unwrap_or_else(|e| p.refuse(&format!("reading ww3_prnc.nml: {e}")));
        let groups = gridio::groups(&text);
        let find = |n: &str| groups.iter().find(|g| g.name == n).map(|g| g.items.clone());
        let (Some(forcing), Some(file)) = (find("FORCING_NML"), find("FILE_NML")) else {
            p.refuse("a ww3_prnc.nml without both FORCING_NML and FILE_NML");
        };
        // OPEN (NDSN, file=TRIM(INFILE)//'.log') comes before the read, so the file exists even if the read stops.
        let logpath = format!("{dir}/ww3_prnc.nml.log");
        if let Err(e) = std::fs::write(&logpath, "") {
            p.w(&format!("ERROR: open full nml file ./ww3_prnc.nml.log failed ({e})\n"));
            p.exit(1);
        }
        let r = read_req(&mut p, &Nml { forcing, file });
        if r.tidal != "unset" {
            if !r.field[10] && !r.field[13] {
                p.w("ERROR: TIDAL must only be used on FIELD%WATER_LEVELS or FIELD%CURRENTS\n");
                p.exit(2);
            } else if !r.asis {
                p.w("ERROR: TIDAL must only be used on GRID%ASIS\n");
                p.exit(3);
            }
        }
        // REPORT_FORCING_NML and REPORT_FILE_NML.
        let mut rep = String::from("\n");
        rep.push_str(&format!("FORCING %  TIMESTART  = {}\nFORCING %  TIMESTOP   = {}\n", r.timestart, r.timestop));
        // The report lists the flags in the order of the type declaration, which is not the order of the IF chain.
        for i in [0, 1, 2, 3, 4, 5, 6, 7, 10, 13, 11, 12, 15, 16, 8, 9, 14] {
            rep.push_str(&format!("FORCING %  FIELD % {:<15}= {}\n", FIELD_KEYS[i], if r.field[i] { "T" } else { "F" }));
        }
        rep.push_str(&format!("FORCING %  GRID % ASIS   = {}\n", if r.asis { "T" } else { "F" }));
        rep.push_str(&format!("FORCING %  GRID % LATLON = {}\n", if r.latlon { "T" } else { "F" }));
        rep.push_str(&format!("FORCING %  TIDAL = {}\n\n", r.tidal));
        rep.push_str(&format!("FILE %  FILENAME    = {}\nFILE %  LONGITUDE   = {}\nFILE %  LATITUDE    = {}\n", r.filename, r.longitude, r.latitude));
        rep.push_str(&format!("FILE %  VAR(1)      = {}\nFILE %  VAR(2)      = {}\nFILE %  VAR(3)      = {}\n", r.var[0], r.var[1], r.var[2]));
        rep.push_str(&format!("FILE %  TIMESHIFT   = {}\n", r.timeshift));
        if let Err(e) = std::fs::write(&logpath, rep) {
            p.refuse(&format!("writing ww3_prnc.nml.log: {e}"));
        }

        // The IF chain on the field flags.
        let chain: [(&str, i32, usize, bool); 17] = [
            ("IC1", -7, 1, false),
            ("IC2", -6, 1, false),
            ("IC3", -5, 1, false),
            ("IC4", -4, 1, false),
            ("IC5", -3, 1, false),
            ("MDN", -2, 1, false),
            ("MTH", -1, 1, false),
            ("MVS", 0, 1, false),
            ("ICE", 1, 1, false),
            ("ISI", 1, 2, true),
            ("LEV", 2, 1, false),
            ("WND", 3, 2, false),
            ("WNS", 3, 3, false),
            ("CUR", 4, 2, false),
            ("DAT", 5, 1, false),
            ("TAU", 6, 2, false),
            ("RHO", 7, 1, false),
        ];
        let Some(sel) = r.field.iter().position(|b| *b) else {
            p.w("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     NO FIELD SELECTED\n\n");
            p.exit(1010)
        };
        let (id, f, nf, berg) = chain[sel];
        if id == "DAT" {
            p.refuse("the field DATA_ASSIM (assimilation data, DAT)");
        }
        (idfld, ifld, nfields) = (id.to_string(), f, nf);
        flberg = berg;
        flstab = id == "WNS";
        itype = if r.asis {
            1
        } else if r.latlon {
            2
        } else {
            p.w("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     NO GRID SELECTED\n\n");
            p.exit(1011)
        };
        let tidal_on = r.tidal != "unset" && r.tidal != "UNSET";
        if tidal_on {
            p.refuse("a tidal analysis (TIDAL, input type AT, ITYPE 6)");
        }
        namef = r.filename.chars().take(80).collect::<String>();
        dimsname[0] = r.longitude.clone();
        dimsname[1] = r.latitude.clone();
        fieldsname[..nfields].clone_from_slice(&r.var[..nfields]);
        timestart = two_ints(&mut p, &r.timestart, "TIMESTART");
        startjulday = d2j(&t2d(timestart));
        timestop = two_ints(&mut p, &r.timestop, "TIMESTOP");
        stpjulday = d2j(&t2d(timestop));
        flhdr = true;
        timeshift = two_ints(&mut p, &r.timeshift, "TIMESHIFT");
        fltime = !(timeshift[0] != 0 || timeshift[1] != 0);
        strdimsname = String::new();
    } else {
        let mut inp = match Inp::open(&format!("{dir}/ww3_prnc.inp")) {
            Ok(i) => i,
            Err(e) => {
                p.w(&format!("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     ERROR IN OPENING INPUT FILE\n     IOSTAT ={:5}\n\n", e.raw_os_error().unwrap_or(0)));
                p.exit(40)
            }
        };
        // READ errors: END= is 801, ERR= is 802 with the IOSTAT of libgfortran (5010 for a bad value).
        macro_rules! rd {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(ReadErr::End) => {
                        p.w("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     PREMATURE END OF INPUT FILE\n\n");
                        p.exit(41)
                    }
                    Err(ReadErr::Bad(_)) => {
                        p.w("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     ERROR IN READING FROM INPUT FILE\n     IOSTAT = 5010\n\n");
                        p.exit(42)
                    }
                }
            };
        }
        macro_rules! nextln {
            () => {
                if let Err(ReadErr::End) = inp.nextln() {
                    p.w("\n *** WAVEWATCH III ERROR IN NEXTLN : \n     PREMATURE END OF INPUT FILE\n\n");
                    p.exit(1)
                }
            };
        }
        let comstr = rd!(inp.comment_char());
        p.w(&format!("  Comment character is '{comstr}'\n\n"));
        nextln!();
        let it = rd!(inp.list(4));
        let cut = |t: &Option<String>, n: usize| t.clone().unwrap_or_default().chars().take(n).collect::<String>();
        let id = cut(&it[0], 3);
        let idtype = cut(&it[1], 2);
        fltime = rd!(crate::finp::logical(&it[2], false));
        flhdr = rd!(crate::finp::logical(&it[3], false));
        flstab = id == "WNS";
        flberg = id == "ISI";
        ifld = match id.as_str() {
            "IC1" => -7,
            "IC2" => -6,
            "IC3" => -5,
            "IC4" => -4,
            "IC5" => -3,
            "MDN" => -2,
            "MTH" => -1,
            "MVS" => 0,
            "ICE" | "ISI" => 1,
            "LEV" => 2,
            "WND" | "WNS" => 3,
            "CUR" => 4,
            "DAT" => 5,
            "TAU" => 6,
            "RHO" => 7,
            _ => {
                p.w(&format!("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     ILLEGAL FIELD ID -->{id:<3}<--\n\n"));
                p.exit(30)
            }
        };
        itype = if id == "DAT" {
            p.refuse("the field DAT (assimilation data, input type 5)")
        } else {
            match idtype.as_str() {
                "AI" => 1,
                "AT" => p.refuse("input type AT (tidal analysis, ITYPE 6)"),
                "LL" => 2,
                "F1" => p.refuse("input type F1 (grid from file, W3FLDP)"),
                "F2" => p.refuse("input type F2 (two grids from file, W3FLDP)"),
                _ => {
                    p.w(&format!("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     ILLEGAL FORMAT ID -->{idtype:<2}<--\n\n"));
                    p.exit(31)
                }
            }
        };
        nextln!();
        let line = rd!(inp.raw_line());
        let l: String = line.chars().take(1024).collect();
        strdimsname = l.clone();
        let d = strsplit(&l);
        if d.len() > 2 {
            p.refuse("more than two dimension names in the input file");
        }
        for (i, n) in d.iter().enumerate() {
            dimsname[i] = n.clone();
        }
        nextln!();
        let line = rd!(inp.raw_line());
        let f = strsplit(&line.chars().take(1024).collect::<String>());
        if f.len() > 3 {
            p.refuse("more than three variable names in the input file");
        }
        nfields = f.len();
        for (i, n) in f.iter().enumerate() {
            fieldsname[i] = n.clone();
        }
        if !fltime {
            nextln!();
            let it = rd!(inp.list(2));
            for k in 0..2 {
                timeshift[k] = rd!(crate::finp::int(&it[k], 0));
            }
            if timeshift[0] < 10000000 {
                p.w("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     ILLEGAL TIME : 00000000 000000\n\n");
                p.exit(35)
            }
        }
        nextln!();
        let it = rd!(inp.list(1));
        namef = cut(&it[0], 80);
        idfld = id;
    }

    // 4. Print logs.
    p.w(&format!(
        "\n  Description of inputs\n --------------------------------------------------\n       Input type        : {}\n       Format type       : {}\n",
        fortfmt::a(14, IDSTR1[(ifld + 7) as usize]),
        IDSTR2[itype - 1]
    ));
    if itype != 1 && itype != 6 && (ifld == 3 || ifld == 4 || ifld == 6) {
        // WNT1 for winds and momentum, CRT1 for currents.
        p.w("          Field conserves velocity.\n");
    }
    if flgnml {
        if timestart[0] != 19000101 || timestart[1] != 0 {
            p.w(&format!("       Start time        : {}\n", stme21(timestart)));
        }
        if timestop[0] != 29001231 || timestop[1] != 0 {
            p.w(&format!("       Stop time         : {}\n", stme21(timestop)));
        }
    }
    if !fltime {
        p.w(&format!("       Shifted time      : {}\n", stme21(timeshift)));
    }
    if flberg {
        p.w("          Icebergs included.\n");
    }
    if flstab {
        p.w("          Air-sea temperature differences included.\n");
    }
    p.w(&format!("\n           File name         : {}\n", fortfmt::a(80, &namef)));
    let nidims = dimsname.iter().filter(|d| !d.trim().is_empty()).count();
    p.w(&format!("           Dimension along x : {}\n           Dimension along y : {}\n", dimsname[0].trim_end(), dimsname[1].trim_end()));
    for (i, f) in fieldsname.iter().take(nfields).enumerate() {
        p.w(&format!("           Field component {} : {}\n", i + 1, f.trim_end()));
    }

    // 5. Read the input NetCDF file.
    let path = format!("{dir}/{}", namef.trim_end());
    let r = nc::open(&path);
    let ncid = p.chk(r, 791);
    let r = nc::inq_varid(ncid, "time").or_else(|_| nc::inq_varid(ncid, "MT"));
    let vartime = p.chk(r, 797);
    match nc::get_att_text(ncid, vartime, "calendar") {
        Err(_) => p.w("\n *** WAVEWATCH III WARNING IN W3PRNC : \n     calendar ATTRIBUTE NOT DEFINED\n     IT MUST RESPECT STANDARD OR GREGORIAN CALENDAR\n"),
        Ok(c) => {
            if c.len() > 50 {
                p.refuse("a calendar attribute longer than 50 characters");
            }
            if index(&c, "standard") == 0 && index(&c, "gregorian") == 0 {
                p.w("\n *** WAVEWATCH III WARNING IN W3PRNC : \n     CALENDAR ATTRIBUTE NOT MATCH\n     IT MUST RESPECT STANDARD OR GREGORIAN CALENDAR\n");
            }
        }
    }
    let r = nc::get_att_text(ncid, vartime, "units");
    let timeunits = p.chk(r, 806);
    if timeunits.len() > 50 {
        p.refuse("a time units attribute longer than 50 characters");
    }
    let refdate = u2d(&mut p, &timeunits);
    let refjulday = d2j(&refdate);

    // The variables: ids, dimensions, fill value.
    let mut varidf = [0i32; 3];
    let (mut ndimsvar, mut dimname, mut dimln) = (0usize, Vec::<String>::new(), Vec::<usize>::new());
    let mut fillvalue: Real = 0.0;
    for i in 0..nfields {
        let r = nc::inq_varid(ncid, fieldsname[i].trim_end());
        varidf[i] = p.chk(r, 813);
        let r = nc::inq_varndims(ncid, varidf[i]);
        ndimsvar = p.chk(r, 815);
        if ndimsvar > 4 {
            p.refuse("a field variable with more than four dimensions");
        }
        let r = nc::inq_vardimids(ncid, varidf[i]);
        let mut ids = p.chk(r, 817);
        // The Fortran interface lists the dimensions the other way round.
        ids.reverse();
        dimname.clear();
        dimln.clear();
        for id in ids {
            let r = nc::inq_dim(ncid, id);
            let (n, l) = p.chk(r, 820);
            if n.chars().count() > 20 {
                p.refuse("a dimension name longer than 20 characters");
            }
            dimname.push(n);
            dimln.push(l);
        }
        match nc::get_att_float(ncid, varidf[i], "_FillValue") {
            Ok(v) => fillvalue = v,
            Err(_) => match nc::get_att_float(ncid, varidf[i], "missing_value") {
                Ok(v) => fillvalue = v,
                Err(_) => {
                    p.w(&format!("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     _FillValue ATTRIBUTE NOT DEFINED FOR : {}\n\n", fieldsname[i].trim_end()));
                    p.exit(27)
                }
            },
        }
    }

    // Generic dimensions.
    let (mut nxi, mut nyi, mut ndimsgrid) = (0usize, 0usize, 2usize);
    let mut nti: Option<usize> = None;
    for i in 0..ndimsvar {
        if dimname[i] == "time" || dimname[i] == "MT" {
            nti = Some(dimln[i]);
        }
        if dimname[i] == dimsname[0].trim_end() {
            nxi = dimln[i];
        }
        if dimname[i] == dimsname[0].trim_end() && nidims == 1 {
            ndimsgrid = 1;
            nyi = 1;
        }
        if nidims >= 2 && dimname[i] == dimsname[1].trim_end() {
            nyi = dimln[i];
        }
    }
    if nxi * nyi == 0 {
        // Label 864 prints TRIM(STRDIMSNAME): in the namelist form that variable was never set, 1024 NUL bytes.
        let s = if flgnml { "\0".repeat(1024) } else { strdimsname.trim_end().to_string() };
        p.w(&format!("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     GRID DIMENSIONS {s} NOT FOUND... CHECK DIMENSION NAMES\n"));
        p.exit(56)
    }
    let Some(nti) = nti else {
        p.refuse("a field variable without a dimension named time or MT (NTI is never set in the Fortran)");
    };
    let factor: Real = if m.flagll { 1. } else { 1.0e-3 };

    // Longitude and latitude of the input.
    let (mut ala, mut alo): (Vec<Real>, Vec<Real>) = (Vec::new(), Vec::new());
    let (mut x0i, mut xni, mut y0i, mut yni): (Real, Real, Real, Real) = (0., 0., 0., 0.);
    if itype != 1 && itype != 6 {
        alo = vec![0.0; nxi * nyi];
        ala = vec![0.0; nxi * nyi];
        let r = first_var(ncid, &["longitude", "lon", "Longitude", "x", "X"]);
        let vid = p.chk(r, 867);
        let r = nc::inq_varndims(ncid, vid);
        let numdims = p.chk(r, 870);
        if numdims > 2 {
            p.refuse("a longitude variable with more than two dimensions");
        }
        if numdims == 1 {
            x0i = get_f32(&mut p, ncid, vid, 1, &[1], &[1], 870)[0];
            xni = get_f32(&mut p, ncid, vid, 1, &[nxi], &[1], 872)[0];
            let v = get_f32(&mut p, ncid, vid, 1, &[1], &[nxi], 874);
            for iy in 0..nyi {
                alo[iy * nxi..(iy + 1) * nxi].copy_from_slice(&v);
            }
        } else {
            x0i = get_f32(&mut p, ncid, vid, 2, &[1, 1], &[1, 1], 880)[0];
            xni = get_f32(&mut p, ncid, vid, 2, &[nxi, 1], &[1, 1], 882)[0];
            alo = get_f32(&mut p, ncid, vid, 2, &[1, 1], &[nxi, nyi], 884);
        }
        let r = first_var(ncid, &["latitude", "lat", "Latitude", "y", "Y"]);
        let vid = p.chk(r, 893);
        let r = nc::inq_varndims(ncid, vid);
        let numdims = p.chk(r, 893);
        if numdims > 2 {
            p.refuse("a latitude variable with more than two dimensions");
        }
        y0i = get_f32(&mut p, ncid, vid, numdims, &[1], &[1], 895)[0];
        if numdims == 1 {
            let v = get_f32(&mut p, ncid, vid, 1, &[1], &[nyi], 898);
            for ix in 0..nxi {
                for iy in 0..nyi {
                    ala[ix + iy * nxi] = v[iy];
                }
            }
            yni = ala[(nyi - 1) * nxi];
        } else {
            ala = get_f32(&mut p, ncid, vid, 2, &[1, 1], &[nxi, nyi], 905);
            yni = ala[(nyi - 1) * nxi];
        }
    }

    // 5.x Input grid and its log.
    let (nxj, nyj);
    if itype == 1 {
        (nxj, nyj) = (nx, ny);
        let rw = [factor * m.xgrd[0], factor * m.xgrd[nx * ny - 1], factor * m.ygrd[0], factor * m.ygrd[nx * ny - 1]];
        p.w(&format!("\n       Input grid dim.   :{}   {}\n", fortfmt::i(9, nx as i64), fortfmt::i(5, ny as i64)));
        let (a, b, c, d) = (f64::from(rw[0]), f64::from(rw[1]), f64::from(rw[2]), f64::from(rw[3]));
        if m.flagll {
            p.w(&format!("       Longitude range   :{}{} (deg)\n       Latitude range    :{}{} (deg)\n", fortfmt::f(8, 2, a), fortfmt::f(8, 2, b), fortfmt::f(8, 2, c), fortfmt::f(8, 2, d)));
        } else {
            p.w(&format!("       X range           :{}{} (km)\n       Y range           :{}{} (km)\n", fortfmt::f(8, 2, a), fortfmt::f(8, 2, b), fortfmt::f(8, 2, c), fortfmt::f(8, 2, d)));
        }
    } else {
        (nxj, nyj) = (nxi, nyi);
        if m.gtype == 1 && y0i > yni {
            p.w("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     LATITUDE VALUES MUST BE REVERSED\n     EXAMPLE: ncpdq -h -O -a -lat file.nc\n\n");
            p.exit(32)
        }
        if nxi < 2 || nyi < 2 {
            p.w(&format!("\n *** WAVEWATCH III ERROR IN W3PRNC : \n     ILLEGAL SIZE OF INPUT GRID : {} {}\n\n", fortfmt::i(5, nxi as i64), fortfmt::i(5, nyi as i64)));
            p.exit(36)
        }
        p.w(&format!("\n       Input grid dim.   :{}   {}\n", fortfmt::i(9, nxi as i64), fortfmt::i(5, nyi as i64)));
        let (a, b, c, d) = (f64::from(factor * x0i), f64::from(factor * xni), f64::from(factor * y0i), f64::from(factor * yni));
        if m.flagll {
            p.w(&format!("       Longitude range   :{}{} (deg)\n       Latitude range    :{}{} (deg)\n", fortfmt::f(8, 2, a), fortfmt::f(8, 2, b), fortfmt::f(8, 2, c), fortfmt::f(8, 2, d)));
        } else {
            p.w(&format!("       X range           :{}{} (km)\n       Y range           :{}{} (km)\n", fortfmt::f(8, 2, a), fortfmt::f(8, 2, b), fortfmt::f(8, 2, c), fortfmt::f(8, 2, d)));
        }
    }

    // 6. Prepare interpolation.
    p.w("\n\n  Preprocessing data\n --------------------------------------------------\n");
    let n = nx * ny;
    let (mut ix21, mut ix22, mut iy21, mut iy22) = (vec![0i32; n], vec![0i32; n], vec![0i32; n], vec![0i32; n]);
    let (mut rd11, mut rd21, mut rd12, mut rd22) = (vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n]);
    if itype == 2 {
        p.w("       Interpolation factors ..... \n          (longitude-latitude grid)\n");
        let sxi = (xni - x0i) / (nxi - 1) as Real;
        let mut iclo = -1; // ICLOSE_NONE
        if m.flagll && ((nxi as Real * sxi).abs() - 360.).abs() < 0.1 * sxi.abs() {
            iclo = 2; // ICLOSE_SMPL
        }
        // W3GSUC( .TRUE., ... ) takes the arrays as (NXI,NYI); the search utility here keeps (NY,NX).
        let (mut xg, mut yg) = (vec![0.0f64; nxi * nyi], vec![0.0f64; nxi * nyi]);
        for i in 0..nxi {
            for j in 0..nyi {
                xg[j + i * nyi] = f64::from(alo[i + j * nxi]);
                yg[j + i * nyi] = f64::from(ala[i + j * nxi]);
            }
        }
        let gsi = Gsu::create(m.flagll, iclo, nxi as i32, nyi as i32, xg, yg);
        for iy in 0..ny {
            for ix in 0..nx {
                let k = ix + iy * nx;
                let g = iy + ix * ny; // XGRD(IY,IX)
                let (ingrid, is, js, rw) = gridsru::w3grmp(&gsi, m.xgrd[g], m.ygrd[g]);
                if !ingrid {
                    p.w(&format!(
                        "\n *** WAVEWATCH-III WARNING W3PRNC : \n     GRID POINT {}{}{}{}\n NOT COVERED BY INPUT GRID.\n\n",
                        fortfmt::i(6, (ix + 1) as i64),
                        fortfmt::i(6, (iy + 1) as i64),
                        fortfmt::f(7, 2, f64::from(m.xgrd[g])),
                        fortfmt::f(7, 2, f64::from(m.ygrd[g]))
                    ));
                    (ix21[k], ix22[k], iy21[k], iy22[k]) = (1, 1, 1, 1);
                    (rd11[k], rd21[k], rd12[k], rd22[k]) = (0., 0., 0., 0.);
                    continue;
                }
                ix21[k] = is[0];
                ix22[k] = is[1];
                iy21[k] = js[0];
                iy22[k] = js[3];
                rd11[k] = rw[0];
                rd21[k] = rw[1];
                rd12[k] = rw[3];
                rd22[k] = rw[2];
            }
        }
    }

    // 6.c Input location and format.
    p.w(&format!("\n           Data file :\n          ---------------------------------------\n           Input grid dim.   :{}   {}\n", fortfmt::i(9, nxj as i64), fortfmt::i(5, nyj as i64)));

    // 7.b Open and prepare the output file.
    p.w("\n       Opening output data file .....\n");
    let fname = fld_name(&idfld);
    let opath = format!("{dir}/{fname}");
    let mut ofile = match std::fs::File::create(&opath) {
        Ok(f) => f,
        Err(e) => {
            p.w(&format!("\n *** WAVEWATCH III ERROR IN W3FLDO : \n     ERROR IN OPENING {idfld} FILE, IOSTAT = {e}\n\n"));
            p.exit(30)
        }
    };
    // One unformatted record at a time, so a stop in the time loop leaves the records written so far, as the Fortran does.
    let mut rec = |p: &mut P, body: &[u8]| {
        let mut r = Vec::with_capacity(body.len() + 8);
        put(&mut r, body);
        if let Err(e) = ofile.write_all(&r) {
            p.w(&format!("\n *** WAVEWATCH III ERROR IN W3FLDG : \n     ERROR IN WRITING TO {idfld} FILE, IOSTAT = {e}\n\n"));
            p.exit(30)
        }
    };
    if flhdr {
        // WRITE (NDS) IDSTR, IDFLD, NX, NY, GTYPE, FILLER(1:2), TIDEFLAG
        let mut h = Vec::new();
        h.extend_from_slice(b"WAVEWATCH III");
        h.extend_from_slice(format!("{idfld:<3}").as_bytes());
        for v in [m.nx, m.ny, m.gtype, 0, 0, 0] {
            h.extend_from_slice(&v.to_be_bytes());
        }
        rec(&mut p, &h);
    }

    // 7.c Initialize fields.
    let (mxm, mym) = (nxj, nyj);
    let (mut fx, mut fy, mut fa) = (vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n]);
    let (mut a1, mut a2, mut a3) = (vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n]);
    let msz = mxm * mym;
    let (mut xc, mut yc, mut ac, mut xtemp) = (vec![0.0 as Real; msz], vec![0.0 as Real; msz], vec![0.0 as Real; msz], vec![0.0 as Real; msz]);

    // 8. Loop over the input fields.
    let (mut xcfac, mut ycfac, mut xcoff, mut ycoff): (Real, Real, Real, Real) = (1.0, 1.0, 0.0, 0.0);
    xcfac = nc::get_att_float(ncid, varidf[0], "scale_factor").unwrap_or(xcfac);
    xcoff = nc::get_att_float(ncid, varidf[0], "add_offset").unwrap_or(xcoff);
    let two = ifld >= 3 && ifld != 7 || flberg;
    if two {
        ycfac = nc::get_att_float(ncid, varidf[1], "scale_factor").unwrap_or(ycfac);
        ycoff = nc::get_att_float(ncid, varidf[1], "add_offset").unwrap_or(ycoff);
    }
    p.w("\n\n  Processing data\n --------------------------------------------------\n");
    let mut timedelay: Real = 0.0;
    let r = nc::inq_varid(ncid, "time").or_else(|_| nc::inq_varid(ncid, "MT"));
    let vartime = p.chk(r, 1808);
    for itime in 1..=nti {
        // 8.a Read new time and fields.
        let mut curjulday = get_f64(&mut p, ncid, vartime, 1, &[itime], &[1], 1810)[0];
        if index(&timeunits, "seconds") != 0 {
            curjulday /= 86400.;
        }
        if index(&timeunits, "minutes") != 0 {
            curjulday /= 1440.;
        }
        if index(&timeunits, "hours") != 0 {
            curjulday /= 24.;
        }
        curjulday += refjulday;
        if startjulday > curjulday {
            continue;
        }
        if stpjulday < curjulday {
            break;
        }
        let mut time = d2t(&j2d(curjulday));
        let idtime = stme21(time);
        if !fltime && timedelay == 0.0 {
            timedelay = dsec21(time, timeshift);
        }
        if timedelay != 0.0 {
            tick21(&mut time, timedelay);
            let idtime2 = stme21(time);
            p.w(&format!("   Shifted Time : {idtime2}    (File time : {idtime})\n"));
        } else {
            p.w(&format!("           Time : {idtime}\n"));
        }

        // Input.
        let read_field = |p: &mut P, vid: i32, a: &mut Vec<Real>, line: u32| {
            let v = if ndimsgrid == 1 {
                get_f32(p, ncid, vid, ndimsvar, &[1, itime], &[mxm, 1], line)
            } else if ndimsvar == 3 {
                get_f32(p, ncid, vid, ndimsvar, &[1, 1, itime], &[mxm, mym, 1], line)
            } else {
                get_f32(p, ncid, vid, ndimsvar, &[1, 1, 1, itime], &[mxm, mym, 1, 1], line)
            };
            // The Fortran reads into XC(:,1) for one grid dimension, the rest of the array stays as it was.
            let len = v.len();
            a[..len].copy_from_slice(&v);
        };
        read_field(&mut p, varidf[0], &mut xc, 1859);
        for x in xc.iter_mut() {
            if x.is_nan() {
                *x = fillvalue;
            }
            if *x != fillvalue {
                *x = *x * xcfac + xcoff;
            }
        }
        if two {
            read_field(&mut p, varidf[1], &mut yc, 1895);
            for x in yc.iter_mut() {
                if x.is_nan() {
                    *x = fillvalue;
                }
                if *x != fillvalue {
                    *x = *x * ycfac + ycoff;
                }
            }
            if flstab {
                read_field(&mut p, varidf[2], &mut ac, 1927);
            }
        }

        // 8.b Interpolate fields.
        if itype == 1 {
            for iy in 0..ny {
                for ix in 0..nx {
                    let k = ix + iy * mxm;
                    if xc[k] == fillvalue {
                        xc[k] = 0.;
                    }
                    if yc[k] == fillvalue {
                        yc[k] = 0.;
                    }
                }
            }
            if (ifld <= 2 || ifld == 7) && !flberg {
                for iy in 0..ny {
                    for ix in 0..nx {
                        fa[ix + iy * nx] = xc[ix + iy * mxm];
                    }
                }
            } else {
                for iy in 0..ny {
                    for ix in 0..nx {
                        fx[ix + iy * nx] = xc[ix + iy * mxm];
                        fy[ix + iy * nx] = yc[ix + iy * mxm];
                        fa[ix + iy * nx] = ac[ix + iy * mxm];
                    }
                }
            }
        } else if (ifld <= 2 || ifld == 7) && !flberg {
            interp(nx, ny, mxm, &xc, &ix21, &ix22, &iy21, &iy22, &rd11, &rd12, &rd21, &rd22, fillvalue, &mut fa);
        } else {
            interp(nx, ny, mxm, &xc, &ix21, &ix22, &iy21, &iy22, &rd11, &rd12, &rd21, &rd22, fillvalue, &mut fx);
            interp(nx, ny, mxm, &yc, &ix21, &ix22, &iy21, &iy22, &rd11, &rd12, &rd21, &rd22, fillvalue, &mut fy);
            if flstab {
                interp(nx, ny, mxm, &ac, &ix21, &ix22, &iy21, &iy22, &rd11, &rd12, &rd21, &rd22, fillvalue, &mut fa);
            }
            for k in 0..msz {
                xtemp[k] = if xc[k] != fillvalue && yc[k] != fillvalue { xc[k] * xc[k] + yc[k] * yc[k] } else { fillvalue };
            }
            interp(nx, ny, mxm, &xtemp, &ix21, &ix22, &iy21, &iy22, &rd11, &rd12, &rd21, &rd22, fillvalue, &mut a3);
            for x in xtemp.iter_mut() {
                if *x != fillvalue {
                    *x = x.sqrt();
                }
            }
            interp(nx, ny, mxm, &xtemp, &ix21, &ix22, &iy21, &iy22, &rd11, &rd12, &rd21, &rd22, fillvalue, &mut a2);
            for k in 0..n {
                a1[k] = (fx[k] * fx[k] + fy[k] * fy[k]).sqrt().max(1.0e-10);
                a3[k] = a3[k].sqrt();
            }
            // WNT1 for winds and momentum, CRT1 for currents: velocity-conserving correction.
            if ifld == 3 || ifld == 4 || ifld == 6 {
                for k in 0..n {
                    let f = 1.5f32.min(a2[k] / a1[k]);
                    fx[k] *= f;
                    fy[k] *= f;
                }
            }
        }

        // 8.c Write fields (W3FLDG 'WRITE').
        let fl2d = matches!(idfld.as_str(), "CUR" | "WND" | "WNS" | "ISI" | "TAU");
        rec(&mut p, &time.iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<u8>>());
        if !fl2d {
            rec(&mut p, &field_record(&fa));
        } else {
            rec(&mut p, &field_record(&fx));
            rec(&mut p, &field_record(&fy));
            if flstab {
                rec(&mut p, &field_record(&fa));
            }
        }
    }
    nc::close(ncid);
    p.w("\n\n  End of program \n =========================================\n         WAVEWATCH III Input preprocessing \n\n");
    p.exit(0)
}
