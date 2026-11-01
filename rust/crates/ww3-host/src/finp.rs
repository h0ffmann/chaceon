// SPDX-License-Identifier: MPL-2.0
//! The input file of a WW3 program as Fortran reads it: `NEXTLN` (`model/src/w3servmd.F90:221-334`) skips
//! comment and blank lines, and a list-directed `READ (NDSI,*)` takes items across records until it has them
//! all, ignores the rest of the last record, and stops at a `/`. What libgfortran reads and this does not
//! (a repeat count other than `r*`, a namelist, a complex) is refused by the caller's type check.

pub struct Inp {
    lines: Vec<String>,
    pos: usize,
    pub com: char,
}

/// What a `READ` stopped on.
#[derive(Debug)]
pub enum ReadErr {
    /// `END=`: no record left.
    End,
    /// `ERR=`: an item of the wrong type.
    Bad(String),
}

impl Inp {
    pub fn open(path: &str) -> Result<Self, std::io::Error> {
        let t = std::fs::read(path)?;
        let t = String::from_utf8_lossy(&t).into_owned();
        let mut lines: Vec<String> = t.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
        if lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        Ok(Self { lines, pos: 0, com: '$' })
    }

    /// `READ (NDSI,'(A)') COMSTR`, then `IF (COMSTR.EQ.' ') COMSTR = '$'`.
    pub fn comment_char(&mut self) -> Result<char, ReadErr> {
        let l = self.lines.get(self.pos).ok_or(ReadErr::End)?;
        self.pos += 1;
        let c = l.chars().next().unwrap_or(' ');
        self.com = if c == ' ' { '$' } else { c };
        Ok(self.com)
    }

    /// `NEXTLN`: positions at the next line that is neither blank nor starts with the comment character.
    pub fn nextln(&mut self) -> Result<(), ReadErr> {
        loop {
            let l = self.lines.get(self.pos).ok_or(ReadErr::End)?;
            let t = l.trim_start();
            if t.is_empty() || t.starts_with(self.com) {
                self.pos += 1;
            } else {
                return Ok(());
            }
        }
    }

    /// `READ (NDSI,'(A)') LINE`: the whole record.
    pub fn raw_line(&mut self) -> Result<String, ReadErr> {
        let l = self.lines.get(self.pos).ok_or(ReadErr::End)?.clone();
        self.pos += 1;
        Ok(l)
    }

    /// `READ (NDSI,*) item1, ..., itemN`: the items as text, `None` for a null value or after a `/`.
    pub fn list(&mut self, n: usize) -> Result<Vec<Option<String>>, ReadErr> {
        let mut out: Vec<Option<String>> = Vec::new();
        let mut slash = false;
        while out.len() < n {
            let l = self.lines.get(self.pos).ok_or(ReadErr::End)?.clone();
            self.pos += 1;
            let b: Vec<char> = l.chars().collect();
            let mut i = 0;
            // A record that starts with a separator before any item has a null first item.
            let mut expect_item = false;
            while i < b.len() && out.len() < n {
                match b[i] {
                    ' ' | '\t' => i += 1,
                    ',' => {
                        if expect_item {
                            out.push(None);
                        }
                        expect_item = true;
                        i += 1;
                    }
                    '/' => {
                        slash = true;
                        break;
                    }
                    q @ ('\'' | '"') => {
                        let mut s = String::new();
                        i += 1;
                        while i < b.len() {
                            if b[i] == q {
                                if i + 1 < b.len() && b[i + 1] == q {
                                    s.push(q);
                                    i += 2;
                                    continue;
                                }
                                break;
                            }
                            s.push(b[i]);
                            i += 1;
                        }
                        i += 1;
                        out.push(Some(s));
                        expect_item = false;
                    }
                    _ => {
                        let st = i;
                        while i < b.len() && !matches!(b[i], ' ' | '\t' | ',' | '/') {
                            i += 1;
                        }
                        let tok: String = b[st..i].iter().collect();
                        if let Some((r, v)) = tok.split_once('*') {
                            let r: usize = r.parse().map_err(|_| ReadErr::Bad(tok.clone()))?;
                            for _ in 0..r {
                                if out.len() < n {
                                    out.push(if v.is_empty() { None } else { Some(v.to_string()) });
                                }
                            }
                        } else {
                            out.push(Some(tok));
                        }
                        expect_item = false;
                        // A separator comma after the item is consumed here.
                        while i < b.len() && matches!(b[i], ' ' | '\t') {
                            i += 1;
                        }
                        if i < b.len() && b[i] == ',' {
                            expect_item = true;
                            i += 1;
                        }
                    }
                }
            }
            if slash {
                break;
            }
        }
        out.resize(n, None);
        Ok(out)
    }
}

pub fn int(t: &Option<String>, old: i32) -> Result<i32, ReadErr> {
    match t {
        None => Ok(old),
        Some(s) => s.trim().parse::<i32>().map_err(|_| ReadErr::Bad(s.clone())),
    }
}

pub fn real(t: &Option<String>, old: f32) -> Result<f32, ReadErr> {
    match t {
        None => Ok(old),
        Some(s) => s.trim().replace(['d', 'D'], "e").parse::<f32>().map_err(|_| ReadErr::Bad(s.clone())),
    }
}

pub fn logical(t: &Option<String>, old: bool) -> Result<bool, ReadErr> {
    match t {
        None => Ok(old),
        Some(s) => {
            let c = s.trim().trim_start_matches('.').chars().next().map(|c| c.to_ascii_uppercase());
            match c {
                Some('T') => Ok(true),
                Some('F') => Ok(false),
                _ => Err(ReadErr::Bad(s.clone())),
            }
        }
    }
}
