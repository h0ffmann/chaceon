# W3SBT4

SHOWEX (Shoaling Waves Experiment) bottom friction over a movable sand bed (Ardhuin et al. 2003), with Tolman's
(1995) subgrid depth: a three-point spread of the depth gives the Shields parameter's slope, and an error-function
table mixes the relict and rippled roughness. `BT4` is in MetOcean's switch (`switches/switch_metocean_st4`), not in
the port's target `switch_chaceon_shrd`. Translated from `model/src/w3sbt4md.F90` at metocean/WW3 `92e2cf66`:
`W3SBT4` (`:279-576`) and `TABU_ERF` (`:205-275`), with `INSBT4` (`:117-203`), which only calls `TABU_ERF`. Kernel:
`rust/crates/ww3-kernels/src/sbt4.rs`. Two `ww3-libm` functions are new for it: `sinhf` and `erff`.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts
and in compiled SPIR-V on lavapipe (ADR-0007). `W3SRCE` calls it when `SrceParams::btopt` is `BT4`, bit-identical on
15 127 captured `W3SRCE` calls (`rust/port_report/W3SRCE.md`, "With `BT4`"). No model run uses it yet: the host reads
`mod_def.ww3` for `BT1` only ⚠. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `CG(NK)`, `WN(NK)` | `REAL`, in | `a`, `cg`, `wn` at offsets `o`, `k_off` |
| `DEPTH`, `D50`, `PSIC` | `REAL`, in | `Sbt4Point` |
| `TAUBBL(2)` | `REAL`, out | `Sbt4Out::taubbl` |
| `BEDFORM(3)` | `REAL`, inout: written, never read | `bedform` at `b_off` |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `o` |
| `IX`, `IY` | `INTEGER`, unused | dropped |
| `CBETA(NK)` | automatic `REAL` | `w` at `w_off` |
| `SBTCX(10)`, `DELAB`, `DELXERF` | module `REAL` | `Sbt4Params` |
| `SIG(0:NK+1)`, `DDEN(NK)`, `ECOS`, `ESIN(NSPEC+NTH)`, `FWTABLE(0:300)`, `ERFTABLE(0:300)` | module `REAL` | `Sbt4Tables` |

No `f64` anywhere (v, `w3sbt4md.F90`).

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `BT1` replaced by `BT4` (`switches/switch_chaceon_bt4`, built by
`just ref-build switch_chaceon_bt4`). gdb records every call of `ww3_ts1` (`regtests/capture/sbt4.py`), stride 1.
Outputs compared: `S`, `D`, `TAUBBL`, `BEDFORM`, and `ERFTABLE` with `DELXERF` as the first call left them.

| Capture | Branches reached: relict, ripples, subgrid | Calls | Host differ | SPIR-V differ |
|---|---|---|---|---|
| default, 2500 m | 2712, 0, 0; every `S` is zero (deep water) | 2712 | 0 | 0 |
| default, 8 m | 55, 0, 2622 | 2677 | 0 | 0 |
| default, 20 m | 136, 0, 2540 | 2676 | 0 | 0 |
| storm (30 m/s), 8 m | 6, 0, 8635 | 8641 | 0 | 0 |
| calm (0 m/s), 8 m | 2161, 0, 0; `S` zero, `D` not | 2161 | 0 | 0 |
| `bt4flat` (`SIGDEPTH=0`), 8 m | 2677, 0, 0 | 2677 | 0 | 0 |
| `bt4ripple` (`SIGDEPTH=0`, `RIPFAC3=0.05`), 8 m | 297, 5641, 0 | 5938 | 0 | 0 |
| `bt4ripple`, 20 m | 699, 2966, 0 | 3665 | 0 | 0 |
| `bt4coarse` (`D50=2 mm`, `RIPFAC3=0.3`), 8 m | 64, 0, 2613 | 2677 | 0 | 0 |

33 824 calls, 58 616 992 values, 0 differ, in both layouts on the host and on SPIR-V. `ERFTABLE`: 301 values and
`DELXERF` equal on every capture. A storm at 3 m was not captured: the reference's `ww3_grid` stops with status 3 at
that depth.

```bash
just ref-build switch_chaceon_bt4
just capture sbt4 bt4ripple 8           # CAPTURE_SWITCH defaults to switch_chaceon_bt4 for sbt4
just sbt4-cmp /work/capture/sbt4_bt4ripple_8m.bin
just spirv && just spirv-replay sbt4 bt4ripple 8
LIBM_ONLY=sinhf,erff just libm-bits     # every f32, against glibc 2.36: 0 differ
```

## Fault runs

On `default, 8 m` unless named.

| Fault | Result |
|---|---|
| `PSIC` one ULP up (`SBT4_FAULT=psic`) | caught: 102 118 `S`, 421 `TAUBBL`, 353 `BEDFORM` values |
| `FACTOR` as `(DDEN/CG)*(2*CBETA*GRAV)` | caught: 247 758 `S` values |
| `TEMP2` as `CONST2*(D*A)` | caught: 1 678 `TAUBBL` values |
| Subgrid `KSUBN` as `PROBA2*(AORB*X)` | caught: 485 `S` values |
| `PROBA2+0.0002` for `+0.0001` | **no effect** at 8 m: `PSIN2` is clamped to `SBTCX(3)` on every call. Caught on storm, 8 m: 17 639 `S` |
| `SQRT(TPI)` one ULP up | **no effect** at 8 m, for the same reason. Caught on storm, 8 m: 1 `BEDFORM` value |
| `ECOS(ITH)` for `ECOS(IS)` in `TAUBBL` | **no effect**: `ECOS(NTH+1:)` repeats `ECOS(1:NTH)` (`w3gridmd.F90:1294`) |
| `DDEN*2/CG` for `DDEN/CG*2` | **no effect**: a doubling is exact |
| `erf(-4.)` called rather than folded to `-1.` | **no effect**: glibc's `erff(-4.)` is `-1.` too |
| `KSUBS` as `AORB*(0.0655*X)`, `bt4ripple`, 8 m | **undetected**: `KSUBS` is far below `KSUBR`; `0.0656` moves 211 `BEDFORM` values, a regrouping moves none. The order rests on the disassembly ⚠ |
| `KSUBR` one ULP up, `bt4ripple`, 8 m | caught: 5 641 `BEDFORM` values; `S` and `D` unchanged, the `FWTABLE` lookup absorbs it |

## GPU code

- SPIR-V: entry point `sbt4` in `rust/gpu-spirv/shader`, 16 storage buffers, passes `spirv-val`. `just spirv-libm`
  compares `sinhf` and `erff` in SPIR-V with the host: 2 000 000 inputs each, 0 differ.
- PTX: kernel `sbt4` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false`. Its body has
  `div.rn.f32` and no float `fma` or `mad` (the one `mad` is `mad.lo.s32`, an address). Not run ⚠.

## Translation notes

- The order of operations is read from `__w3sbt4md_MOD_w3sbt4` in the reference's `ww3_shel` (`objdump`). gfortran
  inlines `TABU_ERF` there. `**2` is a product, `**1.4` and `**SBTCX(2)` call `powf`, `SQRT(TPI)` is folded to
  `0x40206c99`, and `2.*GRAV*(SED_SG-1)` and `(SED_SG-1)*GRAV` are folded with one rounding each, as Rust's `f32`
  constants are. Nothing fuses (`-ffp-contract=off`).
- `TABU_ERF` writes `2**(1/2)*(1+erf(x))/2`. `1/2` is integer division, so the factor is 1 and the table is the normal
  distribution of `x*sqrt(2)`, not of `x`. The arithmetic is what runs, and it is kept.
- gfortran peels `I=0` of `TABU_ERF` and folds `erf(-4.)` to `-1.` with MPFR (`f98cc`). The port writes `-1.` there.
- `DSUM(NK)` only ever holds the value of its own band, so it is a scalar. `WSUB` is declared and never used.
- The output loop uses `CBETA` of the last subgrid pass, `ISUB=3`, the mean depth (`XSUB(3)=0`).
- `MAX` and `MIN` compile to `fmaxnm` and `fminnm`; Rust's `f32::max` and `min` return the same, NaN included.
  `clamp` would not, so `MAX(MIN(...))` stays two calls.
- `sinhf` and `erff` are glibc 2.36's fdlibm code. Their fused sites are read from the reference's `libm.so.6`: every
  Horner step of `erff` is an `fmadd`, as are `x + x*y`, `-z*z - 0.5625` and `(z-x)*(z+x) + R/S`. `sinhf` has one,
  `2t - t*t/(t+1)`, exact either way. Both match glibc on all 2^32 inputs (`just libm-bits`).

## Not translated, not reached

- The state around the call in `W3SRCE`: `SED_D50`, `SED_PSIC`, `BEDFORMS` and
  `TAUBBL` per sea point in `W3WAVE` (`w3wavemd.F90:2186-2253`), `SBTCX`, `SED_D50` and `SED_PSIC` in `mod_def.ww3`
  (`w3iogrmd.F90:1425`), the `SBT4` namelist and `SED_PSIC` in `ww3_grid` (`w3gridmd.F90:2283-2303`, `:5550-5556`).
  `ww3_grid` and `moddef.rs` read `switch_chaceon_shrd`'s layout only ⚠.
- `W3EXPO` and `GXEXPO`, the point post-processors that also call it.
- The guards `MAX(D50,1E-5)` and `MIN(DD50,1.)` with a `D50` outside them, `INDE` clamped at 0 or 299, and a NaN input:
  no capture reaches them ⚠.

## Timing

None. No timing without a device run (`AGENTS.md`).
