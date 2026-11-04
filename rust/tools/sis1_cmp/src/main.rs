// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIS1` calls recorded by `regtests/capture/sis1.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture reached the scattering with a
//! nonzero `S`: a capture that never did is named, and proves nothing about the arithmetic.
//!
//! `SIS1_FAULT` breaks the replay on purpose, a fault the comparison must catch: `ice` moves every
//! `ICE` up by one ULP, `dtmin` moves `DTMIN`, `c1` moves `IS1C1`, `c2` adds 1e-6 to `IS1C2`
//! (one ULP of the default, zero, is a subnormal that the sum with `ICE` absorbs), `order` divides before the product (`A/(NTH-1)*ALPHA`), `nan` lets a NaN
//! bin through the `A >= 0` test, `ge` tests `A > 0`, `alpha` drops the `ALPHA > 0` guard.
//!
//! Usage: sis1_cmp <capture.bin>...

use ww3_host::capture::{Sis1Call, Sis1Capture};
use ww3_kernels::sis1::w3sis1;

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

/// `W3SIS1` with one deliberate fault, for the fault runs only: the kernel stays as translated.
fn faulty(fault: &str, p: &ww3_kernels::sis1::Sis1Params, a: &[f32], ice: f32, s: &mut [f32]) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    s.iter_mut().for_each(|x| *x = 0.);
    let v = p.is1c1 * ice + p.is1c2;
    let alpha = if v > 0. { v } else { 0. };
    let guard = if fault == "alpha" { ice > 0. } else { alpha > 0. && ice > 0. };
    if !guard {
        return;
    }
    for ik in 0..nk {
        for ith in 0..nth {
            let is = ith + ik * nth;
            let pass = match fault {
                "nan" => !(a[is] < 0.),
                "ge" => a[is] > 0.,
                _ => a[is] >= 0.,
            };
            if pass {
                s[is] -= alpha * a[is];
                for ith2 in 0..nth {
                    let is2 = ith2 + ik * nth;
                    if is2 != is {
                        let t = if fault == "order" { a[is] / (nth - 1) as f32 * alpha } else { alpha * a[is] / (nth - 1) as f32 };
                        s[is2] += t;
                    }
                }
            }
        }
    }
    s.iter_mut().for_each(|x| *x /= p.dtmin);
}

/// (no bit differs, the scattering ran with some nonzero `S`)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Sis1Capture::read(path);
    let bump = |x: f32| f32::from_bits(x.to_bits() + 1);
    match fault {
        "ice" => c.calls.iter_mut().for_each(|k| k.point.ice = bump(k.point.ice)),
        "dtmin" => c.params.dtmin = bump(c.params.dtmin),
        "c1" => c.params.is1c1 = bump(c.params.is1c1),
        "c2" => c.params.is1c2 += 1e-6,
        _ => {}
    }
    let p = c.params;
    let ns = (p.nk * p.nth) as usize;
    println!(
        "{path}: NK {} NTH {}, DTMIN {}, IS1C1 {}, IS1C2 {}, {} calls",
        p.nk,
        p.nth,
        p.dtmin,
        p.is1c1,
        p.is1c2,
        c.calls.len()
    );
    let mut tc = Tally::default();
    let mut ran = 0u64;
    let mut s = vec![0.0; ns];
    for (i, k) in c.calls.iter().enumerate() {
        if matches!(fault, "nan" | "ge" | "alpha" | "order") {
            faulty(fault, &p, &k.a, k.point.ice, &mut s);
            ran += s.iter().any(|x| *x != 0.) as u64;
        } else {
            ran += w3sis1(&p, &k.a, &k.point, &mut s, 0) as u64;
        }
        tc.add(i, &s, &k.s);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sis1Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let a = cat(|x| &x.a);
    let mut bs = vec![0.0; ns * np];
    let mut tb = Tally::default();
    for (i, k) in c.calls.iter().enumerate() {
        if matches!(fault, "nan" | "ge" | "alpha" | "order") {
            faulty(fault, &p, &k.a, k.point.ice, &mut bs[i * ns..(i + 1) * ns]);
        } else {
            w3sis1(&p, &a, &k.point, &mut bs, i * ns);
        }
    }
    tb.add(0, &bs, &cat(|x| &x.s));
    let (mut neg, mut nan, mut nofire) = (0, 0, 0);
    for k in &c.calls {
        neg += k.a.iter().any(|x| *x < 0.) as u64;
        nan += k.a.iter().any(|x| x.is_nan()) as u64;
        let v = p.is1c1 * k.point.ice + p.is1c2;
        nofire += !(v > 0. && k.point.ice > 0.) as u64;
    }
    println!(
        "  scattering ran on {ran} of {np} calls ({nofire} with ALPHA or ICE not above 0); {neg} calls with a negative bin, {nan} with a NaN bin"
    );
    tc.show("S");
    tb.show("batch S");
    if tc.nonzero == 0 {
        println!("  every S is zero: this capture proves nothing about the arithmetic");
    }
    (np > 0 && tc.bad == 0 && tb.bad == 0, ran > 0 && tc.nonzero > 0)
}

fn main() {
    let fault = std::env::var("SIS1_FAULT").unwrap_or_default();
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
