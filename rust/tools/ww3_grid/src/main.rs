// SPDX-License-Identifier: MPL-2.0
//! `ww3_grid` in Rust: reads `ww3_grid.nml` and the files it names from a directory and writes
//! `mod_def.ww3` there. `--roundtrip <in> <out>` reads a `mod_def.ww3` and writes it again, which
//! checks the writer alone.

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next().expect("case directory");
    if first == "--roundtrip" {
        let (src, dst) = (args.next().expect("mod_def.ww3 to read"), args.next().expect("file to write"));
        ww3_host::iogr::write(&ww3_host::moddef::ModDef::read(&src), &dst);
    } else {
        ww3_host::gridpre::run(&first);
    }
}
