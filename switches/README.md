# Switches

`switch_chaceon_shrd` is the port's target: the switch of ww3-gpu's `docker/Dockerfile.ondaseventos`
with `DIST MPI` replaced by `SHRD`, because the first target is one process on one GPU.
ondaseventos' real switch was not read ⚠.

`switch_metocean_st4` is a copy of `WW3/model/bin/switch_ST4` at the pin (v, 2026-10-30): MetOcean's
own, with `BT4`, `WCOR`, `RWND`, `SEED` and MPI. It is the target of Batch 7.

`switch_chaceon_bt4` is `switch_chaceon_shrd` with `BT1` replaced by `BT4`, MetOcean's bottom friction. It
exists to capture `W3SBT4` from a running reference (`CAPTURE_SWITCH`, `scripts/capture.sh`); nothing else
uses it yet.

`switch_chaceon_st3` is `switch_chaceon_shrd` with `ST4` replaced by `ST3` (Janssen's WAM cycle 4 input, `W3SIN3`) and
`BT1` kept. It exists to capture `W3SIN3` from a running reference (`CAPTURE_SWITCH`, `scripts/capture.sh`).
