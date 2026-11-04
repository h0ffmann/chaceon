# W3SDS2

ST2's whitecapping dissipation (Tolman and Chalikov 1996). `D` is one factor per band: a turbulent term `(1-FILT) * CDIST * K**2`
that scales with the wave height `HW` of the spectrum above `XFH*FPI`, and, where the filter `FILT` is above zero, a
Phillips-constant term `FILT * CDSA0 * CDISH * SIG**3 * (ALFA/SDSALN)**POW`; it is repeated over the directions, and
`S = D * A`. Translated from `model/src/w3src2md.F90:582-825` at metocean/WW3 `92e2cf66`. Kernel:
`rust/crates/ww3-kernels/src/sds2.rs`. `ww3-libm` supplies `logf`, `powf` and `sqrtf`; no new function.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts and
in compiled SPIR-V on lavapipe (ADR-0007). It is not called from `W3SRCE` yet, and it has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NTH,NK)` | `REAL`, in | `a` at offset `o` |
| `CG(NK)`, `K(NK)`, `ALFA(NK)` | `REAL`, in | `cg`, `k`, `alfa` at offset `v` |
| `FPI`, `USTAR` | `REAL`, in | `Sds2Point` |
| `S(NTH,NK)`, `D(NTH,NK)` | `REAL`, out | `s`, `d` at `o` |
| `SIG(0:NK+1)`, `DDEN(NK)` | module `REAL` arrays | `sig`, `dden` (shared by all points) |
| `NK`, `NTH`, `FTE`, `DTH`, `FPIMIN`, `FACTI1`, `FACTI2`, `XFH`, `XF1`, `XF2`, `SDSALN`, `CDSA0`-`CDSA2`, `CDSB0`-`CDSB3` | module scalars | `Sds2Params` |

No `f64` anywhere (v, `w3src2md.F90:582-825`). The `W3_S`, `W3_T`, `W3_T0`, `W3_T1` and `W3_T2` blocks are output only and are
left out. `EBD(NK)` is a work array in the Fortran; a kernel may not allocate, and each `EBD(IK)` is an independent sum over
the directions of one band, so `ebd()` recomputes it where the source reads it (`EBD(NK)` once, `EBD(IKHW)` and the tail
sum once per band): the same additions in the same order.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST2` and `FLX2` (`switches/switch_chaceon_st2`; image
`chaceon-ref-capture:switch_chaceon_st2`). gdb records every fourth call of `ww3_ts1` (`regtests/capture/sds2.py`, stride
4): the scalars, `SIG` and `DDEN` once, then `FPI`, `USTAR`, `A`, `CG`, `K`, `ALFA` and the `S` and `D` the call returned.

| Capture (variant `st2sds2...`) | Calls | Host differ | SPIR-V differ |
|---|---|---|---|
| default, 12 m/s | 845 | 0 | 0 |
| `storm` (30 m/s) | 1293 | 0 | 0 |
| `light` (6 m/s) | 571 | 0 | 0 |
| `calm` (0 m/s) | 541 | 0 | 0 |
| `phi0` (`&SDS2 PHIMIN=0.`: `SDSB2 = SDSB3 = 0`) | 886 | 0 | 0 |
| `phihi` (`PHIMIN=0.01`: `FPIA > FPIMIN`, `FPIMIN = 0.0181`, `SDSB3 = 7.26`) | 1043 | 0 | 0 |
| `tune` (`SDSA0`, `SDSA1`, `SDSA2`, `SDSB0`, `SDSB1` changed) | 1677 | 0 | 0 |
| `nk` (NK 16: `SIG(NK+1)` low, so the high-frequency `HW`) | 572 | 0 | 0 |
| `grid` (NK 40, XFR 1.07) | 574 | 0 | 0 |
| `poke` (500 calls, stride 1) | 500 | 0 | 0 |

8502 calls, 7 125 272 values of `S` and as many of `D`; with both layouts 28 501 088 values compared, 0 differ on the host;
0 differ on SPIR-V on each capture (max 0 ULP).

Branches, counted by `sds2_cmp` from the same formulas (calls or bands, summed over the captures it prints):

| Branch | Reached by |
|---|---|
| low-frequency `HW` (`FHW < SIG(NK+1)`, `:719`) | every capture except `calm` and `nk` |
| high-frequency `HW` (`:732-735`) | default (18 calls), `light` (49), `calm` (541), `nk` (572), `grid` (61), `poke` (88) |
| `IKHW = NK` and the blend `XHW = MAX(0., XHW-0.5)` clamping to 0 | default, `light`, `grid`, `poke` |
| `FILT` = 0, in (0,1) and 1 | all three in default, `light`, `phi0`, ...; `calm` and `nk` have `FILT = 0` only |
| `FPIT = FPIMIN` floor | `light`, `calm`, `nk`, `phihi`, `poke` |
| `POW` at its cap of 25 | **`poke` only** (903 bands): `USTAR` of 1e-4 and 0 |
| `PHIMIN <= 0`, `FPIA < FPIMIN`, `FPIA >= FPIMIN` of `w3gridmd.F90:2143-2160` | `phi0`, default, `phihi` |

The `calm` run has zero `S` and `D` everywhere (the calm spectrum is empty here), so it proves nothing about the
arithmetic by itself; it still reaches the high-frequency branch with `HW = 0` ⚠. `poke` writes `FPI` and `USTAR` (and zeroes
`A`) at the routine's entry, cycling 12 cases: `FPI` just below, exactly at and above `SIG(NK+1)/2`; `XHW+0.5` just over
and under `NK`; `IKHW` of 1 and 2; the middle of the table; `USTAR` of 0, 1.8 and 1e-4; an empty spectrum. It restores them
and zeroes `S` and `D` at exit. Stride 4 keeps the files small (89 MB for ten) ⚠: it samples one call in four of each run.

```bash
just capture sds2 st2sds2 2500 4              # CAPTURE_SWITCH defaults to switch_chaceon_st2 for sds2 and st2*
just capture sds2 st2sds2poke 2500 4          # variants in scripts/ts1_variants.sh; the poke has a limit of 500 calls
just sds2-cmp /work/capture/sds2_st2sds2_2500m.bin
just spirv && just spirv-replay sds2 st2sds2 2500
```

## Fault runs

`sds2_cmp` on all ten captures. `SDS2_FAULT` moves one input one ULP up; the second table edits the kernel and restores it.

| Fault | Result |
|---|---|
| `FTE`, `DTH`, `FPIMIN`, `FACTI1`, `FACTI2`, `XFH`, `XF1`, `XF2`, `SDSALN`, `CDSA0`-`CDSA2`, `CDSB0`-`CDSB3` | all 16 caught (43 628 to 5.7 million values) |
| `FPI`, `USTAR`, `A(1)`, `CG(NK)`, `K(1)`, `SIG(NK)`, `SIG(NK+1)`, `DDEN(NK)`, `ALFA(NK)` | all caught (`A(1)` only 1671 values: the first band is mostly empty) |
| `ALFA(1)` (`alfa1`) | **not caught** on any capture. In the model runs band 1 lies below `XF1*FPI`, so `FILT = 0` and `ALFA(1)` is not read; whether the poke's `IKHW = 1` cases read it, with the one-ULP change absorbed by the `powf` and the product, was not checked ⚠. The same fault on `ALFA(NK)` is caught (498 `S` values on default, 65 on storm) |

| Kernel edit | Result |
|---|---|
| no `MAX(0., XHW-0.5)` at `IKHW = NK` | caught (default, `light`, `grid`, `poke`) |
| `POW` cap 26 for 25 | caught by `poke` only (10 176 `S`) |
| no `FPIT` floor | caught on `light`, `calm`, `phihi`, `nk`, `poke` |
| `EBD` summed in reverse | caught everywhere |
| `(SIG(NK)/FHW)**5` as `(x2*x2)*x` | caught: 1103 `S` on default |
| `S = D*A + 0.` | caught (negative zero) |
| `HW` low branch: `e*(xhw*dden)` for `(e*xhw)*dden` | caught |
| `SIG(NK+1) > FHW` as `>=` | caught by `poke` only (the tie case, 29 847 `S`) |
| `-(ustar*2.)*(hw*phi)` regrouped | caught |
| `INT(XHW+0.5)` as `INT(XHW+0.5+0.5)` | caught |
| `(1-FILT)*CDIST*K**2` and the `D` second term regrouped | caught |
| no `MAX(0., .)` or no `MIN(1., .)` in `FILT` | caught |
| `IKHW == NK` as `IKHW+1 >= NK` | caught |
| `MOD(.,1.)` replaced | caught |
| `IF (FILT .GE. 0.)` for `.GT.` | **no effect**: where `FILT = 0` the second term is `0 * finite = 0`, and `a - 0 = a`. Equivalent for finite values, so a weak fault, not a blind spot |
| `EBD(NK)/CG(NK) + 0.` | **no effect, by construction**: the sum starts at `+0.` and is never `-0` |

Not reached ⚠: the NaN sign of `CDIST` (a negated product; the kernel keeps `-(((USTAR*2.)*HW)*PHI)` as the GIMPLE has
it, but no capture reaches a NaN input), `FPI` or `USTAR` NaN or infinite, `FPI` so small that `IKHW < 1` (the Fortran
indexes `EBD` out of bounds there; the kernel holds the band at 1), `FHW` so large that `fmodf` sees a huge argument.

## GPU code

- SPIR-V: entry point `sds2` in `rust/gpu-spirv/shader`, passes `spirv-val`; `just spirv` decorated 58 550 float
  operations in the module. Compiled SPIR-V replays every capture at 0 ULP on lavapipe.
- PTX: kernel `sds2` in `rust/gpu-cuda/kernels`, assembled with `ptxas --fmad=false` (`just ptx`):
  the kernel body is 2820 lines of PTX with 9 `div.rn.f32` and no float `fma` or `mad.f32` (v, grep of `rust/gpu-cuda/out/chaceon.ptx`). Not run ⚠.

## Translation notes

Read from the optimised GIMPLE of the routine compiled alone (`gfortran 12.2 -O3 -ffp-contract=off -fdump-tree-optimized`,
`-DW3_ST2`, `SIG` and the other module data stubbed as variables) and from `objdump` of the object:

- `XHW = FACTI2 + FACTI1*LOG(FHW)` calls `logf`; `INT` is `fcvtzs`. `MOD(XHW+0.5, 1.)` calls `fmodf`, which is exact:
  `mod1()` is `x - trunc(x)` below 2^23.
- `MAX` and `MIN` are `fmaxnm` and `fminnm` at all six sites (`objdump`): `MAX(0.,XHW-0.5)`, `MAX(FPIMIN,.)`,
  `MAX(0.,.)` and `MIN(1.,.)` in `FILT`, `MIN(25.,.)`. NaN gives the other operand, and `-0` against `+0` gives `+0`; the
  kernel writes `if v > 0 { v } else { 0 }` and the like, which SPIR-V `FMax` does not do on signed zero.
- `(SIG(NK)/FHW)**5` is `p2 = x*x; p3 = x*p2; p5 = p3*p2`. `SIG**3` is `x*(x*x)`. `K**2`, `USTAR**2` are products. The
  runtime real exponents `CDSB3`, `CDSA2` and `POW` call `powf`, also when `CDSB3` is the integer 4.
- `AFILT + BFILT*SIG(IK)` is `SIG*BFILT - AF1*BFILT` in the GIMPLE (the negation of `AFILT` moved into the subtraction):
  the same number, and the same NaN sign. `G2PI3I`, `G1PI1I` and `TPIINV` are the constants of `constants.rs`; their
  bits equal the GIMPLE's literals only by the parity result ⚠.
- `HW` low branch: `((XHW*EBD)*DDEN)/CG` for the first band, then `+ (EBD*DDEN)/CG` in band order (gfortran vectorises the
  tail four wide but adds the lanes in order, so the sum keeps the Fortran order). `HW = 4*SQRT(HW + FTE*(EBD(NK)/CG(NK)))`.
- `D(1,IK)` is written for every band and copied over the directions; `S = D*A` keeps the sign of zero.

## Not translated, not reached

- The call in `W3SRCE` (needs `W3SPR2` for `FPI`, `W3SIN2` and `W3FLX2`, ported by siblings).
- Other `NTH`: only 24 was captured ⚠. `FTE` and the table follow the grid.
- Long runs: `ww3_ts1` is 2 hours; later stages of a spectrum are not sampled.

## Timing

None. No timing without a device run (`AGENTS.md`).
