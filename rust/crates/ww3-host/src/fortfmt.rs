// SPDX-License-Identifier: MPL-2.0
//! gfortran's formatted output for the edit descriptors `ww3_outf` and `ww3_outp` use: `Fw.d`, `Ew.d`,
//! `ESw.d`, `Gw.d`, `Iw`, `Iw.m`, `Lw`, `Aw`. libgfortran (`libgfortran/io/write_float.def`) prints a `REAL(4)`
//! by promoting it to `double` and calling `snprintf` with `%+-#.*e` (E, ES, G) or `%+-#.*f` (F), whose result is
//! the correctly rounded decimal expansion of the binary value, with an exact tie going to the even digit. Rust's
//! `{:.N}` and `{:.Ne}` on the same `f64` give the same digits; there is no other rounding step. A field that is
//! too narrow is filled with asterisks, and the optional leading zero of `F` and `E` is dropped only to make the
//! value fit. A negative value that rounds to zero keeps its sign (`-fsign-zero`, gfortran's default).

fn stars(w: usize) -> String {
    "*".repeat(w)
}

fn pad(w: usize, s: String) -> String {
    if s.len() >= w { s } else { format!("{}{}", " ".repeat(w - s.len()), s) }
}

fn non_finite(w: usize, x: f64) -> Option<String> {
    if x.is_nan() {
        return Some(pad(w, "NaN".to_string()));
    }
    if x.is_infinite() {
        let s = if x < 0. {
            if w >= 9 { "-Infinity" } else { "-Inf" }
        } else if w >= 8 {
            "Infinity"
        } else {
            "Inf"
        };
        return Some(if s.len() > w { stars(w) } else { pad(w, s.to_string()) });
    }
    None
}

/// `Fw.d`.
pub fn f(w: usize, d: usize, x: f64) -> String {
    if let Some(s) = non_finite(w, x) {
        return s;
    }
    let neg = x.is_sign_negative();
    let mut s = format!("{:.*}", d, x.abs());
    if d == 0 {
        s.push('.');
    }
    let sign = if neg { "-" } else { "" };
    if sign.len() + s.len() > w && s.starts_with("0.") {
        s.remove(0);
    }
    if sign.len() + s.len() > w {
        return stars(w);
    }
    pad(w, format!("{sign}{s}"))
}

/// The digits of `|x|` rounded to `n` significant digits, and the exponent `e` of the value `0.DDD * 10^e`.
fn digits(n: usize, x: f64) -> (String, i32) {
    if x == 0. {
        return ("0".repeat(n), 0);
    }
    let s = format!("{:.*e}", n - 1, x);
    let (m, e) = s.split_once('e').expect("exponent");
    let e: i32 = e.parse().expect("exponent value");
    (m.replace('.', ""), e + 1)
}

fn exp_field(e: i32) -> String {
    if e.abs() <= 99 {
        format!("E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        format!("{}{:03}", if e < 0 { '-' } else { '+' }, e.abs())
    }
}

/// `Ew.d`.
pub fn e(w: usize, d: usize, x: f64) -> String {
    if let Some(s) = non_finite(w, x) {
        return s;
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    let (dg, ex) = digits(d, x.abs());
    let ex = exp_field(ex);
    let mut lead = "0".to_string();
    if sign.len() + lead.len() + 1 + d + ex.len() > w {
        lead.clear();
    }
    if sign.len() + lead.len() + 1 + d + ex.len() > w {
        return stars(w);
    }
    pad(w, format!("{sign}{lead}.{dg}{ex}"))
}

/// `ESw.d`.
pub fn es(w: usize, d: usize, x: f64) -> String {
    if let Some(s) = non_finite(w, x) {
        return s;
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    let (dg, ex) = digits(d + 1, x.abs());
    let ex = if x == 0. { exp_field(0) } else { exp_field(ex - 1) };
    let s = format!("{sign}{}.{}{ex}", &dg[..1], &dg[1..]);
    if s.len() > w { stars(w) } else { pad(w, s) }
}

/// `Gw.d`: `F` for a magnitude in `[0.1, 10^d)` after rounding to `d` digits, `E` otherwise.
pub fn g(w: usize, d: usize, x: f64) -> String {
    if let Some(s) = non_finite(w, x) {
        return s;
    }
    if x == 0. {
        return format!("{}{}", f(w - 4, d - 1, x), " ".repeat(4));
    }
    let (_, ex) = digits(d, x.abs());
    if ex < 0 || ex > d as i32 {
        return e(w, d, x);
    }
    format!("{}{}", f(w - 4, (d as i32 - ex) as usize, x), " ".repeat(4))
}

/// `Iw`.
pub fn i(w: usize, n: i64) -> String {
    let s = n.to_string();
    if s.len() > w { stars(w) } else { pad(w, s) }
}

/// `Iw.m`.
pub fn im(w: usize, m: usize, n: i64) -> String {
    let body = n.unsigned_abs().to_string();
    let body = if body.len() < m { format!("{}{body}", "0".repeat(m - body.len())) } else { body };
    let s = if n < 0 { format!("-{body}") } else { body };
    if s.len() > w { stars(w) } else { pad(w, s) }
}

/// `Lw`.
pub fn l(w: usize, b: bool) -> String {
    pad(w, if b { "T" } else { "F" }.to_string())
}

/// `Aw` with `w` the length of the item: the item is padded with blanks on the right, or cut.
pub fn a(w: usize, s: &str) -> String {
    let mut b: Vec<u8> = s.bytes().take(w).collect();
    b.resize(w, b' ');
    String::from_utf8_lossy(&b).into_owned()
}
