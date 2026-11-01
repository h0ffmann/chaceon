// SPDX-License-Identifier: MPL-2.0
//! Runs a case in Rust, with the kernels on the CPU. `--threads N` (default 1) spreads the per-point
//! and per-component loops of the time loop over N threads. Built with `--features mpi` and started by
//! `mpirun -n R`, the ranks share the sea points and the spectral components as WW3's `DIST` does. The
//! output depends on neither N nor R.

use ww3_host::nml::read_case;
use ww3_host::shel::par::Par;
use ww3_host::shel::run_ranks;

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
    let par = Par::init();
    if par.size() > 1 && par.rank() == 0 {
        eprintln!("ww3_shel: {} ranks, {threads} thread(s) each", par.size());
    }
    let (m, mut r) = read_case(&dir);
    r.threads = threads;
    run_ranks(&m, r, &dir, &par);
}
