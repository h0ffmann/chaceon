// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SPR2` calls recorded by `regtests/capture/spr2.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some call had a nonzero `EMEAN`.
//!
//! It prints which guards and which winds and depths the calls reached, counted from the inputs and
//! the reference's own outputs: a guard that no call reached is named, and its zeros prove nothing.
//!
//! `SPR2_FAULT` moves one input up by one ULP before the replay, a fault the comparison must
//! catch: `a` (every nonzero `A`), `cg`, `wn`, `sig`, `dden` (every band), `dth`, `fte`, `ftf`,
//! `ftwn`, `fpi`, `ustar`.
//!
//! Usage: spr2_cmp <capture.bin>...

use ww3_host::capture::{Spr2Call, Spr2Capture};
use ww3_kernels::constants::{GRAV, TPIINV};
use ww3_kernels::spr2::w3spr2;

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

const NAMES: [&str; 6] = ["EMEAN", "FMEAN", "WNMEAN", "AMAX", "FP", "ALFA"];

fn fields(o: &ww3_kernels::spr2::Spr2Out) -> [f32; 5] {
    [o.emean, o.fmean, o.wnmean, o.amax, o.fp]
}

/// (no bit differs, some `EMEAN` was nonzero)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Spr2Capture::read(path);
    match fault {
        "a" => c.calls.iter_mut().for_each(|k| k.a.iter_mut().filter(|x| **x != 0.).for_each(|x| *x = up(*x))),
        "cg" => c.calls.iter_mut().for_each(|k| k.cg.iter_mut().for_each(|x| *x = up(*x))),
        "wn" => c.calls.iter_mut().for_each(|k| k.wn.iter_mut().for_each(|x| *x = up(*x))),
        "sig" => c.sig.iter_mut().for_each(|x| *x = up(*x)),
        "dden" => c.dden.iter_mut().for_each(|x| *x = up(*x)),
        "dth" => c.params.dth = up(c.params.dth),
        "fte" => c.params.fte = up(c.params.fte),
        "ftf" => c.params.ftf = up(c.params.ftf),
        "ftwn" => c.params.ftwn = up(c.params.ftwn),
        "fpi" => c.calls.iter_mut().for_each(|k| k.point.fpi = up(k.point.fpi)),
        "ustar" => c.calls.iter_mut().for_each(|k| k.point.ustar = up(k.point.ustar)),
        _ => {}
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, (p.nk * p.nth) as usize);
    println!(
        "{path}: NK {} NTH {}, DTH {} FTE {} FTF {} FTWN {}, {} calls",
        p.nk,
        p.nth,
        p.dth,
        p.fte,
        p.ftf,
        p.ftwn,
        c.calls.len()
    );

    // What the inputs reached.
    let (mut ust_lo, mut fpistr_lo, mut floor_f, mut floor_w, mut empty) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut dmin, mut dmax, mut umin, mut umax) = (f32::MAX, 0f32, f32::MAX, 0f32);
    let (mut fpimin, mut fpimax) = (f32::MAX, 0f32);
    for k in &c.calls {
        let pt = &k.point;
        let ust = pt.ustar.max(0.0001);
        ust_lo += (pt.ustar < 0.0001) as u64;
        fpistr_lo += (pt.fpi * ust / GRAV < 0.008) as u64;
        let e = k.out.emean;
        let q = e / 1.0e-7;
        floor_f += (e != 0. && k.out.fmean.to_bits() == (TPIINV * e / 1.0e-7).to_bits()) as u64;
        floor_w += (e != 0. && k.out.wnmean.to_bits() == (q * q).to_bits()) as u64;
        empty += (e == 0.) as u64;
        dmin = dmin.min(pt.depth);
        dmax = dmax.max(pt.depth);
        umin = umin.min(pt.u);
        umax = umax.max(pt.u);
        fpimin = fpimin.min(pt.fpi);
        fpimax = fpimax.max(pt.fpi);
    }
    println!(
        "  reached: USTAR<1E-4 {ust_lo}, FPISTR<0.008 {fpistr_lo}, FMEAN floor {floor_f}, WNMEAN floor {floor_w}, EMEAN=0 {empty}; DEPTH {dmin}..{dmax}, U {umin}..{umax}, FPI {fpimin}..{fpimax}"
    );

    let mut tc: Vec<Tally> = (0..6).map(|_| Tally::default()).collect();
    let mut eb = vec![0.0; nk];
    let mut alfa = vec![0.0; nk];
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3spr2(&p, &c.sig, &c.dden, &k.a, &k.cg, &k.wn, &k.point, &mut eb, &mut alfa, 0, 0, 0);
        for (j, (a, b)) in fields(&o).iter().zip(fields(&k.out)).enumerate() {
            tc[j].add(i, std::slice::from_ref(a), std::slice::from_ref(&b));
        }
        tc[5].add(i, &alfa, &k.alfa);
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Spr2Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|x| f(x).iter().copied()).collect()
    };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let mut ebb = vec![0.0; nk * np];
    let mut alb = vec![0.0; nk * np];
    let mut tb: Vec<Tally> = (0..6).map(|_| Tally::default()).collect();
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3spr2(&p, &c.sig, &c.dden, &a, &cg, &wn, &k.point, &mut ebb, &mut alb, i * ns, i * nk, i * nk);
        for (j, (x, y)) in fields(&o).iter().zip(fields(&k.out)).enumerate() {
            tb[j].add(i, std::slice::from_ref(x), std::slice::from_ref(&y));
        }
        tb[5].add(i, &alb[i * nk..(i + 1) * nk], &k.alfa);
    }
    for (x, name) in tc.iter().zip(NAMES) {
        x.show(name);
    }
    let batch: u64 = tb.iter().map(|t| t.bad).sum();
    println!("  batch layout: {batch} of {} values differ", tb.iter().map(|t| t.n).sum::<u64>());
    (np > 0 && tc.iter().all(|x| x.bad == 0) && batch == 0, tc[0].nonzero > 0)
}

fn main() {
    let fault = std::env::var("SPR2_FAULT").unwrap_or_default();
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
