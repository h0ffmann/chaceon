# 2026-11-01: `ww3_prnc`, `ww3_ounp`, partitions and interpolation on one tree

Four branches were merged and every suite ran again on the result, because each branch had only seen its own changes.

## Result

| Command | Result |
|---|---|
| `just tp-all` | 50 comparisons identical |
| `scripts/ts1_pair.sh`, 20 cases of `work/pair` | 40 comparisons identical |
| `just rust-only` | 17 cases, 81 files identical |
| `scripts/spirv_all.sh`, 17 cases | 46 comparisons identical |
| `RUST_ONLY_SPIRV=1 scripts/rust_only.sh`, `ww3_tp1.6`, `ww3_tp2.15` | 6 and 5 files identical |
| `just mpi-pair` | 40 configurations, 3 repeats each, identical |
| `just grid-all` | 28 files identical |
| `just strt-variants` | 20 restarts identical |
| `just prep-all`, `just prep-variants` | 14 and 16 files identical |
| `just prep-interp` | every case as expected, `REFUSED` only for the forms the report lists |
| `just prnc-all`, `just prnc-variants` | 11 and 78 cases as expected, 8 refused |
| `just outp-all`, `just outf-all` | 43 and 46 identical; 4 and 1 refused |
| `just ounf-all`, `just ounf-variants`, `just ounf-selfcmp` | 36 of 37, 168 of 168, 43 of 43 |
| `scripts/ounf_extra_all.sh` | 45 runs identical, the first time on an integrated tree |
| `just ounp-all`, `just ounp-selfcmp` | 36 runs identical, 1 refused; the reference is self-identical on every file |
| `just ounp-variants` | 296 identical, 45 refused, 6 looped in both, 1 not identical, of 342 runs (below) |
| `just bnd-all` | 23 comparisons identical |

## Two faults of the harness, neither in a program

`ounp_pair.sh` empties `work/ounp/<tag>/` from the host and starts a container at once. With Docker Desktop's file sharing the
container sometimes does not yet see that directory. The redirect then failed before `ww3_ounp` started, and the
`echo $?` after it made the container exit 0, so the script's one retry never fired. One sweep reported 75 of its runs
as different with this message on every one; `ww3_ounp` had not run on the reference side of any of them. The reference
alone, inside one container, gave the same bytes and status 0 on 40 of 40 runs on the input that showed it first.

The script now creates the two logs before the program starts, exits 75 if it cannot, and repeats only that case, up to
five times. A status of `ww3_ounp` itself is not retried. The next sweep had no such failure.

The one difference left, `ww3_tp2.15 sep+buf2+nc4`, is the same delay reached later: the reference stopped with
`ERROR IN OPENING FILE`, `IOSTAT = 2`, on the symbolic link `out_pnt.ww3`, and status 20. Alone, the case was identical
on 8 of 8 runs. It is not retried, because a status from the program must never be, so a sweep can still show such a
line ⚠.

A third anomaly was not the sharing: in the first sweep `ww3_tp1.7 buf2` stopped at the 60 s limit after a gap of about
three hours. `pmset -g log` shows idle sleep from 09:35 and the first user activity near 12:50; the sweep's own time was
12 890 s. The suites after that ran under `caffeinate -i`.

## Other corrections

- `prnc.rs` passed six arguments to `Gsu::create`, which gained `ijg` on the other branch. It passes `false`, because that
  code transposes to `XG(J,I)` before the call. `W3SPLX` existed twice with the same arithmetic; the merged file keeps one.
- `partition.rs` tested NaN with `depth != depth`; clippy refuses it, and `is_nan()` is the same test.
- `port_summary.py`: the Fortran target counted only `model/src/*.F90`, 120 705 code lines, and left out `SCRIP/`, `PDLIB/`
  and the `.f`, `.f90` and `.c` files that Batch 8 has to port. It now counts all model source, 133 810 of 278 855 lines.
  `ww3_ounp` and `ww3_prnc` were missing from `UNLEDGERED`, so their files counted as verification tools.
