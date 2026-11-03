// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SBT4` calls recorded by `regtests/capture/sbt4.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. Also rebuilds `ERFTABLE` with
//! `TABU_ERF` and `DELXERF`, and compares them with what the reference's first call left. A
//! diagnostic of ADR-0002, never a test. Exit 0 only when no bit differs and some capture had a
//! nonzero `S`: a capture of zeros (deep water, every band past `WN*D < 6`) is named, and proves
//! nothing about the arithmetic.
//!
//! `SBT4_FAULT=psic` moves every `PSIC` up by one ULP before the replay: a fault the comparison must
//! catch.
//!
//! Usage: sbt4_cmp <capture.bin>...

use ww3_host::capture::{Sbt4Call, Sbt4Capture};
use ww3_kernels::sbt4::{SIZEERFTABLE, Sbt4Tables, delxerf, tabu_erf, w3sbt4};

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

/// (no bit differs, `S` had a nonzero value)
fn one(path: &str, fault: bool) -> (bool, bool) {
    let mut c = Sbt4Capture::read(path);
    if fault {
        for k in &mut c.calls {
            k.point.psic = f32::from_bits(k.point.psic.to_bits() + 1);
        }
    }
    let mut p = c.params;
    p.delxerf = delxerf();
    let mut erf = vec![0.0f32; SIZEERFTABLE as usize + 1];
    tabu_erf(&mut erf);
    let mut te = Tally::default();
    if let Some(k) = c.calls.first() {
        te.add(0, &erf, &k.erftable);
        te.add(0, &[p.delxerf], &[k.delxerf]);
    }
    let t = Sbt4Tables { sig: &c.sig, dden: &c.dden, ecos: &c.ecos, esin: &c.esin, fwtable: &c.fwtable, erftable: &erf };
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    let first = c.calls.first().map(|k| (k.point.depth, k.point.d50, k.point.psic));
    println!(
        "{path}: NK {} NTH {}, SBTCX(1:7) {:?}, (DEPTH, D50, PSIC) {first:?}, {} calls",
        p.nk,
        p.nth,
        &p.sbtcx[..7],
        c.calls.len()
    );
    let mut tc = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    let mut branches = [0u64; 3];
    let (mut s, mut d, mut w) = (vec![0.0; ns], vec![0.0; ns], vec![0.0; nk]);
    for (i, k) in c.calls.iter().enumerate() {
        let mut bf = k.bedform_in;
        let o = w3sbt4(&p, &t, &k.a, &k.cg, &k.wn, &k.point, &mut bf, &mut s, &mut d, &mut w, 0, 0, 0, 0);
        branches[o.branch as usize] += 1;
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
        tc[2].add(i, &o.taubbl, &k.taubbl);
        tc[3].add(i, &bf, &k.bedform);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sbt4Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let mut bb = cat(|x| &x.bedform_in);
    let (mut bs, mut bd, mut bw) = (vec![0.0; ns * np], vec![0.0; ns * np], vec![0.0; nk * np]);
    let mut tb = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3sbt4(&p, &t, &a, &cg, &wn, &k.point, &mut bb, &mut bs, &mut bd, &mut bw, i * ns, i * nk, i * nk, i * 3);
        tb[2].add(i, &o.taubbl, &k.taubbl);
    }
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    tb[3].add(0, &bb, &cat(|x| &x.bedform));
    println!("  branches: {} relict, {} ripples, {} subgrid", branches[0], branches[1], branches[2]);
    te.show("ERFTABLE");
    for (x, name) in tc.iter().zip(["S", "D", "TAUBBL", "BEDFORM"]) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch S", "batch D", "batch TAU", "batch BED"]) {
        x.show(name);
    }
    if tc[0].nonzero == 0 {
        println!("  every S is zero: this capture proves nothing about the arithmetic");
    }
    (np > 0 && te.bad == 0 && tc.iter().chain(tb.iter()).all(|x| x.bad == 0), tc[0].nonzero > 0)
}

fn main() {
    let fault = std::env::var("SBT4_FAULT").is_ok_and(|v| v == "psic");
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
