// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIN1` calls recorded by `regtests/capture/sin1.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `D`: a capture of
//! zeros is named, and proves nothing about the arithmetic. It also prints how many `D` values the
//! `MAX(0., ...)` clamped to zero and how many it kept, so a capture that never reaches one side is
//! visible.
//!
//! Usage: sin1_cmp [--fault <name>] <capture.bin>...
//! Faults, each applied to the input before the replay and each of which must make the exit nonzero:
//!   ustar  every USTAR up by one ULP        usdir  every USDIR up by one ULP
//!   k      every K up by one ULP            sinc1  SINC1 up by one ULP
//!   sig2   SIG2 up by one ULP               ecos   ECOS up by one ULP
//!   esin   ESIN up by one ULP               a      every A up by one ULP

use ww3_host::capture::{Sin1Call, Sin1Capture};
use ww3_kernels::sin1::w3sin1;

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

fn up(v: f32) -> f32 {
    f32::from_bits(v.to_bits() + 1)
}

/// (no bit differs, `D` had a nonzero value)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Sin1Capture::read(path);
    match fault {
        "" => {}
        "ustar" => c.calls.iter_mut().for_each(|k| k.point.ustar = up(k.point.ustar)),
        "usdir" => c.calls.iter_mut().for_each(|k| k.point.usdir = up(k.point.usdir)),
        "k" => c.calls.iter_mut().for_each(|k| k.k.iter_mut().for_each(|v| *v = up(*v))),
        "a" => c.calls.iter_mut().for_each(|k| k.a.iter_mut().for_each(|v| *v = up(*v))),
        "sinc1" => c.params.sinc1 = up(c.params.sinc1),
        "sig2" => c.sig2.iter_mut().for_each(|v| *v = up(*v)),
        "ecos" => c.ecos.iter_mut().for_each(|v| *v = up(*v)),
        "esin" => c.esin.iter_mut().for_each(|v| *v = up(*v)),
        other => panic!("unknown fault {other}"),
    }
    let p = c.params;
    let ns = p.nspec as usize;
    println!("{path}: NK {} NTH {}, SINC1 {:e}, {} calls", p.nk, p.nth, p.sinc1, c.calls.len());
    let (mut ts, mut td) = (Tally::default(), Tally::default());
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    let (mut zero, mut kept) = (0u64, 0u64);
    for (i, k) in c.calls.iter().enumerate() {
        w3sin1(&p, &c.sig2, &c.ecos, &c.esin, &k.a, &k.k, &k.point, &mut s, &mut d, 0);
        ts.add(i, &s, &k.s);
        td.add(i, &d, &k.d);
        for v in &k.d {
            if *v == 0. {
                zero += 1
            } else {
                kept += 1
            }
        }
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sin1Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, kk) = (cat(|x| &x.a), cat(|x| &x.k));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    for (i, k) in c.calls.iter().enumerate() {
        w3sin1(&p, &c.sig2, &c.ecos, &c.esin, &a, &kk, &k.point, &mut bs, &mut bd, i * ns);
    }
    let (mut tbs, mut tbd) = (Tally::default(), Tally::default());
    tbs.add(0, &bs, &cat(|x| &x.s));
    tbd.add(0, &bd, &cat(|x| &x.d));
    println!("  reference D: {zero} values zero (clamped by MAX(0., ...) or the band is calm), {kept} nonzero");
    let (lo, hi) = c.calls.iter().fold((f32::MAX, f32::MIN), |(l, h), k| (l.min(k.point.ustar), h.max(k.point.ustar)));
    let (dl, dh) = c.calls.iter().fold((f32::MAX, f32::MIN), |(l, h), k| (l.min(k.point.usdir), h.max(k.point.usdir)));
    println!("  USTAR in [{lo}, {hi}], USDIR in [{dl}, {dh}] rad");
    ts.show("S");
    td.show("D");
    tbs.show("batch S");
    tbd.show("batch D");
    if td.nonzero == 0 {
        println!("  every D is zero: this capture proves nothing about the arithmetic");
    }
    (np > 0 && [&ts, &td, &tbs, &tbd].iter().all(|x| x.bad == 0), td.nonzero > 0)
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut fault = String::new();
    if args.first().is_some_and(|a| a == "--fault") {
        args.remove(0);
        fault = args.remove(0);
    }
    let (mut same, mut reached) = (true, false);
    for path in &args {
        let (o, r) = one(path, &fault);
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
