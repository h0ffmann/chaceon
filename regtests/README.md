# regtests/

How a regtest is compared, written in Batch 0. It runs WW3's own regtest inputs
(`WW3/regtests/<case>`, `WW3/regtests/bin/matrix.base`) through the reference and through chaceon,
and judges the files with `WW3/regtests/bin/test.comp` and `cmp`. Nothing here writes to `WW3/`.
Rust tests do not replace this: there are none (ADR-0002).
