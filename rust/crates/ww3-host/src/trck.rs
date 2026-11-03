// SPDX-License-Identifier: MPL-2.0
//! `ww3_trck` (`model/src/ww3_trck.F90` at `92e2cf66`): `track_o.ww3` to the text file `track.ww3`. Log, status and file
//! follow the reference. `NDSE` is unit 6 here, so error messages are in the log. An empty `track_o.ww3` prints `TTST`
//! unset in the Fortran; here the time prints as not set ⚠.

use crate::finp::{self, Inp};
use crate::fortfmt as ff;
use crate::moddef::Records;
use crate::outp::stme21;
use std::io::Write;
use ww3_kernels::real::Real;

const IDTST: &str = "WAVEWATCH III TRACK OUTPUT SPECTRA";
const LINELN: usize = 81;

fn die(code: i32, msg: &str) -> ! {
    print!("{msg}");
    std::io::stdout().flush().unwrap();
    std::process::exit(code);
}

fn head(text: &str) -> String {
    format!("\n *** WAVEWATCH III ERROR IN W3TRCK : \n{text}\n")
}

/// `STRING`, with `ILAST` its length.
struct Line<'a> {
    s: Vec<u8>,
    out: &'a mut Vec<u8>,
}

impl Line<'_> {
    fn add(&mut self, b: &[u8]) {
        if self.s.len() + b.len() > LINELN {
            self.flush();
        }
        self.s.extend_from_slice(b);
    }
    /// `WRITE (NDSOUT,986) STRING(2:ILAST)`.
    fn flush(&mut self) {
        self.out.extend_from_slice(&self.s[1.min(self.s.len())..]);
        self.out.push(b'\n');
        self.s.clear();
    }
}

/// `ZEROS(10-IWZERO:9)`: one zero, or the count.
fn zeros(n: usize) -> Vec<u8> {
    if n == 1 { b" 0".to_vec() } else { format!(" {n}*0").into_bytes() }
}

pub fn run(dir: &str) {
    print!("\n{:15}    *** WAVEWATCH III Track output post.***    \n{:15}===============================================\n\n", "", "");
    let open = |e: std::io::Error| e.raw_os_error().unwrap_or(2) as i64;
    let mut i = Inp::open(&format!("{dir}/ww3_trck.inp"))
        .unwrap_or_else(|e| die(5, &head(&format!("     ERROR IN OPENING INPUT FILE\n     IOSTAT ={}\n", ff::i(5, open(e))))));
    let end = || head("     PREMATURE END OF INPUT FILE\n     IOSTAT =   -1\n");
    let com = i.comment_char().unwrap_or_else(|_| die(6, &end()));
    print!("  Comment character is '{com}'\n\n");
    if i.nextln().is_err() {
        die(1, "\n *** WAVEWATCH III ERROR IN NEXTLN : \n     PREMATURE END OF INPUT FILE\n\n");
    }
    let v = i.list(2).unwrap_or_else(|_| die(6, &end()));
    let bad = || head("     ERROR IN READING FROM INPUT FILE\n     IOSTAT = 5010\n");
    let nk = finp::int(&v[0], 0).unwrap_or_else(|_| die(7, &bad()));
    let nth = finp::int(&v[1], 0).unwrap_or_else(|_| die(7, &bad()));
    print!(
        "  Spectral grid size is {} by {}\n\n  Opening files : \n -----------------------------------------------\n     Input file ...\n",
        ff::i(3, nk as i64),
        ff::i(3, nth as i64)
    );
    let path = format!("{dir}/track_o.ww3");
    if !std::path::Path::new(&path).exists() {
        die(1, &head("     ERROR IN OPENING INPUT DATA FILE\n     IOSTAT =    2\n"));
    }
    let mut r = Records::open(&path);
    let (idstr, flagll, mk, mth);
    {
        let mut c = r.next();
        idstr = c.text(34);
        flagll = c.i() != 0;
        (mk, mth) = (c.i(), c.i());
    }
    let factor: Real = if flagll { 1. } else { 1.0e-3 };
    if idstr != IDTST {
        die(5, &head(&format!("     UNEXPECTED FILE ID\n     READ  : {idstr}\n     CHECK : {IDTST}\n")));
    }
    if nk != mk || nth != mth {
        die(6, &head(&format!("     UNEXPECTED SPECTRAL GRID\n     READ  : {}{}\n     CHECK : {}{}\n", ff::i(5, mk as i64), ff::i(5, mth as i64), ff::i(5, nk as i64), ff::i(5, nth as i64))));
    }
    let (uk, uth) = (mk as usize, mth as usize);
    let (th1, dth, sig, dsip);
    {
        let mut c = r.next();
        (th1, dth) = (c.f(), c.f());
        sig = c.fs(uk);
        dsip = c.fs(uk);
    }
    println!("     Output file ...");
    let mut o: Vec<u8> = Vec::new();
    let e7 = |o: &mut Vec<u8>, v: &[Real]| {
        for c in v.chunks(7) {
            for x in c {
                o.extend(ff::e(11, 4, *x as f64).into_bytes());
            }
            o.push(b'\n');
        }
    };
    o.extend(format!("{idstr}\n").into_bytes());
    o.extend(format!("{}{}{}{}\n", ff::i(6, mk as i64), ff::i(6, mth as i64), ff::e(13, 5, th1 as f64), ff::e(13, 5, dth as f64)).into_bytes());
    e7(&mut o, &sig);
    e7(&mut o, &dsip);
    let (mut iloc, mut ispec) = (0, 0);
    let mut ttst: Time2 = [-1, 0];
    let mut first = true;
    let scale: Real = 0.001;
    let report = |t: Time2, l: i32, s: i32| println!("     {} :{} points and{}  spectra.", stme21(t), ff::i(6, l as i64), ff::i(6, s as i64));
    while let Some(mut c) = r.try_next() {
        let time = [c.i(), c.i()];
        if first {
            ttst = time;
            print!("\n  Processing data : \n -----------------------------------------------\n");
            first = false;
        }
        let (x, y) = (c.f(), c.f());
        let tst = c.text(3);
        let id = c.text(32);
        let t = format!("{}{}", ff::im(8, 8, time[0] as i64), ff::im(7, 6, time[1] as i64));
        let (fx, fy) = ((factor * x) as f64, (factor * y) as f64);
        if flagll {
            o.extend(format!("{t}{}{}  {tst}  {id:<32}\n", ff::f(9, 3, fx), ff::f(9, 3, fy)).into_bytes());
        } else {
            o.extend(format!("{t}{}E3{}E3  {tst}  {id:<32}\n", ff::f(9, 2, fx), ff::f(9, 2, fy)).into_bytes());
        }
        if time == ttst {
            iloc += 1;
            if tst == "SEA" {
                ispec += 1;
            }
        } else {
            report(ttst, iloc, ispec);
            iloc = 1;
            ispec = if tst == "SEA" { 1 } else { 0 };
            ttst = time;
        }
        if tst != "SEA" {
            continue;
        }
        let mut c = r.next();
        let (dw, cx, cy, wx, wy, mut ust, asv) = (c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f());
        let spec = c.fs(uk * uth);
        if ust < 0. {
            ust = -1.0;
        }
        o.extend(
            format!(
                "{}{}{}{}{}{}{}{}\n",
                ff::f(8, 1, dw as f64),
                ff::f(6, 2, cx as f64),
                ff::f(6, 2, cy as f64),
                ff::f(8, 2, wx as f64),
                ff::f(8, 2, wy as f64),
                ff::f(9, 5, ust as f64),
                ff::f(7, 2, asv as f64),
                ff::e(12, 5, scale as f64)
            )
            .into_bytes(),
        );
        let mut l = Line { s: Vec::with_capacity(LINELN), out: &mut o };
        let mut nzero = 0;
        for ik in 0..uk {
            for ith in 0..uth {
                let sv = spec[ik + ith * uk];
                let value = (0.1f32).max(1.1 * sv / scale);
                let mut iwdth = 2 + (0).max(value.log10() as i32);
                let part: String;
                if iwdth > 9 {
                    iwdth = 9;
                    part = " 99999999".to_string();
                } else {
                    part = ff::i(9, ww3_kernels::nint(sv / scale) as i64);
                    if part.as_bytes()[(10 - iwdth) as usize] == b' ' {
                        iwdth -= 1;
                    }
                }
                if &part[7..9] == " 0" {
                    nzero += 1;
                } else {
                    if nzero != 0 {
                        l.add(&zeros(nzero));
                        nzero = 0;
                    }
                    l.add(&part.as_bytes()[(9 - iwdth) as usize..9]);
                }
            }
        }
        if nzero != 0 {
            l.add(&zeros(nzero));
        }
        if !l.s.is_empty() {
            l.flush();
        }
    }
    report(ttst, iloc, ispec);
    std::fs::write(format!("{dir}/track.ww3"), o).unwrap();
    print!("\n  End of program \n =========================================\n         WAVEWATCH III Track output \n\n");
    std::io::stdout().flush().unwrap();
}

type Time2 = [i32; 2];
