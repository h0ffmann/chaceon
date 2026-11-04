# W3FLX1

Wind stress from the 10 m wind speed by Wu (1980), the `FLX1` switch: `CD = 0.001*(0.8 + 0.065*U10)`, `Z0 = ZWND*exp(-0.4/sqrt(CD))`,
`UST = U10*sqrt(CD)`. Translated from `model/src/w3flx1md.F90` at metocean/WW3 `92e2cf66`: `W3FLX1` (`:88-200`; the
computation is `:178-188`). Kernel: `rust/crates/ww3-kernels/src/flx1.rs`. No new `ww3-libm` function: it uses `expf` and
`sqrtf`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host and in compiled SPIR-V on lavapipe
(ADR-0007). It is not called from `W3SRCE` yet, and it has run on no GPU ⚠.

## Arguments

| Fortran | Kind | Rust |
|---|---|---|
| `ZWND`, `U10`, `U10D` | `REAL`, in | `Flx1In` |
| `UST`, `USTD`, `Z0`, `CD` | `REAL`, out | `Flx1Out` |
| the `ABS(ZWND-10.) > 0.01` test (`:178`) | writes an error, `CALL EXTCDE (1)` | `Flx1Out::err`, 1 where the Fortran stops |

No `f64` anywhere (v, `w3flx1md.F90`).

## Comparison with the reference

The reference is `switch_chaceon_st1` (`ST1`, `FLX1`, `BT1`, `DB1`), built by the coordinator. gdb records every call of `ww3_ts1`
(`regtests/capture/flx1.py`, stride 1) and, for `poke`, writes a different `U10` at the routine's entry on 7 of every 8
calls.

| Capture | Calls | `U10` | Host differ | SPIR-V differ |
|---|---|---|---|---|
| default (12 m/s) | 2161 | 11.999999 and 12 | 0 | 0 |
| storm (30 m/s) | 2161 | 30 | 0 | 0 |
| light (6 m/s) | 2161 | 5.9999995 and 6 | 0 | 0 |
| calm (0 m/s, which `ww3_ts1` raises to 0.001) | 2161 | 0.001 | 0 | 0 |
| poke | 500 | 0, 0.001, -5, -12.307692, -20, 300, 10^6 and the run's 12 | 0 | 0 |

9144 calls, 36 576 values, 0 differ. The routine has no branch but the `ZWND` test; the captures cover the arithmetic, and the
poke covers its edges: `CD` zero (`U10 = -12.307692`) and negative (`U10 = -20`), where `sqrt` gives a NaN, and 10^6.

```bash
just capture flx1 default 2500          # CAPTURE_SWITCH defaults to switch_chaceon_st1 for flx1
just capture flx1 poke 2500
just flx1-cmp /work/capture/flx1_default_2500m.bin
just spirv && just spirv-replay flx1 default 2500
```

## Fault runs

Over the five captures; each fault is an edit of `flx1.rs`, undone afterwards.

| Fault | Result |
|---|---|
| `U10` one ULP up (`FLX1_FAULT=u10`), default capture | caught: 2161 `UST`, 1802 `Z0`, 1802 `CD` values |
| `0.065` one ULP up | caught: 8737 values |
| `0.8` one ULP up | caught: 8489 values |
| `0.001` one ULP up | caught: 18 225 values |
| `0.4` one ULP up | caught: 6734 values |
| `CD` computed in `f64` and rounded | caught: 15 819 values |
| `Z0` factor `ZWND*1.0000001` | caught: 9019 values |
| `USTD = 0` | caught: 6979 values |
| `UST = U10*CD` | caught: 9081 values |
| `black_box` removed (host) | caught: 62 `Z0` values, the NaN sign, all in the poke |
| `0.01` of the `ZWND` test raised to `0.5` | **undetected**: `ZWND` is 10 on every call, `err` is not compared, and a poke would end the process in `EXTCDE`. The branch rests on reading ⚠ |

## GPU code

- SPIR-V: entry point `flx1` in `rust/gpu-spirv/shader`, 2 storage buffers.
- PTX: kernel `flx1` in `rust/gpu-cuda/kernels`. Assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body has `div.rn.f32` and no float `fma` or `mad` (the `mad` lines are `mad.lo.s32`, addresses). Not run ⚠.

## Translation notes

- Read from `__w3flx1md_MOD_w3flx1` in the reference's `ww3_shel` (gdb `disassemble`; the image has no `objdump`): `fmul
  U10*0.065` (`0x3d851eb8`), `fadd 0.8` (`0x3f4ccccd`), `fmul 0.001` (`0x3a83126f`), `fsqrt`, `fdiv 0.4 (0x3ecccccd)/sqrt`, `fneg`,
  `bl expf`, `fmul ZWND`, `fmul U10*sqrt`. No `fmadd`. `SQRT(CD)` is computed once.
- `-0.4/SQRT(CD)` is `-(0.4/SQRT(CD))` in Fortran (unary minus ranks below `/`): the `fneg` follows the division. For a
  positive `CD` this equals `(-0.4)/sqrt`. For a negative `CD` it does not: `fsqrt` gives the default NaN `0x7fc00000`, the
  division keeps it, the `fneg` makes it `0xffc00000`, and `expf` and the product keep that. `(-0.4)/sqrt` would give
  `0x7fc00000`. The captured poke has this (62 of 500 calls), and LLVM does the rewrite by itself: `-(a/b)` became
  `(-a)/b` on the host, and `spirv-opt` did the same in SPIR-V. The kernel holds the quotient in `black_box` for the host
  (rust-gpu warns that it ignores it) and flips the sign as an integer for SPIR-V.
- A kernel cannot stop the process, so the `EXTCDE (1)` branch is the flag `err`; the host decides what to do.

## Not translated, not reached

- The `ZWND` error branch: `ww3_ts1` has `ZWND = 10`, and a poke of `ZWND` would end the process in `EXTCDE`. A fault in the test's
  threshold is not caught ⚠.
- `W3SRCE`'s state around the call: `U10ABS`, `U10DIR` from the wind field, `Z0` and `CD` kept per point.

## Timing

None. No timing without a device run (`AGENTS.md`).
