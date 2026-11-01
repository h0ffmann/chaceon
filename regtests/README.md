# regtests/

How a regtest is compared, written in Batch 0. It runs WW3's own regtest inputs
(`WW3/regtests/<case>`, `WW3/regtests/bin/matrix.base`) through the reference and through chaceon,
and judges the files with `WW3/regtests/bin/test.comp` and `cmp`. Nothing here writes to `WW3/`.
Rust tests do not replace this: there are none (ADR-0002).

`prep/input` holds inputs WW3's regtests do not have, for `ww3_prep`: the input types `LL`, `F1`, `F2` and `DAT`,
binary input, format reversal and the stops, on small model grids written for them. `scripts/prep_interp.sh gen`
writes it (`just prep-interp`), `scripts/prep_pair.sh` judges each file against the reference.
`strt/input` holds the start type 2 inputs of `scripts/strt_variants.sh`. Both are inputs, not WW3 source.
