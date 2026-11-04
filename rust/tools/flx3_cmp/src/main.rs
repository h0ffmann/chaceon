// SPDX-License-Identifier: MPL-2.0
//! Replays `W3FLX3` calls recorded by `regtests/capture/flx3.py` and compares every bit of `UST`,
//! `USTD`, `Z0` and `CD` with the reference, one call at a time and again as one batch (the inputs in one
//! buffer, the results in one buffer, the table behind other data). A diagnostic of ADR-0002, never a
//! test. Exit 0 only when no bit differs and some call had a nonzero `CD`: a capture of zeros proves
//! nothing. It prints how many calls reached each branch, so a branch that never ran shows as a zero.
//!
//! `FLX3_FAULT=u` moves every `U` up by one ULP, `FLX3_FAULT=depth` every `DEPTH`: faults the comparison
//! must catch. The kernel's own faults are made by editing `ww3-kernels/src/flx3.rs`.
//!
//! Usage: flx3_cmp <capture.bin>...

use ww3_host::capture::Flx3Capture;
use ww3_kernels::flx3::{Flx3Out, w3flx3};

fn bump(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

fn one(path: &str, fault: &str) -> (bool, bool) {
    let c = Flx3Capture::read(path);
    let p = c.params;
    let names = ["UST", "USTD", "Z0", "CD"];
    let (mut n, mut bad, mut nonzero) = ([0u64; 4], [0u64; 4], [0u64; 4]);
    let mut first = [None; 4];
    let mut inputs: Vec<_> = c.calls.iter().map(|k| k.input).collect();
    for i in &mut inputs {
        match fault {
            "u" => i.u = bump(i.u),
            "depth" => i.depth = bump(i.depth),
            _ => {}
        }
    }
    // The call layout: one call, the table alone.
    let per_call: Vec<Flx3Out> = inputs.iter().map(|i| w3flx3(&p, &c.ewn1, i)).collect();
    // The batch layout: one input buffer, one output buffer, the table inside a larger buffer at an offset.
    let mut table = vec![f32::NAN; 7];
    table.extend_from_slice(&c.ewn1);
    table.extend_from_slice(&[f32::NAN; 5]);
    let mut batch = vec![Flx3Out::default(); inputs.len()];
    for (j, i) in inputs.iter().enumerate() {
        batch[j] = w3flx3(&p, &table[7..7 + 122], i);
    }
    let mut layouts_differ = 0u64;
    let (mut wn_table, mut wn_else, mut fp_floor, mut u_floor, mut ust_floor, mut capped, mut nan_out) = (0u64, 0, 0, 0, 0, 0, 0);
    let (mut lo_d, mut hi_d) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut lo_u, mut hi_u) = (f32::INFINITY, f32::NEG_INFINITY);
    for (idx, (k, o)) in c.calls.iter().zip(per_call.iter()).enumerate() {
        let i = &inputs[idx];
        let b = &batch[idx];
        if [b.ust, b.ustd, b.z0, b.cd].map(f32::to_bits) != [o.ust, o.ustd, o.z0, o.cd].map(f32::to_bits) {
            layouts_differ += 1;
        }
        // Branch counts, from the inputs: a reading aid, never a result.
        let afp = ww3_kernels::constants::TPI * if i.fp > 0.001 { i.fp } else { 0.001 };
        let six = afp * i.depth.sqrt();
        if ((six / p.dsie) as i32) <= p.n1max {
            wn_table += 1;
        } else {
            wn_else += 1;
        }
        fp_floor += !(i.fp >= 0.001) as u64;
        u_floor += !(i.u >= 0.01) as u64;
        ust_floor += !(i.ust >= 0.0001) as u64;
        capped += (p.cap_id != 0 || k.out[3].to_bits() == p.cd_max.to_bits()) as u64;
        nan_out += k.out.iter().any(|x| x.is_nan()) as u64;
        lo_d = lo_d.min(i.depth);
        hi_d = hi_d.max(i.depth);
        lo_u = lo_u.min(i.u);
        hi_u = hi_u.max(i.u);
        for (j, ours) in [o.ust, o.ustd, o.z0, o.cd].into_iter().enumerate() {
            n[j] += 1;
            nonzero[j] += (k.out[j] != 0.) as u64;
            if ours.to_bits() != k.out[j].to_bits() {
                bad[j] += 1;
                first[j].get_or_insert((idx, ours.to_bits(), k.out[j].to_bits()));
            }
        }
    }
    println!(
        "{path}: {} calls, NITTIN {} CAP_ID {} CD_MAX {} CINXSI {} N1MAX {}, DEPTH in [{lo_d}, {hi_d}], U in [{lo_u}, {hi_u}]",
        c.calls.len(),
        p.nittin,
        p.cap_id,
        p.cd_max,
        p.cinxsi,
        p.n1max
    );
    println!(
        "  branches: WAVNU1 table {wn_table}, WAVNU1 else {wn_else}, FP floor {fp_floor}, U floor {u_floor}, UST floor {ust_floor}, cap acting {capped}, NaN in results {nan_out}"
    );
    println!("  batch layout differs from call layout in {layouts_differ} calls");
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
    (!c.calls.is_empty() && bad.iter().all(|b| *b == 0) && layouts_differ == 0, nonzero[3] > 0)
}

fn main() {
    let fault = std::env::var("FLX3_FAULT").unwrap_or_default();
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
