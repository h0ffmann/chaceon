// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SDS1` calls recorded by `regtests/capture/sds1.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`: a capture of
//! zeros is named, and proves nothing about the arithmetic.
//!
//! `SDS1_FAULT` names a fault the comparison must catch: `sdsc1` moves `SDSC1` up by one ULP, `order`
//! reassociates `FACTOR`, `kind` computes it in `f64`, `cube` squares `WNMEAN`, `kshift` reads the
//! next bin's wavenumber, `sfirst` takes `S` as `FACTOR*(K*A)`.
//!
//! Usage: sds1_cmp <capture.bin>...

use ww3_host::capture::{Sds1Call, Sds1Capture};
use ww3_kernels::sds1::{Sds1Params, Sds1Point, w3sds1};

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
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Sds1Capture::read(path);
    if fault == "sdsc1" {
        c.params.sdsc1 = f32::from_bits(c.params.sdsc1.to_bits() + 1);
    }
    let p = c.params;
    let ns = (p.nk * p.nth) as usize;
    let np = c.calls.len();
    let first = c.calls.first().map(|k| (k.point.emean, k.point.fmean, k.point.wnmean));
    println!("{path}: NK {} NTH {}, SDSC1 {}, (EMEAN, FMEAN, WNMEAN) {first:?}, {np} calls", p.nk, p.nth, p.sdsc1);
    let run = |k: &Sds1Call, a: &[f32], wn: &[f32], s: &mut [f32], d: &mut [f32], o: usize| {
        if fault.is_empty() || fault == "sdsc1" {
            w3sds1(&p, &k.point, a, wn, s, d, o, o);
        } else {
            faulty(fault, &p, &k.point, a, wn, s, d, o);
        }
    };
    let (mut tc, mut tb) = ([Tally::default(), Tally::default()], [Tally::default(), Tally::default()]);
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    for (i, k) in c.calls.iter().enumerate() {
        run(k, &k.a, &k.k, &mut s, &mut d, 0);
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
    }
    // The layout of a device: one buffer, every point at its offset.
    let cat = |f: fn(&Sds1Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, wn) = (cat(|x| &x.a), cat(|x| &x.k));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    for (i, k) in c.calls.iter().enumerate() {
        run(k, &a, &wn, &mut bs, &mut bd, i * ns);
    }
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
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

/// Deliberate faults, each a plausible mistake of a translation.
#[allow(clippy::too_many_arguments)]
fn faulty(fault: &str, p: &Sds1Params, pt: &Sds1Point, a: &[f32], wn: &[f32], s: &mut [f32], d: &mut [f32], o: usize) {
    let (w, e) = (pt.wnmean, pt.emean);
    let factor = match fault {
        // Another association of the same four factors.
        "order" => p.sdsc1 * (pt.fmean * ((w * w * w) * (e * e))),
        // `FACTOR` kept in double precision, narrowed once.
        "kind" => ((p.sdsc1 as f64) * (pt.fmean as f64) * ((w * w * w) as f64) * ((e * e) as f64)) as f32,
        // `WNMEAN**2` where the source says `**3`.
        "cube" => p.sdsc1 * pt.fmean * (w * w) * (e * e),
        _ => p.sdsc1 * pt.fmean * (w * w * w) * (e * e),
    };
    let ns = (p.nk * p.nth) as usize;
    for is in 0..ns {
        // `kshift` reads the neighbour's wavenumber (an off-by-one in the bin index).
        let k = if fault == "kshift" { wn[o + (is + 1) % ns] } else { wn[o + is] };
        let dd = factor * k;
        d[o + is] = dd;
        // `sfirst` takes `S` from `FACTOR * K * A` (a different association).
        s[o + is] = if fault == "sfirst" { factor * (k * a[o + is]) } else { dd * a[o + is] };
    }
}

fn main() {
    let fault = std::env::var("SDS1_FAULT").unwrap_or_default();
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
