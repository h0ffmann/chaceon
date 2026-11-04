// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SPR1` calls recorded by `regtests/capture/spr1.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some call had a nonzero `EMEAN`.
//!
//! The two `MAX(1.E-7, ...)` floors of section 4 are counted from the reference's own outputs: a
//! floor acted when `FMEAN` is bit-equal to `TPIINV*EMEAN/1.E-7`, or `WNMEAN` to `(EMEAN/1.E-7)**2`, with `EMEAN` nonzero (at zero both sides are zero).
//!
//! `SPR1_FAULT` moves one input up by one ULP before the replay, a fault the comparison must
//! catch: `a` (every nonzero `A`), `cg`, `wn` (every band), `fte`, `ftf`, `ftwn`.
//!
//! Usage: spr1_cmp <capture.bin>...

use ww3_host::capture::Spr1Capture;
use ww3_kernels::constants::TPIINV;
use ww3_kernels::spr1::w3spr1;

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

/// (no bit differs, some `EMEAN` was nonzero)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Spr1Capture::read(path);
    match fault {
        "a" => c.calls.iter_mut().for_each(|k| k.a.iter_mut().filter(|x| **x != 0.).for_each(|x| *x = up(*x))),
        "cg" => c.calls.iter_mut().for_each(|k| k.cg.iter_mut().for_each(|x| *x = up(*x))),
        "wn" => c.calls.iter_mut().for_each(|k| k.wn.iter_mut().for_each(|x| *x = up(*x))),
        "fte" => c.params.fte = up(c.params.fte),
        "ftf" => c.params.ftf = up(c.params.ftf),
        "ftwn" => c.params.ftwn = up(c.params.ftwn),
        _ => {}
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    println!("{path}: NK {} NTH {}, FTE {} FTF {} FTWN {}, {} calls", p.nk, p.nth, p.fte, p.ftf, p.ftwn, c.calls.len());
    let mut tc = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    let mut eb = vec![0.0; nk];
    let (mut floor_f, mut floor_w) = (0u64, 0u64);
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3spr1(&p, &c.sig, &c.dden, &k.a, &k.cg, &k.wn, &mut eb, 0, 0, 0);
        tc[0].add(i, &[o.emean], &[k.out.emean]);
        tc[1].add(i, &[o.fmean], &[k.out.fmean]);
        tc[2].add(i, &[o.wnmean], &[k.out.wnmean]);
        tc[3].add(i, &[o.amax], &[k.out.amax]);
        let e = k.out.emean;
        let q = e / 1.0e-7;
        floor_f += (e != 0. && k.out.fmean.to_bits() == (TPIINV * e / 1.0e-7).to_bits()) as u64;
        floor_w += (e != 0. && k.out.wnmean.to_bits() == (q * q).to_bits()) as u64;
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&ww3_host::capture::Spr1Call) -> &[f32]| -> Vec<f32> {
        c.calls.iter().flat_map(|x| f(x).iter().copied()).collect()
    };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let mut ebb = vec![0.0; nk * np];
    let mut tb = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3spr1(&p, &c.sig, &c.dden, &a, &cg, &wn, &mut ebb, i * ns, i * nk, i * nk);
        tb[0].add(i, &[o.emean], &[k.out.emean]);
        tb[1].add(i, &[o.fmean], &[k.out.fmean]);
        tb[2].add(i, &[o.wnmean], &[k.out.wnmean]);
        tb[3].add(i, &[o.amax], &[k.out.amax]);
    }
    println!("  floors acting: {floor_f} FMEAN, {floor_w} WNMEAN, of {np} calls");
    for (x, name) in tc.iter().zip(["EMEAN", "FMEAN", "WNMEAN", "AMAX"]) {
        x.show(name);
    }
    for (x, name) in tb.iter().zip(["batch E", "batch F", "batch WN", "batch AMAX"]) {
        x.show(name);
    }
    (np > 0 && tc.iter().chain(tb.iter()).all(|x| x.bad == 0), tc[0].nonzero > 0)
}

fn main() {
    let fault = std::env::var("SPR1_FAULT").unwrap_or_default();
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
