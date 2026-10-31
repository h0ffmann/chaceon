# Switches

`switch_chaceon_shrd` is the port's target: the switch of ww3-gpu's `docker/Dockerfile.ondaseventos`
with `DIST MPI` replaced by `SHRD`, because the first target is one process on one GPU.
ondaseventos' real switch was not read ⚠.

`switch_metocean_st4` is a copy of `WW3/model/bin/switch_ST4` at the pin (v, 2026-10-30): MetOcean's
own, with `BT4`, `WCOR`, `RWND`, `SEED` and MPI. It is the target of Batch 7.
