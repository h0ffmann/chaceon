// SPDX-License-Identifier: MPL-2.0
//! `ww3_libm` against the platform libm: every `f32` for a function of one argument, a large sample
//! for two. Run it on the reference's image: ADR-0005. Usage: libm_bits [samples for two arguments].

use std::thread;

unsafe extern "C" {
    fn expf(x: f32) -> f32;
    fn logf(x: f32) -> f32;
    fn log10f(x: f32) -> f32;
    fn cosf(x: f32) -> f32;
    fn sqrtf(x: f32) -> f32;
    fn tanhf(x: f32) -> f32;
    fn expm1f(x: f32) -> f32;
    fn atanf(x: f32) -> f32;
    fn sincosf(x: f32, s: *mut f32, c: *mut f32);
    fn powf(x: f32, y: f32) -> f32;
    fn atan2f(y: f32, x: f32) -> f32;
}

fn sin_libm(x: f32) -> f32 {
    let (mut s, mut c) = (0., 0.);
    unsafe { sincosf(x, &mut s, &mut c) };
    s
}

fn cos_libm(x: f32) -> f32 {
    let (mut s, mut c) = (0., 0.);
    unsafe { sincosf(x, &mut s, &mut c) };
    c
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn sum(handles: Vec<thread::ScopedJoinHandle<(u64, Option<String>)>>) -> (u64, Option<String>) {
    let (mut bad, mut first) = (0, None);
    for h in handles {
        let (b, f) = h.join().unwrap();
        bad += b;
        first = first.or(f);
    }
    (bad, first)
}

fn threads() -> u64 {
    thread::available_parallelism().map_or(1, |n| n.get()) as u64
}

fn sweep(name: &str, ours: fn(f32) -> f32, theirs: fn(f32) -> f32) -> u64 {
    let n = threads();
    let (bad, first) = thread::scope(|sc| {
        sum((0..n)
            .map(|t| {
                sc.spawn(move || {
                    let (mut bad, mut first) = (0, None);
                    for b in (t << 32) / n..((t + 1) << 32) / n {
                        let x = f32::from_bits(b as u32);
                        let (a, c) = (ours(x), theirs(x));
                        if !same(a, c) {
                            bad += 1;
                            first.get_or_insert(format!("x {b:08x} ours {:08x} libm {:08x}", a.to_bits(), c.to_bits()));
                        }
                    }
                    (bad, first)
                })
            })
            .collect())
    });
    println!("{name:8} all 4294967296 inputs, {bad} differ {}", first.unwrap_or_default());
    bad
}

/// A float with a uniform exponent in `[lo, hi)` and a random mantissa: where WW3's arguments live.
fn scaled(r: u64, lo: u32, hi: u32) -> f32 {
    let e = lo + ((r >> 23) as u32 % (hi - lo));
    f32::from_bits((e << 23) | (r as u32 & 0x7fffff))
}

fn pairs(name: &str, samples: u64, ours: fn(f32, f32) -> f32, theirs: fn(f32, f32) -> f32) -> u64 {
    let n = threads();
    let (bad, first) = thread::scope(|sc| {
        sum((0..n)
            .map(|t| {
                sc.spawn(move || {
                    let (mut bad, mut first) = (0, None);
                    let mut s = 0x9e3779b97f4a7c15u64.wrapping_mul(t + 1);
                    let mut next = || {
                        s ^= s << 13;
                        s ^= s >> 7;
                        s ^= s << 17;
                        s
                    };
                    for i in 0..samples / n {
                        let (ra, rb) = (next(), next());
                        // Half over every float, half in the ranges WW3 uses.
                        let (x, y) = if i & 1 == 0 {
                            (f32::from_bits(ra as u32), f32::from_bits(rb as u32))
                        } else {
                            let y = scaled(rb, 118, 134);
                            (scaled(ra, 90, 150), if rb >> 60 & 1 == 0 { y } else { -y })
                        };
                        let (a, c) = (ours(x, y), theirs(x, y));
                        if !same(a, c) {
                            bad += 1;
                            first.get_or_insert(format!(
                                "{:08x} {:08x} ours {:08x} libm {:08x}",
                                x.to_bits(),
                                y.to_bits(),
                                a.to_bits(),
                                c.to_bits()
                            ));
                        }
                    }
                    (bad, first)
                })
            })
            .collect())
    });
    println!("{name:8} {samples} pairs, {bad} differ {}", first.unwrap_or_default());
    bad
}

fn main() {
    let samples = std::env::args().nth(1).map_or(1 << 34, |s| s.parse().unwrap());
    let mut bad = 0;
    bad += sweep("expf", ww3_libm::expf, |x| unsafe { expf(x) });
    bad += sweep("logf", ww3_libm::logf, |x| unsafe { logf(x) });
    bad += sweep("log10f", ww3_libm::log10f, |x| unsafe { log10f(x) });
    bad += sweep("cosf", |x| ww3_libm::sincosf(x).1, |x| unsafe { cosf(x) });
    bad += sweep("sqrtf", ww3_libm::sqrtf, |x| unsafe { sqrtf(x) });
    bad += sweep("expm1f", ww3_libm::expm1f, |x| unsafe { expm1f(x) });
    bad += sweep("tanhf", ww3_libm::tanhf, |x| unsafe { tanhf(x) });
    bad += sweep("atanf", ww3_libm::atanf, |x| unsafe { atanf(x) });
    bad += sweep("sin", |x| ww3_libm::sincosf(x).0, sin_libm);
    bad += sweep("cos", |x| ww3_libm::sincosf(x).1, cos_libm);
    bad += pairs("powf", samples, ww3_libm::powf, |x, y| unsafe { powf(x, y) });
    bad += pairs("atan2f", samples, ww3_libm::atan2f, |y, x| unsafe { atan2f(y, x) });
    println!("{}", if bad == 0 { "PARITY: every compared bit equal" } else { "DIFFERENT" });
    std::process::exit((bad != 0) as i32);
}
