// SPDX-License-Identifier: MPL-2.0
//! `ww3_strt` in Rust: reads `mod_def.ww3` and `ww3_strt.inp` from a directory and writes `restart.ww3`.

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    ww3_host::strt::run(&dir);
}
