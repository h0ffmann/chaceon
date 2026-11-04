# W3SIN3

The wind input of WAM cycle 4 (WAM, the Wave Model of the WAMDI Group, 1988) in Janssen's quasi-linear form
(Janssen 1991): the growth rate of each spectral bin follows from the critical-layer height of the wind profile,
`ZLOG = ALOG(MU)`, with a gustiness-free `STAB0` build, Janssen's (2004) swell damping through `SWELLF`, and Bidlot's
tabulated high-frequency stress `TAUHFT` for the tail beyond the resolved band. `ST3` is not in MetOcean's switch
(`switches/switch_metocean_st4`) nor in the port's target `switch_chaceon_shrd`. Translated from
`model/src/w3src3md.F90` at metocean/WW3 `92e2cf66`: `W3SIN3` (`:384-717`). Kernel:
`rust/crates/ww3-kernels/src/sin3.rs`. `W3SPR3`, `CALC_USTAR`, `INSIN3` with `TABU_STRESS` and `TABU_TAUHF`, and
`W3SDS3` are other ports; this one takes `TAUHFT`, `DELUST` and `DELALP` from the capture, as `INSIN3` left them.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts
and in compiled SPIR-V on lavapipe (ADR-0007). It is not called from `W3SRCE` (`srce.rs`), and `ww3_grid` and the host
read `switch_chaceon_shrd`'s layout only ⚠. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `CG(NK)`, `K(NSPEC)` | `REAL`, in | `a`, `cg`, `k` at offsets `spec_off`, `cg_off` |
| `USTAR`, `DRAT`, `USDIR`, `Z0` | `REAL`, in | `Sin3Point` |
| `U`, `AS`, `CD`, `ICE`, `IX`, `IY` | in, unused in this build | dropped |
| `TAUWX`, `TAUWY`, `TAUWNX`, `TAUWNY` | `REAL`, out | `Sin3Out` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `spec_off` |
| `LLWS(NSPEC)` | `LOGICAL`, out | `llws: &mut [u32]`, 0 or 1 |
| `NK`, `NTH`, `NSPEC`, `BBETA`, `AALPHA`, `ZZALP`, `SSINTHP`, `DTH`, `SSWELLF(1)`, `DELUST`, `DELALP` | module `REAL` and `INTEGER` | `Sin3Params` |
| `SIG(0:NK+1)`, `SIG2(NSPEC)`, `DDEN2(NSPEC)`, `ECOS`, `ESIN(NSPEC+NTH)` | module `REAL` | slices, as `sin4` |
| `TAUHFT(0:100,0:200)` | module `REAL`, `W3SRC3MD` | `tauhft`, index `ind + 101*j` |

`ZZWND`, `XFR`, `DDEN`, `TH`, `EC2`, `ZZ0RAT` are named in the `USE` list and not read. No `f64` anywhere (v,
`w3src3md.F90`). `Sin3Out::reach` is a bitmask of the branches a call took, written for `sin3_cmp` to print and never
read by the arithmetic.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST4` replaced by `ST3` (`switches/switch_chaceon_st3`, built by
`just ref-build switch_chaceon_st3`; `BT1`, `FLX0`, `STAB0`, `PR3`, `NL1`). gdb records every call of `ww3_ts1`
(`regtests/capture/sin3.py`; `NK = 36`, `NTH = 24`). Outputs compared: `S`, `D`, `LLWS` and the four stresses.

| Capture | Variant | Calls | Host differ | SPIR-V differ |
|---|---|---|---|---|
| default, 2500 m | 12 m/s, `SIN3` defaults (`BETAMAX 1.2`, `SWELLF 0`) | 6934 | 0 | 0 |
| light, 8 m | 6 m/s, stride 3 | 2243 | 0 | 0 |
| storm, 8 m | 30 m/s, stride 3 | 6481 | 0 | 0 |
| calm, 20 m | 0 m/s, stride 3; every `S` is zero, `D` is not | 2161 | 0 | 0 |
| swell, 2500 m | `SWELLF=1`, stride 4: `D < 0` in every call | 1734 | 0 | 0 |
| swell, 20 m | the same, 20 m | 1734 | 0 | 0 |
| tune, 20 m | `BETAMAX 1.52, SINTHP 1, ZALP 0.008, ALPHA0 0.0085, SWELLF 0.8`, stride 3 | 2326 | 0 | 0 |
| poke, 2500 m | entry pokes of `USTAR` and `Z0`, first 500 calls | 500 | 0 | 0 |

24 113 calls, 62 597 348 values (S, D, LLWS, four stresses; call layout plus batch layout), 0 differ.

What the 8 captures reached (calls, of 24 113; `sin3_cmp` prints it per capture):

| Branch | Calls |
|---|---|
| tail limiter scaled `TAUWX`, `TAUWY` (`:702`) | 10 708 |
| `EXP(ZLOG)` growth (`:611`) | 21 702: every capture but calm, and 250 of 500 pokes |
| `ZLOG >= 0` on the wind side (`:616`) | 21 765: all but 2 266 storm and poke calls |
| `COSWIND <= 0.01` (`:620`) | all 24 113 |
| `D < 0`, so `STRESSSTABN` (`:627`) | 5 794 (the swell and tune captures) |
| `IND` clamped to 1 (`:683`) | 2 327: calm, and 166 pokes |
| `IND` clamped to `IUSTAR-1` | 84, pokes only |
| `J` clamped to 1 (`:687`) | 5 612 |
| `J` clamped to `IALPHA-1` | 2 412: calm, and 251 pokes |

```bash
just ref-build switch_chaceon_st3
just capture sin3 st3 2500          # CAPTURE_SWITCH defaults to switch_chaceon_st3 for sin3 and st3* variants
just capture sin3 st3poke 2500 1    # the first 500 calls, with entry pokes
just sin3-cmp /work/capture/sin3_st3_2500m.bin
just spirv && just spirv-replay sin3 st3 2500
```

## Fault runs

Each fault replaces one expression of `sin3.rs`, rebuilds `sin3_cmp` and replays the default, swell (2500 m), poke
and tune (20 m) captures, with storm (8 m) added for the `SIG(NK)**5`, `TPI` and the two `CONST0` ULP faults. Calm and light were not in any
fault run. Differing values are summed over the call and batch layouts.

| Fault | Result |
|---|---|
| `USTAR` one ULP up (`SIN3_FAULT=ustar`) | caught: 13 470 238 values |
| `TAUHFT` one ULP up (`SIN3_FAULT=table`) | caught: 18 470 |
| `CONST2` as `(DDEN2/CG)*(GRAV/(..))` | caught: 12 500 |
| `ZLOG**4` as `((z*z)*z)*z` | caught: 5 577 696 |
| `TEMP2` as `CONST2*(D*A)` | caught: 12 480 |
| Limiter `TAUWX*(TAUWB/TAUW)` | caught: 2 502 |
| `KAPPA/(ZCN*(UST*CM))` | caught: 1 637 182 |
| `CONST3` as `SSWELLF*2*(KAPPA*DRAT)` | caught: 3 939 610 |
| tail `CONST0` one ULP up | caught: 17 136 |
| stress-loop `CONST0` one ULP up | caught: 15 405 498 |
| `UCN` without `ZZALP` | caught: 20 900 544 |
| `COSWIND**SSINTHP` as a square | caught: 2 466 504 |
| `ZLOG >= 0` branch returns 0 instead of `ZBETA` | caught: 7 831 584 |
| `LLWS` 1 where the source says 0 | caught: 10 222 848 |
| `COSWIND` threshold 0.011 for 0.01 | caught: 6 480 |
| tail `MAX(COSWIND,0.)` removed | caught: 41 688 |
| `TAUHF` without `UST**2` | caught: 45 756 |
| `STRESSSTABN(1)` summed with `ESIN` | caught: 8 114 |
| `DELUST` one ULP up | caught: 7 240 |
| `J` lower clamp 0 for 1 | caught: 20 134 |
| `J` upper clamp `IALPHA` for `IALPHA-1` | caught: index out of range at `J+1`, only the poke reaches it |
| `IND` lower clamp 0 for 1 | caught: 352 (pokes) |
| `IND` upper clamp `IUSTAR` | caught: 332 (pokes) |
| `DELI1` without `MIN(..,1.)` / without `MAX(..,0.)` | caught: 332 / 480 (pokes) |
| `DELJ1` without `MAX(..,0.)` / without `MIN(..,1.)` | caught: 20 128 / 998 |
| Limiter floor `MAX(UST2-EPS1, 0.)` for `EPS2**2` | caught: 660 |
| tail `CONST0` regrouped, `SIG(NK)**5` as a left chain, `TPI` moved | **no effect**: each gives the same `f32`, a single number per grid; the order rests on the GIMPLE ⚠ |
| `MAX(UST,1E-5)` as `1E-4` (`:686`) | **no effect**: a smaller `UST` already saturates `XJ`, so `J = IALPHA-1` and `DELJ1 = 1` either way; the floor only protects against a divide by zero, which gives `+Inf` and the same clamp |
| `MAX(USTAR,EPS2)` as `MAX(USTAR,1E-5)` (`:700`) | **no effect, and provably so**: for `USTAR < 1E-6`, `MAX(UST2-EPS1, EPS2**2)` is `EPS2**2` whatever `UST2` is; the entry poke `USTAR = 5E-7` runs the line |
| `XJ` without `MAX(..,0.)` (`:686`) | **no effect**: a negative `XJ` gives `J = 1` and `DELJ1 = MAX(0., MIN(1., XJ-1)) = 0` as the clamped value does |
| `D < 0` as `D <= 0` | **no effect**: `D = 0` adds `TEMP2 = 0` |
| `ZLOG < 0` as `ZLOG <= 0` | **undetected**: needs `ZLOG` exactly 0 |

## GPU code

- SPIR-V: entry point `sin3` in `rust/gpu-spirv/shader`, 15 storage buffers, passes `spirv-val` (`just spirv`).
  `just spirv-replay sin3` is 0 differ on all 8 captures (lavapipe, llvmpipe LLVM 15).
- PTX: kernel `sin3` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`).
  Its body (2 883 lines of PTX) has `div.rn.f32` and no float `fma`; the only `mad` is `mad.lo.s32`, an address. Not
  run ⚠.

## Translation notes

- The order of operations is read from the optimised GIMPLE of `w3sin3` (`gfortran 12.2 -O3 -ffp-contract=off`,
  `-fdump-tree-optimized` on the module built with the switch's `-DW3_*` flags). `UST**2`, `UCN**2` and `COSWIND**3` are
  products, `ZLOG**4` is `(z*z)*(z*z)`, `SIG(NK)**5` is `x2*(x*x2)`, `COS` and `SIN` of `USDIR` are one `sincosf`
  (`__builtin_cexpif`), `EXP`, `ALOG` and `COSWIND**SSINTHP` call `expf`, `logf` and `powf`. `KAPPA**2` and
  `GRAV**2*TPI` are folded constants, with one rounding each, as Rust's `f32` constants are.
- `CONST3 = SSWELLF(1)*2.*KAPPA*DRAT` is `((s*2)*KAPPA)*DRAT`; `CONST2` is `((DDEN2/CG)*GRAV)/((SIG/K)*DRAT)`;
  `ZBETA` is `(CONST3*(COSWIND+KAPPA/((ZCN*UST)*CM)))*UCN**2`.
- gfortran vectorises the `TEMP` loop over `ITH` four wide and then adds the lanes in order, so the sum keeps the
  Fortran order. `S = D*A` is also vectorised, one product per element.
- `MAX` and `MIN` compile to `fmaxnm` and `fminnm` (`MAX_EXPR`); Rust's `f32::max` and `min` return the same, NaN
  included. `INT(XI)` is `fcvtzs`, which Rust's `as i32` matches (saturating, NaN to 0).
- `STAB3` is not in this build: `ISTAB` is 3 and `D = DSTAB(3,:)`, so there is no average over two friction velocities.
  `Z0VISC` (`:549`) is dead and `TAUX`, `TAUY` unused.
- Every `D(IS)` is written whichever branch runs (`ZBETA`, which is 0 for `SWELLF = 0`, and negative with
  `SWELLF > 0` when `COSWIND < 0`), as `DSTAB` is zeroed first and then filled for every `IS`.

## Not translated, not reached

- `STAB3` (`:513-516`, `:557-566`, `:642-649`), `W3_S`, `W3_T`, `W3_T0` and `W3_T1` test output: not in the build.
- The call from `W3SRCE`: `W3SPR3` and `CALC_USTAR` produce `USTAR`, `Z0`, `CD` (`srce.rs` is not changed).
- `INSIN3` and its tables: taken from the capture, so the replays prove nothing about `TABU_TAUHF` ⚠.
- No capture reaches a `NaN` or `Inf` input, `USTAR = 0` (which gives a `NaN` in `D` through `0*Inf`), or a negative
  `Z0` ⚠. `ZLOG` exactly 0 and `COSWIND` exactly 0.01 are not reached.
- Other grids: `NK = 36`, `NTH = 24` only; other `DTH` and `XFR` change `CONST0` of the tail, whose regroupings were
  not caught here ⚠.

## Timing

None. There is no timing without a passed comparison on a device, and this ran on none.
