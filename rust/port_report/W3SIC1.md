# W3SIC1

Ice damping with one uniform exponential decay rate (Rogers and Orzech 2013): the amplitude decays as `exp(-k_i x)`
with `k_i = ICEP1(IX,IY)` for every wavenumber, so `D = S/E = -2*Cg*k_i` and `S = D*A`. `IC1` is in `switch_chaceon_ice1`
(`IC1 IS1`), not in the port's target `switch_chaceon_shrd`. Translated from `model/src/w3sic1md.F90:93-369` at
metocean/WW3 `92e2cf66`. Kernel: `rust/crates/ww3-kernels/src/sic1.rs`. No new `ww3-libm` function.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call, batch and mixed-batch
layouts and in compiled SPIR-V on lavapipe (ADR-0007). It has run on no GPU ⚠. `W3SRCE` does not call it yet. No model
run uses it: `ICEP1` is not read from an input field by the host ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)`, `CG(NK)` | `REAL`, in | `a`, `cg` at offsets `o`, `k_off` |
| `DEPTH` | `REAL`, in | dropped: only the `W3_T38` test output reads it (`w3sic1md.F90:319-321`, not built) |
| `IX`, `IY` | `INTEGER`, in | dropped: they only index `ICEP1` |
| `ICEP1(IX,IY)` | module `REAL`, input field | `icecoef1`, one value per call |
| `S(NSPEC)`, `D(NSPEC)` | `REAL`, out | `s`, `d` at `o` |
| `MAPWN(NSPEC)` | module `INTEGER`, bin to band | `mapwn` |
| `D1D(NK)`, `WN_I(NK)` | local `REAL` | not stored: `D1D(MAPWN(IKTH))` is recomputed per bin |

No `f64` anywhere (v, `w3sic1md.F90`). The check `.NOT.INFLAGS2(-7)` (`:299-302`) writes an error and stops the run. It
belongs to the host, not to a kernel; the capture records the flag and `sic1_cmp` refuses a capture without it.

## What gfortran compiled

Read with `gdb disassemble` on `__w3sic1md_MOD_w3sic1` in the reference's `ww3_shel` (gfortran 12.2, `-O3
-ffp-contract=off`, aarch64; the capture image has no `objdump`):

- `-2.*CG(IK)*WN_I(IK)` is `-((CG+CG)*WN_I)`: `fadd v0,v0,v0`, `fmul`, `fneg` in the 4-wide loop and `fadd`, `fnmul` in
  the tail. The value is the same as `(-2*CG)*WN_I` for every number; a NaN differs in its sign bit, because `fneg` flips
  it where a product by `-2` keeps it. A first translation as `-2. * c * ice` matched on every finite capture and
  differed on 61 344 NaN values of the poked capture.
- `ICECOEF1==0.` is `fcmp s0, #0.0; b.ne`: false for a NaN, so a NaN takes the ice branch, as Rust's `!= 0.` does.
- `S = D*A` is one `fmul` per bin. The vector body multiplies `A*D`, the tail `D*A`: the same value, and the same bits
  unless both operands are NaN, which no capture has ⚠.
- `D = 0.` is a `memset`; the `ICEP1 == 0` branch leaves it, so `S = 0*A`, and `0*Inf` is the default NaN.
- The sign flip is written as `x.to_bits() ^ 0x8000_0000` in `sic1.rs`. On the host `-x` is the same. Both spirv-opt and
  NVPTX move a float negation into the neighbouring product: the module had `OpFMul -2.0 c` and the PTX `mul.rn.f32 %f3,
  %f2, 0fC0000000`, so a NaN `ICEP1` kept its sign (SPIR-V replay of the poked capture: 61 344 of 432 000 values
  differed, 2^31 ULP, the sign bit). With the bit flip SPIR-V is 0 differ, and the PTX has `add.rn`, `mul.rn`, `xor.b32`
  and no float `fma` or `mad` (v, `just spirv-replay`, `just ptx`).

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `ST4 ... IC1 IS1` (`switches/switch_chaceon_ice1`, built by `just
ref-build switch_chaceon_ice1`). `ww3_ts1` has no ice, so its namelist gets homogeneous `ICE` and `IC1` input
(`regtests/capture/ice1_shel.sh`, variants `ice1*` in `scripts/ts1_variants.sh`): `W3SRCE` calls `W3SIC1` only when
`INFLAGS2(4)` is set and `ICE > 0` (`w3srcemd.F90:2079`, `:2105`). gdb records every call (`regtests/capture/sic1.py`,
stride 1), with `ICEP1(IX,IY)` read from the field at the call. Outputs compared: `S` and `D`.

| Capture | `ICEP1` | Calls: no ice, ice | Host differ | SPIR-V differ |
|---|---|---|---|---|
| `ice1`, 2500 m (`ICE=0.5`) | 2e-5 | 0, 2160 | 0 | 0 |
| `ice1`, 8 m | 2e-5 | 0, 2160 | 0 | 0 |
| `ice1zero`, 2500 m | 0 | 2160, 0; `S` and `D` all zero | 0 | 0 |
| `ice1dated`, 2500 m (`ICE=0.9`, `0.9`, `0.3`) | 0 and 3.5e-6 | 720, 1440 | 0 | 0 |
| `ice1dated`, 20 m | 0 and 3.5e-6 | 720, 1440 | 0 | 0 |
| `ice1storm` (30 m/s), 2500 m | 5e-5 | 0, 2160 | 0 | 0 |
| `ice1poke`, 2500 m (`CAPTURE_POKE`) | -1e-5, 0, 1e-40, 2e-5, Inf, NaN; `A` poked to NaN and Inf | 71, 429 | 0 | 0 |
| `mix`: the six captures above except `ice1dated` at 2500 m, joined (`regtests/capture/sic1_mix.py`) | all | 2951, 8349 | 0 | 0 |

13 460 calls in the first seven rows, 11 629 440 values each of `S` and `D`: 0 differ, in the call and the one-buffer
batch layouts, on the host (in the reference's image) and on SPIR-V (lavapipe). The mixed capture is 11 300 of those calls
in one buffer: `ww3_ts1` gives every call of a case the same `CG`, so a wrong `CG` offset in a batch is invisible in any
one capture, and only a buffer that joins depths 2500, 8 and 20 m sees it. The `ice1dated` captures never carry the first
entry's 2e-5, only 0 and 3.5e-6, and why was not looked into ⚠.

```bash
just ref-build switch_chaceon_ice1
just capture sic1 ice1dated 20          # CAPTURE_SWITCH defaults to switch_chaceon_ice1 for sic1 and ice1*
python3 regtests/capture/sic1_mix.py work/capture/sic1_mix_2500m.bin work/capture/sic1_ice1_2500m.bin ...
just sic1-cmp /work/capture/sic1_ice1dated_20m.bin /work/capture/sic1_mix_2500m.bin
just spirv && just spirv-replay sic1 mix 2500
```

## Fault runs

Run on all eight captures, on the host, by editing the kernel (`SIC1_FAULT=cg` is built into `sic1_cmp`).

| Fault | Result |
|---|---|
| `CG` one ULP up (`SIC1_FAULT=cg`) | caught on every capture |
| `(c+c)*ice` without the negation | caught on every capture |
| `-2.*c*ice`, the Fortran line as written | caught on the NaN poke only: 245 376 values in both layouts; finite inputs agree |
| ice branch also for `ICEP1 == 0` | caught: `D = -0` against `+0` on every capture with `ICEP1 = 0` |
| `D` not zeroed when `ICEP1 == 0` | caught (outputs are poisoned with NaN before each call) |
| ice only for `ICEP1 > 0` | caught on the poke only (`ICEP1 = -1e-5` and NaN): 471 042 values |
| `MAPWN` not minus 1 | caught: index out of bounds at band `NK` |
| `CG` offset dropped | caught on the mixed capture only: 3 382 572 values; no single capture sees it |
| `A` offset dropped | caught: batch layout, every capture |
| `S` times `1+2^-23`, and `S` one ULP up | caught on every capture |
| `S = A*D` for `D*A` | **no effect**: a product commutes, and no capture has both operands NaN |
| `2.*c` for `c+c` | **no effect**: both are exact |
| negation as `x*-1.` or `-x` | **no effect** on the host: LLVM turns `x*-1.` into `fneg`, and the host's `fneg` is the reference's. SPIR-V caught the folded form, above |

Not run: `W3SIC1` with `INFLAGS2(-7)` false. The reference stops there.

## GPU code

- SPIR-V: entry point `sic1` in `rust/gpu-spirv/shader`, 8 storage buffers, passes `spirv-val` (v, `just spirv`).
- PTX: kernel `sic1` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (v, `just ptx`).
  Not run ⚠.

## Not translated, not reached

- The test output of `W3_T`, `W3_T0`, `W3_T1`, `W3_T38`, `W3_S` and the `IENT` trace: not in the build.
- `ICEP1` read from an input field, `ICEP2` to `ICEP5` (other `IC` switches), `W3IOGO`'s ice output, and the state
  around the call in `W3SRCE` (`SPEC`, `VSIC`, `VDIC`).
- `W3EXPO`, `W3EXNC` and `GXEXPO`, the point post-processors that also call it.
- A NaN in both `D` and `A`, and `-0.` as `ICEP1`: no capture reaches them ⚠.

## Timing

None. No timing without a device run (`AGENTS.md`).
