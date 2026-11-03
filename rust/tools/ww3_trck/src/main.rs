// SPDX-License-Identifier: MPL-2.0
//! `ww3_trck` in Rust: reads `ww3_trck.inp` and `track_o.ww3` from a directory and writes `track.ww3`.

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    ww3_host::trck::run(&dir);
}
