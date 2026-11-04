# Lead, chaceon-integrate: one package at a time

A package (`ST1`, `ST2`, `ST3`, `ST6`, `FLX1` to `FLX4`, `IC1`, `IS1`, `BT4`) is integrated when a whole
`ww3_ts1` run built with its switch file gives the reference's output files byte for byte from Rust. Its
routines are already at `routine`; this rig takes them to `host`.

1. Pick one package. Read its `#ifdef W3_<SWITCH>` blocks in `model/src/w3srcemd.F90` and the `mod_def.ww3`
   fields its routines read (`model/src/w3iogrmd.F90`), preprocessed with the package's switch file.
2. `rig workflow instantiate rigs/workflows/wire-package.yaml --root-objective "<package, switch, routines>"`.
3. One package in flight at a time: `W3SRCE` (`rust/crates/ww3-kernels/src/srce.rs`) and the `mod_def.ww3`
   reader are the files every package touches, and two packages at once conflict there in ways
   `scripts/merge_additive.py` refuses.
4. The verdict `host` needs `cmp` and `test.comp` on the case's output files, in the reference's image, on the
   same commit (`AGENTS.md`, "Before claiming a verdict" in the `ww3-port` skill). A run where the package
   never acted proves nothing: the regtest seat counts the calls.
5. End with `rigs/workflows/ship-wave.yaml`, as for a port wave.
