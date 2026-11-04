// SPDX-License-Identifier: MPL-2.0
//! Replays `W3FLX2` calls recorded by `regtests/capture/flx2.py` and compares every bit of `UST`,
//! `USTD`, `Z0` and `CD` with the reference, one call at a time and as one batch, the layout of a
//! device. It also rebuilds `DISTAB`'s `EWN1`, `DSIE` and `N1MAX` with `ww3-host` and compares them
//! with what the reference held. A diagnostic of ADR-0002, never a test. Exit 0 only when no bit
//! differs and some call had a nonzero `CD`.
//!
//! It prints which branches the captured calls reached: the deep-water `WNP` (`I1 > N1MAX`), the
//! table's interval, and each limiter of `:194-222`. `RDCH` is not an input, so its floor is read
//! from the result: `RDCH = 0` gives one `CD`, computed here with the routine's own expression.
//!
//! `FLX2_FAULT=` moves one input of every call up by one ULP: `zwind`, `depth`, `fp`, `u` or `ust`.
//! Faults the comparison must catch. The kernel's own faults are made by editing `ww3-kernels/src/flx2.rs`.
//!
//! Usage: flx2_cmp <capture.bin>...

use ww3_host::capture::Flx2Capture;
use ww3_host::disp::Dispersion;
use ww3_kernels::flx2::{Flx2In, Flx2Out, w3flx2};

fn up(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

/// (no bit differs, some `CD` nonzero)
fn one(path: &str, fault: &str) -> (bool, bool) {
    let mut c = Flx2Capture::read(path);
    match fault {
        "" => {}
        "zwind" => c.calls.iter_mut().for_each(|k| k.input.zwind = up(k.input.zwind)),
        "depth" => c.calls.iter_mut().for_each(|k| k.input.depth = up(k.input.depth)),
        "fp" => c.calls.iter_mut().for_each(|k| k.input.fp = up(k.input.fp)),
        "u" => c.calls.iter_mut().for_each(|k| k.input.u = up(k.input.u)),
        "ust" => c.calls.iter_mut().for_each(|k| k.input.ust = up(k.input.ust)),
        other => panic!("unknown FLX2_FAULT {other}"),
    }
    let p = c.params;
    println!("{path}: NITTIN {} CINXSI {} DSIE {} N1MAX {}, {} calls", p.nittin, p.cinxsi, p.dsie, p.n1max, c.calls.len());
    // The table `DISTAB` builds, from the host's own `WAVNU2`: it must be the reference's.
    let disp = Dispersion::new();
    let (dsie, n1max, ewn1, _) = disp.tables();
    let table_ok = dsie.to_bits() == p.dsie.to_bits()
        && n1max == p.n1max
        && ewn1.len() == c.ewn1.len()
        && ewn1.iter().zip(&c.ewn1).all(|(a, b)| a.to_bits() == b.to_bits());
    println!("  DSIE, N1MAX and EWN1(0:121) rebuilt by DISTAB: {}", if table_ok { "equal" } else { "DIFFERENT" });

    let names = ["UST", "USTD", "Z0", "CD"];
    let (mut n, mut bad, mut nonzero, mut nan) = ([0u64; 4], [0u64; 4], [0u64; 4], [0u64; 4]);
    let mut first = [None; 4];
    // CD at RDCH = 0: `0.001 * (0.021 + 10.4 / (0**1.23 + 1.85))`, and `0**1.23` is 0.
    let floor_cd = 0.001f32 * (0.021f32 + 10.4f32 / (0.0f32 + 1.85f32));
    // Branch counts, from the inputs: `SIX/DSIE` as the routine computes it.
    let (mut deep, mut table, mut fpfloor, mut ufloor, mut ustfloor, mut rdch0) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut dmin = f32::INFINITY;
    let mut dmax = f32::NEG_INFINITY;
    let mut dist = std::collections::BTreeSet::new();
    for (i, k) in c.calls.iter().enumerate() {
        let o = w3flx2(&p, &c.ewn1, &k.input);
        for (j, ours) in [o.ust, o.ustd, o.z0, o.cd].into_iter().enumerate() {
            n[j] += 1;
            nonzero[j] += (k.out[j] != 0.) as u64;
            nan[j] += k.out[j].is_nan() as u64;
            if ours.to_bits() != k.out[j].to_bits() {
                bad[j] += 1;
                first[j].get_or_insert((i, ours.to_bits(), k.out[j].to_bits()));
            }
        }
        let x = &k.input;
        let afp = ww3_kernels::constants::TPI * if x.fp > 0.001 { x.fp } else { 0.001 };
        let six = afp * x.depth.sqrt();
        if (six / p.dsie) as i32 <= p.n1max {
            table += 1;
        } else {
            deep += 1;
        }
        fpfloor += !(x.fp >= 0.001) as u64;
        ufloor += !(x.u >= 0.01) as u64;
        ustfloor += !(x.ust >= 0.0001) as u64;
        rdch0 += (k.out[3].to_bits() == floor_cd.to_bits()) as u64;
        if x.depth.is_finite() {
            dmin = dmin.min(x.depth);
            dmax = dmax.max(x.depth);
        }
        dist.insert(x.depth.to_bits());
    }
    // The layout of a device: every point in one array, one thread each, the table once.
    let ins: Vec<Flx2In> = c.calls.iter().map(|k| k.input).collect();
    let mut batch = vec![Flx2Out::default(); ins.len()];
    for point in 0..ins.len() {
        batch[point] = w3flx2(&p, &c.ewn1, &ins[point]);
    }
    let batch_bad: usize = batch
        .iter()
        .zip(&c.calls)
        .map(|(o, k)| [o.ust, o.ustd, o.z0, o.cd].iter().zip(&k.out).filter(|(a, b)| a.to_bits() != b.to_bits()).count())
        .sum();
    println!(
        "  branches: {table} calls in the table (I1 <= N1MAX), {deep} in deep water, {} distinct DEPTH in [{dmin}, {dmax}]",
        dist.len()
    );
    println!(
        "  limiters: FP < 0.001 or NaN {fpfloor}, U < 0.01 or NaN {ufloor}, UST < 0.0001 or NaN {ustfloor}, RDCH floored {rdch0} (CD of RDCH = 0)"
    );
    for j in 0..4 {
        println!(
            "  {:5}{:>8} values, {} differ, {} nonzero, {} NaN {}",
            names[j],
            n[j],
            bad[j],
            nonzero[j],
            nan[j],
            first[j].map_or(String::new(), |f| format!("first (call, ours, theirs) {f:x?}"))
        );
    }
    println!("  batch layout: {batch_bad} values differ");
    let same = !c.calls.is_empty() && table_ok && bad.iter().all(|b| *b == 0) && batch_bad == 0;
    (same, nonzero[3] > 0)
}

fn main() {
    let fault = std::env::var("FLX2_FAULT").unwrap_or_default();
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
