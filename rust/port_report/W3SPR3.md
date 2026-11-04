# W3SPR3

ST3's mean wave parameters (WAM cycle 4): the energy `EMEAN`, the mean frequencies `FMEAN` (weight
`SIG**(2*WWNMEANPTAIL)`, for the tail) and `FMEANS` (weight `SIG**(2*WWNMEANP)`, for the dissipation), the mean
wavenumber `WNMEAN`, the largest action density `AMAX`, a wind-sea mean frequency `FMEANWS` from the components flagged in
`LLWS`, and then the drag coefficient `CD`, `USTAR`, `Z0` and `CHARN` through `CALC_USTAR`. Translated from
`model/src/w3src3md.F90` at metocean/WW3 `92e2cf66`: `W3SPR3` (`:134-347`) and `CALC_USTAR` of the same module
(`:1135-1227`), which is not the ST4 routine of the same name in `sin4.rs`: ST3's has no Newton branch, no
`CAPCHNK` and no `ZZ0MAX`. Kernel: `rust/crates/ww3-kernels/src/spr3.rs`. Reference: `switches/switch_chaceon_st3`
(`ST3`, `STAB0`, `FLX0`, `BT1`, `DB1`; otherwise `switch_chaceon_shrd`).

**Status: `routine`** for both rows of `rust/ports.tsv`: every captured call bit-identical, on the host in call and
batch layouts and in compiled SPIR-V on lavapipe (ADR-0007). `W3SRCE` does not call it yet (`srce.rs` is unchanged),
and no model run uses it. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NTH,NK)`, `CG(NK)`, `WN(NK)` | `REAL`, in | `a`, `cg`, `wn` at offsets `spec_off`, `cg_off` |
| `U`, `UDIR`, `TAUWX`, `TAUWY` | `REAL`, in | `Spr3Point` |
| `USTAR`, `USDIR` | `REAL`, in and out; both are overwritten (below) | `Spr3Point::ustar`, `usdir` are unused; `Spr3Out` |
| `LLWS(NSPEC)` | `LOGICAL`, in | `u32` |
| `EMEAN`, `FMEAN`, `FMEANS`, `WNMEAN`, `AMAX`, `CD`, `Z0`, `CHARN`, `FMEANWS` | `REAL`, out | `Spr3Out` |
| `EB(NK)`, `EB2(NK)` | automatic `REAL` | `w` at `w_off`, `2*NK` per point |
| `SIG(0:NK+1)`, `DDEN(NK)`, `DTH`, `FTE`, `SSTXFTF`, `SSTXFTFTAIL`, `SSTXFTWN`, `WWNMEANP`, `WWNMEANPTAIL`, `ZZWND`, `AALPHA` | module `REAL` | `Spr3Params` and tables |
| `TAUT(0:200,0:200)`, `DELTAUW`, `DELU` | `w3src3md` module `REAL`, built by `TABU_STRESS` | taken from the capture, as the ST4 captures did |

No `f64` anywhere (v, `w3src3md.F90:134-347`, `:1135-1227`).

## What is not translated, and why it cannot be seen

- `ALFA(IK) = 2*DTH*SIG*EB*WN**3` (`:273`) is stored and never read. The object computes it into a `malloc`ed block (v,
  `gdb` disassembly of `__w3src3md_MOD_w3spr3` in the reference's `ww3_shel`: three `malloc`, a product chain with `WN`
  cubed, no later load) and the Release flags carry no `-ffpe-trap`, so only an overflow trap could show it ⚠.
- The first `UNZ` (`:246`), `Z0 = 0.` (`:324`) and the floor `USTAR = MAX(0.0001, USTAR)` (`:247`) are overwritten:
  `CALC_USTAR` assigns `USTAR` and `Z0` on every path, and `UNZ` is assigned again at `:326`. So `USTAR` and `USDIR`
  on entry do not matter, and a floor like MetOcean's ST4 one never acts here.

## Comparison with the reference

gdb records every Nth call of `ww3_shel` on the `ww3_ts1` case (`regtests/capture/spr3.py`). Outputs compared: the
eleven values of `Spr3Out`. `just spr3-cmp` replays each call in the call layout and again in the one-buffer layout
of a device; `just spirv-replay spr3` runs the compiled SPIR-V on lavapipe.

| Capture (`just capture spr3 VARIANT DEPTH STRIDE`) | Calls | What it reaches |
|---|---|---|
| default, 2500 m, stride 2 | 3355 | 12 m/s; 2 empty spectra (`FMEAN`, `FMEANS`, `FMEANWS` tail branch) |
| default, 8 m, stride 4 | 1677 | 272 calls without wind-sea components (`FMEANWS` tail branch) |
| light (6 m/s), 20 m, stride 4 | 1652 | 272 without wind-sea components |
| calm (0 m/s, `U = 0.001`), 8 m, stride 4 | 1621 | an empty spectrum on every call, `USTAR <= 0.001` on every call, `CHARN = AALPHA` |
| storm (30 m/s), 8 m, stride 4 | 3241 | `TAUW > TAUWMAX` on 2154 calls |
| `st3wn` (`&SDS3 WNMEANP=1.`), 20 m, stride 4 | 1678 | `WWNMEANP /= WWNMEANPTAIL` |
| `st3z` (`&SIN3 ZWND=15., ALPHA0=0.0095`), 2500 m, stride 4 | 1674 | another `TAUT` and `ZZWND` |
| poke, 2500 m, stride 1, first 500 calls | 500 | inputs written at the routine's entry (below) |

**15 398 calls, 169 378 values, 0 differ**, in both layouts on the host (`just spr3-cmp`) and in SPIR-V
(`just spirv-replay spr3`).

**Poke.** `regtests/capture/spr3.py` with `CAPTURE_POKE` writes one of ten inputs at the entry of the real Fortran
routine and puts it back at exit, so the branches `ww3_ts1` never reaches run in the reference: `U = 0` (`0/0` in
`SQRTCDM1`, `MIN` gives 100), `U = 60` (`J` clamp), `TAUWX = 3` (`TAUWMAX` clamp), an empty spectrum, no `LLWS`, `U =
-0.1`, a spectrum only in the top six bands (`FMEANWS >= 1.E-7 > EMEANWS`, where `SIG > 1`), `U = 37.3` with a large
stress (both interpolation weights fractional), and `U = 0.1` and `0.03` (`USTAR` between 0.001 and 0.01). The poked
calls perturb the run after them, which stops at 500 calls; every recorded call is a real evaluation anyway.

```bash
just ref-build switch_chaceon_st3                # the coordinator builds it
just capture spr3 default 2500 2                 # CAPTURE_SWITCH defaults to switch_chaceon_st3 for spr3
just capture spr3 poke 2500 1
just spr3-cmp /work/capture/spr3_default_2500m.bin /work/capture/spr3_poke_2500m.bin    # and the other six
just spirv && just spirv-replay spr3 default 2500
SPR3_FAULT=taut just spr3-cmp /work/capture/spr3_default_2500m.bin   # also llws, wn, a
```

### Branches the captures did not reach ⚠

- `IND = MIN(ITAUMAX-1, INT(XI))` and `DELI1 = MIN(1., XI-REAL(IND))` (`:1206-1207`) cannot act. `TAUW_LOCAL <= TAUWMAX`
  and `DELTAUW = TAUWMAX/200` (`0.0111805` in the captures), so `XI <= sqrt(2.2361)/0.0111805 = 133.75`: `IND <= 133` and
  `XI-IND < 1`. Fault runs agree (below). The largest `XI` any capture reached is 133.7471.
- `WINDSPEED` below `-DELU` or NaN (`J < 0`, an out-of-bounds `TAUT` read in the Fortran too), and a NaN anywhere. Not run.
- `TAUW_LOCAL = MAX(.., 0.)` cannot act: `TAUW` is a square root, so it is never negative.

## Fault runs

Each fault edits `spr3.rs`, rebuilds with a clean fingerprint and replays all eight captures (15 398 calls); the harness
is not kept. Counts are values of the call layout that differ.

| Fault | Result |
|---|---|
| `EB` as `EB*(DDEN/CG)`; `EB2` likewise | caught: 16 749 and 4 225 values |
| `FMEAN` power with `WWNMEANP`; `FMEANS` power with `WWNMEANPTAIL`; `FMEANWS` exponent with `WWNMEANP` | caught, 1 676 each, all on `st3wn`: the two exponents are equal (0.5) in every other capture |
| `WNMEAN` power or exponent doubled | caught: 13 706 values each |
| `FMEANWS` summed with `EB` for `EB2` | caught: 12 845 |
| `EB2` ignores `LLWS` | caught: 13 430 |
| tail: `FTE` replaced by `SSTXFTF` in `EMEAN` / `EMEANWS`; `SSTXFTF` by `SSTXFTFTAIL` in `FMEANS`; `SSTXFTFTAIL` by `SSTXFTF` in `FMEANWS`; `WNMEAN` tail dropped | caught: 56 352 / 12 013 / 1 676 / 1 406 / 13 682 |
| `FMEAN < 1.E-7` raised to `1.E-3`; same for `FMEANS` | caught: 77 and 74, only the poked calls with a tiny spectrum |
| `FMEAN` tail value `SIG(NK-1)` for `SIG(NK)` | caught: 1 692 |
| `FMEANWS`: `.OR.` as `.AND.`; `EMEANWS` test dropped | caught: 50 each, only the poke with a spectrum in the top six bands. No default capture has `EMEANWS < 1.E-7 <= FMEANWS`, which needs `SIG > 1` |
| `MAX(1.E-7, EMEAN)` dropped in the `FMEAN` quotient | caught: 50, the same poke |
| `TAUW`: `TAUWY**2` as `TAUWY`; `TAUWX**2` one ULP up | caught: 38 381 and 2 186 |
| `CALC_USTAR`: `TAUWMAX` clamp off | caught: 9 016 (storm, and the poke with `TAUWX = 3`) |
| `J` clamp `JUMAX-1` as `JUMAX` | caught: out-of-bounds panic of the host build, on the poke with `U = 60` |
| `DELJ1` clamp `MIN(1.,..)` off | caught: 200, the poke with `U = 60` |
| interpolation: first term distributed over `DELJ2`; second term distributed over `DELJ1`; `DELI2 = 1-DELI1*(1+2^-23)` | caught: 200, 2 399, 12 832. The first is weak: `DELJ2` is exactly 1 at the integer winds of the captures, so only the poke with `U = 37.3` reaches it |
| `SQRTCDM1` cap 100 as 101 | caught: 50, the poke with `U = 0` (`0/0`) |
| `Z0`: `KAPPA` one ULP up | caught: 28 974 |
| `CHARN`: threshold 0.001 as 0.01; `GRAV*Z0/USTAR**2` as `GRAV*(Z0/USTAR**2)` | caught: 13 (the pokes with `U = 0.1` and `0.03`) and 6 433 |
| `CD`: `UNZ` floor 0.01 as 0.02; `(USTAR/UNZ)**2` as `USTAR**2/UNZ**2` | caught: 1 671 and 7 257 |
| `USDIR` from the input `USDIR` | caught: 2 211 |
| `MAX(0., MIN(TAUW,TAUWMAX))` without `MAX(..,0.)` | **no effect**: `TAUW` is a square root and never negative |
| `IND` clamp 199 as 198; `DELI1` clamp off | **no effect**: `XI <= 133.75`, so neither can act (above) |
| `AMAX` starting at -1 for 0 | **no effect**: `A` is never negative |
| the two products of the first interpolation term added in the other order | **no effect**: addition is commutative, this is not a fault |

Input faults (`SPR3_FAULT`, a diagnostic of `spr3_cmp`): `taut` (every nonzero `TAUT` entry one ULP up) is caught (on `st3z`, 1 376 of 1 674 `CHARN` values differ); `llws` (the first flag flipped)
is caught on one capture; `a` (the first `A` one ULP up) is caught on `AMAX` of 2 values of one capture, weak since one
bin of 864 rarely sets the maximum; `wn` (`WN(9)` one ULP up) is caught, 26 and 8 values of two captures. A one-ULP
change of `WN(1)` was not caught (band 1 holds a negligible share of every sum), which is why the index is 9.

## GPU code

- SPIR-V: entry point `spr3` in `rust/gpu-spirv/shader`, 11 storage buffers, passes `spirv-val`. Eight captures, 0 differ.
- PTX: kernel `spr3` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body
  has 13 `div.rn.f32`, 11 `rcp.rn.f32`, no float `fma` or `mad` (the two `mad` are `mad.lo.s32`, an address). Not run ⚠.

## Translation notes

- The operation order is read from `__w3src3md_MOD_w3spr3` in the reference's `ww3_shel` (`gdb` `disassemble`, since the
  image has no `objdump`): aarch64, no `fmadd` or other fused instruction (v). `MAX` and `MIN` compile to `fmaxnm` and
  `fminnm`, as Rust's `f32::max` and `min`. The three `powf` of section 2 are calls to glibc's scalar `powf` (v, three
  `bl powf` in the loop, none to a vector variant), and `FMEANWS` reuses the power of `FMEAN`: the same bits as two calls.
  Section 4 has four `powf`. `TAUWX**2` is `x*x`; `(USTAR/UNZ)**2` is a quotient squared.
- Unlike the x86 object ww3-gpu read, the aarch64 `ELSE` branches keep `MAX(1.E-7, FMEAN)` (v, `fmaxnm` before the
  `fdiv`), as the source has it.
- `CALC_USTAR` is a call, not inlined (v, `bl __w3src3md_MOD_calc_ustar`). Its `exp` is glibc's `expf`, which
  `ww3-libm` has; no function was added.
- `1/(2.*WWNMEANPTAIL)` is `1.0f / (2*x)`; the integer `1` converts to `REAL` first.
- `LLWS` is `u32` in a buffer. A `LOGICAL(4)` holds 0 or 1 for `.FALSE.`/`.TRUE.` in gfortran.

## Not translated, not reached

- The call from `W3SRCE` (`w3srcemd.F90`), the callers `W3OUTP`, `GXEXPO` and `ww3_outp`, and the state around them.
- `W3SPR3` in a run of a whole model: `moddef.rs` reads `mod_def.ww3` for `switch_chaceon_shrd` only ⚠.
- The branches under "Branches the captures did not reach".

## Timing

None. No timing without a device run (`AGENTS.md`).
