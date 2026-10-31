# ww3_ts1 in Rust: the first whole-file verdict

`rust/tools/ww3_shel` runs a case in Rust from a calm start and writes `out_pnt.ww3`. Code:
`rust/crates/ww3-host/src/{moddef,wtime,shel}.rs`, on the kernels of `ww3-kernels` built for the CPU.

**Status: `host`** (ADR-0005), for `out_pnt.ww3` on `ww3_ts1`. Nothing here has run on a GPU ⚠.

## Result

The reference and Rust run the same case and `cmp` judges the file:

| Variant, depth | `out_pnt.ww3` |
|---|---|
| default 2500, 20, 8, 6 m | identical |
| calm, light 2500 m | identical |
| storm 6, 2500 m | identical |
| strain 20 m, circle 2500 m | identical |
| t700 2500, 8 m, romero2 20 m | identical |
| filipot 2500, 20 m | identical |
| noshelter, flatswell, sharp 2500 m | identical |

18 cases, 7 652 218 bytes each, 137 739 924 bytes in all, 2 161 outputs per file. Every byte of 36 hours of
spectra and scalars, with each step's input coming from Rust's own previous step.

```bash
just ts1-pair storm 6
```

## Why to believe it

The match came on the first run, so it was checked:

- The file was deleted, regenerated in about 2 s, and its SHA-256 equals the reference's
  (`d6e64c201bda591d475b...`).
- One reassociated multiplication in `W3SNL1` changes 1 171 911 bytes.
- A factor of 1.0000001 on the wind interpolation changes 1 448 487 bytes.
- `just moddef-cmp` compares what the reader parses with the reference's memory: 24 tables and the whole
  `SrceParams` struct, bit for bit.

## How far it reaches

| Reached | Not reached ⚠ |
|---|---|
| `W3SRCE` and all its kernels, chained | Propagation here: see [`W3PRO3.md`](W3PRO3.md) |
| `mod_def.ww3`, read | `mod_def.ww3` written: it still comes from `ww3_grid` |
| `W3UWND`, `W3FLDH` winds, homogeneous | Wind fields, currents, water levels, ice |
| `TICK21`, `DSEC21`, standard calendar | Other calendars |
| `WAVNU1`, `DISTAB` | |
| `W3IOPE`, `W3IOPO`, write, one point | `out_grd.ww3`, which the reference also writes: not produced yet |
| `W3WAVE`, the time and output logic | More than one sea point here: see `W3PRO3.md` |

The binary refuses what it does not implement, with a message. It reads only the namelist keys these cases use.

`test.comp` was not run, since `cmp` on the only file produced says more. A full ADR-0005 verdict
for `ww3_ts1` also needs `out_grd.ww3` ⚠.

Bits hold on the arm64 reference with contraction off (ADR-0005), for `NK=36`, `NTH=24`.

`ww3_shel <case dir> [--threads N]` (default 1) shares the per-sea-point loops of `W3SRCE` and `W3KTP3` and the
per-component loop of `W3XYP3` over N threads, each with its own scratch; `W3XYP3` gathers its components from
`va` in parallel, and each sea point scatters its own values back, so no thread writes `va` by component. The
kernels and their arithmetic are unchanged, and `--threads 1` takes the serial path. (v) For `ww3_tp1.1`, `ww3_tp1.4`,
`ww3_tp2.3`, `ww3_tp1.8`, `ww3_tp1.6`, `ww3_tp2.15`, `default_2500m` and `storm_6m`, run with
`scripts/rust_in_ref.sh ww3_shel /work/pair/<case> --threads N` for N of 1, 2, 4 and 8, `cmp` finds `out_grd.ww3`,
`out_pnt.ww3` and `restart001.ww3` (where a `.reference` exists) identical to the reference, with the outputs deleted
before each run; a deliberate fault in the threaded scatter made `ww3_tp1.1` differ at N of 2 and not at 1. ⚠ Only
8 threads on an 8-core Docker laptop were tried, and no case is large enough to show a speed-up.

## Timing

None: this is a laptop in Docker, and a timing needs a named machine and three runs.
