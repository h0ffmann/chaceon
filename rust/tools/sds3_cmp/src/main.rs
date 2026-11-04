// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SDS3` calls recorded by `regtests/capture/sds3.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`: a capture of
//! zeros is named, and proves nothing about the arithmetic. It also prints how many calls had
//! `WNMEAN` below the `1.E-10` floor of `:1382`, the one branch of the routine.
//!
//! `SDS3_FAULT=` moves one input up by one ULP before the replay: `emean`, `fmean`, `wnmean`,
//! `ssdsc1`, `ddelta1`, `ddelta2` or `k` (the first wavenumber). Faults the comparison must catch.
//!
//! Usage: sds3_cmp <capture.bin>...

use ww3_host::capture::{Sds3Call, Sds3Capture};
use ww3_kernels::sds3::w3sds3;

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

fn up(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

/// (no bit differs, `S` had a nonzero value)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Sds3Capture::read(path);
    match fault {
        "" => {}
        "ssdsc1" => c.params.ssdsc1 = up(c.params.ssdsc1),
        "ddelta1" => c.params.ddelta1 = up(c.params.ddelta1),
        "ddelta2" => c.params.ddelta2 = up(c.params.ddelta2),
        "emean" => c.calls.iter_mut().for_each(|k| k.point.emean = up(k.point.emean)),
        "fmean" => c.calls.iter_mut().for_each(|k| k.point.fmean = up(k.point.fmean)),
        "wnmean" => c.calls.iter_mut().for_each(|k| k.point.wnmean = up(k.point.wnmean)),
        "k" => c.calls.iter_mut().for_each(|k| k.k[0] = up(k.k[0])),
        other => panic!("unknown SDS3_FAULT {other}"),
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    println!(
        "{path}: NK {} NTH {}, SSDSC1 {} DDELTA1 {} DDELTA2 {}, {} calls",
        p.nk,
        p.nth,
        p.ssdsc1,
        p.ddelta1,
        p.ddelta2,
        c.calls.len()
    );
    let floored = c.calls.iter().filter(|k| k.point.wnmean < 1.0e-10).count();
    let mut tc = [Tally::default(), Tally::default()];
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    for (i, k) in c.calls.iter().enumerate() {
        w3sds3(&p, &k.k, &k.a, &k.point, &mut s, &mut d, 0, 0);
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sds3Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, k) = (cat(|x| &x.a), cat(|x| &x.k));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    for (i, x) in c.calls.iter().enumerate() {
        w3sds3(&p, &k, &a, &x.point, &mut bs, &mut bd, i * ns, i * nk);
    }
    let mut tb = [Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    println!("  calls with WNMEAN below 1.E-10: {floored}");
    for (x, name) in tc.iter().zip(["S", "D"]) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch S", "batch D"]) {
        x.show(name);
    }
    if tc[0].nonzero == 0 {
        println!("  every S is zero: this capture proves nothing about the arithmetic");
    }
    (np > 0 && tc.iter().chain(tb.iter()).all(|x| x.bad == 0), tc[0].nonzero > 0)
}

fn main() {
    let fault = std::env::var("SDS3_FAULT").unwrap_or_default();
    let (mut same, mut reached) = (true, false);
    for path in std::env::args().skip(1) {
        let (o, r) = one(&path, &fault);
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
