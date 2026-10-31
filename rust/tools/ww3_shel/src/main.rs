// SPDX-License-Identifier: MPL-2.0
//! Runs a case in Rust, with the kernels on the CPU.

use ww3_host::nml::read_case;
use ww3_host::shel::run;

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    let (m, r) = read_case(&dir);
    run(&m, r, &dir, None);
}
