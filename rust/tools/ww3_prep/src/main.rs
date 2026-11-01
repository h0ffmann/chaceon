// SPDX-License-Identifier: MPL-2.0
//! `ww3_prep` in Rust: reads `mod_def.ww3` and `ww3_prep.inp` from a directory and writes the field file there.

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    ww3_host::prep::run(&dir);
}
