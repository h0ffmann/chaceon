// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SPR3` calls recorded by `regtests/capture/spr3.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. It counts which branches the calls reached, from the inputs: a branch that no call
//! reached is named, and its zeros prove nothing.
//!
//! `SPR3_FAULT=taut|llws|wn|a` moves one input by one ULP (or flips one `LLWS`) before the replay: a
//! fault the comparison must catch.
//!
//! Usage: spr3_cmp <capture.bin>...

use ww3_host::capture::{Spr3Call, Spr3Capture};
use ww3_kernels::constants::TPIINV;
use ww3_kernels::spr3::{ITAUMAX, JUMAX, Spr3Out, TAUWMAX, w3spr3};

const NAMES: [&str; 11] =
    ["EMEAN", "FMEAN", "FMEANS", "WNMEAN", "AMAX", "USTAR", "USDIR", "CD", "Z0", "CHARN", "FMEANWS"];

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
            "  {name:10}{:>9} values, {} differ, {} nonzero {}",
            self.n,
            self.bad,
            self.nonzero,
            self.first.map_or(String::new(), |f| format!("first (call, index, ours, theirs) {f:x?}"))
        );
    }
}

fn fields(o: &Spr3Out) -> [f32; 11] {
    [o.emean, o.fmean, o.fmeans, o.wnmean, o.amax, o.ustar, o.usdir, o.cd, o.z0, o.charn, o.fmeanws]
}

/// (no bit differs, calls)
fn one(path: &str, fault: &str) -> (bool, usize) {
    let mut c = Spr3Capture::read(path);
    let bump = |x: &mut f32| *x = f32::from_bits(x.to_bits() + 1);
    for k in &mut c.calls {
        match fault {
            "taut" => {}
            "llws" => k.llws[0] ^= 1,
            "wn" => bump(&mut k.wn[8]),
            "a" => bump(&mut k.a[0]),
            _ => {}
        }
    }
    if fault == "taut" {
        bump(&mut c.taut[2000]);
        for i in 0..c.taut.len() {
            c.taut[i] = f32::from_bits(c.taut[i].to_bits() + (c.taut[i] != 0.) as u32);
        }
    }
    let p = c.params;
    let (nk, ns) = (p.nk as usize, p.nspec as usize);
    // Which branches the inputs reached.
    let (mut calm, mut tauw_hi, mut ind_hi, mut j_hi, mut deli_hi, mut ust_lo) = (0, 0, 0, 0, 0, 0);
    let (mut fmean_lo, mut fmeans_lo, mut fmeanws_lo, mut umax, mut ximax) = (0, 0, 0, 0.0f32, 0.0f32);
    for k in &c.calls {
        let pt = &k.point;
        let tauw = (pt.tauwx * pt.tauwx + pt.tauwy * pt.tauwy).sqrt();
        let tl = tauw.min(TAUWMAX).max(0.);
        let xi = tl.sqrt() / p.deltauw;
        let xj = pt.u / p.delu;
        calm += (pt.u <= 0.) as u32;
        tauw_hi += (tauw > TAUWMAX) as u32;
        ind_hi += ((xi as i32) >= ITAUMAX - 1) as u32;
        j_hi += ((xj as i32) >= JUMAX - 1) as u32;
        deli_hi += (xi - ((ITAUMAX - 1).min(xi as i32)) as f32 > 1.) as u32;
        ust_lo += (k.out[5] <= 0.001) as u32;
        let tail = TPIINV * c.sig[nk];
        fmean_lo += (k.out[1] == tail) as u32;
        fmeans_lo += (k.out[2] == tail) as u32;
        fmeanws_lo += (k.out[10] == tail) as u32;
        umax = umax.max(pt.u);
        ximax = ximax.max(xi);
    }
    println!(
        "{path}: NK {} NTH {}, {} calls, U max {umax}, XI max {ximax}, DELTAUW {} DELU {}, TAUT nonzero {}",
        p.nk,
        p.nth,
        c.calls.len(),
        p.deltauw,
        p.delu,
        c.taut.iter().filter(|x| **x != 0.).count()
    );
    println!(
        "  reached: U<=0 {calm}, TAUW>TAUWMAX {tauw_hi}, IND clamp {ind_hi}, J clamp {j_hi}, DELI1 clamp {deli_hi}, USTAR<=0.001 {ust_lo}; tail-frequency branch FMEAN {fmean_lo} FMEANS {fmeans_lo} FMEANWS {fmeanws_lo}"
    );
    let mut tc: Vec<Tally> = (0..11).map(|_| Tally::default()).collect();
    let mut w = vec![0.0; 2 * nk];
    let mut o = [Spr3Out::default()];
    for (i, k) in c.calls.iter().enumerate() {
        w3spr3(&p, &c.sig, &c.dden, &c.taut, &k.a, &k.cg, &k.wn, &k.llws, &k.point, &mut w, &mut o, 0, 0, 0, 0);
        for (t, (a, b)) in tc.iter_mut().zip(fields(&o[0]).iter().zip(&k.out)) {
            t.add(i, std::slice::from_ref(a), std::slice::from_ref(b));
        }
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Spr3Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|x| f(x).iter().copied()).collect()
    };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let llws: Vec<u32> = c.calls.iter().flat_map(|x| x.llws.iter().copied()).collect();
    let mut bw = vec![0.0; 2 * nk * np];
    let mut bo = vec![Spr3Out::default(); np];
    for (i, k) in c.calls.iter().enumerate() {
        w3spr3(&p, &c.sig, &c.dden, &c.taut, &a, &cg, &wn, &llws, &k.point, &mut bw, &mut bo, i * ns, i * nk, i * 2 * nk, i);
    }
    let mut tb: Vec<Tally> = (0..11).map(|_| Tally::default()).collect();
    for (i, k) in c.calls.iter().enumerate() {
        for (t, (a, b)) in tb.iter_mut().zip(fields(&bo[i]).iter().zip(&k.out)) {
            t.add(i, std::slice::from_ref(a), std::slice::from_ref(b));
        }
    }
    for (x, name) in tc.iter().zip(NAMES) {
        x.show(name);
    }
    let batch = tb.iter().map(|t| t.bad).sum::<u64>();
    println!("  batch layout: {batch} of {} values differ", tb.iter().map(|t| t.n).sum::<u64>());
    let calls_ok = tc.iter().all(|x| x.bad == 0) && batch == 0 && np > 0;
    (calls_ok && tc[0].nonzero > 0, np)
}

fn main() {
    let fault = std::env::var("SPR3_FAULT").unwrap_or_default();
    let (mut same, mut calls) = (true, 0);
    for path in std::env::args().skip(1) {
        let (o, n) = one(&path, &fault);
        same &= o;
        calls += n;
    }
    println!("{calls} calls in all");
    println!("{}", if same && calls > 0 { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!(same && calls > 0) as i32);
}
