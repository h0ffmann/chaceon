# W3SNL1 and INSNL1

The Discrete Interaction Approximation. Translated from `model/src/w3snl1md.F90:99-457` and
`:467-770` at metocean/WW3 `92e2cf66`. Kernel: `rust/crates/ww3-kernels/src/snl1.rs`. Setup, on the
host: `rust/crates/ww3-host/src/insnl1.rs`.

**Status: `routine`** (ADR-0007). No whole-file verdict yet, and it has run on no GPU ⚠.

## Comparison with the reference

Reference: `switch_chaceon_shrd`, gfortran 12.2.0 `-O3 -ffp-contract=off`, glibc 2.36, linux/arm64.
Case: `ww3_ts1`, `NK=36`, `NTH=24`, `LAM=0.25`, with the case's depth changed in its namelist.

| Depth | Calls | `S` and `D` values | Host differ | SPIR-V on lavapipe differ |
|---|---|---|---|---|
| 2500 m | 2712 | 4 686 336 | 0 | 0 |
| 20 m | 2669 | 4 612 032 | 0 | 0 |
| 8 m | 2819 | 4 871 232 | 0 | 0 |

`INSNL1`: 979 real and 29 184 integer table values, 0 differ.

```bash
just capture 2500 && just snl1-cmp 2500 && just spirv && just spirv-replay 2500
just libm-bits   # expf: 4 294 967 296 inputs, 0 differ
```

## Fault runs

| Fault | Result |
|---|---|
| `expf` `C2` from `0x3fe62e42ff0c52d6` to `0x3fe62e52ff0c52d6`, `just libm-bits` | 172 712 591 inputs differ |
| The same, `snl1_cmp` at 8 m | 180 694 `S` and 192 990 `D` values differ |
| The same, `snl1_cmp` at 2500 m | **0 differ**: `EXP(X2)` vanishes in deep water, so that case alone cannot see it |
| `ep1 * (em1 * dal3)` for `ep1 * em1 * dal3`, host, 2500 m | 136 260 `S` values differ |
| The same, SPIR-V on lavapipe, 8 m | 112 774 `S` and 12 669 `D` values differ |
| `pow11` perturbed | 912 table values differ |

## GPU code

- SPIR-V: 32 KB, passes rust-gpu's `spirv-val`, needs `Float64` and `Int64` for `expf`.
- PTX: sm_89, assembled for sm_89 and sm_90. No `fma`, no libdevice call, no flush to zero.

## Translation notes

- Local closures became a macro and a function: SPIR-V's logical addressing refuses the pointers
  they capture. The arithmetic is unchanged, and the replay shows it.
- `x**11` is GCC's power tree, `XFR**IFRP` is libgcc's `__powisf2`.
- `expf` needs no fused multiply-add: the plain Rust form equals aarch64 glibc on every input.

## Not covered ⚠

- Other spectral grids and `LAM`, and any run on a GPU.
- `W3SNL1` inside `W3SRCE`, `ww3_multi`, threads.
- x86_64: the bits proved here are the arm64 reference's.

## Timing

None: there is no whole-model comparison on this commit.
