# W3SIN4, W3SPR4, CALC_USTAR and ST4's tables

ST4's wind input and mean parameters. Kernels in `rust/crates/ww3-kernels/src/sin4.rs`: `W3SIN4`
(`model/src/w3src4md.F90:424-906`), `W3SPR4` (`:143-385`), `CALC_USTAR` (`:1738-1840`). Host setup in
`rust/crates/ww3-host/src/insin4.rs`: `INSIN4` (`:917-1179`), `TABU_STRESS`, `TABU_TAUHF`, `TABU_TAUHF2`
(`:1188-1720`), `TABU_FW`, `KZEONE`, `KERKEI` (`constants.F90:119-440`), `WAVNU2` (`w3dispmd.F90:203-344`).
All at metocean/WW3 `92e2cf66`, built for `STAB0` and `FLX0`.

**Status: `routine`** (ADR-0007), on the host. Bit-identical on SPIR-V: no, except `LLWS`. No whole-file
verdict, no GPU run ⚠.

## Comparison with the reference

Same reference and case as [`W3SNL1`](W3SNL1.md), every third call, namelists and wind changed only.

| Captures | `W3SIN4` calls | `W3SPR4` calls |
|---|---|---|
| default at 2500, 20, 8 m | 7 628 | 7 055 |
| filipot 20 m, noshelter, flatswell, sharp, circle, t700 8 m | 18 841 | 15 904 |
| calm (0 m/s), light (6 m/s) | 4 418 | 4 370 |
| poke: `USTAR` of 1e-5 and 0, NaN and Inf in `A`; every call, 500 | 500 | none |
| **Total** | **31 387** | **27 329** |

Differences: **0** of 54 362 284 `S`, `D` and stress values, 27 118 368 `LLWS`, and 327 948 `W3SPR4`
outputs. In the NaN cases, NaN equals NaN, whatever its payload.

Tables rebuilt in Rust from namelist values, 0 differ:

| Table | Values | Nonzero |
|---|---|---|
| `TAUT` | 40 401 | 40 200 |
| `TAUHFT`, `TAUHFT2` | 20 301, 1 035 351 | 14 459, 737 409 |
| `FWTABLE` | 301 | 301 |
| `SATINDICES`, `SATWEIGHTS`, `CUMULW` | 552, 552, 746 496 | n/a, 264, 285 696 |
| `IKTAB`, filipot | 72 000 | n/a |
| `DCKI`, `QBI`, filipot | 2 600 000 each | 2 596 963, 2 597 134 |

`noshelter` reaches `TABU_TAUHF`'s own `TAUHFT`. On other captures `DCKI` and `QBI` are all zero, which
shows nothing.

```bash
just capture sin4 filipot 20 3 && just capture spr4 filipot 20 3
just st4-cmp sin4 filipot 20 && just st4-cmp spr4 filipot 20
just capture sin4 poke 2500 1 && ST4_NO_TABLES=1 just st4-cmp sin4 poke 2500
just libm-bits   # log10f and cosf, added here: all 2^32 inputs, 0 differ
```

## MetOcean's guards

Natural runs cannot reach them: the lowest `USTAR` in any capture is 0.001, MetOcean's own floor in
`W3SRCE` (`w3srcemd.F90:1048`, `:1178`), and no spectrum there is non-finite. So faults on these lines
pass every natural capture, and only `poke` shows they do something:

| Fault | Natural captures | `poke` |
|---|---|---|
| No `USTAR` floor (`:680`) | undetected | 17 820 `S`, 172 800 `D`, 490 stress values differ |
| No `ISNAN(TEMP2)` (`:780`) | undetected | 502 stress values differ |
| No `ISNAN(TEMP)` (`:835`) | undetected | 284 stress values differ |
| `Z0VISC` floor doubled (`:596`) | undetected | 10 316 `S`, 49 248 `D`, 114 stress values differ |
| `MAX(UST,0.3)` limit to 0.31 (`:706`) | undetected | **undetected** |
| `ZLOG**4 * (UCN*UCN)`, reassociated | 170 552 `S`, 183 407 `D` differ | |
| `W3SPR4` tail coefficient swapped | 2 343 of 28 140 values differ | |

## SPIR-V on lavapipe

`LLWS` is exact. `W3SPR4` is within 1 ULP. `W3SIN4` differs on about a third of its values, by up to
millions of ULP on values that nearly cancel to zero. Measured against the array's peak, the largest
difference is about 1.7e-6 for `S` and 5e-7 for `D` (`default`, `noshelter`, 2500 m), and for the stress
3e-7. The cause is unattributed ⚠, as for `W3SDS4`.

## GPU code

SPIR-V 917 KB and PTX 1.2 MB, all four kernels. `spirv-val` passes. PTX: 12 154 float operations, all
`.rn`; no `fma`, no libdevice, no `ftz`. Neither has run on a GPU ⚠.

## Translation notes

- `IKD=1` uses `KD = 0x384a1eff`: `-O3` peels the first pass and folds `1.01**(-999)` with one rounding,
  where `__powisf2` gives `0x384a1eb3`. Without this, `DCKI` differs in 1 998 values, all in that row.
- `FACLN1` and `FACLN2` are dead.
- `ST4TABUHF2.bin` is only a cache of `TABU_TAUHF2`, so it is neither read nor written.
- `insin4.rs` uses `std`'s `coshf` and `KZEONE`'s `f64` math, so those bits hold on Linux glibc only.

## Not covered ⚠

- The `MAX(UST,0.3)` limit: nothing reaches it.
- `ZZ0MAX != 0` in `TABU_STRESS` and `TABU_TAUHF`, `SSWELLF(4) <= 0` with `SSWELLF(7)` paths beyond the
  variants above, `W3SPR4`'s `USTAR <= 0.001` branch of `CALC_USTAR` other than through zero wind.
- `WAVNU2` for negative frequency, which `INSIN4` never passes.
- `SSDSBCK > 0` with another grid: `DCKI` took about a minute here, for one configuration.
- Other spectral grids, `W3SIN4` inside `W3SRCE`, threads, x86_64, any GPU.

## Timing

None: there is no whole-model comparison on this commit.
