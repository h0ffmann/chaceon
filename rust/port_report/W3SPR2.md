# W3SPR2

The mean parameters of Tolman and Chalikov (`ST2`): mean energy `EMEAN`, mean frequency `FMEAN`, mean wavenumber `WNMEAN`,
the maximum action density `AMAX`, Phillips' constant per band `ALFA(NK)` and the peak frequency `FP` estimated from the
input peak frequency `FPI` and the friction velocity. `FPI` is what `W3SIN2` returns and `W3FLX2` and `W3FLX3` read, so
`FP` ties the three together. `ST2` is not in the port's target `switch_chaceon_shrd`; the reference for it is
`switches/switch_chaceon_st2` (`ST2`, `FLX2`, `STAB0` in place of `ST4`, `FLX0`). Translated from
`model/src/w3src2md.F90:125-288` at metocean/WW3 `92e2cf66`. Kernel: `rust/crates/ww3-kernels/src/spr2.rs`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts and in
compiled SPIR-V on lavapipe (ADR-0007). `W3SRCE` does not call it yet (`W3SRCE` is wired for `ST4` only) ⚠. It has run on
no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NTH,NK)` | `REAL`, in | `a` at `spec_off`, `A(ITH,IK)` is `a[spec_off + ITH-1 + (IK-1)*NTH]` |
| `CG(NK)`, `WN(NK)` | `REAL`, in | `cg`, `wn` at `cg_off` |
| `DEPTH`, `U` | `REAL`, in, never read (`:204-217`, `:240-282`) (v) | `Spr2Point`, kept so a capture says which winds and depths it covered |
| `FPI`, `USTAR` | `REAL`, in | `Spr2Point` |
| `EMEAN`, `FMEAN`, `WNMEAN`, `AMAX`, `FP` | `REAL`, out | `Spr2Out` |
| `ALFA(NK)` | `REAL`, out | `alfa` at `w_off` |
| `EB(NK)` | automatic `REAL` (a `malloc` per call in the reference) | `eb` at `w_off` |
| `NK`, `NTH`, `DTH`, `FTE`, `FTF`, `FTWN` | module | `Spr2Params` |
| `SIG(0:NK+1)`, `DDEN(NK)` | module `REAL` | `sig`, `dden` |

No `f64` anywhere (v, `w3src2md.F90:125-288`). `GRAV` is 9.806 and `TPIINV` is `CONSTANTS`' `REAL` 1/(2π). `NITTIN`, `ZWIND`,
`CINXSI` and the `W3DISPMD` names in the `USE` lines are not read.

## Comparison with the reference

Reference: `chaceon-ref-capture:switch_chaceon_st2`. gdb records the arguments and the results of `W3SPR2` on `ww3_ts1`
(`regtests/capture/spr2.py`). `ww3_ts1` has a fixed wind, and `W3SRCE` calls the routine twice per step. The depth and wind
change the case's namelists only (`scripts/ts1_variants.sh`, variants `st2spr2*`). `poke` rewrites `USTAR`, `FPI` or `A` at
the entry of the real routine and restores them at the exit; the outputs of a poked call reach the next steps, so the
capture stops after 500 calls, and every recorded call is a real evaluation of the Fortran.

| Capture | Wind (m/s) | Depth (m) | Calls | `USTAR` < 1E-4 | `FPISTR` < 0.008 | `FMEAN`, `WNMEAN` floor | Host differ | SPIR-V differ |
|---|---|---|---|---|---|---|---|---|
| st2spr2 | 12 | 2500 | 1847 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2 | 12 | 100 | 1847 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2 | 12 | 20 | 1913 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2light | 6 | 2500 | 1482 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2light | 6 | 10 | 1482 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2gale | 20 | 50 | 2283 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2storm | 30 | 2500 | 2444 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2storm | 30 | 20 | 3090 | 1 | 1 | 0, 0 | 0 | 0 |
| st2spr2calm | 0 (U = 0.001) | 2500 | 1441 | 1 | 1441 | 720, 720 | 0 | 0 |
| st2spr2poke | 12 | 2500 | 500 | 167 | 250 | 166, 166 | 0 | 0 |
| st2spr2poke | 12 | 20 | 500 | 167 | 250 | 166, 166 | 0 | 0 |

18 829 calls, 771 989 results (41 per call), 0 differ, in call and batch layouts and in SPIR-V. The ranges of `FPI` in
the runs are 0.52 (30 m/s) to 9.42 (the start value) and `FP` is nonzero in all 18 829 calls. "Floors" are counted from
the inputs and from the reference's own outputs (`FMEAN` bit-equal to `TPIINV*EMEAN/1.E-7` with `EMEAN` nonzero). The
calm case (wind 0.001 m/s) has `EMEAN` = 0 on 721 calls and reaches both `1.E-7` floors on 720, and `FPISTR` is under its floor on
all 1441. The other captures reach no floor except at the first call of the run (`USTAR` = 1E-5, the
`W3DIMW` start value), which is why the poke exists.

```bash
just capture spr2 st2spr2 2500 3        # switch_chaceon_st2 is picked for spr2 and every st2* variant
just capture spr2 st2spr2storm 20 3
CAPTURE_LIMIT=500 just capture spr2 st2spr2poke 2500 1
just spr2-cmp /work/capture/spr2_st2spr2_2500m.bin /work/capture/spr2_st2spr2poke_2500m.bin
just spirv && just spirv-replay spr2 st2spr2 2500
```

## Fault runs

Input faults move one input up by one ULP (`SPR2_FAULT`), kernel faults edit `spr2.rs`; both over all 11 captures. Counts
are results that differ.

| Fault | Result |
|---|---|
| every nonzero `A` (`a`) | caught: 13 080 `EMEAN`, 17 766 `AMAX`, 11 101 `FMEAN`, 11 195 `WNMEAN`, 377 449 `ALFA` |
| every `CG` (`cg`) | caught: 13 720 `EMEAN`, 12 096 `FMEAN`, 11 433 `WNMEAN` |
| every `WN` (`wn`) | caught: 6830 `WNMEAN`, 552 738 `ALFA` |
| `SIG` (`sig`) | caught: 12 021 `FMEAN`, 422 205 `ALFA` |
| `DDEN` (`dden`) | caught: 13 256 `EMEAN`, 11 894 `FMEAN`, 11 322 `WNMEAN` |
| `DTH` (`dth`) | caught: 499 223 `ALFA` |
| `FTE`, `FTF`, `FTWN` | caught: 930 `EMEAN` (and 165 `FMEAN`, 920 `WNMEAN`); 55 `FMEAN`; 46 `WNMEAN` |
| `FPI` (`fpi`), `USTAR` (`ustar`) | caught: 11 452 and 9078 `FP` |
| `UST` floor 1.E-3 for 1.E-4 | caught: 1783 `FP` |
| `FPISTR` floor 0.009 for 0.008 | caught: 1949 `FP` |
| `FMEAN` floor 1.E-6, `WNMEAN` floor 1.E-6 | caught: 1056 `FMEAN`; 1052 `WNMEAN` |
| `ALFA` from the scaled `EB`; `2.*DTH*SIG*(EB*WN3)`; no `2.` | caught: 558 886; 194 652; 558 749 `ALFA` |
| `EB*(DDEN/CG)` for `EB*DDEN/CG` | caught: 4175 `EMEAN`, 5855 `FMEAN`, 5982 `WNMEAN` |
| `E*(1/SIG)`; `E*(1/SQRT(WN))`; `TPIINV*(EMEAN/MAX)` | caught: 2406 `FMEAN`; 3113 `WNMEAN`; 6529 `FMEAN` |
| no square in `WNMEAN` | caught: 17 930 `WNMEAN` |
| tail from `EB(NK-1)` | caught: 17 930 each of `EMEAN`, `FMEAN`, `WNMEAN` |
| `FTE` term dropped; `FTF` term dropped; `FTWN` term dropped | caught: 17 930 each of `EMEAN`, `FMEAN`, `WNMEAN`; 16 878 `FMEAN`; 16 878 `WNMEAN` |
| `ITH` summed in reverse | caught: 7912 `EMEAN`, 9417 `FMEAN`, 9278 `WNMEAN`, 321 478 `ALFA` |
| `AMAX` starting at `A(1,1)`; `<=` for `<` | caught by the poke captures (negated spectrum): 166 `AMAX` each |
| `FP`: `*(GRAV/UST)`; term order; no `TPIINV`; `USTAR` for `UST`; `FPI*(UST/GRAV)` | caught: 5744; 3955; 18 829; 343; 4173 `FP` |
| `FP`: `0.92`, `3.6E-4`, `6.3E-10` changed | caught: 18 829; 18 829; 13 648 `FP` |
| `ALFA` with `WN*(WN*WN)` | not caught: an equivalent edit, a product is commutative (v) |
| `FP` with `FPISTR*(FPISTR*FPISTR)` | not caught: equivalent, commutative (v) |
| `FP` with `6.3E-10/F2/F` for `6.3E-10/F**3` | not caught in 18 829 calls ⚠: two divisions against one differ in the term's last bit, and that moves the sum only when the term is close to a rounding boundary of the sum; none of the 18 829 calls was. A weak fault, not a blind spot, but the order `6.3E-10/((F*F)*F)` rests on the bits matching, not on a read of the GIMPLE |

Input faults: 11 of 11 caught. Kernel faults: 30 run, 27 caught, 3 not caught and explained above. Every guard (`UST`,
`FPISTR` and both `1.E-7` floors) is reached by at least 166 calls of the poke captures and by the calm capture, and a
fault to each is caught.

## GPU code

- SPIR-V: entry point `spr2` in `rust/gpu-spirv/shader`, 10 storage buffers. 11 captures, 0 differ.
- PTX: kernel `spr2` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false`. The entry (606 lines of PTX) has 9 `div.rn.f32` and no float `fma` or `mad` (v, `grep` on `out/chaceon.ptx`). Not run ⚠.

## Translation notes

- The reference's `__w3src2md_MOD_w3spr2` was read in the image with `gdb` `disassemble` (361 lines, aarch64): no `fmadd`,
  `fmsub`, `fmla` or other fused instruction (v), 5 `fmaxnm` (the five `MAX` sites: `AMAX`, `UST`, two `1.E-7`, `FPISTR`),
  and the only calls are `malloc` and `memset` for `EB(NK)`. The optimised GIMPLE was not read ⚠, because no bit differed.
- `WN(IK)**3` is `(WN*WN)*WN`, `FPISTR**3` is `(F*F)*F` and `(...)**2` is `q*q` (v, bits match on all calls and the faults
  that reorder them are caught except the commutative ones).
- `ALFA(IK)` uses `EB(IK)` before it is scaled by `DDEN/CG` (`:260`, `:261`); the fault that uses the scaled value is caught.
- `MAX(1.E-7, ...)`, `MAX(0.0001, USTAR)` and `MAX(0.008, ...)` are `f32::max`, which is `fmaxnm` and agrees with SPIR-V's
  `FMax` because the constant is the other operand and the result is never zero. `MAX(AMAX, A(ITH,IK))` is
  `if amax < av { amax = av }`, as in `W3SPR1`: `fmaxnm` keeps +0 against a -0 bin, which SPIR-V's `FMax` does not. The `<=` fault
  shows the poke captures see it.
- `SQRT` is `sqrtf` (correctly rounded).

## Not translated, not reached

- `W3EXPO` and `GXEXPO`, the point post-processors that also call the routine, and `W3OUTP`.
- A NaN or infinite spectrum, `USTAR` or `FPI`, and a negative `FPI`: no capture reaches them ⚠.
- `NK` other than 36 and `NTH` other than 24 ⚠: every capture has one grid; the loops are written in `NK` and `NTH`.
- `DEPTH` and `U` are not read, so no depth or wind enters the result except through `A`, `CG`, `WN`, `USTAR` and `FPI`.
- The `W3_T` and `W3_S` test output and tracing, which the switch does not enable.

## Timing

None. No timing without a device run (`AGENTS.md`).
