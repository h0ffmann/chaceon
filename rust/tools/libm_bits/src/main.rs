// SPDX-License-Identifier: MPL-2.0
//! Every `f32` through `ww3_libm` and the platform libm. Run it on the reference's image: ADR-0005.

unsafe extern "C" {
    fn expf(x: f32) -> f32;
}

fn main() {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as u64;
    let mut handles = Vec::new();
    for t in 0..threads {
        handles.push(std::thread::spawn(move || {
            let (lo, hi) = ((t << 32) / threads, ((t + 1) << 32) / threads);
            let (mut bad, mut first) = (0u64, None);
            for b in lo..hi {
                let x = f32::from_bits(b as u32);
                let ours = ww3_libm::expf(x);
                let theirs = unsafe { expf(x) };
                if ours.to_bits() != theirs.to_bits() && !(ours.is_nan() && theirs.is_nan()) {
                    bad += 1;
                    first.get_or_insert((b as u32, ours.to_bits(), theirs.to_bits()));
                }
            }
            (bad, first)
        }));
    }
    let (mut bad, mut first) = (0u64, None);
    for h in handles {
        let (b, f) = h.join().unwrap();
        bad += b;
        first = first.or(f);
    }
    println!("expf: 4294967296 inputs, {bad} differ, first {first:08x?}");
    std::process::exit((bad != 0) as i32);
}
