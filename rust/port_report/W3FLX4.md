# W3FLX4

Drag coefficient and wind stress from the 10 m wind speed by Hwang (2011), the `FLX4` switch: `CD = FLX4A0*(8.058 + 0.967*U10 -
0.016*U10**2)*1E-4`, `UST = U10*SQRT(CD)`, and for `U10 >= 50.33` a cap `UST = 2.026*SQRT(FLX4A0)`, `CD = (UST/U10)**2`;
`Z0 = ZWND*EXP(-0.4/SQRT(CD))`. Translated from `model/src/w3flx4md.F90` at metocean/WW3 `92e2cf66`: `W3FLX4` (`:105-231`;
the test is `:199-202`, the computation `:210-220`). Kernel: `rust/crates/ww3-kernels/src/flx4.rs`. No new `ww3-libm`
function: it uses `expf` and `sqrtf`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host and in compiled SPIR-V on lavapipe
(ADR-0007). It is not called from `W3SRCE` (`w3srcemd.F90:1147`), and it has run on no GPU ⚠.

## Arguments

| Fortran | Kind | Rust |
|---|---|---|
| `ZWND`, `U10`, `U10D` | `REAL`, in | `Flx4In` |
| `FLX4A0` (`W3GDATMD`, set from the namelist `&FLX4 CDFAC`, default 1.0: `w3gridmd.F90:1651-1655`) | `REAL`, module variable | `Flx4In::flx4a0`, read per call by the capture |
| `UST`, `USTD`, `Z0`, `CD` | `REAL`, out | `Flx4Out` |
| the `ABS(ZWND-10.) > 0.01` test (`:199`) | writes an error, `CALL EXTCDE (1)` | `Flx4Out::err`, 1 where the Fortran stops |

No `f64` anywhere (v, `w3flx4md.F90`). Every literal is `f32`: `50.33`, `2.026`, `8.058`, `0.967`, `0.016`, `1E-4`, `0.4`.

## Comparison with the reference

The reference is `switch_chaceon_st6` (`ST6`, `FLX4`, `BT1`, `DB1`), built by the coordinator. gdb records every call of
`ww3_ts1` (`regtests/capture/flx4.py`, stride 1); for the `poke` variants it writes a different `U10` at the routine's entry
on 12 of every 13 calls.

| Capture (variant) | Calls | `U10` | `FLX4A0` | Host differ | SPIR-V differ |
|---|---|---|---|---|---|
| default | 2161 | 11.999999 and 12 | 1 | 0 | 0 |
| st6flx4calm (0 m/s, which `ww3_ts1` raises to 0.001) | 2161 | 0.001 | 1 | 0 | 0 |
| st6flx4storm | 2161 | 30 | 1 | 0 | 0 |
| st6flx4strong | 2161 | 45 | 1 | 0 | 0 |
| st6flx4cap | 2161 | 55 (cap) | 1 | 0 | 0 |
| st6flx4wild | 2161 | 120 (cap) | 1 | 0 | 0 |
| st6flx4cd (`&FLX4 CDFAC=1.2`) | 2161 | 12 | 1.2 | 0 | 0 |
| st6flx4cdcap (`CDFAC=0.8`) | 2161 | 55 (cap) | 0.8 | 0 | 0 |
| st6flx4poke | 500 | 13 values, -20 to 10^6, 50.33 and the float below it | 1 | 0 | 0 |
| st6flx4pokecd (`CDFAC=1.3`) | 500 | the same | 1.3 | 0 | 0 |

18 288 calls, 73 152 values, 0 differ on the host (`just flx4-cmp`). 6867 calls take the cap branch (U10 >= 50.33) and 11 421
the polynomial. The poke covers the edges: `U10 = 50.33` (the `f32` 50.330002, which is the literal) and the float just below it (50.329998), `U10 = 0`
(`CD = 8.058e-4*FLX4A0`), `CD` negative (`U10 = -20`, 38 calls, where `sqrt` gives a NaN in `Z0` whose sign matters), 10^6 (cap branch, `Z0`
underflows to 0 in 38 calls). The cap needs 50.33 m/s or more, which `ww3_ts1` reaches at 55 and 120 m/s; `CDFAC` is reached
by three variants (12 m/s, 55 m/s and the poke).

```bash
just capture flx4 default 2500          # CAPTURE_SWITCH defaults to switch_chaceon_st6 for flx4
just capture flx4 st6flx4cap 2500       # also calm, storm, strong, wild, cd, cdcap, poke, pokecd
just flx4-cmp /work/capture/flx4_default_2500m.bin /work/capture/flx4_st6flx4cap_2500m.bin
just spirv && just spirv-replay flx4 st6flx4cap 2500
```

## Fault runs

Over the ten captures; each fault is an edit of `flx4.rs`, undone afterwards (23 runs).

| Fault | Result |
|---|---|
| cap threshold `50.33` to `50.34` | caught: 234 values (the poke at exactly 50.33) |
| cap test `>=` to `>` | caught: 234 values |
| `2.026` one ULP up | caught: 20 525 values |
| cap `CD = UST/U10` without the square | caught: 13 658 values |
| cap `CD` squared in `f64` and rounded | **undetected, and equivalent**: the product of two `f32` is exact in `f64`, so the rounding is the same |
| `8.058` one ULP up | caught: 11 019 values |
| `0.967` one ULP up | caught: 10 630 values |
| `0.016` one ULP up | caught: 6945 values |
| `1E-4` one ULP up | caught: 25 042 values |
| `FLX4A0` applied last (`poly*1E-4*FLX4A0`) | caught: 38 values |
| `FLX4A0` ignored in `CD` | caught: 7292 values |
| `FLX4A0` ignored in the cap's `UST` | caught: 7021 values |
| polynomial in `f64` | caught: 11 151 values |
| `0.016*U10*U10` as `(0.016*U10)*U10` | caught: 952 values |
| `0.4` one ULP up | caught: 15 667 values |
| `UST = U10*CD` | caught: 11 343 values |
| `USTD = 0` | caught: 16 118 values |
| `Z0` factor `ZWND*1.0000001` | caught: 18 098 values |
| `black_box` removed (host) | caught: 76 `Z0` values, the NaN sign, all in the two pokes (38 each) |
| plain `expf(-q)` for the sign flip (host) | **undetected on the host**: `black_box` already holds `q`. Caught in SPIR-V: 38 `Z0` NaN signs on the `poke` capture |
| `FLX4_FAULT=u10`, `U10` one ULP up | caught: 26 448 values |
| `FLX4_FAULT=a0`, `FLX4A0` one ULP up | caught: 31 219 values |
| `0.01` of the `ZWND` test raised to `0.5` | **undetected**: `ZWND` is 10 on every call, `err` is not compared, and a poke would end the process in `EXTCDE`. The branch rests on reading ⚠ |

Two earlier fault edits (`8.058`, `0.016`) hit a comment and a literal that rounds to the same `f32`; they were redone on the
code line, and the rows above are the redone ones.

## GPU code

- SPIR-V: entry point `flx4` in `rust/gpu-spirv/shader`, 2 storage buffers.
- PTX: kernel `flx4` in `rust/gpu-cuda/kernels`. Assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body has no float `fma` and
  no `mad.f32` (the `mad` lines are `mad.lo.s32`, addresses). Not run ⚠.

## Translation notes

- Read from `__w3flx4md_MOD_w3flx4` in the reference's `ww3_shel` (gdb `disassemble`): `fabd` `|ZWND-10|` against `0.01`;
  `fcmpe U10, 50.33`; polynomial `fmul U10*U10`, `fmul` `*0.016`, `fmul U10*0.967`, `fadd 8.058`, `fsub`, `fmul FLX4A0`,
  `fmul 1E-4`; `fsqrt`, `fmul U10*sqrt`; `fdiv 0.4/sqrt`, `fneg`, `bl expf`, `fmul ZWND`. No `fmadd`.
  `SQRT(CD)` is computed once.
- The polynomial is `(8.058 + 0.967*U10) - 0.016*(U10*U10)` and the product with `FLX4A0` and `1E-4` goes left to right, as
  the disassembly shows; the fault `FLX4A0` last differs in 38 values, so the order is checked.
- `-0.4/SQRT(CD)` is `-(0.4/SQRT(CD))` (the `fneg` after the `fdiv`); the NaN sign trap of `W3FLX1` applies, and the
  kernel handles it the same way (`black_box` on the host, an integer sign flip for SPIR-V and PTX).
- A kernel cannot stop the process, so the `EXTCDE (1)` branch is the flag `err`; the host decides.
- `FLX4A0` is a `W3GDATMD` pointer into the grid's parameters. A kernel takes it as an input per point.

## Not translated, not reached

- The `ZWND` error branch: `ww3_ts1` has `ZWND = 10`, and a poke of `ZWND` would end the process in `EXTCDE`. A fault in the
  test's threshold is not caught ⚠.
- `W3SRCE`'s state around the call: `U10ABS`, `U10DIR` from the wind field, `Z0` and `CD` kept per point.
- The model-run behaviour at wind speeds above 50.33 m/s is exercised only through `ww3_ts1` homogeneous winds ⚠.

## Timing

None. No timing without a device run (`AGENTS.md`).
