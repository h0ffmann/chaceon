// SPDX-License-Identifier: MPL-2.0
//! `ww3_bound` in Rust: reads `mod_def.ww3`, `ww3_bound.nml` or `ww3_bound.inp` and point spectra from a directory, and
//! writes `nest.ww3`.

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    ww3_host::bound::run(&dir);
}
