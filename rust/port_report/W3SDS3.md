# W3SDS3

ST3's whitecapping dissipation: WAM cycle 4 (WAMDI Group 1988; Komen et al. 1994) with the mean wavenumber of Bidlot
et al. (2005). `D` is one factor per band, `FACTOR * (DDELTA1*K/WNMEAN + DDELTA2*(K/WNMEAN)**2)`, repeated over the
directions, and `S = D * A`. Translated from `model/src/w3src3md.F90:1253-1433` at metocean/WW3 `92e2cf66`. Kernel:
`rust/crates/ww3-kernels/src/sds3.rs`. No `ww3-libm` function: the routine has no `exp`, `pow` or `sqrt`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts and
in compiled SPIR-V on lavapipe (ADR-0007). It is not called from `W3SRCE` yet, and it has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `K(NK)` | `REAL`, in | `a`, `k` at offsets `o`, `k_off` |
| `EMEAN`, `FMEAN`, `WNMEAN` | `REAL`, in | `Sds3Point` (`FMEAN` is `FMEANS` at the call in `W3SRCE`) |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `o` |
| `CG`, `USTAR`, `USDIR`, `DEPTH`, `IX`, `IY` | in, never read | dropped |
| `NK`, `NTH`, `SSDSC1`, `DDELTA1`, `DDELTA2` | module `REAL` and `INTEGER` | `Sds3Params` |

No `f64` anywhere (v, `w3src3md.F90:1253-1433`). The `W3_S`, `W3_T`, `W3_T0` and `W3_T1` blocks are output only and are
left out.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST3` for `ST4` and `FLX0` (`switches/switch_chaceon_st3`; the image
`chaceon-ref-capture:switch_chaceon_st3`). gdb records every fourth call of `ww3_ts1` (`regtests/capture/sds3.py`,
stride 4): the scalars from `W3GDATMD` once, then `EMEAN`, `FMEAN`, `WNMEAN`, `A`, `K` and the `S` and `D` the call
returned. The recorded `SSDSC1`, `DDELTA1` and `DDELTA2` equal the namelist values on every capture (-2.1, 0.4, 0.6;
-4.5, 0.5, 0.5 for `st3wam4`).

| Capture | Calls | Host differ | SPIR-V differ |
|---|---|---|---|
| default, 2500 m | 597 | 0 | 0 |
| default, 8 m | 597 | 0 | 0 |
| calm (0 m/s), 2500 m | 541 | 0 | 0 |
| light (6 m/s), 2500 m | 571 | 0 | 0 |
| storm (30 m/s), 2500 m | 650 | 0 | 0 |
| storm, 8 m | 2161 | 0 | 0 |
| `st3wam4` (`SDSC1=-4.5`, `SDSDELTA1=SDSDELTA2=0.5`), 2500 m | 602 | 0 | 0 |
| `st3wnp` (`WNMEANP=-0.5`), 2500 m | 597 | 0 | 0 |
| `st3poke`, 2500 m (500 calls, stride 1) | 500 | 0 | 0 |

6816 calls, 23 556 096 values (`S` and `D`, both layouts), 0 differ on the host; 0 differ on SPIR-V for each capture.

Branches: the only branch is the floor `MAX(1.E-10, WNMEAN)` (`:1382`). No run of `ww3_ts1` reaches it (WNMEAN is
at least 1e-10 on all 8 captures of the model). `st3poke` writes `WNMEAN` at the routine's entry, cycling 0, 1e-12, -0.5
and NaN, and restores it and zeroes `S` and `D` at exit: 375 calls are below the floor, 125 are NaN. Stride 4 keeps the
files small (6 MB) ⚠: it samples one call in four of each run.

```bash
just ref-build switch_chaceon_st3
just capture sds3 st3 2500 4            # CAPTURE_SWITCH defaults to switch_chaceon_st3 for sds3 and st3*
just capture sds3 st3poke 2500 1
just sds3-cmp /work/capture/sds3_st3_2500m.bin
just spirv && just spirv-replay sds3 st3 2500
```

## Fault runs

On `default, 8 m`, `st3poke` and `storm, 8 m` together. `SDS3_FAULT` moves one input one ULP up before the replay.

| Fault | Result |
|---|---|
| `EMEAN`, `FMEAN`, `WNMEAN`, `SSDSC1`, `DDELTA2` one ULP up | caught on default 8 m and on `st3poke` (51 to 85 thousand `S` values each) |
| `DDELTA1` one ULP up | caught on default 8 m (110 825 `S`); **not on `st3poke`**: with `WNMEAN2 = 1e-10` the `DDELTA2` term is 1e10 times the `DDELTA1` term, which is far below one ULP of the sum |
| `K(1)` one ULP up | caught: 14 208 `D` values on default 8 m. `S` unchanged: `A` is zero in the first band of these runs |
| `ssdsc1*(tpi*fmean)` for `(ssdsc1*tpi)*fmean` | caught: 71 307 `S` on default 8 m |
| `wn*(wn*emean)` for `(wn*wn)*emean` | caught: 110 077 `S` on default 8 m |
| `ddelta1*(k/wn)` for `(ddelta1*k)/wn` | caught: 45 031 `S` on default 8 m |
| `ddelta2*((k*k)/(wn*wn))` for `ddelta2*(r*r)` | caught: 114 852 `S` on default 8 m |
| floor `1E-9` for `1E-10` | caught by `st3poke` only (87 154 `S`); 0 differ on both model captures |
| no floor | caught by `st3poke` only (215 463 `S`, signed zeros first); 0 differ on both model captures |
| `MAX` that propagates NaN instead of `fmaxnm` | **no effect, by construction**: a NaN `WNMEAN` makes `ALFAMEAN`, `FACTOR`, `D` and `S` NaN either way, so the floor's NaN rule cannot show in the output. The rule is the binary's, read below |
| `S = D*A + 0.0` | caught: 62 696 `S` values (negative zero becomes positive) |

## GPU code

- SPIR-V: entry point `sds3` in `rust/gpu-spirv/shader`, passes `spirv-val`; the pass that decorates float operations
  reported 52 339 operations. Compiled SPIR-V replays every capture at 0 ULP on lavapipe.
- PTX: kernel `sds3` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false`. The module has
  no float `fma` or `mad.f32`. Not run ⚠. (`docker/Dockerfile.ptx` is ignored by the `*.ptx` line of `.gitignore`;
  a fresh worktree must copy it.)

## Translation notes

- Read with gdb `disassemble` on `ww3_shel` of the capture image: `MAX(1.E-10, WNMEAN)` is `fmaxnm s8, s8, s1`, which
  returns the number when the other operand is NaN. `max_ww3` does the same (`y != y || x > y`); the first version of
  the kernel returned NaN.
- `**2` is a product, `ALFAMEAN**2` too. `SSDSC1 * TPI*FMEAN * ALFAMEAN**2` is left to right. `(K/WNMEAN2)**2` is one
  `fdiv` and one product, and `DDELTA1*K(IK)/WNMEAN2` is a product then a `fdiv`. Nothing fuses (`-ffp-contract=off`).
- `D=0.` then `D(IS)=FACTOR2` for every `IS`: `D` is written in full. `S = D*A` keeps the sign of zero.

## Not translated, not reached

- The call in `W3SRCE` and the ST3 state around it (`FMEANS`, `WNMEAN` from `W3SPR3`, the ST3 `mod_def.ww3` fields).
- `WNMEAN` below `1.E-10` or NaN occur only by poke ⚠.
- A shallower or deeper case than 8 and 2500 m: `W3SDS3` ignores `DEPTH`, so no branch depends on it.

## Timing

None. No timing without a device run (`AGENTS.md`).
