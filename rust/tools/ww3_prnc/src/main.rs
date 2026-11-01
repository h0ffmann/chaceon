// SPDX-License-Identifier: MPL-2.0
//! `ww3_prnc` in Rust: reads `mod_def.ww3` and `ww3_prnc.nml` (or `ww3_prnc.inp`) and the NetCDF file from a
//! directory and writes the forcing file there.

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    ww3_host::prnc::run(&dir);
}
