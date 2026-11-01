// SPDX-License-Identifier: MPL-2.0
//! `ww3_prep` (`model/src/ww3_prep.F90` at `92e2cf66`) for `switch_chaceon_shrd`: the input pre-processor.
//! Every input type of the program: `AI` (the field is already on the model grid), `LL` (a regular
//! longitude-latitude grid), `F1` and `F2` (one or two grids from files, with a land mask) and `DAT`
//! (assimilation data), for every field id (`IC1`..`IC5`, `MDN`, `MTH`, `MVS`, `ICE`, `ISI`, `LEV`, `WND`,
//! `WNS`, `CUR`, `TAU`, `RHO`), with free format (`IDFM` 1), formatted (`IDFM` 2, with format reversal) and
//! binary (`IDFM` 3) input. What the Fortran accepts and this does not is refused with a message and exit
//! status 64: a null value, a slash or a repeat count without a value in a list-directed read, an edit
//! descriptor other than `I`, `F`, `E`, `D` and `nX`, a comma inside a numeric field, a binary record shorter
//! than the values asked of it, and the forms named where `refuse` is called. The output channels `O3`,
//! `O15` and `T*` of the Fortran are off in this switch, and the log it prints on unit 6 is replaced by one
//! summary line; the Fortran log is not compared.

use crate::grid;
use crate::gridsru::{self, Gsu};
use crate::moddef::ModDef;
use ww3_kernels::real::Real;

/// `EXTCDE ( code )` after the message the Fortran writes on unit 6 or 0.
fn die(code: i32, msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(code)
}

/// Refused: a case the Fortran runs and this program does not.
fn refuse(what: &str) -> ! {
    die(64, &format!("ww3_prep: {what} is not implemented"))
}

/// One list-directed item.
struct Item {
    text: String,
}

/// What a list-directed or formatted read can meet besides a value.
enum Stop {
    Eof,
    Bad(String),
    /// `LIBERROR_SHORT_RECORD`: an unformatted record shorter than the values read from it.
    Short,
    /// `LIBERROR_FORMAT`: an item of the wrong type for its edit descriptor.
    Type(String),
}

/// One record of items of a list-directed `READ (nds,*)`: blank, tab and comma separate, a quoted string is
/// one item, `r*c` repeats `c` when `repeat` is set.
fn split_line(line: &str, repeat: bool) -> Result<Vec<Item>, String> {
    let b: Vec<char> = line.chars().collect();
    let (mut i, mut out) = (0, Vec::new());
    loop {
        while i < b.len() && (b[i] == ' ' || b[i] == '\t') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i] == ',' {
            refuse("a null value in a list-directed read");
        }
        if b[i] == '/' {
            refuse("a slash in a list-directed read");
        }
        if b[i] == '\'' || b[i] == '"' {
            let q = b[i];
            i += 1;
            let mut s = String::new();
            loop {
                if i >= b.len() {
                    refuse("a string that continues on the next record");
                }
                if b[i] == q {
                    if i + 1 < b.len() && b[i + 1] == q {
                        s.push(q);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(b[i]);
                i += 1;
            }
            out.push(Item { text: s });
        } else {
            let mut s = String::new();
            while i < b.len() && !matches!(b[i], ' ' | '\t' | ',') {
                s.push(b[i]);
                i += 1;
            }
            match s.split_once('*') {
                Some((r, c)) if repeat => {
                    let r: usize = r.parse().map_err(|_| format!("{s:?} is not a repeat count"))?;
                    if c.is_empty() {
                        refuse("a null value in a list-directed read");
                    }
                    for _ in 0..r {
                        out.push(Item { text: c.to_string() });
                    }
                }
                Some(_) => refuse("a repeat count on this item of a list-directed read"),
                None => out.push(Item { text: s }),
            }
        }
        while i < b.len() && (b[i] == ' ' || b[i] == '\t') {
            i += 1;
        }
        if i < b.len() && b[i] == ',' {
            i += 1;
        }
    }
    Ok(out)
}

/// A sequential formatted file, read by records: `ww3_prep.inp` (unit `NDSI`) and the input data files.
struct Stream {
    lines: Vec<String>,
    at: usize,
}

impl Stream {
    fn open(path: &str) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        Some(Self { lines: text.lines().map(str::to_string).collect(), at: 0 })
    }

    /// `NEXTLN`: skip comment and blank records, stay on the next one.
    fn nextln(&mut self, comstr: char) {
        loop {
            let Some(line) = self.lines.get(self.at) else { die(1, "\n *** WAVEWATCH III ERROR IN NEXTLN : \n     PREMATURE END OF INPUT FILE\n") };
            let t = line.trim_start_matches(' ');
            if t.starts_with(comstr) || t.trim().is_empty() {
                self.at += 1;
            } else {
                return;
            }
        }
    }

    /// `READ (nds,*) item, ...` of `n` items: items run on to the next records until `n` are read, and the rest
    /// of the last record is dropped.
    fn list(&mut self, n: usize, repeat: bool) -> Result<Vec<Item>, Stop> {
        let mut v: Vec<Item> = Vec::with_capacity(n);
        while v.len() < n {
            let Some(line) = self.lines.get(self.at) else { return Err(Stop::Eof) };
            self.at += 1;
            for it in split_line(line, repeat).map_err(Stop::Bad)? {
                if v.len() < n {
                    v.push(it);
                }
            }
        }
        Ok(v)
    }

    fn record(&mut self) -> Result<String, Stop> {
        let l = self.lines.get(self.at).cloned().ok_or(Stop::Eof)?;
        self.at += 1;
        Ok(l)
    }
}

/// A sequential unformatted file (`FORM='UNFORMATTED'`, `CONVERT=file_endian`, big-endian): records of a
/// length marker, the bytes, the marker again.
struct Bin {
    bytes: Vec<u8>,
    at: usize,
}

impl Bin {
    fn open(path: &str) -> Option<Self> {
        Some(Self { bytes: std::fs::read(path).ok()?, at: 0 })
    }

    /// The next record. End of file before a marker is `END=`; a marker the data do not match is an error.
    fn record(&mut self) -> Result<Vec<u8>, Stop> {
        if self.at >= self.bytes.len() {
            return Err(Stop::Eof);
        }
        let corrupt = || Stop::Bad("unformatted file structure has been corrupted".into());
        let marker = |at: usize, b: &[u8]| -> Result<usize, Stop> {
            let m = b.get(at..at + 4).ok_or_else(corrupt)?;
            Ok(u32::from_be_bytes([m[0], m[1], m[2], m[3]]) as usize)
        };
        let n = marker(self.at, &self.bytes)?;
        let body = self.bytes.get(self.at + 4..self.at + 4 + n).ok_or_else(corrupt)?.to_vec();
        if marker(self.at + 4 + n, &self.bytes)? != n {
            return Err(corrupt());
        }
        self.at += 8 + n;
        Ok(body)
    }
}

/// A connected unit: the text of `ww3_prep.inp` (unit `NDSI`, slot 0), a formatted data file or an
/// unformatted one.
enum Unit {
    Text(Stream),
    Bin(Bin),
}

/// `n` values from a binary record, `REAL(4)` or `INTEGER(4)`; a shorter record is `LIBERROR_SHORT_RECORD`.
fn bin_vals(rec: &[u8], n: usize, real: bool) -> Result<Vec<f64>, Stop> {
    if rec.len() < 4 * n {
        return Err(Stop::Short);
    }
    Ok(rec
        .as_chunks::<4>()
        .0
        .iter()
        .take(n)
        .map(|&w| if real { f32::from_be_bytes(w) as f64 } else { i32::from_be_bytes(w) as f64 })
        .collect())
}

fn to_int(t: &str) -> Result<i32, Stop> {
    t.trim().parse::<i32>().map_err(|_| Stop::Bad(format!("{t:?} is not an integer")))
}

fn to_real(t: &str) -> Result<Real, Stop> {
    // The Fortran runtime accepts a D exponent; Infinity and NaN are not used by any input here.
    let t = t.trim().replace(['d', 'D'], "e");
    if t.chars().any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E') {
        return Err(Stop::Bad(format!("{t:?} is not a real")));
    }
    t.parse::<Real>().map_err(|_| Stop::Bad(format!("{t:?} is not a real")))
}

fn to_logical(t: &str) -> Result<bool, Stop> {
    match t.trim().trim_start_matches('.').to_ascii_uppercase().chars().next() {
        Some('T') => Ok(true),
        Some('F') => Ok(false),
        _ => Err(Stop::Bad(format!("{t:?} is not a logical"))),
    }
}

/// An edit descriptor of an input format, `(I10,1x,I10)` or `(1000(F5.2))`: `I` `F` `E` and `D` read a field
/// of `w` columns, `X` skips.
#[derive(Clone, Copy)]
enum Ed {
    I(usize),
    F(usize, usize),
    X(usize),
}

/// A format as the list of descriptors it expands to, and where format reversal restarts: the rightmost group
/// of the outermost parentheses with its repeat count, or the start when there is none (Fortran 2003, 10.4).
struct Fmt {
    eds: Vec<Ed>,
    rev: usize,
}

fn parse_group(b: &[char], i: &mut usize, out: &mut Vec<Ed>, fmt: &str, rev: &mut Option<usize>) {
    loop {
        while *i < b.len() && b[*i] == ' ' {
            *i += 1;
        }
        let mut r = 0usize;
        let mut has_r = false;
        while *i < b.len() && b[*i].is_ascii_digit() {
            r = r * 10 + b[*i].to_digit(10).unwrap() as usize;
            has_r = true;
            *i += 1;
        }
        let rep = if has_r { r } else { 1 };
        if *i >= b.len() {
            refuse(&format!("the format {fmt:?}"));
        }
        let c = b[*i].to_ascii_uppercase();
        if c == '(' {
            if let Some(r) = rev.as_mut() {
                *r = out.len();
            }
            *i += 1;
            let mut g = Vec::new();
            parse_group(b, i, &mut g, fmt, &mut None);
            for _ in 0..rep {
                out.extend_from_slice(&g);
            }
        } else if c == 'X' {
            *i += 1;
            if !has_r {
                refuse(&format!("the format {fmt:?}"));
            }
            out.push(Ed::X(r));
        } else if matches!(c, 'I' | 'F' | 'E' | 'D') {
            *i += 1;
            let num = |i: &mut usize| {
                let mut n = 0usize;
                let mut any = false;
                while *i < b.len() && b[*i].is_ascii_digit() {
                    n = n * 10 + b[*i].to_digit(10).unwrap() as usize;
                    any = true;
                    *i += 1;
                }
                any.then_some(n)
            };
            let Some(w) = num(i) else { refuse(&format!("the format {fmt:?}")) };
            let mut d = 0;
            if *i < b.len() && b[*i] == '.' {
                *i += 1;
                d = num(i).unwrap_or_else(|| refuse(&format!("the format {fmt:?}")));
            }
            if c != 'I' && c != 'F' && *i < b.len() && b[*i].eq_ignore_ascii_case(&'E') {
                // The exponent width of Ew.dEe is not used on input.
                *i += 1;
                num(i);
            }
            if w == 0 {
                refuse(&format!("the format {fmt:?}"));
            }
            for _ in 0..rep {
                out.push(if c == 'I' { Ed::I(w) } else { Ed::F(w, d) });
            }
        } else {
            refuse(&format!("the format {fmt:?}"));
        }
        while *i < b.len() && b[*i] == ' ' {
            *i += 1;
        }
        match b.get(*i) {
            Some(',') => *i += 1,
            Some(')') => {
                *i += 1;
                return;
            }
            _ => refuse(&format!("the format {fmt:?}")),
        }
    }
}

fn parse_format(fmt: &str) -> Fmt {
    let b: Vec<char> = fmt.trim().chars().collect();
    if b.first() != Some(&'(') {
        refuse(&format!("the format {fmt:?}"));
    }
    let (mut i, mut eds, mut rev) = (1, Vec::new(), Some(0));
    parse_group(&b, &mut i, &mut eds, fmt, &mut rev);
    if !eds.iter().any(|e| !matches!(e, Ed::X(_))) {
        refuse(&format!("the format {fmt:?} (no data descriptor)"));
    }
    let rev = rev.unwrap_or(0);
    if !eds[rev..].iter().any(|e| !matches!(e, Ed::X(_))) {
        refuse(&format!("the format {fmt:?} (no data descriptor to revert to)"));
    }
    Fmt { eds, rev }
}

/// An `Iw` field: blanks are ignored, an empty field is 0.
fn field_int(s: &str) -> Result<i32, Stop> {
    let t: String = s.chars().filter(|c| *c != ' ').collect();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i32>().map_err(|_| Stop::Bad(format!("{s:?} is not an integer field")))
}

/// An `Fw.d` field (and `Ew.d`, `Dw.d`, which the runtime reads the same way): blanks ignored, an empty field
/// is 0, without a decimal point the last `d` digits are the fraction, and the string goes to a correctly
/// rounded `strtof`, which `str::parse` is too.
fn field_real(s: &str, d: usize) -> Result<Real, Stop> {
    let bad = || Stop::Bad(format!("{s:?} is not a real field"));
    let t: String = s.chars().filter(|c| *c != ' ').collect();
    if t.is_empty() {
        return Ok(0.0);
    }
    let (sign, rest) = match t.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", t.strip_prefix('+').unwrap_or(&t)),
    };
    let cut = rest.find(['e', 'E', 'd', 'D', '+', '-']);
    let (man, exp) = match cut {
        Some(k) => {
            let e = rest[k..].trim_start_matches(['e', 'E', 'd', 'D']);
            (&rest[..k], e)
        }
        None => (rest, ""),
    };
    if exp.is_empty() && cut.is_some_and(|k| rest[k..].len() == 1 && !rest[k..].starts_with(['+', '-'])) {
        return Err(bad());
    }
    let (ip, fp) = match man.split_once('.') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => {
            if man.len() > d {
                (man[..man.len() - d].to_string(), man[man.len() - d..].to_string())
            } else {
                ("0".to_string(), format!("{}{}", "0".repeat(d - man.len()), man))
            }
        }
    };
    if !ip.chars().chain(fp.chars()).all(|c| c.is_ascii_digit()) || (ip.is_empty() && fp.is_empty()) {
        return Err(bad());
    }
    let ex = if exp.is_empty() { "0" } else { exp };
    format!("{sign}{}.{}e{ex}", if ip.is_empty() { "0" } else { &ip }, if fp.is_empty() { "0" } else { &fp })
        .parse::<Real>()
        .map_err(|_| bad())
}

/// A formatted `READ (nds,fmt) list` of `n` values: one record is read, and when the format has run out of
/// descriptors with values left the next record is read and the format restarts at the reversion point. The
/// rest of the last record is dropped. A field past the end of a record is blank, which reads as zero.
fn formatted(s: &mut Stream, fmt: &Fmt, n: usize, real: bool) -> Result<Vec<f64>, Stop> {
    let (mut rec, mut pos, mut at): (Vec<char>, usize, usize) = (s.record()?.chars().collect(), 0, 0);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        if at == fmt.eds.len() {
            rec = s.record()?.chars().collect();
            pos = 0;
            at = fmt.rev;
        }
        // A comma in a fixed-width numeric field ends the field and is consumed with it: the next field starts
        // after the comma (v, `just prep-variants`, the variants `comma` and `commai`).
        let take = |pos: &mut usize, w: usize| -> String {
            let f: String = (0..w).map(|k| rec.get(*pos + k).copied().unwrap_or(' ')).collect();
            match f.find(',') {
                Some(k) => {
                    *pos += k + 1;
                    f[..k].to_string()
                }
                None => {
                    *pos += w;
                    f
                }
            }
        };
        match fmt.eds[at] {
            Ed::X(k) => pos += k,
            Ed::I(w) => {
                let f = take(&mut pos, w);
                if real {
                    return Err(Stop::Type("Expected REAL for item in formatted transfer, got INTEGER".into()));
                }
                out.push(field_int(&f)? as f64);
            }
            Ed::F(w, d) => {
                let f = take(&mut pos, w);
                if !real {
                    return Err(Stop::Type("Expected INTEGER for item in formatted transfer, got REAL".into()));
                }
                out.push(field_real(&f, d)? as f64);
            }
        }
        at += 1;
    }
    Ok(out)
}

/// How the data of one array are read: `IDFM` 1 list-directed, 2 formatted, 3 unformatted.
struct Layout {
    idfm: i32,
    idla: i32,
    fmt: Option<Fmt>,
}

impl Layout {
    fn new(idfm: i32, idla: i32, form: &str) -> Self {
        Self { idfm, idla, fmt: (idfm == 2).then(|| parse_format(form)) }
    }
}

/// One `READ` statement of `n` values from a unit: `IDFM` 1 `(nds,*)`, 2 `(nds,fmt)`, 3 `(nds)`.
fn read_vals(u: &mut Unit, l: &Layout, n: usize, real: bool) -> Result<Vec<f64>, Stop> {
    match (u, l.idfm) {
        (Unit::Bin(b), 3) => bin_vals(&b.record()?, n, real),
        (Unit::Text(s), 1) => s.list(n, true)?.iter().map(|it| if real { to_real(&it.text).map(|v| v as f64) } else { to_int(&it.text).map(|v| v as f64) }).collect(),
        (Unit::Text(s), 2) => formatted(s, l.fmt.as_ref().unwrap(), n, real),
        _ => refuse("unformatted input from a formatted unit, or formatted input from an unformatted one"),
    }
}

/// The values `INA2R` and `INA2I` read into `(LX:HX,LY:HY)` = `(1:nx,1:ny)`, as `[iy*nx + ix]`: `IDLA` 1 and 3
/// are one `READ` a row (top row first, bottom row first), 2 and 4 one `READ` for the array.
fn read_array(u: &mut Unit, l: &Layout, nx: usize, ny: usize, real: bool) -> Result<Vec<f64>, Stop> {
    let mut a = vec![0.0f64; nx * ny];
    match l.idla {
        1 | 3 => {
            let rows: Vec<usize> = if l.idla == 1 { (0..ny).collect() } else { (0..ny).rev().collect() };
            for iy in rows {
                a[iy * nx..(iy + 1) * nx].copy_from_slice(&read_vals(u, l, nx, real)?);
            }
        }
        _ => {
            let r = read_vals(u, l, nx * ny, real)?;
            if l.idla == 2 {
                a.copy_from_slice(&r);
            } else {
                // ((A(IX,IY),IX=LX,HX),IY=HY,LY,-1): the first values read belong to row HY.
                for (k, iy) in (0..ny).rev().enumerate() {
                    a[iy * nx..(iy + 1) * nx].copy_from_slice(&r[k * nx..(k + 1) * nx]);
                }
            }
        }
    }
    Ok(a)
}

/// `LIBERROR_READ_VALUE`, the `IOSTAT` of a value the runtime cannot read, which `EXTCDE ( ISTAT )` exits with.
const IOSTAT_READ_VALUE: i32 = 5010;
/// `LIBERROR_SHORT_RECORD` and `LIBERROR_FORMAT`.
const IOSTAT_SHORT_RECORD: i32 = 5016;
const IOSTAT_FORMAT: i32 = 5006;

fn ina_stop(name: &str, e: Stop) -> ! {
    let (code, why) = match e {
        Stop::Eof => die(-1, &format!("\n *** ERROR {name} : \n     PREMATURE END OF FILE\n")),
        Stop::Bad(w) => (IOSTAT_READ_VALUE, w),
        Stop::Short => (IOSTAT_SHORT_RECORD, "Short record on unformatted read".to_string()),
        Stop::Type(w) => (IOSTAT_FORMAT, w),
    };
    die(code, &format!("\n *** ERROR {name} : \n     ERROR IN READING FROM FILE\n     IOSTAT = {code}  {why}\n"))
}

/// The text of a stop that is not an end of file, for a message with a fixed exit code.
fn why(e: Stop) -> String {
    match e {
        Stop::Eof => "IOSTAT = -1".into(),
        Stop::Bad(w) | Stop::Type(w) => w,
        Stop::Short => "Short record on unformatted read".into(),
    }
}

/// `INA2R` (`w3arrymd.F90:76-291`) into `a(ix + iy*mx)`, `VSC = 1` and `VOF = 0`.
fn ina2r(u: &mut Unit, a: &mut [Real], mx: usize, nx: usize, ny: usize, l: &Layout) {
    let r = read_array(u, l, nx, ny, true).unwrap_or_else(|e| ina_stop("INA2R", e));
    for iy in 0..ny {
        for ix in 0..nx {
            // ARRAY = VSC * ARRAY + VOF, with VSC = 1. and VOF = 0.: a negative zero becomes +0.
            a[iy * mx + ix] = 1. * (r[iy * nx + ix] as Real) + 0.;
        }
    }
}

/// `INA2I` (`w3arrymd.F90:293-461`), `VSC = 1`, `VOF = 0`.
fn ina2i(u: &mut Unit, nx: usize, ny: usize, l: &Layout) -> Vec<i32> {
    let r = read_array(u, l, nx, ny, false).unwrap_or_else(|e| ina_stop("INA2I", e));
    r.into_iter().map(|v| v as i32).collect()
}

/// A Fortran unformatted sequential record, big-endian.
fn put(out: &mut Vec<u8>, body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
}

fn field_record(f: &[Real]) -> Vec<u8> {
    f.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn str_of(it: &Item, len: usize) -> String {
    it.text.chars().take(len).collect::<String>().trim_end().to_string()
}

const NDSI: usize = 10;
/// `ILAND`, the overlay value of a land point.
const ILAND: i32 = -999;
const IDSTR: &[u8; 13] = b"WAVEWATCH III";

fn eof41() -> ! {
    die(41, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     PREMATURE END OF INPUT FILE\n")
}

fn bad42(why: &str) -> ! {
    die(42, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING FROM INPUT FILE\n     {why}\n"))
}

/// The text of `ww3_prep.inp`, slot 0 of the units.
fn inp(units: &mut [Unit]) -> &mut Stream {
    match &mut units[0] {
        Unit::Text(s) => s,
        Unit::Bin(_) => unreachable!(),
    }
}

/// `READ (NDSI,*,END=801,ERR=802) item, ...` of `n` items.
fn inp_list(units: &mut [Unit], n: usize) -> Vec<Item> {
    match inp(units).list(n, false) {
        Ok(v) => v,
        Err(Stop::Eof) => eof41(),
        Err(e) => bad42(&why(e)),
    }
}

fn int42(it: &Item, what: &str) -> i32 {
    to_int(&it.text).unwrap_or_else(|_| bad42(what))
}

/// `OPEN ( nds, FILE = FNMPRE//name, STATUS = 'OLD' )`, or the unnamed form, which gfortran connects to
/// `fort.<nds>`.
fn open_unit(dir: &str, byname: bool, name: &str, nds: i32, binary: bool) -> Option<Unit> {
    let path = if byname { format!("{dir}/{name}") } else { format!("{dir}/fort.{nds}") };
    if binary { Bin::open(&path).map(Unit::Bin) } else { Stream::open(&path).map(Unit::Text) }
}

/// The interpolation factors and addresses of one input grid, `RD11 RD21 RD12 RD22` and `IX1 IX2 IY1 IY2`,
/// as `[ix + iy*nx]` of the model grid.
struct Weights {
    rd11: Vec<Real>,
    rd21: Vec<Real>,
    rd12: Vec<Real>,
    rd22: Vec<Real>,
    ix1: Vec<i32>,
    ix2: Vec<i32>,
    iy1: Vec<i32>,
    iy2: Vec<i32>,
}

impl Weights {
    fn new(n: usize) -> Self {
        Self { rd11: vec![0.; n], rd21: vec![0.; n], rd12: vec![0.; n], rd22: vec![0.; n], ix1: vec![1; n], ix2: vec![1; n], iy1: vec![1; n], iy2: vec![1; n] }
    }
}

/// What `W3FLDP` and the interpolation did, for the summary line: the branches a run reached.
#[derive(Default)]
struct Reached {
    outside: usize,
    part: usize,
    full: usize,
    corrected: usize,
    single: usize,
    lost: usize,
    overlap: usize,
    ybound: usize,
    clamp: usize,
    pole: usize,
}

/// `W3FLDP` (`w3fldsmd.F90:1745-2136`) for the model grid `m`: bilinear weights from the input grid
/// `(ala, alo)` of `nxi` x `nyi`, whose mask `mask` (0 = land) takes land points out. `mapovr` counts the input
/// grids that cover each model point.
#[allow(clippy::too_many_arguments)]
fn fldp(m: &ModDef, mapovr: &mut [i32], nxi: usize, nyi: usize, closed: bool, ala: &[Real], alo: &[Real], mask: &[i32], w: &mut Weights, r: &mut Reached) {
    let (nx, ny) = (m.nx as usize, m.ny as usize);
    *w = Weights::new(nx * ny);
    let iclo = if m.flagll && closed { 2 } else { -1 };
    let lmsk: Vec<bool> = mask.iter().map(|&v| v == 0).collect();
    let gsu = Gsu::create(true, m.flagll, iclo, nxi as i32, nyi as i32, alo.iter().map(|&v| v as f64).collect(), ala.iter().map(|&v| v as f64).collect());
    for iy in 0..ny {
        for ix in 0..nx {
            let k = ix + iy * nx;
            if mapovr[k] == ILAND {
                continue;
            }
            let g = gridsru::w3grmp_mask(&gsu, m.xgrd[iy + ix * ny], m.ygrd[iy + ix * ny], Some(&lmsk), Some(2));
            if !g.ingrid {
                r.outside += 1;
                continue;
            }
            r.pole += usize::from(g.near_pole);
            if g.mskc != 2 {
                if g.mskc == 1 {
                    r.part += 1;
                }
                // Counter-clockwise order to column-major.
                (w.ix1[k], w.ix2[k], w.iy1[k], w.iy2[k]) = (g.is[0], g.is[1], g.js[0], g.js[3]);
                (w.rd11[k], w.rd21[k], w.rd12[k], w.rd22[k]) = (g.rw[0], g.rw[1], g.rw[3], g.rw[2]);
            } else {
                r.part += 1;
                if g.nnbr > 0 {
                    r.corrected += 1;
                    if g.nnbr == 1 {
                        r.single += 1;
                    }
                    w.ix1[k] = g.is[0];
                    w.iy1[k] = g.js[0];
                    w.rd11[k] = g.rw[0];
                    if g.nnbr > 1 {
                        // The Fortran overwrites the first address with the second, and keeps IX2 and IY2 at 1.
                        w.ix1[k] = g.is[1];
                        w.iy1[k] = g.js[1];
                        w.rd22[k] = g.rw[1];
                    }
                } else {
                    r.lost += 1;
                }
                r.full += 1;
            }
            mapovr[k] += 1;
        }
    }
}

/// `READ (nds) TIME` or `READ (nds,*) TIME` or `READ (nds,fmt) TIME`.
fn read_time(u: &mut Unit, idfm: i32, tfmt: Option<&Fmt>) -> Result<[i32; 2], Stop> {
    match (u, idfm) {
        (Unit::Bin(b), 3) => {
            let v = bin_vals(&b.record()?, 2, false)?;
            Ok([v[0] as i32, v[1] as i32])
        }
        (Unit::Text(s), 1) => {
            let v = s.list(2, false)?;
            Ok([to_int(&v[0].text)?, to_int(&v[1].text)?])
        }
        (Unit::Text(s), 2) => {
            let v = formatted(s, tfmt.unwrap(), 2, false)?;
            Ok([v[0] as i32, v[1] as i32])
        }
        _ => refuse("unformatted input from a formatted unit, or formatted input from an unformatted one"),
    }
}

/// The two info lines of a lat-lon or a mask file (`FROMLL, IDLALL, IDFMLL, FORMLL`, then `NDSLL, NAMELL`) and
/// the file they name. `file` is `None` when the data follow in `ww3_prep.inp` (`NDSLL` = `NDSI`).
struct Aux {
    lay: Layout,
    file: Option<Unit>,
}

fn open_aux(units: &mut [Unit], dir: &str, comstr: char, code: i32) -> Aux {
    inp(units).nextln(comstr);
    let it = inp_list(units, 4);
    let fromll = str_of(&it[0], 4);
    let idlall = int42(&it[1], "IDLALL");
    let idfmll = int42(&it[2], "IDFMLL");
    let idlall = if (1..=4).contains(&idlall) { idlall } else { 1 };
    let idfmll = if (1..=3).contains(&idfmll) { idfmll } else { 1 };
    let formll = str_of(&it[3], 16);
    inp(units).nextln(comstr);
    let it = inp_list(units, 2);
    let ndsll = int42(&it[0], "NDSLL");
    let namell = str_of(&it[1], 65);
    let lay = Layout::new(idfmll, idlall, &formll);
    if ndsll as usize == NDSI {
        inp(units).nextln(comstr);
        return Aux { lay, file: None };
    }
    let u = open_unit(dir, fromll == "NAME", &namell, ndsll, idfmll == 3)
        .unwrap_or_else(|| die(code, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN OPENING LAT-LONG OR MASK FILE\n     NAMELL = {namell}\n")));
    Aux { lay, file: Some(u) }
}

/// Reads `mod_def.ww3` and `ww3_prep.inp` from `dir`, writes the field file (`current.ww3`, `wind.ww3`, ...)
/// there. Names in the input file are relative to `dir`, as the Fortran's `FNMPRE = './'` makes them.
pub fn run(dir: &str) {
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    let mut stream = match Stream::open(&format!("{dir}/ww3_prep.inp")) {
        Some(s) => s,
        None => die(40, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN OPENING INPUT FILE\n"),
    };
    // READ (NDSI,'(A)') COMSTR; IF (COMSTR.EQ.' ') COMSTR = '$'
    let Some(first) = stream.lines.first() else { eof41() };
    let comstr = match first.chars().next() {
        None | Some(' ') => '$',
        Some(c) => c,
    };
    stream.at = 1;
    let mut units: Vec<Unit> = vec![Unit::Text(stream)];
    let (nx, ny) = (m.nx as usize, m.ny as usize);

    // 3.a Read types from the input file.
    inp(&mut units).nextln(comstr);
    let it = inp_list(&mut units, 4);
    let idfld_in = str_of(&it[0], 3);
    let idtype = str_of(&it[1], 2);
    let fltime = to_logical(&it[2].text).unwrap_or_else(|_| bad42("FLTIME"));
    let flhdr = to_logical(&it[3].text).unwrap_or_else(|_| bad42("FLHDR"));

    // 3.b Check types.
    let flstab = idfld_in == "WNS";
    let flberg = idfld_in == "ISI";
    let ifld: i32 = match idfld_in.as_str() {
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
        _ => die(1, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL FIELD ID -->{idfld_in}<--\n")),
    };
    let mut nfcomp = 1usize;
    let itype = if idfld_in == "DAT" {
        5
    } else {
        match idtype.as_str() {
            "AI" => 1,
            "LL" => 2,
            "F1" => 3,
            "F2" => {
                nfcomp = 2;
                4
            }
            _ => die(2, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL FORMAT ID -->{idtype}<--\n")),
        }
    };
    if itype == 4 && ifld > 2 {
        die(3, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     THIS FORMAT TYPE IS ALLOWED FOR ICE AND LEV ONLY\n");
    }

    // 3.c Additional input for format types and time.
    let mut time = [0i32; 2];
    if !fltime {
        inp(&mut units).nextln(comstr);
        let it = inp_list(&mut units, 2);
        for k in 0..2 {
            time[k] = int42(&it[k], "TIME");
        }
        if time[0] < 10000000 {
            die(4, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL TIME : {:08}{:07}\n", time[0], time[1]));
        }
    }
    // NXI and NYI are NXJ(1) and NYJ(1) (EQUIVALENCE); NXJ(2) and NYJ(2) follow in 4.c.
    let mut nxj = [nx, nx];
    let mut nyj = [ny, ny];
    let (mut x0i, mut xni, mut y0i, mut yni) = (0. as Real, 0. as Real, 0. as Real, 0. as Real);
    let mut recldt = 0usize;
    let mut idfld = idfld_in.clone();
    match itype {
        2 => {
            inp(&mut units).nextln(comstr);
            let it = inp_list(&mut units, 6);
            let real = |it: &Item, w: &str| to_real(&it.text).unwrap_or_else(|_| bad42(w));
            (x0i, xni, y0i, yni) = (real(&it[0], "X0I"), real(&it[1], "XNI"), real(&it[3], "Y0I"), real(&it[4], "YNI"));
            let (n1, n2) = (int42(&it[2], "NXI"), int42(&it[5], "NYI"));
            if n1 < 2 || n2 < 2 {
                die(5, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL SIZE OF INPUT GRID : {n1:5} {n2:5}\n"));
            }
            (nxj[0], nyj[0]) = (n1 as usize, n2 as usize);
        }
        5 => {
            inp(&mut units).nextln(comstr);
            let it = inp_list(&mut units, 3);
            let dattyp = int42(&it[0], "DATTYP");
            let rl = int42(&it[1], "RECLDT");
            to_real(&it[2].text).unwrap_or_else(|_| bad42("NODATA"));
            if !(0..=2).contains(&dattyp) {
                die(6, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL DATA RECORD LENGTH : {dattyp:6}\n"));
            }
            if rl <= 0 {
                die(7, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL DATA TYPE : {rl:2}\n"));
            }
            recldt = rl as usize;
            idfld = format!("DT{dattyp}");
        }
        _ => {}
    }

    // 4 Prepare interpolation.
    let n = nx * ny;
    let mut reached = Reached::default();
    let (sta, _) = grid::unpack(&m.maptmp);
    let mut mapovr = vec![0i32; n];
    let mut rd = Weights::new(n);
    let mut xd = Weights::new(n);
    if itype == 2 {
        // 4.a Longitude - latitude grid.
        let (nxi, nyi) = (nxj[0], nyj[0]);
        let sxi = (xni - x0i) / (nxi - 1) as Real;
        let syi = (yni - y0i) / (nyi - 1) as Real;
        let mut iclo = -1;
        if m.flagll && ((nxi as Real * sxi).abs() - 360.).abs() < 0.1 * sxi.abs() {
            iclo = 2;
        }
        let mut alo = vec![0 as Real; nxi * nyi];
        let mut ala = vec![0 as Real; nxi * nyi];
        for iy in 0..nyi {
            for ix in 0..nxi {
                alo[ix + iy * nxi] = x0i + ix as Real * sxi;
                ala[ix + iy * nxi] = y0i + iy as Real * syi;
            }
        }
        let gsi = Gsu::create(true, m.flagll, iclo, nxi as i32, nyi as i32, alo.iter().map(|&v| v as f64).collect(), ala.iter().map(|&v| v as f64).collect());
        for iy in 0..ny {
            for ix in 0..nx {
                let k = ix + iy * nx;
                let g = gridsru::w3grmp_mask(&gsi, m.xgrd[iy + ix * ny], m.ygrd[iy + ix * ny], None, None);
                if !g.ingrid {
                    // The Fortran warns, and leaves the addresses it will index with at 1 and the factors at 0.
                    reached.outside += 1;
                    continue;
                }
                reached.pole += usize::from(g.near_pole);
                (rd.ix1[k], rd.ix2[k], rd.iy1[k], rd.iy2[k]) = (g.is[0], g.is[1], g.js[0], g.js[3]);
                (rd.rd11[k], rd.rd21[k], rd.rd12[k], rd.rd22[k]) = (g.rw[0], g.rw[1], g.rw[3], g.rw[2]);
            }
        }
    } else if itype == 3 || itype == 4 {
        // 4.b Grid(s) from file.
        for iy in 0..ny {
            for ix in 0..nx {
                mapovr[ix + iy * nx] = if sta[iy + ix * ny] == 0 { ILAND } else { 0 };
            }
        }
        for j in 0..nfcomp {
            inp(&mut units).nextln(comstr);
            let it = inp_list(&mut units, 3);
            let (a, b) = (int42(&it[0], "NXJ"), int42(&it[1], "NYJ"));
            let closed = to_logical(&it[2].text).unwrap_or_else(|_| bad42("CLO"));
            if a < 2 || b < 2 {
                die(10, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL SIZE OF INPUT GRID : {a:5} {b:5}\n"));
            }
            (nxj[j], nyj[j]) = (a as usize, b as usize);
            let (nxi, nyi) = (nxj[j], nyj[j]);
            let mut ala = vec![0 as Real; nxi * nyi];
            let mut alo = vec![0 as Real; nxi * nyi];
            let mut ll = open_aux(&mut units, dir, comstr, 47);
            {
                let u = match ll.file.as_mut() {
                    Some(u) => u,
                    None => &mut units[0],
                };
                ina2r(u, &mut ala, nxi, nxi, nyi, &ll.lay);
                ina2r(u, &mut alo, nxi, nxi, nyi, &ll.lay);
            }
            let mut mk = open_aux(&mut units, dir, comstr, 48);
            let mask = {
                let u = match mk.file.as_mut() {
                    Some(u) => u,
                    None => &mut units[0],
                };
                ina2i(u, nxi, nyi, &mk.lay)
            };
            fldp(&m, &mut mapovr, nxi, nyi, closed, &ala, &alo, &mask, if j == 0 { &mut rd } else { &mut xd }, &mut reached);
        }
        // Average two fields.
        if nfcomp == 2 {
            for ix in 0..nx {
                for iy in 0..ny {
                    let k = ix + iy * nx;
                    if mapovr[k] >= 2 {
                        reached.overlap += 1;
                        let factor = 1. / mapovr[k] as Real;
                        rd.rd11[k] *= factor;
                        rd.rd12[k] *= factor;
                        rd.rd21[k] *= factor;
                        rd.rd22[k] *= factor;
                        xd.rd11[k] *= factor;
                        xd.rd12[k] *= factor;
                        xd.rd21[k] *= factor;
                        xd.rd22[k] *= factor;
                    }
                }
            }
        }
    }

    // 4.c Input location and format.
    let mut layout: Vec<Layout> = Vec::new();
    let mut tfmt: Vec<Option<Fmt>> = Vec::new();
    let mut ui: Vec<usize> = Vec::new();
    let mut ndsf: Vec<i32> = Vec::new();
    let (mut fromf_last, mut namef_last) = (String::new(), String::new());
    for j in 0..nfcomp {
        inp(&mut units).nextln(comstr);
        let it = inp_list(&mut units, 5);
        let fromf = str_of(&it[0], 4);
        let idlaf = int42(&it[1], "IDLAF");
        let idfmf = int42(&it[2], "IDFMF");
        let idlaf = if (1..=4).contains(&idlaf) { idlaf } else { 1 };
        let idfmf = if (1..=3).contains(&idfmf) { idfmf } else { 1 };
        let (formt, formf) = (str_of(&it[3], 32), str_of(&it[4], 32));
        inp(&mut units).nextln(comstr);
        let it = inp_list(&mut units, 2);
        let nds = int42(&it[0], "NDSF");
        let namef = str_of(&it[1], 65);
        // The data format is not used by DAT, which reads its records with the time format.
        layout.push(if itype == 5 { Layout { idfm: idfmf, idla: idlaf, fmt: None } } else { Layout::new(idfmf, idlaf, &formf) });
        tfmt.push((idfmf == 2 && (fltime || itype == 5)).then(|| parse_format(&formt)));
        ndsf.push(nds);
        fromf_last = fromf.clone();
        namef_last = namef.clone();

        // 5.a Open the input file.
        if idfmf == 3 {
            if nds as usize == NDSI {
                die(20, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     CANNOT READ UNFORMATTED FROM UNIT{NDSI:3}\n"));
            }
            let mut b = match Bin::open(&if fromf == "NAME" { format!("{dir}/{namef}") } else { format!("{dir}/fort.{nds}") }) {
                Some(b) => b,
                None => die(49, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN OPENING INPUT DATA FILE\n     NDSF   ={nds:5}\n     NAMEF  = {namef}\n")),
            };
            // A check that the file is not a WAVEWATCH III file of another field or grid: READ (NDSF) TSTR,
            // TSFLD, NXT, NYT. An empty file ends the program (END=888); a record too short or another
            // string rewinds the file.
            match b.record() {
                Err(Stop::Eof) => {
                    println!("ww3_prep: {idfld} {idtype}: the input file is empty, no output written");
                    return;
                }
                Ok(rec) if rec.len() >= 24 && &rec[..13] == IDSTR => {
                    let tsfld = String::from_utf8_lossy(&rec[13..16]).to_string();
                    let (nxt, nyt) = (i32::from_be_bytes([rec[16], rec[17], rec[18], rec[19]]), i32::from_be_bytes([rec[20], rec[21], rec[22], rec[23]]));
                    if itype == 5 {
                        refuse("a WAVEWATCH III header on a DAT input file (the Fortran compares it with NXI and NYI, which DAT never sets)");
                    }
                    if tsfld != idfld || nxt as usize != nxj[0] || nyt as usize != nyj[0] {
                        die(21, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING FROM INPUT DATA FILE\n     IN FILE , VARIABLE ID = {tsfld}\n     ARRAY DIMENSION = {nxt:5}{nyt:5}\n     EXPECTING , VARIABLE ID = {idfld}\n     ARRAY DIMENSION = {:5}{:5}\n", nxj[0], nyj[0]));
                    }
                }
                _ => b.at = 0,
            }
            units.push(Unit::Bin(b));
            ui.push(units.len() - 1);
        } else if nds as usize == NDSI {
            inp(&mut units).nextln(comstr);
            ui.push(0);
        } else {
            if j == 1 && nds == ndsf[0] {
                refuse("two input files on one unit number");
            }
            match open_unit(dir, fromf == "NAME", &namef, nds, false) {
                Some(u) => units.push(u),
                None => die(49, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN OPENING INPUT DATA FILE\n     NDSF   ={nds:5}\n     NAMEF  = {namef}\n")),
            }
            ui.push(units.len() - 1);
        }
    }
    let _ = (&fromf_last, &namef_last);
    // With one component the second field is read from the first one's file.
    let sec = if nfcomp == 2 { 1 } else { 0 };
    let idfmf0 = layout[0].idfm;
    let idlaf0 = layout[0].idla;
    let src = if ndsf[0] as usize == NDSI { "inline".to_string() } else { namef_last.clone() };

    // 5.b Open and prepare the output file.
    let fname = match idfld.as_str() {
        "LEV" => "level.ww3".to_string(),
        "CUR" => "current.ww3".to_string(),
        "WND" | "WNS" => "wind.ww3".to_string(),
        "ICE" | "ISI" => "ice.ww3".to_string(),
        "TAU" => "momentum.ww3".to_string(),
        "RHO" => "density.ww3".to_string(),
        "MDN" => "muddens.ww3".to_string(),
        "MTH" => "mudthk.ww3".to_string(),
        "MVS" => "mudvisc.ww3".to_string(),
        "IC1" => "ice1.ww3".to_string(),
        "IC2" => "ice2.ww3".to_string(),
        "IC3" => "ice3.ww3".to_string(),
        "IC4" => "ice4.ww3".to_string(),
        "IC5" => "ice5.ww3".to_string(),
        d => format!("data{}.ww3", &d[2..]),
    };
    let path = format!("{dir}/{fname}");
    let _ = std::fs::remove_file(&path);
    let mut out: Vec<u8> = Vec::new();
    let write_out = |out: &[u8]| {
        if let Err(e) = std::fs::write(&path, out) {
            die(3, &format!("\n *** WAVEWATCH III ERROR IN W3FLDO : \n     ERROR IN WRITING TO {idfld} FILE, {e}\n"));
        }
    };
    // For DAT, W3FLDO is called with NX = RECLDT, NY = 0, GTYPE = 0 and no FHDR, so the header is always there.
    if flhdr || itype == 5 {
        // WRITE (NDS) IDSTR, IDFLD, NX, NY, GTYPE, FILLER(1:2), TIDEFLAG
        let mut h = Vec::new();
        h.extend_from_slice(IDSTR);
        h.extend_from_slice(format!("{idfld:<3}").as_bytes());
        let hdr = if itype == 5 { [recldt as i32, 0, 0, 0, 0, 0] } else { [m.nx, m.ny, m.gtype, 0, 0, 0] };
        for v in hdr {
            h.extend_from_slice(&v.to_be_bytes());
        }
        put(&mut out, &h);
    }
    write_out(&out);

    // 5.c Initialize fields.
    let (mut fx, mut fy, mut fa) = (vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n]);
    let mxm = nxj[0].max(nxj[1]);
    let mym = nyj[0].max(nyj[1]);
    let (mut xc, mut yc, mut ac) = (vec![0.0 as Real; mxm * mym], vec![0.0 as Real; mxm * mym], vec![0.0 as Real; mxm * mym]);

    // 6 Begin loop over input fields.
    let mut records = 0;
    'fields: loop {
        // 6.a Read new time and fields.
        if fltime {
            let t = |units: &mut Vec<Unit>, j: usize| {
                let r = read_time(&mut units[ui[j]], layout[j].idfm, tfmt[j].as_ref());
                match r {
                    Ok(t) => t,
                    Err(Stop::Eof) => [i32::MIN, i32::MIN],
                    Err(e) => die(50, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING TIME FROM FILE ({})\n     {}\n", j + 1, why(e))),
                }
            };
            time = t(&mut units, 0);
            if time == [i32::MIN, i32::MIN] {
                break 'fields;
            }
            if nfcomp == 2 {
                let time2 = t(&mut units, 1);
                if time2 == [i32::MIN, i32::MIN] {
                    break 'fields;
                }
                if time2 != time {
                    die(51, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     INCOMPATIBLE FIELD TIMES \n            FIELD #1 : {:08}{:7}\n            FIELD #2 : {:08}{:7}\n", time[0], time[1], time2[0], time2[1]));
                }
            }
        }

        // Input.
        let mut ndat = 0usize;
        let mut data: Vec<Real> = Vec::new();
        if itype <= 4 {
            ina2r(&mut units[ui[0]], &mut xc, mxm, nxj[0], nyj[0], &layout[0]);
            if nfcomp == 2 || ifld >= 3 || flberg {
                ina2r(&mut units[ui[sec]], &mut yc, mxm, nxj[sec], nyj[sec], &layout[sec]);
                if flstab {
                    ina2r(&mut units[ui[sec]], &mut ac, mxm, nxj[sec], nyj[sec], &layout[sec]);
                }
            }
        } else {
            let u = &mut units[ui[0]];
            let nd: Result<i32, Stop> = match (&mut *u, idfmf0) {
                (Unit::Bin(b), 3) => b.record().and_then(|r| bin_vals(&r, 1, false)).map(|v| v[0] as i32),
                (Unit::Text(s), _) => s.list(1, false).and_then(|v| to_int(&v[0].text)),
                _ => refuse("unformatted input from a formatted unit"),
            };
            let nd = match nd {
                Ok(v) => v,
                Err(Stop::Eof) => die(52, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING NDAT FROM FILE\n     IOSTAT =   -1\n"),
                Err(e) => die(52, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING NDAT FROM FILE\n     {}\n", why(e))),
            };
            if nd < 0 {
                refuse("a negative number of data records (the Fortran writes from an unallocated array)");
            }
            ndat = nd as usize;
            for idat in 1..=ndat {
                let r: Result<Vec<f64>, Stop> = match (&mut *u, idfmf0) {
                    (Unit::Bin(b), 3) => b.record().and_then(|r| bin_vals(&r, recldt, true)),
                    (Unit::Text(s), 1) => s.list(recldt, true).and_then(|v| v.iter().map(|it| to_real(&it.text).map(|x| x as f64)).collect()),
                    (Unit::Text(s), 2) => formatted(s, tfmt[0].as_ref().unwrap(), recldt, true),
                    _ => refuse("unformatted input from a formatted unit"),
                };
                match r {
                    Ok(v) => data.extend(v.into_iter().map(|x| x as Real)),
                    Err(Stop::Eof) => die(53, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING DATA RECORD{idat:6} FROM FILE\n     IOSTAT =   -1\n")),
                    Err(e) => die(53, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING DATA RECORD{idat:6} FROM FILE\n     {}\n", why(e))),
                }
            }
        }

        // 6.b Interpolate fields.
        if itype == 1 {
            // No interpolation, type AI.
            if ifld <= 2 && !flberg {
                fa.copy_from_slice(&xc);
            } else {
                fx.copy_from_slice(&xc);
                fy.copy_from_slice(&yc);
                fa.copy_from_slice(&ac);
            }
        } else if itype != 5 {
            let xat = |i: i32, j: i32| (i - 1) as usize + (j - 1) as usize * mxm;
            if ifld <= 2 && !flberg {
                // One-component fields.
                for iy in 0..ny {
                    for ix in 0..nx {
                        let k = ix + iy * nx;
                        fa[k] = rd.rd11[k] * xc[xat(rd.ix1[k], rd.iy1[k])]
                            + rd.rd21[k] * xc[xat(rd.ix2[k], rd.iy1[k])]
                            + rd.rd12[k] * xc[xat(rd.ix1[k], rd.iy2[k])]
                            + rd.rd22[k] * xc[xat(rd.ix2[k], rd.iy2[k])];
                    }
                }
                if nfcomp == 2 {
                    for iy in 0..ny {
                        for ix in 0..nx {
                            let k = ix + iy * nx;
                            fa[k] = fa[k]
                                + xd.rd11[k] * yc[xat(xd.ix1[k], xd.iy1[k])]
                                + xd.rd21[k] * yc[xat(xd.ix2[k], xd.iy1[k])]
                                + xd.rd12[k] * yc[xat(xd.ix1[k], xd.iy2[k])]
                                + xd.rd22[k] * yc[xat(xd.ix2[k], xd.iy2[k])];
                        }
                    }
                }
            } else {
                // Two-component fields.
                let mut a1 = vec![0 as Real; n];
                let mut a2 = vec![0 as Real; n];
                for iy in 0..ny {
                    for ix in 0..nx {
                        let k = ix + iy * nx;
                        if rd.iy1[k] < 1 {
                            rd.iy1[k] = 1;
                            rd.ix1[k] = 1;
                            rd.ix2[k] = 1;
                            reached.ybound += 1;
                        }
                        if rd.iy2[k] < 1 {
                            rd.iy2[k] = 1;
                        }
                        if rd.iy1[k] > mym as i32 {
                            rd.iy1[k] = mym as i32;
                        }
                        if rd.iy2[k] > mym as i32 {
                            rd.iy2[k] = mym as i32;
                            rd.ix1[k] = 1;
                            rd.ix2[k] = 1;
                            reached.ybound += 1;
                        }
                        let (p11, p21, p12, p22) = (xat(rd.ix1[k], rd.iy1[k]), xat(rd.ix2[k], rd.iy1[k]), xat(rd.ix1[k], rd.iy2[k]), xat(rd.ix2[k], rd.iy2[k]));
                        fx[k] = rd.rd11[k] * xc[p11] + rd.rd21[k] * xc[p21] + rd.rd12[k] * xc[p12] + rd.rd22[k] * xc[p22];
                        fy[k] = rd.rd11[k] * yc[p11] + rd.rd21[k] * yc[p21] + rd.rd12[k] * yc[p12] + rd.rd22[k] * yc[p22];
                        fa[k] = rd.rd11[k] * ac[p11] + rd.rd21[k] * ac[p21] + rd.rd12[k] * ac[p12] + rd.rd22[k] * ac[p22];
                        a1[k] = (1.0e-10 as Real).max((fx[k] * fx[k] + fy[k] * fy[k]).sqrt());
                        a2[k] = rd.rd11[k] * (xc[p11] * xc[p11] + yc[p11] * yc[p11]).sqrt()
                            + rd.rd21[k] * (xc[p21] * xc[p21] + yc[p21] * yc[p21]).sqrt()
                            + rd.rd12[k] * (xc[p12] * xc[p12] + yc[p12] * yc[p12]).sqrt()
                            + rd.rd22[k] * (xc[p22] * xc[p22] + yc[p22] * yc[p22]).sqrt();
                    }
                }
                // Winds (WNT1), currents (CRT1) and momentum (WNT1): correct for velocity conservation.
                // WNT2 and CRT2, which use A3, are off in this switch.
                if ifld == 3 || ifld == 4 || ifld == 6 {
                    for k in 0..n {
                        let r = a2[k] / a1[k];
                        let factor = if r < 1.5 { r } else { 1.5 };
                        if r >= 1.5 {
                            reached.clamp += 1;
                        }
                        fx[k] *= factor;
                        fy[k] *= factor;
                    }
                }
            }
        }

        // 6.c Write fields (W3FLDG 'WRITE', W3FLDD 'WRITE').
        if itype <= 4 {
            let fl2d = matches!(idfld.as_str(), "CUR" | "WND" | "WNS" | "ISI" | "TAU");
            put(&mut out, &time.iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<u8>>());
            if !fl2d {
                put(&mut out, &field_record(&fa));
            } else {
                put(&mut out, &field_record(&fx));
                put(&mut out, &field_record(&fy));
                if flstab {
                    put(&mut out, &field_record(&fa));
                }
            }
        } else if ndat > 0 {
            // WRITE (NDS) TD, ND then WRITE (NDS) DATA
            put(&mut out, &[time[0], time[1], ndat as i32].iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<u8>>());
            put(&mut out, &field_record(&data));
        }
        write_out(&out);
        records += 1;
        if !fltime {
            break;
        }
    }
    let cover = if (2..=4).contains(&itype) {
        format!(
            "; points outside the input grid {}, cells with a land corner {}, of which all four {} ({} corrected, {} with one neighbour, {} lost), covered by two grids {}, address clamps {}, factor 1.5 {}, near-pole cells {}",
            reached.outside, reached.part, reached.full, reached.corrected, reached.single, reached.lost, reached.overlap, reached.ybound, reached.clamp, reached.pole
        )
    } else {
        String::new()
    };
    println!(
        "ww3_prep: {idfld} {idtype} FLTIME={} header={} IDLA={idlaf0} IDFM={idfmf0} {src}: {records} time record(s), {fname} {} bytes{cover}",
        if fltime { 'T' } else { 'F' },
        if flhdr { 'T' } else { 'F' },
        out.len()
    );
}
