// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIN3` calls recorded by `regtests/capture/sin3.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`. It also prints
//! which branches the captures reached, so a zero difference is read next to what ran.
//!
//! `SIN3_FAULT=ustar` moves every `USTAR` up by one ULP before the replay, `SIN3_FAULT=table` moves
//! the first `TAUHFT` entries a call reads: faults the comparison must catch.
//!
//! Usage: sin3_cmp <capture.bin>...

use ww3_host::capture::{Sin3Call, Sin3Capture};
use ww3_kernels::sin3::{Sin3Out, w3sin3};

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

const REACH: [&str; 9] = [
    "tail limiter acted",
    "IND clamped to 1",
    "IND clamped to IUSTAR-1",
    "J clamped to 1",
    "J clamped to IALPHA-1",
    "EXP(ZLOG) growth",
    "ZLOG >= 0 on the wind side",
    "D < 0",
    "COSWIND <= 0.01",
];

/// (no bit differs, `S` had a nonzero value, calls that reached each branch)
fn one(path: &str, fault: &str) -> (bool, bool, [u64; 9]) {
    let mut c = Sin3Capture::read(path);
    match fault {
        "ustar" => {
            for k in &mut c.calls {
                k.point.ustar = f32::from_bits(k.point.ustar.to_bits() + 1);
            }
        }
        "table" => {
            for v in c.tauhft.iter_mut() {
                *v = f32::from_bits(v.to_bits() + 1);
            }
        }
        _ => {}
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    println!(
        "{path}: NK {} NTH {}, BBETA {} AALPHA {} ZZALP {} SSINTHP {} SSWELLF(1) {} DELUST {} DELALP {}, {} calls",
        p.nk,
        p.nth,
        p.bbeta,
        p.aalpha,
        p.zzalp,
        p.ssinthp,
        p.sswellf1,
        p.delust,
        p.delalp,
        c.calls.len()
    );
    let mut tc = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    let mut reached = [0u64; 9];
    let (mut s, mut d, mut l) = (vec![0.0; ns], vec![0.0; ns], vec![0u32; ns]);
    let mut o = [Sin3Out::default()];
    for (i, k) in c.calls.iter().enumerate() {
        w3sin3(&p, &c.sig, &c.sig2, &c.dden2, &c.ecos, &c.esin, &c.tauhft, &k.a, &k.cg, &k.k, &k.point, &mut s, &mut d, &mut l, &mut o, 0, 0, 0);
        for (b, r) in reached.iter_mut().enumerate() {
            *r += ((o[0].reach >> b) & 1) as u64;
        }
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
        tc[2].add(i, &[o[0].tauwx, o[0].tauwy, o[0].tauwnx, o[0].tauwny], &[k.out.tauwx, k.out.tauwy, k.out.tauwnx, k.out.tauwny]);
        let lw: Vec<f32> = l.iter().map(|&x| x as f32).collect();
        let lt: Vec<f32> = k.llws.iter().map(|&x| x as f32).collect();
        tc[3].add(i, &lw, &lt);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sin3Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, kk) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.k));
    let (mut bs, mut bd, mut bl) = (vec![0.0; ns * np], vec![0.0; ns * np], vec![0u32; ns * np]);
    let mut bo = vec![Sin3Out::default(); np];
    for (i, k) in c.calls.iter().enumerate() {
        w3sin3(&p, &c.sig, &c.sig2, &c.dden2, &c.ecos, &c.esin, &c.tauhft, &a, &cg, &kk, &k.point, &mut bs, &mut bd, &mut bl, &mut bo, i * ns, i * nk, i);
    }
    let mut tb = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    let ours: Vec<f32> = bo.iter().flat_map(|x| [x.tauwx, x.tauwy, x.tauwnx, x.tauwny]).collect();
    let theirs: Vec<f32> = c.calls.iter().flat_map(|x| [x.out.tauwx, x.out.tauwy, x.out.tauwnx, x.out.tauwny]).collect();
    tb[2].add(0, &ours, &theirs);
    let bl: Vec<f32> = bl.iter().map(|&x| x as f32).collect();
    let rl: Vec<f32> = c.calls.iter().flat_map(|x| x.llws.iter().map(|&v| v as f32)).collect();
    tb[3].add(0, &bl, &rl);
    let names = ["S", "D", "TAU", "LLWS"];
    for (x, name) in tc.iter().zip(names) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch S", "batch D", "batch TAU", "batch LLWS"]) {
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
    let fault = std::env::var("SIN3_FAULT").unwrap_or_default();
    let (mut same, mut reached, mut total) = (true, false, [0u64; 9]);
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
