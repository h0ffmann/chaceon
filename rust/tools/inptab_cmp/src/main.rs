// SPDX-License-Identifier: MPL-2.0
//! Rebuilds `INPTAB`'s `BETATB` on the host and compares every bit with the reference's module
//! array (`regtests/capture/inptab.py`). Usage: inptab_cmp <capture>... A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs and the table has nonzero values.
//!
//! `INPTAB_FAULT` breaks one thing on purpose, to see the comparison fail: `bump` moves one value of the
//! first column's low end one ULP, `bump_hi` one of the last column's high end, `dsiga` moves `DSIGA`,
//! `scale` multiplies every value by 1.1. Faults inside `W3BETA` itself are edits of `inptab.rs`
//! (`rust/port_report/W3SIN2.md`).

use ww3_host::capture_sin2::InptabCapture;
use ww3_host::inptab::inptab;
use ww3_kernels::sin2::{NRDRAG, NRSIGA, betatb_at};

fn key(x: f32) -> i64 {
    let u = x.to_bits() as i64;
    if u & 0x8000_0000 != 0 { 0x8000_0000 - u } else { u }
}

fn bump(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

fn one(path: &str, fault: &str) -> bool {
    let c = InptabCapture::read(path);
    let mut o = inptab();
    match fault {
        "bump" => o.betatb[betatb_at(-NRSIGA, 3)] = bump(o.betatb[betatb_at(-NRSIGA, 3)]),
        "bump_hi" => o.betatb[betatb_at(NRSIGA + 1, NRDRAG)] = bump(o.betatb[betatb_at(NRSIGA + 1, NRDRAG)]),
        "dsiga" => o.dsiga = bump(o.dsiga),
        "scale" => o.betatb.iter_mut().for_each(|v| *v *= 1.1),
        _ => {}
    }
    let (mut bad, mut max, mut nonzero, mut first) = (0u64, 0u64, 0u64, None);
    let mut neg = 0u64;
    for (i, (a, b)) in o.betatb.iter().zip(&c.betatb).enumerate() {
        nonzero += (*b != 0.) as u64;
        neg += (*b < 0.) as u64;
        if a.to_bits() != b.to_bits() {
            bad += 1;
            max = max.max((key(*a) - key(*b)).unsigned_abs());
            first.get_or_insert((i, *a, *b));
        }
    }
    let dels = [o.dsiga.to_bits() != c.dsiga.to_bits(), o.ddrag.to_bits() != c.ddrag.to_bits()];
    println!(
        "{path}: BETATB {} values, {bad} differ, max {max} ULP; {nonzero} nonzero, {neg} negative; DSIGA {} DDRAG {} {}",
        o.betatb.len(),
        if dels[0] { "differs" } else { "equal" },
        if dels[1] { "differs" } else { "equal" },
        first.map_or(String::new(), |f| format!("first (index, ours, theirs) {f:?}"))
    );
    bad == 0 && !dels[0] && !dels[1] && nonzero > 0
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!paths.is_empty(), "captures");
    let fault = std::env::var("INPTAB_FAULT").unwrap_or_default();
    let ok = paths.iter().map(|p| one(p, &fault)).fold(true, |a, b| a & b);
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
