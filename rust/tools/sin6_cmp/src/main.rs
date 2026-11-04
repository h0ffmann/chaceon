// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIN6` calls recorded by `regtests/capture/sin6.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`. It also prints
//! which branches the captures reached, so a zero difference is read next to what ran.
//!
//! `SIN6_FAULT=ustar` moves every `USTAR` up by one ULP before the replay, `SIN6_FAULT=dair` moves
//! every `DAIR`, `SIN6_FAULT=a` moves the first nonzero action of each call, `SIN6_FAULT=wn` moves
//! the first `WN2`: faults the comparison must catch.
//!
//! Usage: sin6_cmp <capture.bin>...

use ww3_host::capture::{Sin6Call, Sin6Capture};
use ww3_kernels::sin6::{Sin6Out, nk10hz, w3sin6, work_len};

#[derive(Default)]
struct Tally {
    n: u64,
    bad: u64,
    nonzero: u64,
    first: Option<(usize, usize, u32, u32)>,
}

impl Tally {
    fn add(&mut self, call: usize, ours: &[f32], theirs: &[f32]) {
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            self.n += 1;
            self.nonzero += (*b != 0.) as u64;
            if a.to_bits() != b.to_bits() {
                self.bad += 1;
                self.first.get_or_insert((call, i, a.to_bits(), b.to_bits()));
            }
        }
    }
    fn show(&self, name: &str) {
        println!(
            "  {name:10}{:>10} values, {} differ, {} nonzero {}",
            self.n,
            self.bad,
            self.nonzero,
            self.first.map_or(String::new(), |f| format!("first (call, index, ours, theirs) {f:x?}"))
        );
    }
}

const REACH: [&str; 14] = [
    "LFACTOR iteration ran",
    "ITERMAX reached (warning)",
    "OVERSHOT slowed DRTAU",
    "grid extended to 10 Hz (LFACTOR)",
    "grid not extended (LFACTOR)",
    "SUM(LFACT) < NK",
    "negative input (SIN6A0 > 0)",
    "KMAX < 1E-34",
    "TAU_VIS capped at 0.95 TAU_TOT",
    "viscous stress zero (U10 > 22)",
    "SIGN_NEW = -1 in the loop",
    "loop converged (ABS(ERR) < 1.54E-4)",
    "grid extended (TAU_WAVE_ATMOS)",
    "some S < 0 at TAU_WAVE_ATMOS",
];

/// (no bit differs, `S` had a nonzero value, calls that reached each branch)
fn one(path: &str, fault: &str) -> (bool, bool, [u64; 14]) {
    let mut c = Sin6Capture::read(path);
    match fault {
        "ustar" => {
            for k in &mut c.calls {
                k.point.ustar = f32::from_bits(k.point.ustar.to_bits() + 1);
            }
        }
        "dair" => {
            for k in &mut c.calls {
                k.point.dair = f32::from_bits(k.point.dair.to_bits() + 1);
            }
        }
        "a" => {
            for k in &mut c.calls {
                if let Some(v) = k.a.iter_mut().find(|v| **v != 0.) {
                    *v = f32::from_bits(v.to_bits() + 1);
                }
            }
        }
        "wn" => {
            for k in &mut c.calls {
                k.wn2[0] = f32::from_bits(k.wn2[0].to_bits() + 1);
            }
        }
        _ => {}
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    let wl = work_len(nk, ns, nk10hz(p.nk, c.sig2[0], p.xfr) as usize);
    println!(
        "{path}: NK {} NTH {}, DTH {} XFR {} SIN6A0 {} SIN6WS {}, NK10Hz {}, {} calls",
        p.nk,
        p.nth,
        p.dth,
        p.xfr,
        p.sin6a0,
        p.sin6ws,
        nk10hz(p.nk, c.sig2[0], p.xfr),
        c.calls.len()
    );
    let mut tc = [Tally::default(), Tally::default(), Tally::default()];
    let mut reached = [0u64; 14];
    let (mut s, mut d, mut w) = (vec![0.0; ns], vec![0.0; ns], vec![0.0; wl]);
    let mut o = [Sin6Out::default()];
    let mut maxit = 0;
    for (i, k) in c.calls.iter().enumerate() {
        w3sin6(&p, &c.sig2, &c.dden2, &c.ecos, &c.esin, &k.a, &k.cg, &k.wn2, &k.point, &mut s, &mut d, &mut w, &mut o, 0, 0, 0, 0);
        for (b, r) in reached.iter_mut().enumerate() {
            *r += ((o[0].reach >> b) & 1) as u64;
        }
        maxit = maxit.max(o[0].iters);
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
        tc[2].add(i, &[o[0].tauwx, o[0].tauwy, o[0].tauwnx, o[0].tauwny], &[k.out.tauwx, k.out.tauwy, k.out.tauwnx, k.out.tauwny]);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sin6Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn2));
    let (mut bs, mut bd, mut bw) = (vec![0.0; ns * np], vec![0.0; ns * np], vec![0.0; wl * np]);
    let mut bo = vec![Sin6Out::default(); np];
    for (i, k) in c.calls.iter().enumerate() {
        w3sin6(&p, &c.sig2, &c.dden2, &c.ecos, &c.esin, &a, &cg, &wn, &k.point, &mut bs, &mut bd, &mut bw, &mut bo, i * ns, i * nk, i * wl, i);
    }
    let mut tb = [Tally::default(), Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    let ours: Vec<f32> = bo.iter().flat_map(|x| [x.tauwx, x.tauwy, x.tauwnx, x.tauwny]).collect();
    let theirs: Vec<f32> = c.calls.iter().flat_map(|x| [x.out.tauwx, x.out.tauwy, x.out.tauwnx, x.out.tauwny]).collect();
    tb[2].add(0, &ours, &theirs);
    for (x, name) in tc.iter().zip(["S", "D", "TAU"]) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch S", "batch D", "batch TAU"]) {
        x.show(name);
    }
    let r: Vec<String> = REACH.iter().zip(reached).map(|(n, c)| format!("{n} {c}")).collect();
    println!("  calls that reached: {}; most LFACTOR passes {maxit}", r.join(", "));
    if tc[0].nonzero == 0 {
        println!("  every S is zero: this capture proves nothing about the arithmetic");
    }
    (np > 0 && tc.iter().chain(tb.iter()).all(|x| x.bad == 0), tc[0].nonzero > 0, reached)
}

fn main() {
    let fault = std::env::var("SIN6_FAULT").unwrap_or_default();
    let (mut same, mut reached, mut total) = (true, false, [0u64; 14]);
    for path in std::env::args().skip(1) {
        let (o, r, t) = one(&path, &fault);
        same &= o;
        reached |= r;
        for (a, b) in total.iter_mut().zip(t) {
            *a += b;
        }
    }
    let r: Vec<String> = REACH.iter().zip(total).map(|(n, c)| format!("{n} {c}")).collect();
    println!("all captures, calls that reached: {}", r.join(", "));
    let ok = same && reached;
    println!(
        "{}",
        if !same {
            "DIFFERENT"
        } else if !reached {
            "NOTHING REACHED"
        } else {
            "PARITY: every bit equal"
        }
    );
    std::process::exit(!ok as i32);
}
