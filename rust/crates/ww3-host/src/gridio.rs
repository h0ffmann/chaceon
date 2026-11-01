// SPDX-License-Identifier: MPL-2.0
//! Fortran input that `ww3_grid` uses, for the cases it implements: namelist groups, and list-directed
//! reads of the depth, coordinate, mask and obstruction files (`INA2R`, `INA2I`,
//! `model/src/w3arrymd.F90:76-291`). What a Fortran runtime would read and this does not is refused,
//! not guessed.

/// One namelist group: `&NAME key = value ... /`.
pub struct Group {
    pub name: String,
    /// Keys upper-cased, values as written.
    pub items: Vec<(String, String)>,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '%' || c == '(' || c == ')'
}

/// `text` from the `&` of a group to its closing `/`, comments and line ends out.
fn group_body(text: &str) -> (String, String, usize) {
    let at = text.find('&').expect("no namelist group");
    let mut name = String::new();
    let mut chars = text[at + 1..].char_indices().peekable();
    while let Some(&(_, c)) = chars.peek() {
        if c.is_whitespace() || c == '/' {
            break;
        }
        name.push(c);
        chars.next();
    }
    let mut body = String::new();
    let (mut quote, mut comment) = (None, false);
    let mut used = text.len();
    for (i, c) in chars {
        if comment {
            if c == '\n' {
                comment = false;
                body.push(' ');
            }
            continue;
        }
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                body.push(c);
            }
            (Some(_), c) => body.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                body.push(c);
            }
            (None, '!') => comment = true,
            (None, '/') => {
                used = at + 1 + i + 1;
                break;
            }
            (None, '\n' | '\r') => body.push(' '),
            (None, c) => body.push(c),
        }
    }
    (name.to_uppercase(), body, used)
}

fn items(body: &str) -> Vec<(String, String)> {
    // Positions of '=' outside quotes.
    let mut eq = Vec::new();
    let mut quote = None;
    for (i, c) in body.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '=') => eq.push(i),
            _ => {}
        }
    }
    let key_start = |p: usize| {
        let head = body[..p].trim_end();
        let n = head.chars().rev().take_while(|c| is_ident(*c)).count();
        head.len() - n
    };
    let mut v = Vec::new();
    for (k, &p) in eq.iter().enumerate() {
        let from = key_start(p);
        let end = eq.get(k + 1).map_or(body.len(), |&q| key_start(q));
        let value = body[p + 1..end].trim().trim_end_matches(',').trim().to_string();
        v.push((body[from..body[..p].trim_end().len()].to_uppercase(), value));
    }
    v
}

/// Every group of a file, in order (`ww3_grid.nml`).
pub fn groups(text: &str) -> Vec<Group> {
    let mut out = Vec::new();
    let mut rest = text;
    // A '&' inside a comment line does not open a group.
    loop {
        let mut off = None;
        let mut pos = 0;
        for line in rest.split_inclusive('\n') {
            let t = line.trim_start();
            if t.starts_with('&') {
                off = Some(pos + (line.len() - t.len()));
                break;
            }
            pos += line.len();
        }
        let Some(o) = off else { break };
        let (name, body, used) = group_body(&rest[o..]);
        out.push(Group { name, items: items(&body) });
        rest = &rest[o + used..];
    }
    out
}

/// `READNL`: the first line whose first non-blank character is `&` and that continues with `name`
/// (`model/src/w3gridmd.F90`, `READNL`), and the group it opens.
pub fn readnl(text: &str, name: &str) -> Option<Group> {
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim_start_matches(' ');
        if t.starts_with('&') && t.len() >= 5 && t[1..5].eq_ignore_ascii_case(name) && line.len() - t.len() < 70 {
            let (n, body, _) = group_body(&text[pos..]);
            return Some(Group { name: n, items: items(&body) });
        }
        pos += line.len();
    }
    None
}

pub fn real(tok: &str) -> f32 {
    let t = tok.trim().replace(['d', 'D'], "e");
    t.parse().unwrap_or_else(|_| panic!("{tok:?} is not a real"))
}

pub fn int(tok: &str) -> i32 {
    tok.trim().parse().unwrap_or_else(|_| panic!("{tok:?} is not an integer"))
}

pub fn logical(tok: &str) -> bool {
    let t = tok.trim().trim_start_matches('.').to_ascii_uppercase();
    match t.chars().next() {
        Some('T') => true,
        Some('F') => false,
        _ => panic!("{tok:?} is not a logical"),
    }
}

/// A quoted string without its quotes.
pub fn text(tok: &str) -> String {
    let t = tok.trim();
    let q = t.chars().next().filter(|c| *c == '\'' || *c == '"');
    match q {
        Some(q) => t[1..].split(q).next().unwrap_or("").to_string(),
        None => t.to_string(),
    }
}

/// First blank-separated token of a value.
pub fn first(v: &str) -> &str {
    v.split(|c: char| c.is_whitespace() || c == ',').find(|s| !s.is_empty()).unwrap_or("")
}

/// A sequential formatted file read by list-directed `READ` statements. Each statement starts on a
/// new record and leaves the rest of its last record unread.
pub struct ListFile {
    lines: Vec<String>,
    at: usize,
}

impl ListFile {
    pub fn open(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        Self { lines: text.lines().map(str::to_string).collect(), at: 0 }
    }

    /// The next `n` values of a list-directed `READ (nds,*)`, as text: separators are blanks and commas, and
    /// `r*c` repeats `c`. A null value (`,,` or `r*`) is refused.
    fn values(&mut self, n: usize) -> Vec<String> {
        let mut v: Vec<String> = Vec::with_capacity(n);
        while v.len() < n {
            let line = self.lines.get(self.at).unwrap_or_else(|| panic!("PREMATURE END OF FILE"));
            self.at += 1;
            for tok in line.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()) {
                assert!(!tok.starts_with('/') && !tok.starts_with('\'') && !tok.starts_with('"'), "list-directed item {tok:?} is not implemented");
                match tok.split_once('*') {
                    Some((r, c)) => {
                        assert!(!c.is_empty(), "a null value in a list-directed read is not implemented");
                        let r: usize = r.parse().unwrap_or_else(|_| panic!("{tok:?} is not a repeat count"));
                        for _ in 0..r {
                            if v.len() < n {
                                v.push(c.to_string());
                            }
                        }
                    }
                    None => {
                        if v.len() < n {
                            v.push(tok.to_string());
                        }
                    }
                }
            }
        }
        v
    }

    /// `READ (nds,*) (A(i), i=1,n)`.
    pub fn read(&mut self, n: usize) -> Vec<f32> {
        self.values(n).iter().map(|t| real(t)).collect()
    }

    pub fn read_int(&mut self, n: usize) -> Vec<i32> {
        self.values(n).iter().map(|t| int(t)).collect()
    }
}

/// `INA2R` for `IDFM = 1`: `a(ix + iy*mx)`, `0 <= ix < mx`, `0 <= iy < my`, then `vsc*a + vof`.
pub fn ina2r(f: &mut ListFile, mx: usize, my: usize, idfm: i32, idla: i32, vsc: f32, vof: f32) -> Vec<f32> {
    assert_eq!(idfm, 1, "IDFM = {idfm}: only free format is implemented");
    let mut a = vec![0.0f32; mx * my];
    match idla {
        1 | 3 => {
            let rows: Vec<usize> = if idla == 1 { (0..my).collect() } else { (0..my).rev().collect() };
            for iy in rows {
                let r = f.read(mx);
                a[iy * mx..(iy + 1) * mx].copy_from_slice(&r);
            }
        }
        _ => {
            let r = f.read(mx * my);
            if idla == 2 {
                a.copy_from_slice(&r);
            } else {
                // ((A(IX,IY),IX=LX,HX),IY=HY,LY,-1): the first value read belongs to row HY.
                for (k, iy) in (0..my).rev().enumerate() {
                    a[iy * mx..(iy + 1) * mx].copy_from_slice(&r[k * mx..(k + 1) * mx]);
                }
            }
        }
    }
    for x in a.iter_mut() {
        *x = vsc * *x + vof;
    }
    a
}

/// `INA2I` for `IDFM = 1`, scale 1 and offset 0.
pub fn ina2i(f: &mut ListFile, mx: usize, my: usize, idfm: i32, idla: i32) -> Vec<i32> {
    assert_eq!(idfm, 1, "IDFM = {idfm}: only free format is implemented");
    let mut a = vec![0i32; mx * my];
    match idla {
        1 | 3 => {
            let rows: Vec<usize> = if idla == 1 { (0..my).collect() } else { (0..my).rev().collect() };
            for iy in rows {
                let r = f.read_int(mx);
                a[iy * mx..(iy + 1) * mx].copy_from_slice(&r);
            }
        }
        _ => {
            let r = f.read_int(mx * my);
            if idla == 2 {
                a.copy_from_slice(&r);
            } else {
                for (k, iy) in (0..my).rev().enumerate() {
                    a[iy * mx..(iy + 1) * mx].copy_from_slice(&r[k * mx..(k + 1) * mx]);
                }
            }
        }
    }
    a
}
