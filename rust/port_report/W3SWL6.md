# W3SWL6 and IRANGE

Turbulent dissipation of narrow-banded swell of the `ST6` package (Babanin 2011, section 7.5): per wavenumber,
`D = -(2/3) B1 sigma sqrt(Bn)`, with `Bn` the spectral saturation of Banner et al. (2002) built from the directionally
normalised spectrum, applied to every direction, and `S = D A`. Translated from `model/src/w3swldmd.F90` at metocean/WW3
`92e2cf66`: `W3SWL6` (`:251-413`) and the module's `IRANGE` (`:430-460`). Kernel: `rust/crates/ww3-kernels/src/swl6.rs`. No new
`ww3-libm` function: it uses `sqrtf`. `W3SWL4` of the same file is not ported.

**Status: `routine`** in `rust/ports.tsv` for both: every captured call bit-identical, on the host and in compiled SPIR-V on
lavapipe (ADR-0007). It is not called from `W3SRCE` yet, and it has run on no GPU ⚠.

`W3SRCE` calls it only when `SWL6S6` is true (`w3srcemd.F90:1329`). `SWL6S6 = SWLB1 > 0.` (`w3gridmd.F90:2254`) and the
default `SWLB1` is `0.41E-02` (`:2133`), so every `ST6` run calls it unless `&SWL6 SWLB1` is set to zero or less. The second
namelist key, `CSTB1` (default `.FALSE.`, `:2132`), picks the form of `B1`: false scales `SWLB1` by the steepness at the spectral
peak (`:376-386`), true keeps the constant. Both sides are captured.

## Arguments

| Fortran | Kind | Rust |
|---|---|---|
| `A(NSPEC)`, `CG(NK)`, `WN(NK)` | `REAL`, in | slices; `a` at `o`, `cg` and `wn` at `k_off` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | slices at `o` |
| `NK`, `NTH`, `DTH`, `SWL6B1`, `SWL6CSTB1` | module scalars | `Swl6Params` (`LOGICAL` as `i32`) |
| `SIG(1:NK)`, `DDEN(1:NK)` | module arrays | `sig`, `dden` slices, shared by all points |
| `FTE`, `FTWN` | imported, never read | not present |

No `f64` anywhere (v, `w3swldmd.F90:251-413`). The automatic arrays `ABAND`, `KMAX`, `ANAR`, `BN`, `DDIS` (NK) and `K` (NTH,NK)
are not stored: a kernel may not allocate, so each is recomputed where it is used with the same operations in the same order
(`ABAND` and `SUM(K,1)` are in-order sums from `0.`, so a second evaluation gives the same bits). `IKN = IRANGE(1,NSPEC,NTH)` is
`irange_len` and `irange_at`; the kernel indexes with `irange_at(1,NTH,IK)` and does not build the vector.

## Comparison with the reference

The reference is `switch_chaceon_st6` (`ST6`, `FLX4`, `BT1`, `DB1`), built by the coordinator. gdb records the arguments and
results of `__w3swldmd_MOD_w3swl6` in `ww3_ts1` (`regtests/capture/swl6.py`); `calm`, `light` and `storm` change the wind, the
`SWL6` namelist variants are in `scripts/ts1_variants.sh`, and `poke` writes rare inputs at the routine's entry on 9 of every 10
calls (zero spectrum, spectrum times 1e-25, negative `WN` on every third band, negative `CG`, NaN in two bands, a NaN column
with +Inf and -Inf in others, every element NaN, a constant spectrum, a column of -1 and a -0).

| Capture (`just capture swl6 <variant> 2500 <stride>`) | `CSTB1` `SWLB1` NK x NTH | Calls | Host differ | SPIR-V differ |
|---|---|---|---|---|
| `default` (stride 4) | false 0.0041, 36 x 24 | 1228 | 0 | 0 |
| `st6swl6cst` (8) | true 0.0041 | 973 | 0 | 0 |
| `st6swl6b1` (8) | false 0.02 | 775 | 0 | 0 |
| `st6swl6cstb1` (8) | true 0.0123 | 297 | 0 | 0 |
| `st6swl6storm` (8, 30 m/s) | false | 974 | 0 | 0 |
| `st6swl6light` (8, 6 m/s) | false | 284 | 0 | 0 |
| `st6swl6calm` (8, 0 m/s) | false | 271 | 0 | 0 |
| `st6swl6grid` (8) | false, 40 x 36, XFR 1.07 | 290 | 0 | 0 |
| `st6swl6grid2` (8) | true, 25 x 12 | 295 | 0 | 0 |
| `st6swl6poke` (1, limit 500) | false | 500 | 0 | 0 |
| `st6swl6cstpoke` (1, limit 500) | true | 500 | 0 | 0 |

6387 calls, 11 038 056 values of `S` and `D`, 0 differ in call layout and in the one-buffer layout of a device (22 076 112
comparisons), host (`just swl6-cmp`) and compiled SPIR-V on lavapipe (`just spirv-replay swl6 <variant>`). Every comparison
prints how many values were nonzero (`S` 1 004 635 of 1 060 992 on `default`). The `calm` capture is all zero (`S` is zero on every value and no band exceeds `1.E-30`; why the spectrum stays empty at 0 m/s was not investigated ⚠), so it proves nothing about the arithmetic and only exercises the zero branch of `:391`. Every other capture has nonzero `S`.

```bash
just capture swl6 default 2500 4          # CAPTURE_SWITCH defaults to switch_chaceon_st6 for swl6 and st6*
just capture swl6 st6swl6poke 2500 1
just swl6-cmp /work/capture/swl6_default_2500m.bin /work/capture/swl6_st6swl6poke_2500m.bin
just spirv && just spirv-replay swl6 st6swl6poke 2500   # kernel swl6, capture/swl6_<variant>_2500m.bin
```

## Branches reached

- `ABAND > 1.E-30` (`:391`): both sides on every capture but `storm` (36 bands at or below of 35 064); `poke` x 1e-25 puts bands
  between 0 and the threshold. The faults `1.E-30 -> 1.E-29` and `> 0.` are caught.
- `SWL6CSTB1` (`:376`): both.
- `KMAX < 1.E-34` (`:367`): never true where it matters. `ABAND > 1.E-30` needs `KMAX >= ABAND/NTH > 1.E-34` (for `NTH < 10^4`),
  and a `DDIS` that `:391` rejects is never read, so the guard cannot change an output. Removing it is not caught, by this
  argument and not by a blind spot (v, fault run); the guard stays in the kernel because the Fortran has it.
- `MAXVAL` and `MAXLOC` on NaN: reached by `poke` (NaN in two bands, a NaN column, every element NaN), and the outputs match.
  A NaN or a `+Inf` with a `-Inf` makes an `ABAND` NaN, which makes `SUM(ABAND*DDEN/CG)` NaN and `B1` NaN, so `MAXLOC`'s
  choice of `IK` cannot reach an output; faults in that choice are not caught (v, explained, fault run). The NaN branch of
  `MAXVAL` is likewise unreachable from an output: a column with a NaN has a NaN `ABAND`.
- Ties of `MAXLOC`: a constant spectrum (poke) and the zero bands of `calm`. The fault `>` to `>=` (last maximum) is caught.
- Negative `WN` (negative `BN`, `SQRT` of a negative, the sign of the NaN): reached by `poke`.

## Fault runs

Edits of `swl6.rs` run one at a time, over `default`, `poke`, `cstpoke`, `cst`, `grid2` and `storm`, undone afterwards.
Input faults (`SWL6_FAULT=`) move one input by one ULP over `default`.

| Fault | Result |
|---|---|
| `DTH` one ULP up | caught: 246 384 values of `D` |
| `SWL6B1` one ULP up | caught: 864 864 of `D` |
| `SWL6CSTB1` flipped | caught: 1 027 080 of `D` |
| `SIG(1)` one ULP up | caught: 16 656 of `D` |
| `DDEN` every entry one ULP up | caught: 351 792 of `D`. The first entry alone is **not** caught (the sum of `:385` swallows it): a weak fault, not a blind spot |
| `CG` every entry one ULP up | caught: 338 592 of `D`. First entry alone not caught, same reason |
| `WN(1)` one ULP up | caught: 16 728 of `D` |
| `A(1)` of each call one ULP up | caught: 480 of `D` |
| `ABAND` summed backwards | caught |
| `SUM(K,1)` summed backwards | caught |
| `(ABAND*DDEN)/CG` regrouped as `ABAND*(DDEN/CG)` | caught |
| `MAXLOC` takes the last maximum | caught |
| `1.E-30` to `1.E-29`, and to `0.` | caught, caught |
| `1.E-34` to `1.E-3` | caught |
| `K/KMAX` as `K*(1/KMAX)` | caught |
| `ANAR` as `(1/SUM)/DTH` | caught |
| `BN` regrouped, `SIG*DTH` first | caught, caught |
| `DDIS` regrouped | caught |
| `DDIS`: the sign as a constant `-(2/3)` instead of the flip after the product | caught: 279 552 values, all NaN signs, on the pokes |
| `2/3` in `f64` | caught |
| `B1 = (SWL6B1*2.)*SQRT*WN` | caught |
| the index `IKN` reversed | caught |
| `S` scaled by `1.0000001` | caught |
| `MAXLOC` without the NaN skip; with the all-NaN result `NK` | **undetected**, explained above: any NaN `ABAND` makes `B1` NaN |
| `1.E-34` guard removed | **undetected**, explained above |
| `B1 = SWL6B1*(2.*(SQRT*WN))` | **undetected**: multiplying by 2 is exact, so the regrouping is the same bits (v, a power of two scales a float without rounding) |
| `D = 0.` of `:358` replaced by `D = 1.` | **undetected**: every element of `D` is written afterwards by `:397-399` when `NK = INT(REAL(NSPEC-1)/REAL(NTH))+1`, which holds for the three grids captured |

## GPU code

- SPIR-V: entry point `swl6` in `rust/gpu-spirv/shader`, 8 storage buffers. `just spirv` builds and `spirv-val` passes. The
  `black_box` of the product before the sign flip is ignored by rust-gpu (a warning), so the flip is an integer xor as in
  `W3FLX1`; with it the SPIR-V matches on the NaN-sign pokes.
- PTX: kernel `swl6` in `rust/gpu-cuda/kernels`. Assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`). Its body
  has three `div.rn.f32` and no float `fma` or `mad` (the square root is `ww3-libm`'s, so there is no `sqrt` instruction). Not run ⚠.

## Translation notes

- Read from `__w3swldmd_MOD_w3swl6` in the reference's `ww3_shel` (gdb `disassemble`): the five automatic arrays are `malloc`ed
  per call; `MAXVAL` and `MAXLOC` are inline (no call of `_gfortran_maxval_r4`); the only calls are `irange`,
  `_gfortran_reshape_r4`, `malloc`, `memcpy` and `free`; `fsqrt` at two sites, `fadd s1,s1,s1` for `2.*SQRT`, and
  `-(2./3.)*B1*SIG*SQRT(BN)` ends in `fnmul`, a negation after the product. No `fmadd`.
- `-(2./3.)*B1*SIG(IK)*SQRT(BN(IK))` is `-((((2./3.)*B1)*SIG)*SQRT(BN))` (unary minus ranks below `*`). The two forms
  differ only in the sign of a NaN (negative `WN` gives a negative `BN`): the pokes catch it, and the kernel holds the
  product in `black_box` and flips the sign as an integer, as `W3FLX1` does.
- `WN**3` is `(WN*WN)*WN` at `-O3`; `2./3.` is folded to `0x3f2aaaab`; `1.E-30` and `1.0E-34` are `f32` literals.
- `MAXVAL(K,1)` on a column that holds a NaN, and `K` after `:366-372`, are written as the source says; see "Branches reached"
  for why no output sees them.
- `IRANGE(X0,X1,DX)`: `N = INT(REAL(X1-X0)/REAL(DX))+1` in `f32`. For `IRANGE(1,NSPEC,NTH)` this is `NK` unless `NK` is so large
  that `(NSPEC-1)/NTH` rounds up to `NK` (`NTH > 2^24/NK`) ⚠. The Fortran assigns the `N` elements into `IKN(NK)` without a size
  check at this pin (the check that ww3-gpu's port report quotes at NOAA's pin is not in this source), and the kernel does not
  test `N = NK`.

## Not reached

- `NK` or `NTH` large enough for `IRANGE`'s `f32` division to round up ⚠.
- A grid with `NK = 1` or `NTH = 1` ⚠.
- `SWL6S6` false (`SWLB1 <= 0.`): `W3SWL6` is not called, so there is nothing to compare.
- The `W3_T6` (`NDST`) and `W3_S` (`STRACE`) blocks: output only, left out.

## Timing

None. No timing without a device run (`AGENTS.md`).
