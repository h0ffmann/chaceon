// SPDX-License-Identifier: MPL-2.0
//! Runs a case in Rust, with the kernels on the CPU. `--threads N` (default 1) spreads the per-point
//! and per-component loops of the time loop over N threads; the output does not depend on N.

use ww3_host::nml::read_case;
use ww3_host::shel::run;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: ww3_shel <case directory> [--threads N]");
    let mut threads = 1;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--threads" => {
                threads = args.next().and_then(|v| v.parse().ok()).filter(|&n| n >= 1).expect("--threads needs an integer >= 1");
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let (m, mut r) = read_case(&dir);
    r.threads = threads;
    run(&m, r, &dir, None);
}
