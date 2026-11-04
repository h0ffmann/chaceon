// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIC1` calls recorded by `regtests/capture/sic1.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs, some capture had an ice call (`ICEP1 != 0`) with a
//! nonzero `S`, and `INFLAGS2(-7)` was set: a capture with no ice proves nothing about the
//! arithmetic and is named.
//!
//! `SIC1_FAULT=cg` moves every `CG` up by one ULP before the replay: a fault the comparison must
//! catch. Other faults are made by editing the kernel (`rust/port_report/W3SIC1.md`).
//!
//! Usage: sic1_cmp <capture.bin>...

use ww3_host::capture::{Sic1Call, Sic1Capture};
use ww3_kernels::sic1::w3sic1;

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

/// (no bit differs, an ice call had a nonzero `S`)
fn one(path: &str, fault: bool) -> (bool, bool) {
    let mut c = Sic1Capture::read(path);
    if fault {
        for k in &mut c.calls {
            for v in &mut k.cg {
                *v = f32::from_bits(v.to_bits() + 1);
            }
        }
    }
    let p = c.params;
    let ns = (p.nk * p.nth) as usize;
    let nk = p.nk as usize;
    let np = c.calls.len();
    let mut values: Vec<f32> = c.calls.iter().map(|k| k.icecoef1).collect();
    values.sort_by(f32::total_cmp);
    values.dedup_by(|a, b| a.to_bits() == b.to_bits());
    println!(
        "{path}: NK {} NTH {}, INFLAGS2(-7) {}, {np} calls, ICEP1 values {values:?}",
        p.nk, p.nth, c.inflags2_m7
    );
    let mut tc = [Tally::default(), Tally::default()];
    let mut branches = [0u64; 2];
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    let mut ice_nonzero = 0u64;
    for (i, k) in c.calls.iter().enumerate() {
        // Poison the outputs: `D` and `S` are intent OUT and every element must be written.
        s.fill(f32::from_bits(0x7fc0_0001));
        d.fill(f32::from_bits(0x7fc0_0001));
        let o = w3sic1(&p, &c.mapwn, &k.a, &k.cg, k.icecoef1, &mut s, &mut d, 0, 0);
        branches[o.branch as usize] += 1;
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
        if o.branch == 1 {
            ice_nonzero += k.s.iter().filter(|v| **v != 0.).count() as u64;
        }
    }
    // The layout of a device: one buffer, every point at its offset.
    let cat = |f: fn(&Sic1Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg) = (cat(|x| &x.a), cat(|x| &x.cg));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    let mut tb = [Tally::default(), Tally::default()];
    for (i, k) in c.calls.iter().enumerate() {
        w3sic1(&p, &c.mapwn, &a, &cg, k.icecoef1, &mut bs, &mut bd, i * ns, i * nk);
    }
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    println!("  branches: {} no ice, {} ice; {ice_nonzero} nonzero S in ice calls", branches[0], branches[1]);
    for (x, name) in tc.iter().zip(["S", "D"]) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch S", "batch D"]) {
        x.show(name);
    }
    if ice_nonzero == 0 {
        println!("  no ice call with a nonzero S: this capture proves nothing about the arithmetic");
    }
    if !c.inflags2_m7 {
        println!("  INFLAGS2(-7) was not set: W3SIC1 stops in the reference");
    }
    (np > 0 && c.inflags2_m7 && tc.iter().chain(tb.iter()).all(|x| x.bad == 0), ice_nonzero > 0)
}

fn main() {
    let fault = std::env::var("SIC1_FAULT").is_ok_and(|v| v == "cg");
    let (mut same, mut reached) = (true, false);
    for path in std::env::args().skip(1) {
        let (o, r) = one(&path, fault);
        same &= o;
        reached |= r;
    }
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
