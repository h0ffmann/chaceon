# W3SIS1

Scattering of wave action by sea ice (Zieger, switch `IS1`): a linear transfer of action from each direction to the
other `NTH-1` of the same frequency, at a rate `ALPHA = MAX(0, IS1C1*ICE + IS1C2)`. `W3SRCE` calls it per sea point
with the ice concentration (`w3srcemd.F90:2127`, inside `IF ( INFLAGS2(4).AND.ICE.GT.0 )` at `:2079`). It passes
`VSIR` as `S`; the attenuation at `:2155` reads `VDIR` ⚠ (not followed up: `W3SRCE` is not part of this port). Translated from `model/src/w3sis1md.F90:74-228` at metocean/WW3
`92e2cf66`, without the `W3_S` and `W3_T` blocks (tracing and test output, not in the switch). Kernel:
`rust/crates/ww3-kernels/src/sis1.rs`. No `ww3-libm` function is needed: the routine has products, quotients and
comparisons only.

**Status: `routine`** in `rust/ports.tsv`: every captured call bit-identical, on the host in call and batch layouts
and in compiled SPIR-V on lavapipe (ADR-0007). `W3SRCE` does not call it yet (`srce.rs` is untouched), and the host
reads no `IS1` fields of `mod_def.ww3` (`w3iogrmd.F90:1466`) ⚠. It has run on no GPU ⚠.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `A(NSPEC)` | `REAL`, in | `a` at offset `o` |
| `ICE` | `REAL`, in | `Sis1Point::ice` |
| `S(NSPEC)` | `REAL`, out | `s` at offset `o` |
| `NK`, `NTH`, `DTMIN`, `IS1C1`, `IS1C2` | module `INTEGER` and `REAL` of `W3GDATMD` | `Sis1Params` |

The other `W3GDATMD` names the `USE` lists (`SIG`, `SIG2`, `DDEN2`, `TH`, `DTH`, `ECOS`) are used only in the `W3_T`
block. No `f64` anywhere (v, `w3sis1md.F90`). The Rust function returns whether `ALPHA > 0 .AND. ICE > 0` held, for
reading what a capture reached; the Fortran has no such output.

## Comparison with the reference

The reference is `switch_chaceon_shrd` with `IC0 IS0` replaced by `IC1 IS1` and `BT1` kept
(`switches/switch_chaceon_ice1`, built by `just ref-build switch_chaceon_ice1`; the build also serves the `W3SIC1`
port). `ww3_ts1` has no ice, so `scripts/capture.sh` gives the case a homogeneous ice concentration: it sets
`INPUT%FORCING%ICE_CONC='H'` (and `ICE_PARAM1='H'`, which `W3SIC1` demands), `HOMOG_COUNT%N_ICE=1` and one `ICE`
input, and records every call under gdb (`regtests/capture/sis1.py`, stride 1). The `icepoke` variants also write rare
inputs at the routine's entry and put them back at exit, so the real Fortran runs those lines.

| Capture | `ICE`, `IS1C1`, `IS1C2` | Calls | Scattering ran | Host differ | SPIR-V differ |
|---|---|---|---|---|---|
| `ice`, 2500 m | 0.4, 1, 0 | 2160 | 2160 | 0 | 0 |
| `ice2`, 2500 m | 0.2, 1, 0 | 2160 | 2160 | 0 | 0 |
| `icesplit`, 2500 m | 0.45, 0.5, 0.1 | 2160 | 2160 | 0 | 0 |
| `iceoff`, 2500 m | 0.4, -1, 0.4 (`ALPHA` = 0) | 2160 | 0; every `S` is zero | 0 | 0 |
| `ice`, 8 m | 0.4, 1, 0 | 2160 | 2160 | 0 | 0 |
| `icesplit`, 20 m | 0.45, 0.5, 0.1 | 2160 | 2160 | 0 | 0 |
| `icepoke`, 2500 m | 0.4, 1, 0; poked | 500 | 312 | 0 | 0 |
| `icepoke`, 8 m | 0.4, 1, 0; poked | 500 | 312 | 0 | 0 |
| `iceoffpoke`, 2500 m | 0.4, -1, 0.4; poked | 500 | 0; every `S` is zero | 0 | 0 |

14 460 calls, 12 493 440 values, 0 differ, in both layouts on the host and on SPIR-V. A poke, every eighth call in
turn: `ICE` = 0, `ICE` = -0.5, `ICE` = NaN, one bin = -0.001, = NaN, = -0.0, = 1, = +Inf (63 calls each carry a
negative bin and a NaN bin; the +Inf call's `S` holds Inf and NaN, and the capture zeroes it afterwards so the integration
goes on). `IS1C1` and `IS1C2` are the `ww3_grid` defaults, 1 and 0 (`w3gridmd.F90:2757-2758`), except where the
`&SIS1` namelist sets them. The run's spectra stay nonnegative, so a negative or NaN bin appears only by poke.

```bash
just ref-build switch_chaceon_ice1
just capture sis1 ice 8                 # CAPTURE_SWITCH defaults to switch_chaceon_ice1 for sis1 and ice* variants
just sis1-cmp /work/capture/sis1_ice_8m.bin
just spirv && just spirv-replay sis1 ice 8
```

## Fault runs

`SIS1_FAULT` in `sis1_cmp` breaks a copy of the routine, or a parameter, on purpose.

| Fault | Result |
|---|---|
| `ICE` one ULP up | caught: 655 208 of 1 866 240 values on `ice`, 2500 m |
| `DTMIN` one ULP up | caught: 672 282 values |
| `IS1C1` one ULP up | caught: 898 930 values |
| `IS1C2` + 1e-6 | caught: 984 875 values |
| `ALPHA*A/(NTH-1)` as `A/(NTH-1)*ALPHA` | caught: 225 415 values; 31 013 on `icepoke`, 2500 m |
| `A >= 0` as `.NOT. (A < 0)` (a NaN bin scatters) | caught by the pokes only: 1512 values on `icepoke`, none on the unpoked captures |
| `ALPHA > 0` guard dropped, `ICE > 0` kept | caught by `iceoffpoke` only, through the +Inf bin: `Inf*0` is NaN, 1488 values. No unpoked capture sees it, since a finite `A` times `ALPHA = 0` is 0 |
| `A >= 0` as `A > 0` | **no effect, by construction**: a bin of 0 or -0 would scatter `ALPHA*0 = ±0`, and adding or subtracting a zero leaves `S`, which starts at +0, as it was. The two tests are equivalent on every input, so no capture can tell them apart |
| `IS1C2` one ULP up (the default 0 makes it a subnormal) | **weak**: absorbed by the sum with `ICE`; the 1e-6 fault above replaced it |

## GPU code

- SPIR-V: entry point `sis1` in `rust/gpu-spirv/shader`, 4 storage buffers, passes `spirv-val` (`just spirv`).
- PTX: kernel `sis1` in `rust/gpu-cuda/kernels`, assembled for sm_89 and sm_90 with `ptxas --fmad=false` (`just ptx`);
  its body has no float `fma` or `mad`. Not run ⚠.

## Translation notes

- Read from `__w3sis1md_MOD_w3sis1` in the reference's `ww3_shel` (`gdb disassemble`, aarch64, gfortran 12.2
  `-O3 -ffp-contract=off`): `MAX(0.,v)` is `fmaxnm`, whose NaN gives 0, the same as `v > 0 ? v : 0`; `ALPHA*A` is one
  `fmul`, then the target loop's `fdiv` by `REAL(NTH-1)` (true divisions, none turned into a reciprocal) and the
  final `S/DTMIN` is a vectorised `fdiv`. No `fmadd`, `fmla` or `frecpe`.
- The order of accumulation is the Fortran's: for each source bin in turn, `S(IS)` loses `ALPHA*A(IS)` and every other
  bin of the same frequency gains `ALPHA*A(IS)/REAL(NTH-1)`. A different order would change low bits.
- `A(IS).GE.0.` is false for NaN, so a NaN bin neither scatters nor receives from itself.
- ww3-gpu's `W3SIS1_KOKKOS.md` (NOAA's pin) read the same code; it was used for traps only.

## Not translated, not reached

- The `W3_S` and `W3_T` blocks, and the callers other than `W3SRCE` (`W3EXPO`, `W3EXNC`, `GXEXPO`).
- The `IS1C1` and `IS1C2` of a grid other than the defaults and the two `&SIS1` settings run here; `DTMIN` other than
  15 s (the `ww3_ts1` grid) ⚠. All three are plain `f32` operands, so a value is no new branch, but no capture ran it.
- `NTH = 1` (a division by `REAL(0)`): not a grid WW3 accepts for `IS1` ⚠, not run.
- `W3SRCE`'s state around the call: ice from a file, the `INFLAGS2(4)` gate, `ATT` and the `IC1` or `IS2` interplay.
  `W3SRCE` is not changed.

## Timing

None. No timing without a device run (`AGENTS.md`).
