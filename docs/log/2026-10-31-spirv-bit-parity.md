# SPIR-V bit-identical on lavapipe: `spirv-opt` ran before `NoContraction`

2026-10-31, agent session `cd86e8f5` for the owner, who asked to find code whose notes say it lacks parity and fix
what can be fixed; no issue. Branch `goal/cd86e8f5-6190-parity-gaps`. Machine: the owner's Apple Silicon Mac, Docker,
`chaceon-lavapipe`.

## Question

`W3SNL1` was bit-identical as SPIR-V and `W3SDS4`, `W3SIN4`, `W3SPR4` and `W3SRCE` were not, with the cause recorded
as unattributed ⚠. In `W3XYP3`, exact division and `NoContraction` had closed the gap. Does the same treatment close
the source terms?

## Result

No, and yes. **Neither change alone did anything for the source terms; the order of the passes did.** With nothing
changed in `ww3-kernels` or `ww3-libm`, all 66 captures are bit-identical, 0 of 108 316 135 values (v).

`rustc_codegen_spirv` runs `spirv-opt -O` inside the build (`crates/rustc_codegen_spirv/src/link.rs:286-291` at
`60d1e25`). `spirv_exact` came after it, so `spirv-opt` had already folded floats on an undecorated module. It folds
`(a-c1)-c2` into `a-(c1+c2)`, and with `NoContraction` set it does not. The fix is `--no-spirv-opt` in the builder,
then `spirv_exact`, then `spirv-opt -O`, then `spirv-val`, which `scripts/spirv.sh` now does.

Values that differ, with the largest difference. Each row is one module, on every capture of that kernel; `W3SRCE` on
`default_2500m`, `storm_6m` and `t700_8m`, 1 931 559 values.

| Module | `W3SNL1` | `W3SPR4` | `W3SIN4` | `W3SDS4` | `W3SRCE`, 3 captures |
|---|---|---|---|---|---|
| The reports' numbers: `spirv-opt`, undecorated | 0 | 7 542, 1 ULP | 16 308 251, 29 571 072 ULP | 3 352 005, 12 288 ULP | 290 988, 2 534 235 ULP |
| `spirv-opt`, then `NoContraction` (the branch's start) | 0 | 7 542, 1 ULP | 11 940 769, 29 571 072 ULP | 3 349 846, 12 288 ULP | 289 752, 2 534 235 ULP |
| `spirv-opt`, then `NoContraction` and exact division | 0 | 7 542, 1 ULP | 11 940 769, 29 571 072 ULP | 3 349 846, 12 288 ULP | 289 752, 2 534 235 ULP |
| No decoration, `spirv-opt` last | 0 | 7 542, 1 ULP | 16 308 251, 29 571 072 ULP | 3 627 864, 12 288 ULP | 290 801, 2 534 235 ULP |
| Exact division undecorated, `spirv-opt` last | 0 | 7 542, 1 ULP | 11 186 360, 29 571 072 ULP | 50 532, 4 018 ULP | 182 186, 47 206 ULP |
| `NoContraction`, `spirv-opt` last | 0 | **0** | **0** | **0** | **0** |
| `NoContraction` and exact division, `spirv-opt` last (`just spirv`) | 0 | **0** | **0** | **0** | **0**; all 11: 0 of 7 082 383 |

Values: 14 186 880 on 11 captures, 328 032 on 14, 54 377 872 on 15, 32 340 968 on 15. The first row reproduces the
reports: `SPEC` on `default_2500m` 20 482 values, 8 ULP; `storm_6m` 197 036, 45 ULP.

Mesa folds too. With no `spirv-opt` at all, on `default_2500m`: undecorated, 747 `W3SPR4`, 962 917 `W3SIN4` and 419
`W3SDS4` values differ; decorated, 0. So both passes need the decoration, and the division change is not what
closes this on lavapipe.

Propagation, `ww3_tp1.1`, whole run through `spirv_shel`, with `pro3.rs` as committed and with `div` and `pos` taken
out by hand. Bytes that differ, of 169 906, 62 902 and 44 100:

| Module | `pro3.rs` | `out_grd` | `out_pnt` | restart |
|---|---|---|---|---|
| `spirv-opt`, undecorated | no `div`, no `pos` | 13 035 | 1 044 | 1 076 |
| `spirv-opt`, undecorated | `div`, `pos` | 15 121 | 735 | 1 173 |
| `spirv-opt`, then `NoContraction` | no `div`, no `pos` | 13 190 | 969 | 1 124 |
| `spirv-opt`, then `NoContraction` | `div`, `pos` | 0 | 0 | 0 |
| `NoContraction`, `spirv-opt` last | no `div`, no `pos` | 0 | 0 | 0 |
| `NoContraction` and exact division, `spirv-opt` last | no `div`, no `pos` | 0 | 0 | 0 |

So on the old pipeline both of the owner's changes were needed, and `div` and `pos` worked by hiding operations from
`spirv-opt`. On the new one neither is needed on lavapipe. `just spirv`, then `ww3_tp1.4`, `ww3_tp1.7`, `ww3_tp2.1`
and `ww3_tp2.4` through `scripts/spirv_pair.sh`: every file byte-identical (v).

`f32::max` was a suspect and is not the cause: rust-gpu emits no `FMax`. `core`'s fallback,
`if x.is_nan() || y >= x { y } else { x }`, gives `fmaxnm`'s answer for `x.max(0.)`, `-0.` included. It differs only when the second argument is a zero of the sign `fmaxnm` or `fminnm` would not pick. The one such
site, `.min(0.)` at `srce.rs:570`, adds to `1.`, so the sign has no effect.

## What found it

`just spirv-libm`, new: each `ww3-libm` function in SPIR-V against the host on random inputs. Nine functions gave 0 of
2 000 000. `atan2f` gave 587 126, and a probe of its last line alone, `(z-pi_lo)-pi`, gave 244 429. `sqrtf`,
`powf`, `sincosf`, division and the rest were never the problem. The note in `W3SDS4`'s report, that `(a+b)-a` might be
simplified, pointed at the driver and at error-free sums. Those were intact in the module that failed.

## Fault runs

- `z - (pi_lo + pi)` written into `atan2f`'s last line, on the final pipeline: `just spirv-libm` gives 295 708 of
  2 000 000 different, and its first input gives `bfc93da6`, the old module's bits.
- With `pb * (1/TWO_PI2)` for `pb / TWO_PI2` as well, on `default_2500m`: 224 of 28 140 `W3SPR4`, 604 035 of 4 378 496
  `W3SIN4` and 91 662 of 2 346 784 `W3SDS4` values differ.
- The modules of the tables above that differ are the fault runs of `spirv_exact` itself.

## What stayed open

- `W3SRCE` has only replayed call by call as SPIR-V. A chained run needs the `Device` hook of `ww3-host` to dispatch
  it ⚠.
- `just spirv-libm` samples: 3 900 000 inputs per function, 0 differ (v), against every `f32` on the host.
- `spirv_exact`'s division step changed no bit here. It is kept for a device that uses the 2.5 ULP Vulkan allows ⚠,
  which lavapipe does not. Its cost on a device is unmeasured ⚠.
- On a device: `f64` division, subnormals and the rounding of `OpFConvert` ⚠. lavapipe offers no `DenormPreserve`, so
  that execution mode cannot be tried here.
- `spirv-opt` honouring the decoration is shown by these runs for `SPIRV-Tools v2026.4`, not as a rule of the tool ⚠,
  so a new version repeats the replay.
- ADR-0005 and ADR-0007 are accepted, so they stand: ADR-0007's closing line says `W3SDS4` is not bit-identical on
  lavapipe, which is no longer so.

## Commands

```bash
just spirv && just spirv-libm 3900000
just spirv-replay sds4 t700 2500     # and the other kernels, one per capture
bash scripts/tp_pair.sh ww3_tp2.4 && bash scripts/spirv_pair.sh ww3_tp2.4
SPIRV_EXACT=contract rust/target/release/spirv_exact <module> <out>   # leaves out a step: contract, div
```
