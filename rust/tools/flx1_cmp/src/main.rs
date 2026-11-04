// SPDX-License-Identifier: MPL-2.0
//! Replays `W3FLX1` calls recorded by `regtests/capture/flx1.py` and compares every bit of `UST`,
//! `USTD`, `Z0` and `CD` with the reference. A diagnostic of ADR-0002, never a test. Exit 0 only
//! when no bit differs and some call had a nonzero `CD`: a capture of zeros proves nothing.
//!
//! `FLX1_FAULT=u10` moves every `U10` up by one ULP before the replay: a fault the comparison must
//! catch. The kernel's own faults are made by editing `ww3-kernels/src/flx1.rs`.
//!
//! Usage: flx1_cmp <capture.bin>...

use ww3_host::capture::Flx1Capture;
use ww3_kernels::flx1::w3flx1;

fn one(path: &str, fault: bool) -> (bool, bool) {
    let c = Flx1Capture::read(path);
    let names = ["UST", "USTD", "Z0", "CD"];
    let (mut n, mut bad, mut nonzero) = ([0u64; 4], [0u64; 4], [0u64; 4]);
    let mut first = [None; 4];
    let mut errs = 0u64;
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    let mut seen = std::collections::BTreeSet::new();
    for (i, k) in c.calls.iter().enumerate() {
        let mut input = k.input;
        if fault {
            input.u10 = f32::from_bits(input.u10.to_bits() + 1);
        }
        lo = lo.min(input.u10);
        hi = hi.max(input.u10);
        seen.insert(input.u10.to_bits());
        let o = w3flx1(&input);
        errs += o.err as u64;
        for (j, ours) in [o.ust, o.ustd, o.z0, o.cd].into_iter().enumerate() {
            n[j] += 1;
            nonzero[j] += (k.out[j] != 0.) as u64;
            if ours.to_bits() != k.out[j].to_bits() {
                bad[j] += 1;
                first[j].get_or_insert((i, ours.to_bits(), k.out[j].to_bits()));
            }
        }
    }
    println!("{path}: {} calls, {} distinct U10 in [{lo}, {hi}], {errs} with ZWND != 10", c.calls.len(), seen.len());
    for j in 0..4 {
        println!(
            "  {:5}{:>8} values, {} differ, {} nonzero {}",
            names[j],
            n[j],
            bad[j],
            nonzero[j],
            first[j].map_or(String::new(), |f| format!("first (call, ours, theirs) {f:x?}"))
        );
    }
    (!c.calls.is_empty() && bad.iter().all(|b| *b == 0), nonzero[3] > 0)
}

fn main() {
    let fault = std::env::var("FLX1_FAULT").is_ok_and(|v| v == "u10");
    let (mut same, mut reached) = (true, false);
    for path in std::env::args().skip(1) {
        let (o, r) = one(&path, fault);
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
