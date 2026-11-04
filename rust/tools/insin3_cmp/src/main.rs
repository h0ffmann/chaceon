// SPDX-License-Identifier: MPL-2.0
//! Rebuilds `INSIN3`'s tables from what a capture holds (`regtests/capture/insin3.py`) and compares
//! every bit with the reference's module arrays. Usage: insin3_cmp <capture>... A diagnostic of
//! ADR-0002, never a test. `INSIN3_FAULT` (1 to 6) breaks one thing on purpose, to see the comparison fail.

use ww3_host::capture::Insin3Capture;
use ww3_host::insin3::{Insin3Input, insin3};

fn key(x: f32) -> i64 {
    let u = x.to_bits() as i64;
    if u & 0x8000_0000 != 0 { 0x8000_0000 - u } else { u }
}

fn table(name: &str, ours: &[f32], theirs: &[f32]) -> bool {
    assert_eq!(ours.len(), theirs.len(), "{name}");
    let (mut bad, mut max, mut nonzero, mut first) = (0u64, 0u64, 0u64, None);
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        nonzero += (*b != 0.) as u64;
        if a.to_bits() != b.to_bits() {
            bad += 1;
            max = max.max((key(*a) - key(*b)).unsigned_abs());
            first.get_or_insert((i, *a, *b));
        }
    }
    println!(
        "  {name:8}{:>8} values, {bad} differ, max {max} ULP; {nonzero} nonzero {}",
        ours.len(),
        first.map_or(String::new(), |f| format!("first {f:?}"))
    );
    bad == 0 && nonzero > 0
}

fn one(path: &str, fault: u32) -> bool {
    let c = Insin3Capture::read(path);
    let f = &c.reals;
    println!(
        "{path}: NK {}, ZWND {}, ALPHA0 {}, BETAMAX {}, ZALP {}, Z0MAX {}, SIG(NK) {}",
        c.nk, f[0], f[1], f[2], f[3], f[4], f[5]
    );
    let bump = |x: f32| f32::from_bits(x.to_bits() + 1);
    let mut p = Insin3Input { sig_nk: f[5], zzwnd: f[0], aalpha: f[1], bbeta: f[2], zzalp: f[3], zz0max: f[4] };
    match fault {
        1 => p.zz0max = 0.,
        2 => p.sig_nk = bump(p.sig_nk),
        3 => p.zzalp = bump(p.zzalp),
        _ => {}
    }
    let mut o = insin3(&p);
    match fault {
        4 => o.taut[1 + 201 * 100] = bump(o.taut[1 + 201 * 100]),
        5 => o.tauhft[50 + 101 * 100] = bump(o.tauhft[50 + 101 * 100]),
        6 => o.delalp = bump(o.delalp),
        _ => {}
    }
    let mut ok = table("TAUT", &o.taut, &c.taut);
    ok &= table("TAUHFT", &o.tauhft, &c.tauhft);
    ok &= table("DELS", &[o.deltauw, o.delu, o.delust, o.delalp], &f[6..10]);
    ok
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!paths.is_empty(), "captures");
    let fault: u32 = std::env::var("INSIN3_FAULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let ok = paths.iter().map(|p| one(p, fault)).fold(true, |a, b| a & b);
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
