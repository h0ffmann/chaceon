// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SWL6` calls recorded by `regtests/capture/swl6.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`: a capture of
//! zeros is named, and proves nothing about the arithmetic. It also prints which branches the
//! captures reached: bands above and below the `1.E-30` test of `:391`, calls with a NaN in `A`,
//! calls whose `MAXLOC` ties, and whether `SWL6CSTB1` was set.
//!
//! `SWL6_FAULT=` moves one input up by one ULP before the replay: `dth`, `swl6b1`, `sig`, `dden` and
//! `cg` (every entry: the sum of `:385` swallows one ULP in one of 36 terms), `a`, `wn` (the first entry of each call), or flips `cstb1`.
//!
//! Usage: swl6_cmp <capture.bin>...

use ww3_host::capture::{Swl6Call, Swl6Capture};
use ww3_kernels::swl6::w3swl6;

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
    let mut c = Swl6Capture::read(path);
    match fault {
        "" => {}
        "dth" => c.params.dth = up(c.params.dth),
        "swl6b1" => c.params.swl6b1 = up(c.params.swl6b1),
        "cstb1" => c.params.swl6cstb1 ^= 1,
        "sig" => c.sig[0] = up(c.sig[0]),
        "dden" => c.dden.iter_mut().for_each(|x| *x = up(*x)),
        "a" => c.calls.iter_mut().for_each(|k| k.a[0] = up(k.a[0])),
        "cg" => c.calls.iter_mut().for_each(|k| k.cg.iter_mut().for_each(|x| *x = up(*x))),
        "wn" => c.calls.iter_mut().for_each(|k| k.wn[0] = up(k.wn[0])),
        other => panic!("unknown SWL6_FAULT {other}"),
    }
    let p = c.params;
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let ns = nk * nth;
    println!(
        "{path}: NK {} NTH {}, DTH {} SWL6B1 {} CSTB1 {}, {} calls",
        p.nk,
        p.nth,
        p.dth,
        p.swl6b1,
        p.swl6cstb1 != 0,
        c.calls.len()
    );
    // What the inputs reach: the `ABAND(IK) > 1.E-30` test, NaN and negative inputs, MAXLOC ties.
    let (mut above, mut below, mut nan_calls, mut neg_wn, mut neg_cg, mut tie) = (0, 0, 0, 0, 0, 0);
    for k in &c.calls {
        let mut best: f32 = f32::NEG_INFINITY;
        let mut ties = false;
        for ik in 0..nk {
            let mut ab = 0.0f32;
            for ith in 0..nth {
                ab += k.a[ith + ik * nth];
            }
            if ab > 1.0e-30 {
                above += 1;
            } else {
                below += 1;
            }
            ties |= ab == best;
            if ab > best {
                best = ab;
            }
        }
        tie += ties as u64;
        nan_calls += k.a.iter().any(|x| x.is_nan()) as u64;
        neg_wn += k.wn.iter().any(|x| *x < 0.) as u64;
        neg_cg += k.cg.iter().any(|x| *x < 0.) as u64;
    }
    let mut tc = [Tally::default(), Tally::default()];
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    for (i, k) in c.calls.iter().enumerate() {
        w3swl6(&p, &c.sig, &c.dden, &k.a, &k.cg, &k.wn, &mut s, &mut d, 0, 0);
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Swl6Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    for i in 0..np {
        w3swl6(&p, &c.sig, &c.dden, &a, &cg, &wn, &mut bs, &mut bd, i * ns, i * nk);
    }
    let mut tb = [Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    println!(
        "  bands above 1.E-30: {above}, at or below: {below}; calls with NaN in A: {nan_calls}, with a MAXLOC tie: {tie}, \
         with WN < 0: {neg_wn}, with CG < 0: {neg_cg}"
    );
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
    let fault = std::env::var("SWL6_FAULT").unwrap_or_default();
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
