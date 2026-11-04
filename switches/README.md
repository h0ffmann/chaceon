# Switches

`switch_chaceon_shrd` is the port's target: the switch of ww3-gpu's `docker/Dockerfile.ondaseventos`
with `DIST MPI` replaced by `SHRD`, because the first target is one process on one GPU.
ondaseventos' real switch was not read ⚠.

`switch_metocean_st4` is a copy of `WW3/model/bin/switch_ST4` at the pin (v, 2026-10-30): MetOcean's
own, with `BT4`, `WCOR`, `RWND`, `SEED` and MPI. It is the target of Batch 7.

`switch_chaceon_bt4` is `switch_chaceon_shrd` with `BT1` replaced by `BT4`, MetOcean's bottom friction. It
exists to capture `W3SBT4` from a running reference (`CAPTURE_SWITCH`, `scripts/capture.sh`); nothing else
uses it yet.

`switch_chaceon_st1`, `switch_chaceon_st3` and `switch_chaceon_ice1` are `switch_chaceon_shrd` with one package swapped,
to capture Batch 7 routines from a running reference: `ST1` with `FLX1` for `ST4` and `FLX0` (`ST1` takes its stress
from a flux routine, as `regtests/mww3_test_05/input/switch_ST1_PR1` does); `ST3` for `ST4`, keeping `FLX0`; `IC1` and
`IS1` for `IC0` and `IS0`. Nothing but captures uses them.
