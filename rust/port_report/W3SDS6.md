# W3SDS6

ST6's whitecapping dissipation, after Babanin et al. (2010) as implemented by Rogers et al. (2012). Per frequency band it
computes the spectral density `EDENS(f)` and a threshold density `ETDENS(f)`, normalises the excess by the threshold
(`SDSET` true) or by `EDENS` itself (`SDSET` false), and builds an inherent term `T1` and a cumulative term `T2` that
sums the normalised excess of all bands up to the current one. `D = -(MAX(0,T1) + MAX(0,T2))`, repeated over the
directions, and `S = D * A`. Translated from `model/src/w3src6md.F90:546-741` at metocean/WW3 `92e2cf66`. Kernel:
`rust/crates/ww3-kernels/src/sds6.rs`. No `ww3-libm` function: the routine has no `exp`, `pow` or `sqrt`. The run-time
integer powers are libgcc's `__powisf2`, written out in the kernel.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts and
in compiled SPIR-V on lavapipe (ADR-0007). It is not called from `W3SRCE` yet, and it has run on no GPU ⚠. The siblings
`W3SPR6`, `W3SIN6`, `W3SWL6` and `W3FLX4` are not part of this port. `LFACTOR`, `TAU_WAVE_ATMOS`, `TAUWINDS` and
`IRANGE` of the module are not translated either; `IRANGE(1,NSPEC,NTH)` is inlined as `1 + (IK-1)*NTH`.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)` | `REAL`, in | `a` at offset `o` |
| `CG(NK)`, `WN(NK)` | `REAL`, in | `cg`, `wn` at offset `k_off` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `o` |
| `NK`, `NTH`, `SDS6ET`, `SDS6P1`, `SDS6P2`, `SDS6A1`, `SDS6A2`, `DTH`, `XFR` | module `INTEGER`, `LOGICAL`, `REAL` | `Sds6Params` (`SDS6ET` as 0 or 1) |
| `SIG2(NSPEC)`, `DSII(NK)` | module `REAL` tables | `sig2`, `dsii`, one copy for all points |
| `EDENS`, `ETDENS`, `NEXDENS`, `ADF` (automatic arrays of `NK`) | `REAL` | scratch `w` at `w_off`, `work_len` = 4 `NK` |

No `f64` anywhere (v, `w3src6md.F90:546-741`). `GRAV`, `DDEN`, `NDSE` and `EXTCDE` are imported and unused. The `W3_T6`
and `W3_S` blocks are output only and are left out. `FREQ`, `T1`, `T2`, `T12`, `EXDENS` and `DFII` are folded into the
loops that use them; the scratch holds only what a later band needs.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST6` and `FLX4` (`switches/switch_chaceon_st6`; the image
`chaceon-ref-capture:switch_chaceon_st6`). gdb records the arguments of `ww3_ts1` (`regtests/capture/sds6.py`): the
scalars, `SIG2` and `DSII` from `W3GDATMD` once, then `A`, `CG`, `WN` and the `S` and `D` the call returned. The recorded
scalars equal the namelist values on every capture (`SDSET` T, `SDSA1` 4.75E-6, `SDSA2` 7.00E-5, `SDSP1` and `SDSP2` 4 by
default, `ww3_grid.F90:2127-2131`).

| Capture (`st6sds6...`, 2500 m) | Change | Calls | Host differ | SPIR-V differ |
|---|---|---|---|---|
| (default), stride 2 | none | 2455 | 0 | 0 |
| `storm` | wind 30 m/s, stride 8 | 974 | 0 | 0 |
| `light` | wind 6 m/s, stride 8 | 284 | 0 | 0 |
| `calm` | wind 0 m/s, stride 8: every spectrum is zero | 271 | 0 | 0 |
| `noet` | `SDSET=F`, stride 8 | 1058 | 0 | 0 |
| `noetp` | `SDSET=F, SDSP1=2, SDSP2=1`, stride 8 | 1059 | 0 | 0 |
| `p` | `SDSA1=9E-6, SDSA2=1.4E-4, SDSP1=3, SDSP2=5`, stride 8 | 669 | 0 | 0 |
| `p0` | `SDSP1=0, SDSP2=1`, stride 8 | 1056 | 0 | 0 |
| `neg` | `SDSP1=-1, SDSP2=-2`, first 300 calls | 300 | 0 | 0 |
| `noetneg` | `SDSET=F, SDSP1=-1, SDSP2=-2`, first 300 calls | 300 | 0 | 0 |
| `a1neg` | `SDSA1=-4.75E-6` (T1 clipped), stride 8 | 598 | 0 | 0 |
| `a2neg` | `SDSA2=-7E-5` (T2 clipped), stride 8 | 473 | 0 | 0 |
| `aneg` | both coefficients negative, stride 8 | 1059 | 0 | 0 |
| `grid` | `NK=40`, `XFR=1.07`, stride 8 | 290 | 0 | 0 |
| `grid2` | `NTH=36`, `XFR=1.05`, stride 8 | 274 | 0 | 0 |
| `poke` | inputs rewritten at the routine's entry, first 500 calls | 500 | 0 | 0 |
| `noetpoke` | the same with `SDSET=F` | 500 | 0 | 0 |

12 120 calls, 42 471 552 values (`S` and `D`, call and one-buffer batch layouts), 0 differ on the host; 17 captures, 0
differ on SPIR-V (lavapipe). Sampling is one call in two or eight, so the files stay at 132 MB ⚠: a call that is rare in a
run can fall between two samples.

```bash
just capture sds6 st6sds6 2500 2        # CAPTURE_SWITCH defaults to switch_chaceon_st6 for sds6 and st6*
just capture sds6 st6sds6noet 2500 8    # likewise st6sds6storm, light, calm, p, p0, a1neg, a2neg, aneg, grid, grid2
just capture sds6 st6sds6neg 2500 1     # the first 300 calls; st6sds6noetneg, st6sds6poke and st6sds6noetpoke likewise
just sds6-cmp /work/capture/sds6_st6sds6_2500m.bin /work/capture/sds6_st6sds6noet_2500m.bin ...
just spirv && just spirv-replay sds6 st6sds6 2500
```

### What the captures reach

The routine has three data-dependent branches (`SDS6ET`; for `SDSET` false, `ALL(EDENS > EDENSMAX)`; and the two
`MAX(0.,...)` guards) and four run-time inputs that matter (`SDSP1`, `SDSP2`, the signs of `SDSA1` and `SDSA2`, the
grid). Counted with an instrumented copy of the kernel, outside the repository:

| Branch | Calls | Where |
|---|---|---|
| `SDSET` true | all captures except `noet`, `noetp`, `noetneg`, `noetpoke` | model runs |
| `SDSET` false, `ALL` false | 1058 of `noet`, 1059 of `noetp`, 400 of `noetpoke` | model runs: the bands above the peak fall below `1E-5` of the maximum |
| `SDSET` false, `ALL` true | 100 of `noetpoke` | **poke only**: a flat spectrum, which no run of `ww3_ts1` makes |
| `EXDENS > 0` in some band | 58 472 of 88 380 bands in the default capture; none in `calm` and `neg` | model runs |
| `MAX(0,T1)` or `MAX(0,T2)` clips a negative or NaN | `a1neg`, `a2neg`, `aneg`, and a NaN `WN` in `poke` | namelist, poke |
| zero spectrum | every call of `calm`, `neg`; the first call of the others | model runs |
| NaN in `A` | 50 of `poke`, 50 of `noetpoke` | poke only |

`poke` (`CAPTURE_POKE`) rewrites the inputs at the routine's entry in a cycle of ten calls: unchanged; a zero `A`; one
band at 1E6 or 1E3 over a floor of 1E-12; a NaN in `A`; a NaN in `WN`; `CG` 0, `WN` -1 and `CG` -1 in one band; a flat `A`
of 1 and of 1E4. It restores the inputs and zeroes `S` and `D` at exit, so the model goes on. The real Fortran runs
every line on the written values.

## Fault runs

On all 17 captures, natively on arm64 macOS (the kernel has no libm call and the arithmetic is the same IEEE `f32`; the
host verdict above is in the reference's own image). `SDS6_FAULT` moves one input one ULP up, or flips `SDSET` or adds 1
to an exponent, before the replay. The other rows edit a copy of the kernel by hand. "Differ" counts `S` and `D` in the call
layout.

| Fault | Result |
|---|---|
| `SDSA1`, `SDSA2`, `DTH`, `XFR` one ULP up | caught on 12 to 13 captures each (2.7 to 9.8 million values). `SDSA1` is not seen by `a1neg` and `SDSA2` not by `a2neg`: the negative coefficient is clipped by `MAX` |
| `SDSP1` or `SDSP2` plus 1 | caught on 12 captures (11.6 and 11.4 million) |
| `SDSET` flipped | caught on 13 captures (12.1 million) |
| `SIG2(1)` one ULP up | caught by `p0`, `poke`, `noetpoke` only (56 139 values): band 1 has no excess in the model runs, and only `SDSP1=0` shows `T1` without one |
| `DSII(1)`, `CG(1)`, `WN(1)` one ULP up | caught by `poke` and `noetpoke` only: the first band's `A` is zero in the model runs and its threshold is the highest, so only a 1E6 spike there makes an excess |
| `A(1)` one ULP up | caught by `neg`, `noetneg`, `p0`, `poke`, `noetpoke` (2724 values), not by the others, for the same reason |
| no `XFAC` | caught, 10.9 million |
| `XFAC` also at `IK=NK` | caught, 452 863 values on 12 captures |
| `XFAC` also at `IK=1` | caught by `poke` and `noetpoke` only (5088): band 1 has no excess in the model runs |
| `XFAC` on every band up to `IK` (the reading `DFII(1:IK)*XFAC`) | caught, 10.5 million: the Fortran resets `DFII` for every `IK` and scales only `DFII(IK)` |
| `XFAC` one ULP up | caught, 889 895 values |
| `ALL` forced true | caught by `noet`, `noetp`, `noetneg` and `noetpoke` (914 558 values) |
| `ALL` forced false, or no `ALL` test | **not caught, by construction**: the `ELSE` loop gives the `ALL` branch's result bin by bin when every bin passes, so the two are one function. Only the 100 flat-spectrum calls take the `ALL` path ⚠ |
| `EDENSMAX` `1E-4` or `1E-6` for `1E-5` | caught by `noet`, `noetp`, `noetpoke` (144 624 values) and by `noetpoke` alone (3312) |
| `ALL(EDENS >= EDENSMAX)` for `>` | caught by `noetneg` only (518 400): a zero spectrum makes `0/0` instead of 0, and `MAX(0,NaN)` hides it unless the exponent is negative |
| `MAXVAL` as the first element | caught by `noetpoke` only (81 703) |
| `MAX` that returns a NaN (`x < 0 ? 0 : x`) | caught by `aneg`, `poke`, `noetpoke` (681 024) |
| `MAX` that returns `-0` (`x >= 0 ? x : 0`) | caught by `aneg` only (473 952): `-0 + -0` needs both coefficients negative |
| no `MAX` on `EXDENS`, `SDSET` true or false | caught (12.2 and 3.6 million) |
| no `MAX` on `T1` or `T2` | caught by `a1neg`, `a2neg`, `aneg` and `poke` (2.0 and 1.9 million) |
| `T12 = 0 - x` for `-(x)` | caught, 7.7 million (`+0` for `-0`) |
| `T12 = -1.0 * x` | **not caught, and not catchable**: `MAX(0,.)` never returns a NaN, so the sum is never NaN and `-1.0*x` is `-x` for every value that reaches the line. The kernel still flips the sign as an integer, as the disassembly does (`fneg`) |
| `__powisf2` as repeated products | caught, 2.1 million (`x*x*x*x` differs from `(x*x)*(x*x)`) |
| a negative exponent without the reciprocal | caught by `neg` and `noetneg` (1 036 800) |
| `ADF` with `SDSP1` | caught by `noetp`, `p`, `p0` (3.9 million); the default `SDSP1 = SDSP2 = 4` cannot show it |
| `EDENS` as `sum*(TPI*SIG2)`, `(sum*DTH)*SIG2*TPI`, or `*(1/CG)` | caught, 4.0 to 5.5 million |
| `ETDENS` denominator `((CG*WN)*WN)*WN` | caught, 5.2 million |
| `FREQ = SIG2*(1/TPI)`, `DFII = DSII*(1/TPI)` | caught, 338 233 and 1.46 million |
| `T1 = A1*(FREQ*N**P)` for `(A1*FREQ)*N**P` | caught, 776 292 |
| `BNT = 0.035*0.035` in `f64` | **not caught**: the product has the same `f32` value either way |
| the `SUM` over directions starting at `-0.0` | **not caught**: it differs only for an `A` of all `-0`, which nothing makes |
| `S = D*A + 0.0` | caught, 3.8 million (`-0` becomes `+0`) |

Faults 41 of 46 caught; the five that are not are an equivalent program (two), an unreachable NaN sign, an identical
`f32`, and a sign of zero that no input reaches.

## GPU code

- SPIR-V: entry point `sds6` in `rust/gpu-spirv/shader`, passes `spirv-val`; the pass that decorates float operations
  reported 57 456 operations in the whole module. Compiled SPIR-V replays every capture at 0 ULP on lavapipe. The `MAX` is
  written as `x > 0.0 ? x : 0.0`: `FMax` returned `-0` for `MAX(+0,-0)` there (ww3-port skill).
- PTX: kernel `sds6` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false`. The module has
  no float `fma` or `mad.f32`. Not run ⚠.
- One thread per spectrum, an `O(NK^2)` loop for `T2` as in the Fortran, and a scratch of 4 `NK` per thread in a global
  buffer. `sig2` and `dsii` are shared and read-only.

## Translation notes

- Read with gdb `disassemble` on `ww3_shel` of the capture image (`__w3src6md_MOD_w3sds6`, 4.3 kB):
  - `MAX(0.0, x)` is `fmaxnm` against a zero register, vector and scalar: a NaN or `-0` gives `+0`. `max0` is
    `x > 0.0 ? x : 0.0`.
  - `T12 = -1.0*(...)` is `fadd` then `fneg`, not a multiply.
  - `NEXDENS**SDS6P1` and `**SDS6P2` are one call of `__powisf2@plt` per band each, in two loops; `ANAR` is a real array
    of ones multiplied in (`1.0*x` is exact).
  - `EDENS` is a sequential `fadd` chain from 0 over the directions, then `fmul` by `TPI`, `fmul` by `SIG2`, `fmul` by
    `DTH`, `fdiv` by `CG`. `ETDENS` is `fdiv` of the folded `TPI*BNT` (`0x3bfc364c`) by `(ANAR*CG)*((WN*WN)*WN)`.
  - `DFII = DSII/TPI` is redone, with `fdiv`, inside the `IK` loop; only `DFII(IK)` is multiplied by `XFAC`, for
    `1 < IK < NK` (`:714-716`). The sum of `T2` is a sequential `fadd` chain of `ADF*DFII` from 0, lane by lane even where
    the products are vector multiplies. `A2*sum` follows.
  - `MAXVAL(EDENS)` starts at `-Inf`, takes the first element that is not NaN, and replaces it on `<`; all NaN gives the
    constant NaN without the multiply by `1E-5`. The kernel multiplies a NaN instead, which only reaches comparisons.
  - `XFAC = (1-1/XFR)/(XFR-1/XFR)` has one `fdiv` by `XFR`.
- `IKN = IRANGE(1,NSPEC,NTH)` has `INT(REAL(NSPEC-1)/REAL(NTH))+1` entries, which is `NK` for the grids captured (36 x 24,
  40 x 24, 36 x 36) ⚠. The kernel takes `NK` without the division; a grid where the `f32` quotient rounds up to an integer
  (`NK*NTH` near 2^24) would change the Fortran's shape.
- The `**` with `SDS6P1` and `SDS6P2` is `INTEGER`, as declared in `w3gdatmd.F90:901`.

## Not translated, not reached

- The call in `W3SRCE` (`w3srcemd.F90:1312`) and the ST6 state around it. `W3SDS6` also has callers in `W3EXPO` and
  `GXEXPO` (post-processors), not built.
- `ALL(EDENS > EDENSMAX)` true occurs only by poke ⚠. NaN in `A`, a NaN or negative `WN` and a zero or negative `CG` occur
  only by poke; negative `SDSA1` or `SDSA2` and negative exponents only by namelist.
- `NK` of 1 or 2 (no band with `1 < IK < NK`), and a grid where `NK*NTH` approaches 2^24.
- Depth is not an input: the routine ignores it.
- The `W3_T6` diagnostic output.

## Timing

None. No timing without a device run (`AGENTS.md`).
