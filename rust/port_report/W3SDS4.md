# W3SDS4

ST4's dissipation: the schemes of Ardhuin et al. (2010), Filipot and Ardhuin (2012) and Romero (2019),
wave-turbulence interaction, cumulative breaking and whitecap output. Translated from
`model/src/w3src4md.F90:1872-2599` at metocean/WW3 `92e2cf66`, MetOcean's commits included. Kernel:
`rust/crates/ww3-kernels/src/sds4.rs`.

**Status: `routine`** (ADR-0007), on the host. Bit-identical on SPIR-V: **no**. No whole-file verdict,
no GPU run ⚠.

## Comparison with the reference

Same reference and case as [`W3SNL1`](W3SNL1.md). Variants change the case's namelists only, every
third call kept, 12 452 calls. Outputs: `SRHS`, `DDIAG`, `BRLAMBDA`, `WHITECAP`, 32 325 392 values.

| Capture | Reaches | Calls | Host differ | SPIR-V max ULP (`SRHS`, `DDIAG`, `BRLAMBDA`, `WHITECAP`) |
|---|---|---|---|---|
| default, 2500 m | Scheme 1, `atan2f` through `SSDSC(18)` | 904 | 0 | 5, 4, 2, 4 |
| default, 8 m | The same, shallow | 940 | 0 | 4, 4, 2, 4 |
| strain, 20 m | Straining, smoothing, `SSDSISO=1`, `TANH` | 870 | 0 | 4, 4, 3292, 3 |
| strain, 2500 m | The same, deep | 868 | 0 | 4, 4, 4019, 6 |
| circle, 2500 m | `SSDSDTH=180` | 808 | 0 | 6, 6, 2, 4 |
| t700, 2500 m | Scheme 3, the regtest's own settings | 767 | 0 | 1624, 2298, 0, 3 |
| t700, 8 m | The same, shallow | 767 | 0 | 93, 63, 0, 4 |
| romero2, 20 m | Scheme 3 with straining | 766 | 0 | 299, 455, 455, 3 |
| filipot, 2500 m | Scheme 2, `IKTAB`, `DCKI`, `QBI` | 2881 | 0 | 6, 12288, 1, 13 |
| filipot, 20 m | The same, shallow | 2881 | 0 | 6, 8192, 1, 4 |

Host means both layouts: one call at a time, and every call in one buffer at its offsets, as a device
runs them.

```bash
just capture sds4 strain 20 3 && just sds4-cmp strain 20
just spirv && just spirv-replay sds4 strain 20
```

`scripts/capture.sh` lists the variants.

## Fault runs

| Fault | Result |
|---|---|
| `SIG*(1-C6)` associated first in `COEF2` | 13 418 `SRHS` values differ, max 4 ULP |
| One bit of `2.*PI**2.` | 252 493 `BRLAMBDA` values differ |
| `KBAR**(-2.4999)` for `-2.5`, filipot, 20 m | Detected in `DDIAG`; `SRHS` alone misses it |

## GPU code

- SPIR-V: 473 KB with `W3SNL1`, passes `spirv-val`. Needs `Float64` and `Int64`.
- PTX, 668 KB with `W3SNL1`: 5 959 float operations, all `.rn`; no `fma`, no libdevice, no `ftz`.

## Translation notes

- `LOG(FAC_KD1)`, `2.*PI**2.`, `GRAV**2`, `0.8*2*PI` and `5.*2*PI` are the bits gfortran folds to,
  read from a probe at `-O3`.
- GCC merges `SIN` and `COS` of one argument into `sincosf`.
- `GAM` and `XT` in scheme 2, and `PB` in scheme 3, are dead in the Fortran and left out.
- `IK1` is 1: `IG1` is not built.

## Not covered ⚠

- `sds4_cmp` still reads its tables from the capture. They are translated now, and `st4_cmp` shows
  `INSIN4`'s equal the reference's ([`W3SIN4`](W3SIN4.md)).
- `SSDSC(18) = 0`, and the `SSDSC(21)` viscous term with a non-zero value.
- Other spectral grids, `W3SDS4` inside `W3SRCE`, threads, x86_64, any GPU.

## Timing

None: there is no whole-model comparison on this commit. Soft `fma64` and `sqrtf` make the kernel
slower than it needs to be, which Batch 6 addresses ⚠.
