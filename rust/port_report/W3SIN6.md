# W3SIN6

The wind input of `ST6` (Donelan, Babanin, Young and Banner 2006, as Rogers et al. 2012 implement it): the growth
rate of each bin follows the directional narrowness and the saturation of its frequency band, a reduction factor
`LFACT(sigma)` then limits the stress the input carries to what the friction velocity allows, and a smaller negative
input acts on adverse winds. `ST6` is not in MetOcean's switch (`switches/switch_metocean_st4`) nor in the port's
target `switch_chaceon_shrd`. Translated from `model/src/w3src6md.F90` at metocean/WW3 `92e2cf66`: `W3SIN6`
(`:290-519`) and the four helpers it uses in the same module, `LFACTOR` (`:769-1044`), `TAU_WAVE_ATMOS`
(`:1066-1197`), `IRANGE` (`:1214-1243`) and `TAUWINDS` (`:1267-1296`). Kernel:
`rust/crates/ww3-kernels/src/sin6.rs`. `W3SPR6`, `W3SDS6`, `W3SWL6` (which has its own `IRANGE`) and `W3FLX4` are
other ports.

**Status: `routine`** in `rust/ports.tsv` for `W3SIN6` and for each helper: every captured call bit-identical, on the
host in call and batch layouts and in compiled SPIR-V on lavapipe (ADR-0007). The helpers are compared through
`W3SIN6`, since gfortran inlines `LFACTOR` into it and `TAU_WAVE_ATMOS`, `IRANGE` and `TAUWINDS` have no other caller
at the pin (v, `grep -rn` over `model/src` finds no other caller). It is not called from `W3SRCE` (`srce.rs`), and `ww3_grid` and the
host read `switch_chaceon_shrd`'s layout only ⚠. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `CG(NK)`, `WN2(NSPEC)` | `REAL`, in | `a`, `cg`, `wn2` at offsets `spec_off`, `cg_off` |
| `UABS`, `USTAR`, `USDIR`, `DAIR` | `REAL`, in | `Sin6Point` |
| `CD` | `REAL`, in, never read by the body | `Sin6Point::cd`, unused |
| `TAUWX`, `TAUWY`, `TAUNWX`, `TAUNWY` | `REAL`, out | `Sin6Out` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `spec_off` |
| `NK`, `NTH`, `NSPEC`, `DTH`, `XFR`, `SIN6A0`, `SIN6WS` | module `REAL` and `INTEGER` | `Sin6Params` |
| `SIG2(NSPEC)`, `DDEN2(NSPEC)`, `ECOS`, `ESIN(NSPEC+NTH)` | module `REAL` | slices, as `sin4` |
| the temporaries (`K`, `SDENSIG`, `CINV`, `SIG10Hz` and the rest) | `ALLOCATE` and automatic arrays | `work`, `work_len(nk, nspec, nk10)` per spectrum |

No `f64` anywhere (v, `w3src6md.F90`). `CONSTANTS` gives `DWAT`, `TPI`, `GRAV` to `W3SIN6` and `DAIR`, `GRAV`, `TPI` to
`LFACTOR`. `LFACTOR` uses the module constant `DAIR = 1.225` (`constants.F90:63`) and not `W3SIN6`'s `DAIR`
argument, which only scales `D`: a capture that moves the argument (`DAIR = 1.0` in the poke) shows it.
`Sin6Out::reach` is a bitmask of the branches a call took and `iters` the final `IK` of `LFACTOR`, written for
`sin6_cmp` to print and never read by the arithmetic.

`work` is the caller's: a kernel may not allocate and the Fortran `ALLOCATE`s the eight `NK10Hz` arrays of `LFACTOR`
on every call (`NK10Hz = 57` for `NK = 36`, `XFR = 1.1`, `FR1 = 0.0485`; 111 for `XFR = 1.05`). One spectrum needs
`5*NK + NSPEC + 9*NK10Hz` values, 1 557 for the default grid. The entry points compute `NK10Hz` from `SIG2(1)` and
`XFR` with `nk10hz` and stride `work` by it.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST4` and `FLX0` replaced by `ST6` and `FLX4`
(`switches/switch_chaceon_st6`, built by `just ref-build switch_chaceon_st6`). gdb records every call of `ww3_ts1`
(`regtests/capture/sin6.py`; `NK = 36`, `NTH = 24`, but see the grid variants). Outputs compared: `S`, `D` and the four
stresses.

| Capture | Variant | Calls | Host differ | SPIR-V differ |
|---|---|---|---|---|
| default, 2500 m | 12 m/s, `SIN6` defaults (`SINA0 0.09`, `SINWS 32`), stride 3 | 1637 | 0 | 0 |
| light, 8 m | 6 m/s, stride 3 | 755 | 0 | 0 |
| storm, 20 m | 30 m/s, stride 3: `U10 > 22`, so no viscous stress | 2647 | 0 | 0 |
| calm, 2500 m | 0 m/s, stride 3; every `S` is zero | 721 | 0 | 0 |
| `SINA0 = 0`, 2500 m | the negative input off, stride 3 | 1624 | 0 | 0 |
| `SINA0 = -0.05`, 2500 m | a negative `SINA0`, stride 3 | 1624 | 0 | 0 |
| `SINA0 0.04, SINWS 28`, 2500 m | stride 3 | 1723 | 0 | 0 |
| `NK = 60`, 2500 m | `NK10Hz = 60 = NK`: the grid is not extended, stride 3 | 1833 | 0 | 0 |
| `XFR = 1.05`, 2500 m | `NK10Hz = 111`, stride 3 | 728 | 0 | 0 |
| poke, 2500 m | entry pokes, first 500 calls | 500 | 0 | 0 |

13 792 calls, 51 998 720 values (`S`, `D` and the four stresses; call layout plus batch layout), 0 differ on the host;
`just spirv-replay sin6` is 0 differ on the same 10 (lavapipe, `max 0 ULP`).

What the 10 captures reached (calls, of 13 792; `sin6_cmp` prints it per capture):

| Branch | Calls |
|---|---|
| `LFACTOR` iteration ran (`TAU > TAU_TOT`, `:975`) | 12 655 |
| `OVERSHOT` slowed `DRTAU` (`:1010`) | 12 646 |
| loop left at `ABS(ERR) < 1.54E-4` (`:1014`) | 12 617 |
| `ITERMAX` reached, warning written (`:1017`) | 38, entry pokes only (81 passes) |
| `SIGN_NEW = -1` at some pass | 12 646 |
| grid extended to 10 Hz (`NK < NK10Hz`, `:919`) / not extended (`:935`) | 11 959 / 1 833 |
| `SUM(LFACT) < NK`, the spectrum reduced (`:493`) | 12 655 |
| negative input (`SIN6A0 > 0`, `:505`) | 10 544 |
| `TAU_WAVE_ATMOS` extended its grid / some `S < 0` there | 8 711 / 9 777 |
| `KMAX < 1.E-34`, so `K = 1` (`:459`) | 5 917 |
| `TAU_VIS` capped at `0.95*TAU_TOT` (`:952`) | 836: calm and pokes |
| `-5.E-5*U10 + 1.1E-3 < 0`, no viscous stress (`:950`) | 2 686: storm and pokes |

```bash
just ref-build switch_chaceon_st6
just capture sin6 st6sin6 2500 3      # CAPTURE_SWITCH defaults to switch_chaceon_st6 for sin6 and st6* variants
just capture sin6 st6sin6poke 2500 1  # the first 500 calls, with entry pokes
just sin6-cmp /work/capture/sin6_st6sin6_2500m.bin /work/capture/sin6_st6sin6poke_2500m.bin
just spirv && just spirv-replay sin6 st6sin6 2500
```

The other variants are `st6sin6storm 20`, `st6sin6light 8`, `st6sin6calm`, `st6sin6noneg`, `st6sin6aneg`,
`st6sin6tune`, `st6sin6wide` and `st6sin6coarse` (`scripts/ts1_variants.sh`), all at stride 3 and 2500 m unless
named.

The entry pokes (`regtests/capture/sin6.py`) write one value at the real routine's entry and put it back at exit. They
are the only way to some branches: `ww3_ts1` holds `USTAR` near 0.5 m/s. Twelve kinds in rotation: `USTAR` 0.02, 2,
1E-5 and 0.1; `UABS` 30 and 0; `USDIR` 2; `DAIR` 1 and 1E7 (the stress starts a factor 1E7 above `TAU_TOT`, `RTAU`
near 1E5, and the loop does not converge in 80 passes); a negative action in one bin; `USDIR` NaN and `UABS` NaN.

## Fault runs

Each fault replaces one expression of `sin6.rs`, rebuilds `sin6_cmp` in the reference's image and replays the
captures (the first pass the nine then present, the second pass all ten). Four input faults (`SIN6_FAULT`) move one
ULP of `USTAR`, `DAIR`, the first nonzero `A` and `WN2(1)`. 107 runs: 96 caught, 11 not, each explained. A caught
fault counts the values that differ over the call and batch layouts.

| Fault | Result |
|---|---|
| input: `USTAR`, `DAIR`, `A` or `WN2` one ULP up | caught |
| `D`: `DAIR/DWAT` regrouped; tanh argument `10*(SQRTBN2*W1)`; `2.8` as `2.80001`; `1+tanh` as `1-tanh`; offset 11 as 11.001; last two factors regrouped | caught: 2.2 M, 0.36 M, 7.1 M, 7.1 M, 6.8 M, 2.6 M |
| `W1`: `MAX(0,.)` removed; `UPROXY*CINV2` regrouped; cos and sin swapped | caught: 23.7 M, 3.2 M, 10.5 M |
| `UPROXY` with 28 for `SIN6WS` | caught: 16.0 M (the retuned capture also moves it) |
| `DAIR/DWAT` as `DAIR*0.001`; the `DAIR` argument replaced by the constant | caught: 13.2 M, 0.14 M |
| `DSII` as `DDEN2/(DTH*SIG2)`; `CINV` inverted twice | caught: 0.19 M, 1.0 M |
| `ADENSIG` as `SUM*(SIG*DTH)`; `ANAR` as `1/KSUM/DTH`; `SQRTBN` regrouped; `WN**3` as `WN**2` | caught: 3.0 M, 2.9 M, 2.8 M, 19.0 M |
| `KMAX` test off; threshold `1E-4`; `<= 0` | caught: 2.0 M, 12.1 M, 0.07 M |
| `SDENSIG` as `S*(SIG2/CG2)` | caught: 0.65 M |
| `SUM(LFACT) < NK` never reduce; reduce with `LFACT(IK+1)` | caught: 7.1 M, 6.8 M |
| `SUM(LFACT) < NK` as `<=`; always reduce | **no effect**, see below |
| step 7: `MIN` removed; `D - ..` as `D + ..`; `SIN6A0` regrouped; tanh offset 10 | caught: 6.4 M, 12.1 M, 3.4 M, 2.9 M |
| `SIN6A0 > 0` as always true | caught: 1.9 M, only the `SINA0 = -0.05` capture |
| `SIN6A0 > 0` as `>= 0` | **no effect**, see below |
| `TAU_WAVE_ATMOS`: `ABS` removed; `Y` with `ECOS`; `MIN` as `MAX`; `DTH` dropped; tail slope not applied | caught: 9 815 each for the first four, 7 982 |
| `TAUWINDS`: `S*(CINV*DSII)`; `GRAV*(DWAT*SUM)`; last band skipped | caught: 0.75 M, 1.0 M, 7.1 M |
| `LFACTOR` sums: `SDENSX` with `ESIN`; tail slope not squared; `SDENSY` slope dropped; `SIG10Hz(NK)` from `SIG` | caught: 7.1 M, 5.1 M, 5.1 M, 1.2 M |
| `SDENSX` or `SDENSY` without `MAX(0.,S)` | caught: 1 value each, only the poke with a negative action |
| `SDENS10Hz` not times `DTH` | **no effect**, see below |
| 10 Hz grid: `XFR**IK10Hz`; `0.101978` as `0.10198`; `DSII` without `1/XFR`; first bin with `XFR-1/XFR`; last bin without `/XFR`; `CINV10Hz(1:NK)` from `SIG`; `NK <= NK10Hz`; `DSII` times `1+2^-23` | caught: 5.1 M, 5.1 M, 5.1 M, 1.4 M, 5.1 M, 5.1 M, 0.4 M, 1.1 M |
| `NK10Hz`: `CEILING` as `INT+1` | caught: 5.1 M |
| `NK10Hz`: `FRQMAX*TPI/SIG1`; `LOG` ratio as a product with `1/ALOG(XFR)` | **no effect**, see below |
| `TAU_TOT` with 1.2251; with `UABS` | caught: 7.0 M, 7.1 M |
| `TAU_VIS`: 1.2E-3; `MAX(0.)` removed; `U10` for `U10**2`; `(v*u)*u`; `0.9` for `0.95`; `MIN` as `MAX`; cap off | caught: 5.0 M, 2.1 M, 5.0 M, 0.8 M, 7 654, 7.9 M, 7 654 |
| `TAUVX` with `SIN`; `TAUVY` with `COS`; `TAU` without `TAUY**2`; `ERR` divided by `TAU` | caught: 4.4 M, 4.4 M, 7.1 M, 7.1 M |
| `RTAU0 = ERR/91`; `DRTAU` starts at 2.5 | caught: 7.0 M, 7.0 M |
| `TAU > TAU_TOT` as `>=` | **no effect**, see below |
| `UPROXY` of `LFACTOR` from `U10`; `UCINV` as `UPROXY*CINV - 1` | caught: 7.1 M, 7.1 M |
| `LF10Hz`: `MIN(1,.)` removed; the argument scaled | caught: 33 288, 3.3 M |
| loop: `TAUWX` from `SDENSY`; `TAUWY` without `LF`; `SIGN_NEW` negated; `OVERSHOT` never / at once | caught: 7.0 M, 7.0 M, 7.0 M, 7.1 M, 6.9 M |
| loop: `DRTAU` `0.5*DRTAU`; floor `1.001`; `RTAU` times `DRTAU**1`; `RTAU` divided | caught: 7.0 M, 3.4 M, 7.0 M, 7.0 M |
| loop: `ABS(ERR) < 1.55E-4`; signed `ERR` | caught: 35 243, 7.0 M |
| loop: `ABS(ERR) <= 1.54E-4`; `SIGN_NEW` from `TAU - TAU_TOT` | **no effect**, see below |
| `ITERMAX` 4; 79; 81 | caught: 7.0 M, 26, 25 (the last two only with the 38 calls that run out) |
| `__powisf2` with the sign ignored; with the inverse on the positive side | caught: 7.0 M, 7.0 M |
| `LFACT` read one band late | caught: 7.0 M |
| `max_nm`, `min_nm` returning a NaN `x` | caught: 65 742, 84 258 (the NaN pokes) |
| `COS` and `SIN` swapped in `W3SIN6`, and in `LFACTOR` | caught: 18.6 M, 4.2 M |

The eleven that changed nothing:

- `SUM(LFACT) < NK` as `<=`, and always reduce. At equality every `LFACT` is 1 and `D*1` is `D` for any `D`, so
  the statement is a no-op there (provable, v).
- `SIN6A0 > 0` as `>= 0`. At `SINA0 = 0` the branch computes `D - 0*x`, which is `D`, and `TAUNW` from
  `ABS(MIN(0.,S))` of a nonnegative `S`, which is 0 (provable for finite input, v). A negative `SINA0` is caught by
  the always-true fault.
- `SDENS10Hz` not times `DTH`: `SDENS10Hz` feeds only `TAU_NND` (`:960`, `:991`), which feeds the test output and the
  warning `WRITE` (`:1005`, `:1017`). Dead in this build; the kernel does compute `SDENS10Hz` for its tail slope.
- `NK10Hz` with `FRQMAX*TPI/SIG1`, or `ALOG` ratio as `ALOG(...)*(1/ALOG(XFR))`: `CEILING` absorbs a one-ULP
  difference unless the ratio is within an ULP of an integer, which none of the captured grids is (`NK` 36 and 60, `XFR` 1.1 and 1.05) ⚠.
- `TAU > TAU_TOT` as `>=`, and `ABS(ERR) < 1.54E-4` as `<=`: they differ only at exact equality, not reached ⚠.
- `SIGN_NEW` from `TAU - TAU_TOT`: `TAU_TOT > 0` makes the sign of `ERR` the sign of the numerator (equivalent).
- Two first drafts of the NaN fault, `x > y` for `y != y || x > y`, change nothing because `y` is always a constant
  that is not NaN at these sites; the real NaN-`x` faults above replaced them and are caught.

## GPU code

- SPIR-V: entry point `sin6` in `rust/gpu-spirv/shader`, 13 storage buffers, passes `spirv-val` (`just spirv`).
  `just spirv-replay sin6` is 0 differ on all 10 captures (lavapipe).
- PTX: kernel `sin6` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`).
  Its body (10 740 lines of PTX) has 32 `div.rn.f32` and no `fma.` and no float `mad`. Not run ⚠.

## Translation notes

- The order of operations is read from the optimised GIMPLE of `w3src6md.F90` (`gfortran 12.2 -O3 -ffp-contract=off`,
  `-fdump-tree-optimized`, the module built alone against stub modules for `CONSTANTS`, `W3GDATMD`, `W3ODATMD`,
  `W3SERVMD`, `W3TIMEMD` and `W3WDATMD`, with `-DW3_ST6 -DW3_FLX4`). `LFACTOR` is inlined into `W3SIN6`;
  `IRANGE` is a call (`irange.constprop.isra`); `TAU_WAVE_ATMOS` stays a call.
- `COS(USDIR)`, `SIN(USDIR)` are one `sincosf` (`__builtin_cexpif`); `ALOG`, `EXP`, `TANH` and `XFR**x` call `logf`,
  `expf`, `tanhf`, `powf`; `x**2` and `WN**3` are products; `(SIG(NK)/SIG10Hz)**2` is `r*r`.
- `DRTAU**SIGN_NEW` is `__powisf2` (`__builtin_powif`): `powi_f` is libgcc's loop, so `SIGN_NEW = -1` is `1/DRTAU`.
- `GRAV*DWAT*SUM(..)` is `9806 * sum`, the product folded once; Rust's `f32` constant does the same.
- Every `SUM` adds in index order starting from `0.`, including the ones gfortran vectorises (it adds the four lanes
  one by one). `MAXVAL` is `_gfortran_maxval_r4`: the largest element, which for nonnegative action is the plain
  maximum.
- `CEILING(x)+1` compiles to `INT(x)+1` if `x <= REAL(INT(x))`, else `INT(x)+2` (`nk10hz`).
- `SIG10Hz = SIG(1)*XFR**(IK10Hz-1.0)` replaces `SIG` for **all** `NK10Hz` bands, and `DSII10Hz` is rebuilt from it
  (`0.5*SIG10Hz*(XFR-1/XFR)`, ends special), so the `DSII` argument reaches only the branch `NK >= NK10Hz`.
  `UCINV10Hz = 1 - (UPROXY*CINV10Hz)` keeps its parentheses.
- `-5.E-5*U10 + 1.1E-3` compiles to `1.1E-3 - U10*5.E-5`, equal bit for bit. `MAX`/`MIN` are `fmaxnm`/`fminnm`
  (`MAX_EXPR`): `max_nm`, `min_nm` return the other operand for a NaN. The two zeros `+0`/`-0` differ from
  `fmaxnm` in these helpers, and no site can see it (`x - 1.` is never `-0`, every `MIN(0.,S)` is squared or `ABS`ed).
- `STME21 ( TIME , IDTIME )` and the `WRITE (NDST,280)` of `LFACTOR` are I/O: the warning is `reach` bit 1, the
  time string is not built. `TAU_NND` is computed in the Fortran and used only by the `WRITE`: the kernel does not
  compute it. `W3_S` (`STRACE`) and `W3_T6` (test output) are not in the build.

## Not translated, not reached

- The call from `W3SRCE`: `U10ABS`, `USTAR`, `USTDIR`, `CD`, `DAIR` come from `W3FLX4` and `W3SPR6` (`srce.rs` is not
  changed).
- `ITERMAX` (80) is reached only by the entry poke with `DAIR = 1E7`; no run of the model reaches it ⚠ (the longest
  natural run is 17 passes).
- No capture reaches `USTAR = 0` (with a zero `TAU_TOT`, `ERR` is `Inf` or `NaN`), an infinite input, or a negative
  `DAIR` ⚠. `ABS(ERR)` exactly 1.54E-4, `TAU` exactly `TAU_TOT` and a ceiling input within an ULP of an integer are
  not reached.
- `IRANGE` is called with `(1,NSPEC,NTH)` and `(1,NK10Hz,1)` only; `irange_len` gives `NK` and `NK10Hz` there, and the
  kernel uses it for the second. A length different from `NK` (a `REAL` division off by one ULP) cannot happen for
  `NSPEC-1 < 2^24` ⚠.
- Other `NTH`: only 24 was captured ⚠. `NK = 36` and 60 and `XFR` 1.1 and 1.05 were.
- `ST6`'s other terms: `W3SPR6`, `W3SDS6`, `W3SWL6`, `W3FLX4` are not in this port.

## Timing

None. There is no timing without a passed comparison on a device, and this ran on none.
