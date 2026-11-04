# W3SIN2, INPTAB and W3BETA

The wind input of Tolman and Chalikov (1996) for the `ST2` switch: the growth rate of each spectral bin is read from an
interpolation table of the wave age `OMA` and the drag `CLAM`, then a swell filter damps the negative input below a cut-off
frequency taken from the input's own mean frequency. `INPTAB` builds the table once, with `W3BETA` for each node. Translated
from `model/src/w3src2md.F90` at metocean/WW3 `92e2cf66`: `W3SIN2` (`:308-566`), `INPTAB` (`:847-1228`) and `W3BETA`
(`:1081-1224`, a function internal to `INPTAB`). Kernel: `rust/crates/ww3-kernels/src/sin2.rs`; table:
`rust/crates/ww3-host/src/inptab.rs`. `W3SPR2`, `W3SDS2` and `W3FLX2` are other ports.

**Status: `routine`** in `rust/ports.tsv` for all three: every captured call bit-identical, `W3SIN2` on the host in call and
batch layouts and in compiled SPIR-V on lavapipe, `INPTAB` on the host (ADR-0007). `W3SIN2` is not called from `W3SRCE`
(`srce.rs`), and it takes `BETATB` from the capture; `INPTAB` is compared as a table, not through `W3SIN2` ⚠. It has run
on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `CG(NK)`, `K(NSPEC)` | `REAL`, in | `a`, `cg`, `k` at offsets `spec_off`, `cg_off` |
| `U`, `UDIR`, `CD`, `Z0` | `REAL`, in | `Sin2Point` |
| `FPI` | `REAL`, out | `Sin2Out::fpi` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `spec_off` |
| `NK`, `NTH`, `NSPEC`, `ZWIND`, `FTE`, `FTTR`, `XFR`, `FPIMIN`, `FACTI1`, `FACTI2`, `FSWELL` | module `INTEGER` and `REAL` (`W3GDATMD`) | `Sin2Params` |
| `DSIGA`, `DDRAG` | module `REAL` (`W3SRC2MD`), set by `INPTAB` | `Sin2Params` |
| `SIG(0:NK+1)`, `SIG2(NSPEC)`, `DDEN(NK)`, `ECOS`, `ESIN(NSPEC+NTH)` | module `REAL` | slices |
| `BETATB(-400:401, 21)` | module `REAL` (`W3SRC2MD`), column-major | `betatb`, index `(ioma+400) + 802*(icl-1)` |

No `f64` anywhere in the three routines (v, `w3src2md.F90`). `Sin2Out::reach` is a bitmask of the branches a call took,
written for `sin2_cmp` to print and never read by the arithmetic. `SIN1A(NK)` is a work array in the Fortran; the kernel forms
each `SIN1A(IK)` where the second loop uses it, because nothing reads `S` in between, so the additions come in the same order.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST4` and `FLX0` replaced by `ST2` and `FLX2` (`switches/switch_chaceon_st2`, built
by `just ref-build switch_chaceon_st2`). gdb records every call of `ww3_ts1` (`regtests/capture/sin2.py`; `NK = 36`, `NTH = 24`
except where noted) with the module values read at the first call, and `BETATB` as `INPTAB` left it. Outputs compared: `S`,
`D`, `FPI`.

| Capture | Variant | Calls | Host differ | SPIR-V differ |
|---|---|---|---|---|
| default, 2500 m | 12 m/s, `SIN2` defaults (`ZWND 10`, `SWELLF 0.1`), stride 5 | 676 | 0 | 0 |
| storm, 8 m | 30 m/s, stride 6 | 1441 | 0 | 0 |
| light, 20 m | 6 m/s, stride 6 | 381 | 0 | 0 |
| calm, 20 m | 0 m/s, stride 6; every `S` is zero, `D` is not | 361 | 0 | 0 |
| dir, 2500 m | wind from 45 degrees, stride 6 | 563 | 0 | 0 |
| swell, 2500 m | `SWELLF=1`, stride 6: the filter changes no `D` | 563 | 0 | 0 |
| noswell, 20 m | `SWELLF=0`, stride 6 | 597 | 0 | 0 |
| zwnd, 2500 m | `ZWND=20`, stride 6 | 589 | 0 | 0 |
| grid, 20 m | `NK=40`, `XFR=1.07`, stride 6 | 383 | 0 | 0 |
| grid2, 8 m | `XFR=1.05`, stride 6 | 430 | 0 | 0 |
| poke, 2500 m | entry pokes of `U`, `CD`, `Z0`, `A`, first 500 calls | 500 | 0 | 0 |

6484 calls, 22 568 744 values (`S`, `D`, `FPI`; call layout plus batch layout), 0 differ. `INPTAB`: `DSIGA`, `DDRAG` and the
16 842 values of `BETATB` equal, 16 840 nonzero, 8722 negative, 0 differ (`just inptab-cmp`; the table depends on no input,
so one capture is the whole evidence).

What the 11 captures reached (calls, of 6484; `sin2_cmp` prints it per capture):

| Branch | Calls |
|---|---|
| `\|COSFAC\| < 0.0087`, the floor (`:459`) | 2206: the spectral directions are multiples of 15 degrees, and the winds used (270 and 45 degrees) are 90 degrees from one of them |
| `IOMA` clamped to `-NRSIGA`, to `NRSIGA` (`:469`) | 136 and 136, pokes only |
| `ICL` clamped to 1, to `NRDRAG` (`:470`) | 1979 and 1542 |
| `OMA < 0` (the `MIN(0, INT(SIGN))` term, `:464`) | 6484 |
| `M1 < 1E-20` (`:503`) | 419: 361 calm, 49 pokes, 9 first calls of a run |
| `NKFILT` cut at `NK` (`:516`) | 407: calm and pokes |
| `NKFIL2` raised to 0 (`:518`) | 47, pokes only |
| `FPISTR = FPIMIN` (`:512`) | 452 |
| first filter loop, transition loop (`:520`, `:525`) | 6392 and 5985 |
| `FSWELL*D > D` (the filter acts) | 5829 |

```bash
just ref-build switch_chaceon_st2
just capture inptab st2sin2 2500                    # CAPTURE_SWITCH defaults to switch_chaceon_st2
just capture sin2 st2sin2 2500 5                    # every fifth call; also st2sin2storm 8, st2sin2light 20, st2sin2calm 20,
                                                    # st2sin2dir, st2sin2swell, st2sin2noswell 20, st2sin2zwnd, st2sin2grid 20,
                                                    # st2sin2grid2 8 (stride 6), and st2sin2poke 2500 1 (the first 500 calls)
just inptab-cmp /work/capture/inptab_st2sin2_2500m.bin
just sin2-cmp /work/capture/sin2_st2sin2_2500m.bin ...
just spirv && just spirv-replay sin2 st2sin2 2500
```

## Fault runs

Each fault replaces one expression of `sin2.rs` (or of `inptab.rs`), rebuilds the comparison and replays all 11 captures (the
table: the one `INPTAB` capture). Differing values are summed over the call and batch layouts. A run that ends in a panic is
an out-of-range index in the host build, caught by the capture that reaches it; nothing in a kernel checks bounds.

`W3SIN2`, 61 faults of `sin2.rs` and 3 of the comparison tool, 62 caught:

| Fault | Result |
|---|---|
| `U` one ULP up (`SIN2_FAULT=u`) | caught: 13 468 620 |
| `BETATB` one ULP up (`SIN2_FAULT=table`) | caught: 14 511 024 |
| `FSWELL` one ULP up (`SIN2_FAULT=fswell`) | caught: 4 114 706 |
| floor 0.0088 for 0.0087 | caught: 584 056 |
| the sign of `COSFAC` not restored after the floor, or `COSFAC` for `ABS(COSFAC)` in `LAMBDA` | caught: 10 802 224 and 11 454 710 |
| `MIN(0, INT(SIGN(-1.1,OMA)))` dropped, or `SIGN(0.9,OMA)` | caught: 10 041 778 each |
| `IOMA` clamp at 399 / only the upper / only the lower | caught: 80 436 / panic (pokes) / 58 674 |
| `ICL` clamp at 19 / lower 2 / upper missing | caught: 509 368 / 5 423 214 / panic (the default capture reaches it) |
| `RD1` from the clamped `IOMA`, `RD2` from the clamped `ICL` | caught: 82 978 and 1 532 774 (pokes) |
| `BETATB` index `ICL+1` for `ICL`, `IOMA+1` for `IOMA`; the weights; the four-term sum regrouped | caught: 20 156 142, 20 147 254, 20 200 196, 20 195 638, 4 137 190 |
| `D` without `SIG2`, `OMA` without `/SIG2`, `CLAM` without the square | caught: 20 265 124, 20 238 222, 20 218 344 |
| `FACLN2` from `ZWIND`, `FACLN1` without `LOG` | caught: 20 362 670 and 20 347 618 |
| `MAX(0.,S)` dropped, `SIG**3` as `SIG**2`, `SIN1A/SIG` as `SIN1A`, `FTE` and `FTTR` swapped, `/DDEN(NK)` dropped | caught: 2 162 862, 1 633 958, 3 255 300, 1 483 026, 1 384 928 |
| `M1` threshold 1E-3 for 1E-20 | caught: 6 529 190 (the calm and poke captures) |
| `FPI = XFR*SIG(NK-1)` | caught: 2190 (calm and pokes) |
| `FPISTR` without `FPIMIN` | caught: 766 802 |
| `FP1STR` constants 0.93, 3.7E-4, 6.4E-10; `FPISTR**3` as a square | caught: 1 328 382, 1 283 446, 921 304, 1 251 784 |
| `PEAKFC` 0.81, `TRANSF` 0.76, `TRANSF*FP1` as `FP1` | caught: 1 334 800, 1 287 656, 1 265 258 |
| `GRAV` as 9.81 in `FPISTR`, `USTAR = U*CD` | caught: 1 284 358 and 3 787 120 |
| `FACTI1` and `FACTI2` swapped | caught: 944 290 (the grid captures separate them) |
| `NKFILT` without `MIN(NK,..)`, `NKFIL2` without `MAX(0,..)`, without `MIN(NK,..)` | caught: panics, pokes only (`NKFILT` past `NK`, `NKFIL2` negative) |
| first filter loop one bin short / one bin long | caught: 19 840 / panic after differences (poke) |
| transition loop from `NKFIL2` / to `NKFILT-1` | caught: panic after differences (poke) / 369 852 |
| `TRANS` offset 0.74, `*3.9`, weights swapped, `FSWELL` missing from the transition | caught: 1 286 662, 1 322 644, 1 231 020, 1 231 014 |
| `FSWELL*D` replaced by `D`, `MAX` as `MIN` in the filter, `S` not refreshed | caught: 5 494 772, 6 509 652, 2 023 726 |
| `COS` and `SIN` of `UDIR` swapped, `ECOS` and `ESIN` swapped, `TPI` as 6.2832 | caught: 18 742 866, 18 742 866, 16 298 734 |
| `fmaxnm` as a plain `x > y` compare (a NaN `S` kept) | caught: 44 436 (the NaN poke of `A(1)`) |
| `TRANS` as `/(1.-TRANSF)` for `*4.` | **no effect, by design**: `x/0.25` and `x*4` are the same `f32`; the kernel follows the GIMPLE |
| `fmaxnm`'s `+0` against `-0` tie returning `-0` | **no effect, and provably so**: a tie of opposite zeros needs `D = 0` with `FSWELL*D` of the other sign (`FSWELL >= 0`, so never) or `S = -0` in `MAX(0.,S)`, whose sum starts at `+0` and stays `+0` |

`INPTAB`, 22 edits of `inptab.rs` and 4 of the comparison tool, 21 caught:

| Fault | Result |
|---|---|
| `A0+A4+A5` as `(A5+A4)+A0`, `A3`'s numerator as `A0-(A2+A1)` | caught: 44 and 81 values, up to 3 ULP |
| first branch `OMA <= -1` (a node sits at exactly -1) | caught: 6 values |
| `OM1/2` as `OM1*0.51`, `OM2` plus 1E-3 | caught: 3 and 4 values |
| `W3BETA*1.0001E-4`, `A10` sign, `A9` 0.36, `OM1` 1.0751 | caught: 16 840, 8118, 8118, 803 values |
| `A3*(OMA*(..))` regrouped, `A6 = A0 - A0*A3`, `(OMA-1)` not squared, `A7`'s divisor +1E-6, `-(A1*OMA**2-A2)` | caught: 23, 64, 7833, 267, 8190 values |
| `DRAG` and `SIGA` as `IDRAG*DRAGMX/NRDRAG`, `ISIGA*SIGAMX/NRSIGA`; the loops one node short | caught: 4679, 3223, 21 values |
| `INPTAB_FAULT=bump`, `bump_hi`, `dsiga`, `scale` | caught: 1 value, 1 value, `DSIGA` differs, 16 840 values |
| `A0+A4+A5` as `(A0+A4)+A5` | **undetected**: the same table. The grouping in the kernel is the GIMPLE's `A5+(A4+A0)`, and this fault shows the data does not separate it from `(A0+A4)+A5` ⚠ |
| `A0` as `(0.25*A5)*A5/A4` or `(A5*A5/A4)*0.25` | **no effect**: scaling by 0.25 is exact |
| third branch `OMA <= OM1` | **undetected**: needs `OMA = OM1` exactly, which no node has |
| `-(A1*OMA**2)` as `(-A1)*OMA**2` | **no effect**: negation is exact |

## GPU code

- SPIR-V: entry point `sin2` in `rust/gpu-spirv/shader`, 14 storage buffers, passes `spirv-val` (`just spirv`).
  `just spirv-replay sin2` is 0 differ on all 11 captures (lavapipe).
- PTX: kernel `sin2` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`). Its body
  (2555 lines of PTX) has 17 `div.rn.f32` and no float `fma`; the only `mad` is `mad.lo.s32`, an address. Not run ⚠.
- `INPTAB` stays on the host: the table is built once and uploaded, as `INSIN3`'s are.

## Translation notes

- The order of operations is read from the optimised GIMPLE of `w3sin2` and `inptab` (`gfortran 12.2 -O3 -ffp-contract=off`,
  `-fdump-tree-optimized`, the module built alone with `switch_chaceon_st2`'s `-DW3_*` flags): no `**` is a call, `COS` and `SIN`
  of `UDIR` are one `sincosf`, `LOG` is `logf`, `SQRT` is `sqrtf`, and nothing fuses.
- `TPI/(K*ABS(COSFAC))`: the product is protected by parentheses, so `K*|COSFAC|` rounds first. `ULAM` is
  `FACLN1*(LOG(LAMBDA)-FACLN2)`, `CLAM` is `CD*(U/ULAM)**2` with the square a product, and `OMA` is `((K*ULAM)*COSFAC)/SIG2`.
- `DSIGA` and `DDRAG` are loaded at run time in `W3SIN2`, not folded: the kernel divides by the module values it is given.
  `INPTAB` folds them (`DSIGA = 0.1`, `DDRAG = 4.99999966E-4` as `f32`).
- `IOMA = INT(OMA/DSIGA) + MIN(0, INT(SIGN(-1.1,OMA)))` is `copysignf(1.1, OMA)` converted, so `-0.` takes the negative side.
  `RD1` and `RD2` use the unclamped `IOMA` and `ICL`; the quotients `OMA/DSIGA` and `CLAM/DDRAG` are computed once.
- `MAX` compiles to `fmaxnm` (`MAX_EXPR`). The kernel writes it out (`fmaxnm` in `sin2.rs`) so that SPIR-V's `FMax` cannot
  differ on a NaN or on `+0` against `-0`; for the finite inputs that reach the sites it equals `f32::max`.
- `SIN1A(IK) = SIN1A(IK)*DDEN(IK)/(CG(IK)*SIG(IK)**3)` is `((SIN1A*DDEN)/(CG*(SIG*SIG*SIG)))`; gfortran vectorises the loop
  four wide and adds the lanes in order, so the sums keep the Fortran order.
- `TRANS = (SIG(IK)/FP1 - TRANSF)/(1.-TRANSF)` is `*4.` in the compiled code, and `D = (1-TRANS)*MAX(..) + TRANS*D`.
- `W3BETA` is inlined in `INPTAB` and the coefficients each branch does not use are computed only in that branch, which changes no
  bit. `A0+A4+A5` is `A5+(A4+A0)` there, and the result is multiplied by `1.E-4` (`f32`, 9.99999975E-5).
- `INT` is `fcvtzs`; Rust's `as i32` saturates and gives 0 for NaN, as it does. `IOMA` is added with wrapping arithmetic.

## Not translated, not reached

- `W3_S`, `W3_T`, `W3_T0`, `W3_T1` and `W3_T2` test output: not in the build.
- The call from `W3SRCE`: `U10ABS`, `U10DIR`, `CD` and `Z0` come from `W3FLX2` (another port); `srce.rs` is not changed.
- `W3SPR2`, `W3SDS2`, `W3FLX2`: other ports.
- No capture has `OMA/DSIGA` or `CLAM/DDRAG` outside the range of `i32`, where `INT` is undefined in Fortran and `OpConvertFToS`
  in SPIR-V is too ⚠. The poke's `U = 300` reaches the `IOMA` clamps, not the saturation.
- No capture has `FSWELL > 1` (`W3GRID` resets it to 1) or `FPIMIN` other than 0.009 ⚠.
- A grid with `NTH` other than 24, or a `ZWND` other than 10 and 20, was not captured ⚠. `NK` 36 and 40 and `XFR` 1.05, 1.07
  and 1.10 were, which separates `FACTI1` and `FACTI2` from each other.
- `GIMPLE` was read from gfortran 12.2 in `gcc:12-bookworm`; the reference's own compiler is the same series, and the comparison
  with the running reference is what confirms it.

## Timing

None. There is no timing without a passed comparison on a device, and this ran on none.
