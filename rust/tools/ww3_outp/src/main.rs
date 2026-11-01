// SPDX-License-Identifier: MPL-2.0
//! `ww3_outp` in Rust: reads `ww3_outp.inp`, `mod_def.ww3` and `out_pnt.ww3` from a directory and writes the log
//! to standard output and the tables and transfer files into the directory.

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    std::process::exit(ww3_host::outp::run(&dir));
}
