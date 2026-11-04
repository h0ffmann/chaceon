// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SPR6` calls recorded by `regtests/capture/spr6.py` and compares bits with the
//! reference, in call layout and in the one-buffer layout of a device. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and some call had a nonzero `EMEAN` and a nonzero `FP`.
//!
//! It counts which branches the calls reached, from the inputs: the `HSMIN` gate of section 5 open
//! and closed, how close `4*SQRT(EMEAN)` came to `HSMIN`, the `MAX(1.E-7, ...)` floors of section 4,
//! the `MAX(1.E-10, ...)` floor of section 5, and the NaN and negative inputs. A branch that no call
//! reached is named, and its zeros prove nothing.
//!
//! `SPR6_FAULT` moves one input up by one ULP before the replay, a fault the comparison must catch:
//! `a` (every nonzero `A`), `cg`, `wn`, `sig`, `dden`, `dsii`, `dth`, `fte`, `ftf`, `ftwn`.
//!
//! Usage: spr6_cmp <capture.bin>...

use ww3_host::capture::{Spr6Call, Spr6Capture};
use ww3_kernels::constants::TPIINV;
use ww3_kernels::spr6::{HSMIN, Spr6Out, w3spr6};

const NAMES: [&str; 5] = ["EMEAN", "FMEAN", "WNMEAN", "AMAX", "FP"];

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

fn fields(o: &Spr6Out) -> [f32; 5] {
    [o.emean, o.fmean, o.wnmean, o.amax, o.fp]
}

/// (no bit differs, some `EMEAN` and some `FP` were nonzero)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Spr6Capture::read(path);
    match fault {
        "a" => c.calls.iter_mut().for_each(|k| k.a.iter_mut().filter(|x| **x != 0.).for_each(|x| *x = up(*x))),
        "cg" => c.calls.iter_mut().for_each(|k| k.cg.iter_mut().for_each(|x| *x = up(*x))),
        "wn" => c.calls.iter_mut().for_each(|k| k.wn.iter_mut().for_each(|x| *x = up(*x))),
        "sig" => c.sig.iter_mut().skip(1).for_each(|x| *x = up(*x)),
        "dden" => c.dden.iter_mut().for_each(|x| *x = up(*x)),
        "dsii" => c.dsii.iter_mut().for_each(|x| *x = up(*x)),
        "dth" => c.params.dth = up(c.params.dth),
        "fte" => c.params.fte = up(c.params.fte),
        "ftf" => c.params.ftf = up(c.params.ftf),
        "ftwn" => c.params.ftwn = up(c.params.ftwn),
        _ => {}
    }
    let p = c.params;
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let ns = nk * nth;
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

    // Which branches the reference's own outputs and the inputs reached.
    let (mut gate_open, mut gate_shut, mut near, mut den_floor, mut fmean_floor, mut wn_floor) = (0, 0, 0, 0, 0, 0);
    let (mut nan_in, mut neg_e, mut nan_out, mut zero_e) = (0, 0, 0, 0);
    for k in &c.calls {
        let o = &k.out;
        let hs = 4.0 * o.emean.sqrt();
        if hs > HSMIN {
            gate_open += 1;
        } else {
            gate_shut += 1;
        }
        near += ((hs / HSMIN - 1.0).abs() < 1.0e-5) as u32;
        zero_e += (o.emean == 0.) as u32;
        neg_e += (o.emean < 0.) as u32;
        nan_in += k.a.iter().any(|x| x.is_nan()) as u32;
        nan_out += o.emean.is_nan() as u32;
        // The 1.E-7 floors: the reference's result bit-equal to the floored quotient, EMEAN nonzero.
        let e = o.emean;
        let q = e / 1.0e-7;
        fmean_floor += (e != 0. && o.fmean.to_bits() == (TPIINV * e / 1.0e-7).to_bits()) as u32;
        wn_floor += (e != 0. && o.wnmean.to_bits() == (q * q).to_bits()) as u32;
        // The 1.E-10 floor: the gate open and SUM(EB**4*DSII) below it.
        if hs > HSMIN {
            let mut den = 0f32;
            for ik in 0..nk {
                let s = k.a[ik * nth..(ik + 1) * nth].iter().fold(0f32, |s, x| s + x);
                let e = s * c.sig[ik + 1] / k.cg[ik] * p.dth;
                let e2 = e * e;
                den += e2 * e2 * c.dsii[ik];
            }
            den_floor += (den < 1.0e-10) as u32;
        }
    }
    println!(
        "  reached: HSMIN gate open {gate_open}, shut {gate_shut}, within 1e-5 of HSMIN {near}; FMEAN floor {fmean_floor}, WNMEAN floor {wn_floor}, FP floor (SUM < 1.E-10, gate open) {den_floor}; EMEAN zero {zero_e}, negative {neg_e}, NaN {nan_out}; calls with a NaN in A {nan_in}"
    );

    let mut tc: Vec<Tally> = (0..5).map(|_| Tally::default()).collect();
    let mut eb = vec![0.0; nk];
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3spr6(&p, &c.sig, &c.dden, &c.dsii, &k.a, &k.cg, &k.wn, &mut eb, 0, 0, 0);
        for (t, (a, b)) in tc.iter_mut().zip(fields(&o).iter().zip(fields(&k.out).iter())) {
            t.add(i, std::slice::from_ref(a), std::slice::from_ref(b));
        }
    }
    // The layout of a device: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&Spr6Call) -> &Vec<f32>| -> Vec<f32> { c.calls.iter().flat_map(|x| f(x).iter().copied()).collect() };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let mut ebb = vec![0.0; nk * np];
    let mut tb: Vec<Tally> = (0..5).map(|_| Tally::default()).collect();
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3spr6(&p, &c.sig, &c.dden, &c.dsii, &a, &cg, &wn, &mut ebb, i * ns, i * nk, i * nk);
        for (t, (x, y)) in tb.iter_mut().zip(fields(&o).iter().zip(fields(&k.out).iter())) {
            t.add(i, std::slice::from_ref(x), std::slice::from_ref(y));
        }
    }
    for (x, name) in tc.iter().zip(NAMES) {
        x.show(name);
    }
    let batch = tb.iter().map(|t| t.bad).sum::<u64>();
    println!("  batch layout: {batch} of {} values differ", tb.iter().map(|t| t.n).sum::<u64>());
    (np > 0 && tc.iter().all(|x| x.bad == 0) && batch == 0, tc[0].nonzero > 0 && tc[4].nonzero > 0)
}

fn main() {
    let fault = std::env::var("SPR6_FAULT").unwrap_or_default();
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
