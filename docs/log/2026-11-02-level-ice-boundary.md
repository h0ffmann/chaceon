# 2026-11-02: water levels, ice and boundary input in a run; `ww3_bound`

`ww3_shel` now takes water levels, `IC0` ice concentration and file winds, and `nest.ww3` with `W3IOBC` READ, `W3UBPT`,
`W3CSPC`, `W3GFPT` and step 3.c of `W3XYP3`. `ww3_bound` is ported. All host; `ww3_shel` refuses levels, ice and a `nest.ww3`
on a device ⚠. Nothing ran on a GPU. No timings.

## Result

| Command | Result |
|---|---|
| `just bound-all` | 214 comparisons identical over 34 cases, with the status of every run that must stop |
| `just bound-faults` | 16 faults, 16 caught |
| `just forcing-all` | 48 comparisons identical |
| `just tp-all` | 50 comparisons identical |
| `just rust-only` | 95 files identical, 81 before this change |
| `scripts/spirv_all.sh`, 18 cases of `tp-all` | 50 comparisons identical |
| `scripts/mpi_pair.sh ww3_tp2.3 ww3_tp2.15` | 10 configurations, 3 repeats each, identical; no `nest.ww3` in them |
| `just spirv-replay` (`snl1`, `sds4`), `just spirv-libm` | every bit equal |
| `just spirv`, `just ptx` | build |
| `just bnd-all` | 23 comparisons identical |
| `scripts/ts1_pair.sh`, 20 cases of `work/pair` | 40 comparisons identical |

## A defect in the level input, found by a boundary case

`level`, a homogeneous level with no other input, differed from the reference in four bytes of the restart: `TLEV` was
`20010101 120000` against the reference's `20010102 000001`. `FLLSTL` is set when the input ends, but `INFLAGS1(1)` goes false
only after the next `W3WAVE` (`ww3_shel.F90:2565-2582`), and in that call `TLN` is `TN` plus a second, so `W3ULEV` fires at the
midpoint. The first translation turned the field off when `FLLSTL` was set. `forcing-all`'s `homog` passed anyway, because
file winds ended that segment before the midpoint: a passing case there proved less than it seemed to. Rust now keeps `last`
and `off` apart.

## What `ww3_tr1` does on this switch

`input_bndin` needs `RTD`. Without it the grid runs unrotated and its three stations are far outside, so every boundary point
takes one spectrum at weight 1, and no `out_pnt.ww3` is written because `points.list` falls outside too. A first pass of
the suite therefore compared no point output. `regtests/bound/tr1/points.list` puts three points in the grid; `bound_gen.py`
puts stations on the boundary, for real weights. The run starts calm, so `nonest` differing from `edge` shows the waves in
these runs came through the boundary.

## Faults that said something about the faults

- `noclear`, which left `FLBPI` set at the end of the file, made the read loop in `W3WAVE` spin for ever. A hang is not a
  difference, and it cost half an hour before the silence was read. The fault now keeps forcing the boundary past the end of
  the file, which is the behaviour `FLBPI` guards.
- `clamp` was not caught at first: on `edge` every boundary point lies between its stations, so `COS1` never leaves 0 to 1.
  On `two` it does.
- `spirv_all.sh` takes its cases as arguments. Run with none it exits 0 having compared nothing, and it did so once here before
  the empty log was noticed. A count of comparisons is the verdict, not its status. ⚠ The script still does this.

## Not reached

The closing `⚠`s are in `rust/port_report/W3IOBC.md`, `WW3_BOUND.md` and `W3ULEV.md`: boundary input under `mpirun`, poles in
`W3GFPT`, `ww3_bound`'s status 70, heap bytes where the Fortran reads or writes past its data, and `RD2` unset in `W3ULEV`.
