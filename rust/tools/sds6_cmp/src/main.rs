// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SDS6` calls recorded by `regtests/capture/sds6.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`: a capture of
//! zeros is named, and proves nothing about the arithmetic. It also prints which calls reached the
//! branches: a zero spectrum, a NaN in `A`, a nonzero `D`, a NaN in `D`.
//!
//! `SDS6_FAULT=` moves one input up by one ULP before the replay: `a1`, `a2`, `dth`, `xfr`, `sig2`
//! (the first entry), `dsii` (the first entry), `cg`, `wn` or `a` (the first element of every
//! call); `p1` and `p2` add one to an exponent, and `et` flips `SDS6ET`. Faults the comparison
//! must catch.
//!
//! Usage: sds6_cmp <capture.bin>...

use ww3_host::capture::{Sds6Call, Sds6Capture};
use ww3_kernels::sds6::{w3sds6, work_len};

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
    let mut c = Sds6Capture::read(path);
    match fault {
        "" => {}
        "a1" => c.params.sds6a1 = up(c.params.sds6a1),
        "a2" => c.params.sds6a2 = up(c.params.sds6a2),
        "dth" => c.params.dth = up(c.params.dth),
        "xfr" => c.params.xfr = up(c.params.xfr),
        "p1" => c.params.sds6p1 += 1,
        "p2" => c.params.sds6p2 += 1,
        "et" => c.params.sds6et ^= 1,
        "sig2" => c.sig2[0] = up(c.sig2[0]),
        "dsii" => c.dsii[0] = up(c.dsii[0]),
        "cg" => c.calls.iter_mut().for_each(|k| k.cg[0] = up(k.cg[0])),
        "wn" => c.calls.iter_mut().for_each(|k| k.wn[0] = up(k.wn[0])),
        "a" => c.calls.iter_mut().for_each(|k| k.a[0] = up(k.a[0])),
        other => panic!("unknown SDS6_FAULT {other}"),
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    println!(
        "{path}: NK {} NTH {}, SDSET {} SDSA1 {:e} SDSP1 {} SDSA2 {:e} SDSP2 {}, DTH {} XFR {}, {} calls",
        p.nk,
        p.nth,
        p.sds6et,
        p.sds6a1,
        p.sds6p1,
        p.sds6a2,
        p.sds6p2,
        p.dth,
        p.xfr,
        c.calls.len()
    );
    let wl = work_len(&p);
    let mut tc = [Tally::default(), Tally::default()];
    let (mut s, mut d, mut w) = (vec![0.0; ns], vec![0.0; ns], vec![0.0; wl]);
    let (mut all_zero, mut some_d, mut nan_in, mut nan_out) = (0u32, 0u32, 0u32, 0u32);
    for (i, k) in c.calls.iter().enumerate() {
        w3sds6(&p, &c.sig2, &c.dsii, &k.a, &k.cg, &k.wn, &mut s, &mut d, &mut w, 0, 0, 0);
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
        all_zero += k.a.iter().all(|x| *x == 0.) as u32;
        some_d += k.d.iter().any(|x| *x != 0. && !x.is_nan()) as u32;
        nan_in += k.a.iter().any(|x| x.is_nan()) as u32;
        nan_out += k.d.iter().any(|x| x.is_nan()) as u32;
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sds6Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let (mut bs, mut bd, mut bw) = (vec![0.0; ns * np], vec![0.0; ns * np], vec![0.0; wl * np]);
    for i in 0..np {
        w3sds6(&p, &c.sig2, &c.dsii, &a, &cg, &wn, &mut bs, &mut bd, &mut bw, i * ns, i * nk, i * wl);
    }
    let mut tb = [Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    println!("  calls: zero spectrum {all_zero}, nonzero D {some_d}, NaN in A {nan_in}, NaN in D {nan_out}");
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
    let fault = std::env::var("SDS6_FAULT").unwrap_or_default();
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
