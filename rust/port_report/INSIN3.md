# INSIN3, TABU_STRESS and TABU_TAUHF

ST3's tables, built once on the host when `W3IOGR` reads the grid (`model/src/w3iogrmd.F90:1227`). `INSIN3`
(`model/src/w3src3md.F90:726-813`) calls `TABU_STRESS` (`:822-943`), which fills `TAUT(0:200,0:200)`, the friction
velocity against wave-supported stress and 10 m wind, and `TABU_TAUHF(SIG(NK)*TPIINV)` (`:958-1116`), which fills
`TAUHFT(0:100,0:200)`, the high-frequency wave-supported stress. Rust: `rust/crates/ww3-host/src/insin3.rs`, at
metocean/WW3 `92e2cf66`, for `ST3`, `STAB0`, `FLX0` (`switches/switch_chaceon_st3`). Host side only: nothing here runs
on a device, so there is no kernel, no SPIR-V and no PTX entry point (the tables are inputs to `W3SIN3` and `W3SPR3`,
which other reports cover).

**Status: `routine`** in `rust/ports.tsv`, one row each for `INSIN3`, `TABU_STRESS`, `TABU_TAUHF`: the tables the
Rust builds equal the reference's module arrays bit for bit on every capture (ADR-0007). Not yet called from a model
run ⚠. It has run on no GPU, and needs none.

## Arguments

| Fortran | Kind, bounds | Rust |
|---|---|---|
| `ZZWND`, `AALPHA`, `BBETA`, `ZZALP`, `ZZ0MAX` | `W3GDATMD` `REAL` | `Insin3Input` |
| `SIG(0:NK+1)` | `W3GDATMD` `REAL`; only `SIG(NK)` is read | `Insin3Input::sig_nk` |
| `FRMAX` | `REAL`, in, hertz | `tabu_tauhf(frmax, ..)` |
| `TAUT(0:ITAUMAX,0:JUMAX)`, `DELTAUW`, `DELU` | module `REAL`, out | `Insin3Output` (column-major) |
| `TAUHFT(0:IUSTAR,0:IALPHA)`, `DELUST`, `DELALP` | module `REAL`, out | `Insin3Output` |

No `f64` anywhere (v, `w3src3md.F90:726-1116`).

## Comparison with the reference

`regtests/capture/insin3.py` stops gdb at `INSIN3`'s entry to read its inputs, then at its return to read the module
arrays, and ends the run there (one call per run). `ww3_ts1` on `switch_chaceon_st3`, namelists and grid changed only.

| Capture | `SIG(NK)` | `ZZ0MAX` | `TAUT` nonzero | `TAUHFT` nonzero |
|---|---|---|---|---|
| `st3`: default | 8.563788 | 0 | 40 200 | 14 460 |
| `st3zmax`: `Z0MAX=0.002` | 8.563788 | 0.002 | 40 200 | 20 301 |
| `st3tab`: `ALPHA0=0.012`, `BETAMAX=1.4`, `ZALP=0.008`, `ZWND=8.` | 8.563788 | 0 | 40 200 | 13 173 |
| `st3grid`: `XFR=1.07`, `NK=40` | 4.2647123 | 0 | 40 200 | 20 200 |
| `st3grid2`: `XFR=1.05` | 1.6809176 | 0 | 40 200 | 20 301 |

Differences: **0** of 303 530 values (`TAUT` 40 401, `TAUHFT` 20 301, `DELTAUW DELU DELUST DELALP` per capture,
five captures). The 201 zeros of `TAUT(:,0)` are forced by the Fortran (`:939-941`).

```bash
just capture insin3 st3 2500 1   # and st3zmax, st3tab, st3grid, st3grid2
just insin3-cmp /work/capture/insin3_st3_2500m.bin /work/capture/insin3_st3zmax_2500m.bin \
  /work/capture/insin3_st3tab_2500m.bin /work/capture/insin3_st3grid_2500m.bin /work/capture/insin3_st3grid2_2500m.bin
```

## Faults (`INSIN3_FAULT` of `insin3_cmp`, and one source edit)

| Fault | Result |
|---|---|
| 1: `ZZ0MAX` taken as 0 | caught only by `st3zmax`: 22 881 `TAUT` and 14 472 `TAUHFT` values differ. The other four captures have `ZZ0MAX=0` and cannot see it: weak there, not blind |
| 2: `SIG(NK)` one ULP up | caught on all five in `TAUHFT` (11 971 to 18 825 values, up to 40 024 ULP) |
| 3: `ZZALP` one ULP up | caught on all five, 149 to 404 `TAUHFT` values, 2 to 3 ULP |
| 4, 5, 6: one `TAUT` value, one `TAUHFT` value, `DELALP`, each one ULP up | each caught, 1 value, 1 ULP: the comparison reads every cell |
| 7: `OMEGAC` as the unrounded `SIG(NK)`, edit of the source | caught by `st3grid2` only: 11 241 `TAUHFT` values differ, up to 10 ULP |
| 8: `SIG(NK)/TPI` in place of `SIG(NK)*TPIINV`, edit of the source | **undetected** on all five: the two roundings agree for these `SIG(NK)` ⚠ |

Fault 7 is why `st3grid2` exists. `TPI*(SIG(NK)*TPIINV)` is not `SIG(NK)` in `f32` for 15 % of the grids searched
(103 of 700 combinations of `XFR`, `FREQ1` and `NK`, by a Python model of `f32` rounding); on the first four captures
it is, and a port that dropped the double rounding would have matched them. Fault 8 is a regrouping that these
captures do not tell apart; the code uses the Fortran's own order.

## Translation notes

- The only difference from ST4's `TABU_TAUHF` (`w3src4md.F90:1325-1477`) is `OMEGAC = TPI*FRMAX` with
  `FRMAX = SIG(NK)*TPIINV` (`:1058-1062` against `w3src4md.F90:1023`). `-O3` cannot fold the pair, and the captures
  need both roundings. `TABU_STRESS` is the same text as ST4's (v, `diff` of the two bodies without comments).
- `XM = 0.5`: `(1.-X)**XM` stays a `powf` call (no `-ffast-math`); `UST**2.` is `UST*UST`; `ZLOG**4` is two
  squarings. The optimised GIMPLE was not read: the bit match on five captures stands in for it ⚠.
- `TAUHFT(K,L)` accumulates in memory in the Fortran and in a local here; both are `f32` additions in the same order.
- `ST3` has no `TABU_TAUHF2` and no cache file.

## Not covered ⚠

- `ZZ0MAX != 0` with another `AALPHA`, `ZWND` or grid (one value, 0.002, was run).
- Other `ZZALP` signs and `BBETA` extremes; `SIG(NK)` above 8.6 or below 1.7.
- Calling `INSIN3` from a model run (`W3SRCE`, `W3IOGR` in Rust): not wired.
- `x86_64` and any glibc but 2.36: `insin3.rs` uses `ww3-libm`, whose functions are glibc 2.36's.

## Timing

None: there is no whole-model comparison on this commit.
