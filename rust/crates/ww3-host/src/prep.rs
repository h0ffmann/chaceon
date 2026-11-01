// SPDX-License-Identifier: MPL-2.0
//! `ww3_prep` (`model/src/ww3_prep.F90` at `92e2cf66`) for `switch_chaceon_shrd`: the input pre-processor.
//! Input type `AI` (the field is already on the model grid, `ITYPE` 1) is translated for every field id
//! the program knows (`IC1`..`IC5`, `MDN`, `MTH`, `MVS`, `ICE`, `ISI`, `LEV`, `WND`, `WNS`, `CUR`, `TAU`,
//! `RHO`), with free format (`IDFM` 1) and formatted (`IDFM` 2) input. What the Fortran accepts and this does
//! not is refused with a message and exit status 64: types `LL`, `F1`, `F2` and `DAT`, binary input
//! (`IDFM` 3), format reversal, and list-directed forms with a slash or a null value. The output channels
//! `O3`, `O15` and `T*` of the Fortran are off in this switch, and the log it prints on unit 6 is replaced by
//! one summary line; the Fortran log is not compared.

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
            return Err("a null value in a list-directed read is not implemented".into());
        }
        if b[i] == '/' {
            return Err("a slash in a list-directed read is not implemented".into());
        }
        if b[i] == '\'' || b[i] == '"' {
            let q = b[i];
            i += 1;
            let mut s = String::new();
            loop {
                if i >= b.len() {
                    return Err("a string that continues on the next record is not implemented".into());
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
                        return Err("a null value in a list-directed read is not implemented".into());
                    }
                    for _ in 0..r {
                        out.push(Item { text: c.to_string() });
                    }
                }
                Some(_) => return Err("a repeat count on this item is not implemented".into()),
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

fn parse_group(b: &[char], i: &mut usize, out: &mut Vec<Ed>, fmt: &str) {
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
            *i += 1;
            let mut g = Vec::new();
            parse_group(b, i, &mut g, fmt);
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

fn parse_format(fmt: &str) -> Vec<Ed> {
    let b: Vec<char> = fmt.trim().chars().collect();
    if b.first() != Some(&'(') {
        refuse(&format!("the format {fmt:?}"));
    }
    let (mut i, mut out) = (1, Vec::new());
    parse_group(&b, &mut i, &mut out, fmt);
    out
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

/// The reference accepts a comma inside a numeric field; the runtime most likely ends the field there, which
/// would shift the columns of every field after it. Not translated, not probed.
fn comma(field: &str) {
    if field.contains(',') {
        refuse("a comma inside a numeric field of a formatted read");
    }
}

/// One formatted record read against the leading descriptors of `ed`: `n` values, integer or real.
fn formatted(rec: &str, ed: &[Ed], n: usize, real: bool) -> Result<Vec<f64>, Stop> {
    let ch: Vec<char> = rec.chars().collect();
    let (mut pos, mut out) = (0usize, Vec::with_capacity(n));
    for e in ed {
        if out.len() == n {
            break;
        }
        let take = |pos: &mut usize, w: usize| -> String {
            let s: String = (0..w).map(|k| ch.get(*pos + k).copied().unwrap_or(' ')).collect();
            *pos += w;
            s
        };
        match *e {
            Ed::X(k) => pos += k,
            Ed::I(w) => {
                let s = take(&mut pos, w);
                comma(&s);
                if real {
                    refuse("an integer descriptor for a real value");
                }
                out.push(field_int(&s)? as f64);
            }
            Ed::F(w, d) => {
                let s = take(&mut pos, w);
                comma(&s);
                if !real {
                    refuse("a real descriptor for an integer value");
                }
                out.push(field_real(&s, d)? as f64);
            }
        }
    }
    if out.len() < n {
        refuse("format reversal (more values than the format has descriptors)");
    }
    Ok(out)
}

/// How the data of one field are read: `IDFM` 1 list-directed, 2 formatted.
struct Layout {
    idfm: i32,
    idla: i32,
    ed: Vec<Ed>,
}

/// `INA2R` (`w3arrymd.F90:76-291`) for `IDFM` 1 and 2, into `a(ix + iy*mx)`, `VSC = 1` and `VOF = 0`.
fn ina2r(s: &mut Stream, a: &mut [Real], mx: usize, nx: usize, ny: usize, l: &Layout) -> Result<(), Stop> {
    let vals = |s: &mut Stream, n: usize| -> Result<Vec<Real>, Stop> {
        if l.idfm == 1 {
            s.list(n, true)?.iter().map(|it| to_real(&it.text)).collect()
        } else {
            Ok(formatted(&s.record()?, &l.ed, n, true)?.into_iter().map(|v| v as Real).collect())
        }
    };
    match l.idla {
        1 | 3 => {
            let rows: Vec<usize> = if l.idla == 1 { (0..ny).collect() } else { (0..ny).rev().collect() };
            for iy in rows {
                let r = vals(s, nx)?;
                a[iy * mx..iy * mx + nx].copy_from_slice(&r);
            }
        }
        _ => {
            // One READ statement for the whole array.
            let r = if l.idfm == 1 {
                vals(s, nx * ny)?
            } else {
                if l.ed.len() < nx * ny {
                    refuse("format reversal (more values than the format has descriptors)");
                }
                vals(s, nx * ny)?
            };
            if l.idla == 2 {
                for iy in 0..ny {
                    a[iy * mx..iy * mx + nx].copy_from_slice(&r[iy * nx..(iy + 1) * nx]);
                }
            } else {
                // ((A(IX,IY),IX=LX,HX),IY=HY,LY,-1): the first values read belong to row HY.
                for (k, iy) in (0..ny).rev().enumerate() {
                    a[iy * mx..iy * mx + nx].copy_from_slice(&r[k * nx..(k + 1) * nx]);
                }
            }
        }
    }
    // ARRAY = VSC * ARRAY + VOF, with VSC = 1. and VOF = 0.: a negative zero becomes +0.
    for iy in 0..ny {
        for x in &mut a[iy * mx..iy * mx + nx] {
            *x = 1. * *x + 0.;
        }
    }
    Ok(())
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

/// Reads `mod_def.ww3` and `ww3_prep.inp` from `dir`, writes the field file (`current.ww3`, `wind.ww3`, ...)
/// there. Names in the input file are relative to `dir`, as the Fortran's `FNMPRE = './'` makes them.
pub fn run(dir: &str) {
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    let mut inp = match Stream::open(&format!("{dir}/ww3_prep.inp")) {
        Some(s) => s,
        None => die(40, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN OPENING INPUT FILE\n"),
    };
    // READ (NDSI,'(A)') COMSTR; IF (COMSTR.EQ.' ') COMSTR = '$'
    let Some(first) = inp.lines.first() else { die(41, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     PREMATURE END OF INPUT FILE\n") };
    let comstr = match first.chars().next() {
        None | Some(' ') => '$',
        Some(c) => c,
    };
    inp.at = 1;
    let (nx, ny) = (m.nx as usize, m.ny as usize);

    let eof41 = || -> ! { die(41, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     PREMATURE END OF INPUT FILE\n") };
    let bad42 = |why: &str| -> ! { die(42, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING FROM INPUT FILE\n     {why}\n")) };
    let list = |inp: &mut Stream, n: usize| -> Vec<Item> {
        match inp.list(n, false) {
            Ok(v) => v,
            Err(Stop::Eof) => eof41(),
            Err(Stop::Bad(w)) => bad42(&w),
        }
    };

    // 3.a Read types from the input file.
    inp.nextln(comstr);
    let it = list(&mut inp, 4);
    let idfld = str_of(&it[0], 3);
    let idtype = str_of(&it[1], 2);
    let fltime = to_logical(&it[2].text).unwrap_or_else(|_| bad42("FLTIME"));
    let flhdr = to_logical(&it[3].text).unwrap_or_else(|_| bad42("FLHDR"));

    // 3.b Check types.
    let flstab = idfld == "WNS";
    let flberg = idfld == "ISI";
    let ifld: i32 = match idfld.as_str() {
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
        _ => die(1, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL FIELD ID -->{idfld}<--\n")),
    };
    let itype = if idfld == "DAT" {
        5
    } else {
        match idtype.as_str() {
            "AI" => 1,
            "LL" => 2,
            "F1" => 3,
            "F2" => 4,
            _ => die(2, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL FORMAT ID -->{idtype}<--\n")),
        }
    };
    if itype == 4 && ifld > 2 {
        die(3, "\n *** WAVEWATCH III ERROR IN W3PREP : \n     THIS FORMAT TYPE IS ALLOWED FOR ICE AND LEV ONLY\n");
    }

    // 3.c Additional input for format types and time.
    let mut time = [0i32; 2];
    if !fltime {
        inp.nextln(comstr);
        let it = list(&mut inp, 2);
        for k in 0..2 {
            time[k] = to_int(&it[k].text).unwrap_or_else(|_| bad42("TIME"));
        }
        if time[0] < 10000000 {
            die(4, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ILLEGAL TIME : {:08}{:07}\n", time[0], time[1]));
        }
    }
    match itype {
        1 => {}
        2 => refuse("input type LL (longitude-latitude grid, interpolation)"),
        3 => refuse("input type F1 (grid from file, interpolation)"),
        4 => refuse("input type F2 (two grids from file, interpolation)"),
        _ => refuse("input type DAT (assimilation data)"),
    }

    // 4.c Input location and format. For type 1 the field has the model's size.
    inp.nextln(comstr);
    let it = list(&mut inp, 5);
    let (fromf, idlaf, idfmf) = (str_of(&it[0], 4), to_int(&it[1].text).unwrap_or_else(|_| bad42("IDLAF")), to_int(&it[2].text).unwrap_or_else(|_| bad42("IDFMF")));
    let idlaf = if (1..=4).contains(&idlaf) { idlaf } else { 1 };
    let idfmf = if (1..=3).contains(&idfmf) { idfmf } else { 1 };
    let (formt, formf) = (str_of(&it[3], 32), str_of(&it[4], 32));
    inp.nextln(comstr);
    let it = list(&mut inp, 2);
    let ndsf = to_int(&it[0].text).unwrap_or_else(|_| bad42("NDSF"));
    let namef = str_of(&it[1], 65);

    // 5.a Open the input file.
    if idfmf == 3 {
        refuse("binary field input (IDFM 3)");
    }
    const NDSI: i32 = 10;
    let mut data: Option<Stream> = None;
    if ndsf == NDSI {
        inp.nextln(comstr);
    } else {
        let path = if fromf == "NAME" { format!("{dir}/{namef}") } else { format!("{dir}/fort.{ndsf}") };
        match Stream::open(&path) {
            Some(s) => data = Some(s),
            None => die(49, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN OPENING INPUT DATA FILE\n     NDSF   ={ndsf:5}\n     NAMEF  = {namef}\n")),
        }
    }
    let layout = Layout { idfm: idfmf, idla: idlaf, ed: if idfmf == 2 { parse_format(&formf) } else { Vec::new() } };
    let tfmt = if idfmf == 2 { parse_format(&formt) } else { Vec::new() };

    // 5.b Open the output file and write its header.
    let fname = match idfld.as_str() {
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
    };
    let path = format!("{dir}/{fname}");
    let _ = std::fs::remove_file(&path);
    let mut out: Vec<u8> = Vec::new();
    let write_out = |out: &[u8]| {
        if let Err(e) = std::fs::write(&path, out) {
            die(3, &format!("\n *** WAVEWATCH III ERROR IN W3FLDO : \n     ERROR IN WRITING TO {idfld} FILE, {e}\n"));
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
        put(&mut out, &h);
    }
    write_out(&out);

    // 5.c Initialize fields.
    let n = nx * ny;
    let (mut fx, mut fy, mut fa) = (vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n]);
    let (mut xc, mut yc, mut ac) = (vec![0.0 as Real; n], vec![0.0 as Real; n], vec![0.0 as Real; n]);

    // 6 Loop over input fields.
    let mut records = 0;
    loop {
        let stream: &mut Stream = match data.as_mut() {
            Some(s) => s,
            None => &mut inp,
        };
        // 6.a Read new time and fields.
        if fltime {
            let r = if idfmf == 1 {
                stream.list(2, false).and_then(|v| Ok([to_int(&v[0].text)?, to_int(&v[1].text)?]))
            } else {
                stream.record().and_then(|rec| {
                    let v = formatted(&rec, &tfmt, 2, false)?;
                    Ok([v[0] as i32, v[1] as i32])
                })
            };
            match r {
                Ok(t) => time = t,
                Err(Stop::Eof) => break,
                Err(Stop::Bad(w)) => die(50, &format!("\n *** WAVEWATCH III ERROR IN W3PREP : \n     ERROR IN READING TIME FROM FILE (1)\n     {w}\n")),
            }
        }
        let read = |s: &mut Stream, a: &mut [Real]| {
            if let Err(e) = ina2r(s, a, nx, nx, ny, &layout) {
                match e {
                    Stop::Eof => die(-1, "\n *** ERROR INA2R : \n     PREMATURE END OF FILE\n"),
                    Stop::Bad(w) => die(1, &format!("\n *** ERROR INA2R : \n     ERROR IN READING FROM FILE\n     {w}\n")),
                }
            }
        };
        read(stream, &mut xc);
        if ifld >= 3 || flberg {
            read(stream, &mut yc);
            if flstab {
                read(stream, &mut ac);
            }
        }

        // 6.b No interpolation, type AI.
        if ifld <= 2 && !flberg {
            fa.copy_from_slice(&xc);
        } else {
            fx.copy_from_slice(&xc);
            fy.copy_from_slice(&yc);
            fa.copy_from_slice(&ac);
        }

        // 6.c Write fields (W3FLDG 'WRITE').
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
        write_out(&out);
        records += 1;
        if !fltime {
            break;
        }
    }
    println!(
        "ww3_prep: {idfld} {idtype} FLTIME={} header={} IDLA={idlaf} IDFM={idfmf} {}: {records} time record(s), {fname} {} bytes",
        if fltime { 'T' } else { 'F' },
        if flhdr { 'T' } else { 'F' },
        if ndsf == NDSI { "inline".to_string() } else { namef.clone() },
        out.len()
    );
}
