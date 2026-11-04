// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SDS2` calls recorded by `regtests/capture/sds2.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some capture had a nonzero `S`: a capture of
//! zeros is named, and proves nothing about the arithmetic. It also counts which branches the
//! captures reached: the low and high-frequency `HW`, `IKHW = NK` and the `MAX(0., XHW-0.5)` clamp,
//! the three kinds of `FILT`, `POW` at its cap of 25, and `FPIT` at `FPIMIN`.
//!
//! `SDS2_FAULT=` moves one input up by one ULP before the replay: a scalar of the grid (`fte`, `dth`,
//! `fpimin`, `facti1`, `facti2`, `xfh`, `xf1`, `xf2`, `sdsaln`, `cdsa0` to `cdsa2`, `cdsb0` to `cdsb3`),
//! a point's `fpi` or `ustar`, the first `a`, the last `cg`, the first `k`, the last (or with `alfa1` the first) `alfa`, `sig` (the last
//! band), `sig1` (`SIG(NK+1)`) or `dden` (the last). Faults the comparison must catch.
//!
//! Usage: sds2_cmp <capture.bin>...

use ww3_host::capture::{Sds2Call, Sds2Capture};
use ww3_kernels::sds2::w3sds2;

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

/// Branches of one capture, counted with the same formulas as the kernel (`sds2.rs`).
#[derive(Default)]
struct Reach {
    low: u64,
    high: u64,
    ikhw_nk: u64,
    clamp: u64,
    filt0: u64,
    filt_mid: u64,
    filt1: u64,
    pow_cap: u64,
    fpit_floor: u64,
}

fn reach(c: &Sds2Capture) -> Reach {
    let p = &c.params;
    let nk = p.nk as usize;
    let mut r = Reach::default();
    for k in &c.calls {
        let (fpi, ustar) = (k.point.fpi, k.point.ustar);
        let fhw = p.xfh * fpi;
        let xr = (ww3_libm::logf(fhw) * p.facti1 + p.facti2) + 0.5;
        let ikhw = (xr as i32).min(nk as i32);
        if c.sig[nk + 1] > fhw {
            r.low += 1;
            if ikhw as usize == nk {
                r.ikhw_nk += 1;
                let frac = xr - (xr as i32) as f32;
                if (1. - frac) - 0.5 <= 0. {
                    r.clamp += 1;
                }
            }
        } else {
            r.high += 1;
        }
        let x = ((fpi * 1.5915493667125701904296875e-1) * ustar) / 9.806f32;
        r.fpit_floor += !(x > p.fpimin) as u64;
        let af2 = fpi * p.xf2;
        let af1 = fpi * p.xf1;
        let bfilt = 1. / (af2 - af1);
        let cdisp = ustar * 1.62303633987903594970703125e-2;
        for ik in 1..=nk {
            let t = c.sig[ik] * bfilt - af1 * bfilt;
            let m = if t > 0. { t } else { 0. };
            let filt = if m < 1. { m } else { 1. };
            if filt > 0. {
                if filt < 1. {
                    r.filt_mid += 1;
                } else {
                    r.filt1 += 1;
                }
                let q = p.cdsa1 / ww3_libm::powf(c.sig[ik] * cdisp, p.cdsa2);
                r.pow_cap += !(q < 25.) as u64;
            } else {
                r.filt0 += 1;
            }
        }
    }
    r
}

/// (no bit differs, `S` had a nonzero value)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Sds2Capture::read(path);
    let nk = c.params.nk as usize;
    match fault {
        "" => {}
        "fte" => c.params.fte = up(c.params.fte),
        "dth" => c.params.dth = up(c.params.dth),
        "fpimin" => c.params.fpimin = up(c.params.fpimin),
        "facti1" => c.params.facti1 = up(c.params.facti1),
        "facti2" => c.params.facti2 = up(c.params.facti2),
        "xfh" => c.params.xfh = up(c.params.xfh),
        "xf1" => c.params.xf1 = up(c.params.xf1),
        "xf2" => c.params.xf2 = up(c.params.xf2),
        "sdsaln" => c.params.sdsaln = up(c.params.sdsaln),
        "cdsa0" => c.params.cdsa0 = up(c.params.cdsa0),
        "cdsa1" => c.params.cdsa1 = up(c.params.cdsa1),
        "cdsa2" => c.params.cdsa2 = up(c.params.cdsa2),
        "cdsb0" => c.params.cdsb0 = up(c.params.cdsb0),
        "cdsb1" => c.params.cdsb1 = up(c.params.cdsb1),
        "cdsb2" => c.params.cdsb2 = up(c.params.cdsb2),
        "cdsb3" => c.params.cdsb3 = up(c.params.cdsb3),
        "fpi" => c.calls.iter_mut().for_each(|k| k.point.fpi = up(k.point.fpi)),
        "ustar" => c.calls.iter_mut().for_each(|k| k.point.ustar = up(k.point.ustar)),
        "a" => c.calls.iter_mut().for_each(|k| k.a[0] = up(k.a[0])),
        "cg" => c.calls.iter_mut().for_each(|k| k.cg[nk - 1] = up(k.cg[nk - 1])),
        "k" => c.calls.iter_mut().for_each(|k| k.k[0] = up(k.k[0])),
        "alfa" => c.calls.iter_mut().for_each(|k| k.alfa[nk - 1] = up(k.alfa[nk - 1])),
        "alfa1" => c.calls.iter_mut().for_each(|k| k.alfa[0] = up(k.alfa[0])),
        "sig" => c.sig[nk] = up(c.sig[nk]),
        "sig1" => c.sig[nk + 1] = up(c.sig[nk + 1]),
        "dden" => c.dden[nk - 1] = up(c.dden[nk - 1]),
        other => panic!("unknown SDS2_FAULT {other}"),
    }
    let p = c.params;
    let ns = (p.nk * p.nth) as usize;
    println!(
        "{path}: NK {} NTH {}, FTE {} DTH {} FPIMIN {} CDSB2 {} CDSB3 {}, {} calls",
        p.nk,
        p.nth,
        p.fte,
        p.dth,
        p.fpimin,
        p.cdsb2,
        p.cdsb3,
        c.calls.len()
    );
    let mut tc = [Tally::default(), Tally::default()];
    let (mut s, mut d) = (vec![0.0; ns], vec![0.0; ns]);
    for (i, k) in c.calls.iter().enumerate() {
        w3sds2(&p, &c.sig, &c.dden, &k.point, &k.a, &k.cg, &k.k, &k.alfa, &mut s, &mut d, 0, 0);
        tc[0].add(i, &s, &k.s);
        tc[1].add(i, &d, &k.d);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Sds2Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, k, al) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.k), cat(|x| &x.alfa));
    let (mut bs, mut bd) = (vec![0.0; ns * np], vec![0.0; ns * np]);
    for (i, x) in c.calls.iter().enumerate() {
        w3sds2(&p, &c.sig, &c.dden, &x.point, &a, &cg, &k, &al, &mut bs, &mut bd, i * ns, i * nk);
    }
    let mut tb = [Tally::default(), Tally::default()];
    tb[0].add(0, &bs, &cat(|x| &x.s));
    tb[1].add(0, &bd, &cat(|x| &x.d));
    let r = reach(&c);
    println!(
        "  reached: low HW {} high HW {} IKHW=NK {} (clamp {}), FILT 0/mid/1 {}/{}/{} bands, POW at 25: {}, FPIT at FPIMIN: {} calls",
        r.low, r.high, r.ikhw_nk, r.clamp, r.filt0, r.filt_mid, r.filt1, r.pow_cap, r.fpit_floor
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
    let fault = std::env::var("SDS2_FAULT").unwrap_or_default();
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
