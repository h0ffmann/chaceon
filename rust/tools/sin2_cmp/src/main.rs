// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIN2` calls recorded by `regtests/capture/sin2.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`. It also prints
//! which branches the captures reached, so a zero difference is read next to what ran.
//!
//! `SIN2_FAULT=u` moves every `U` up by one ULP before the replay, `SIN2_FAULT=table` moves every
//! `BETATB` value by one ULP, `SIN2_FAULT=fswell` moves `FSWELL`: faults the comparison must catch.
//!
//! Usage: sin2_cmp <capture.bin>...

use ww3_host::capture_sin2::{Sin2Call, Sin2Capture};
use ww3_kernels::sin2::{Sin2Out, w3sin2};

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

const REACH: [&str; 13] = [
    "|COSFAC| < 0.0087",
    "IOMA clamped to -NRSIGA",
    "IOMA clamped to NRSIGA",
    "ICL clamped to 1",
    "ICL clamped to NRDRAG",
    "OMA < 0",
    "M1 < 1E-20",
    "NKFILT cut at NK",
    "NKFIL2 raised to 0",
    "filter loop ran",
    "transition loop ran",
    "FSWELL*D > D",
    "FPISTR = FPIMIN",
];

/// (no bit differs, `S` had a nonzero value, calls that reached each branch)
fn one(path: &str, fault: &str) -> (bool, bool, [u64; 13]) {
    let mut c = Sin2Capture::read(path);
    let bump = |x: f32| f32::from_bits(x.to_bits() + 1);
    match fault {
        "u" => c.calls.iter_mut().for_each(|k| k.point.u = bump(k.point.u)),
        "table" => c.betatb.iter_mut().for_each(|v| *v = bump(*v)),
        "fswell" => c.params.fswell = bump(c.params.fswell),
        _ => {}
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    println!(
        "{path}: NK {} NTH {}, DSIGA {} DDRAG {} ZWIND {} XFR {} FPIMIN {} FACTI1 {} FACTI2 {} FSWELL {}, {} calls",
        p.nk,
        p.nth,
        p.dsiga,
        p.ddrag,
        p.zwind,
        p.xfr,
        p.fpimin,
        p.facti1,
        p.facti2,
        p.fswell,
        c.calls.len()
    );
    let mut tc = [Tally::default(), Tally::default(), Tally::default()];
    let mut reached = [0u64; 13];
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    let mut o = [Sin2Out::default()];
    for (i, k) in c.calls.iter().enumerate() {
        s.fill(0.);
        d.fill(0.);
        w3sin2(&p, &c.sig, &c.sig2, &c.dden, &c.ecos, &c.esin, &c.betatb, &k.a, &k.cg, &k.k, &k.point, &mut s, &mut d, &mut o, 0, 0, 0);
        for (b, r) in reached.iter_mut().enumerate() {
            *r += ((o[0].reach >> b) & 1) as u64;
        }
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
        tc[2].add(i, &[o[0].fpi], &[k.fpi]);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sin2Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, kk) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.k));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    let mut bo = vec![Sin2Out::default(); np];
    for (i, k) in c.calls.iter().enumerate() {
        w3sin2(&p, &c.sig, &c.sig2, &c.dden, &c.ecos, &c.esin, &c.betatb, &a, &cg, &kk, &k.point, &mut bs, &mut bd, &mut bo, i * ns, i * nk, i);
    }
    let mut tb = [Tally::default(), Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    let ours: Vec<f32> = bo.iter().map(|x| x.fpi).collect();
    let theirs: Vec<f32> = c.calls.iter().map(|x| x.fpi).collect();
    tb[2].add(0, &ours, &theirs);
    for (x, name) in tc.iter().zip(["S", "D", "FPI"]) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch S", "batch D", "batch FPI"]) {
        x.show(name);
    }
    let r: Vec<String> = REACH.iter().zip(reached).map(|(n, c)| format!("{n} {c}")).collect();
    println!("  calls that reached: {}", r.join(", "));
    if tc[0].nonzero == 0 {
        println!("  every S is zero: this capture proves nothing about the arithmetic");
    }
    (np > 0 && tc.iter().chain(tb.iter()).all(|x| x.bad == 0), tc[0].nonzero > 0, reached)
}

fn main() {
    let fault = std::env::var("SIN2_FAULT").unwrap_or_default();
    let (mut same, mut reached, mut total) = (true, false, [0u64; 13]);
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
