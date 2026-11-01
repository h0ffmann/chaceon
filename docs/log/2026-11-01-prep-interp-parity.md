# 2026-11-01: `ww3_prep` types LL, F1, F2 and DAT, binary input and format reversal; `ww3_strt` type 2

2026-11-01, agent session `cd86e8f5` for the owner; no issue. Branch `goal/cd86e8f5-3302-prep-interp`.
Results: `rust/port_report/WW3_PREP.md` and `WW3_STRT.md`. Follows `2026-11-01-prep-parity.md`, which stopped with
status 64 at every form below.

## Question

`ww3_prep` translated the input type `AI` and refused `LL`, `F1`, `F2`, `DAT`, binary input (`IDFM` 3), a format
that reverses and a comma in a field. `ww3_strt` refused start type 2. Can Rust run those forms, bit for bit, when no
single-grid regtest has an input of them?

## Result

Yes. 65 forcing files from `regtests/prep/input` are byte-identical to the reference's, 306 992 bytes
(v, `just prep-interp`): `LL` on a spherical, a global
(simple closure), a polar, a dateline-crossing and a Cartesian model grid; `F1` and `F2` with a land mask in the
input (inline, text files by name, `fort.<n>` by unit, formatted, binary), `DAT` for the three record types, binary
data, time and header records, and formats that revert. The 14 regtest inputs are still identical (406 220 bytes, `just prep-all`), and so are the 12 variants of the day
before with four more (16 files, 260 200 bytes, `just prep-variants`). 25 inputs the Fortran stops on end with the same exit status in Rust, and the field file the
Fortran had written by then is identical. Three the Fortran accepts and Rust refuses are refused with status 64.
`ww3_strt` type 2: 20 restarts, 526 791 456 bytes, two regtests (`ww3_tp2.5`, `ww3_tp2.11`) and 18 variants, 0 differ
(v, `just strt-variants`). The ledger row `w3prep` stays `translated`: branches listed in the report have not run.

Every fault went through the pair once: 18 faults of the interpolating `ww3_prep` and 10 of `ww3_strt` type 2,
27 caught and one not (the loop that adds 360 to a negative `THM`, which `MOD(630.-THM,360.)` cancels at the angles
tried); the reports have the tables (v, `bash scripts/interp_faults.sh`).

## What it took

- **The grid search was already there.** `gridsru.rs` held `GSU_CREATE`, `W3GFCL_R8`, `W3CKCL_R8`, `GETPQR` and
  the bilinear branch of `W3GRMP_R8` for `ww3_grid`'s boundary points, for a grid stored `XG(J,I)`. `ww3_prep` stores
  `XG(I,J)` (`IJG` true) and needs the land mask, the nearest-neighbour fallback for a fully masked cell
  (`W3NNSC`, `W3SORT`, `W3ISRT`, `W3DIST`) and the cell near a pole (`W3SPLX`), where the old code stopped. All
  three ran on the first sweep and agreed: 4 cells fell back on their neighbours in the smallest case, 76 and 43
  lost in the one with a land block of 10 by 10, 336 lost in the all-land case.
- **A Cartesian grid takes the pole branch.** `W3GRMP` asks `D90-ABS(LAT0) > NEAR_POLE` whatever the coordinates
  are, and a Cartesian `y` in metres is never within 80 degrees. Every point of an `LL` or `F1` input on `ww3_ts3`'s
  grid (1 849 of 1 849 points, `llc_wnd`) goes through `W3SPLX` with metres taken as degrees, and the bits agree.
  That is a Fortran defect in effect, translated, and the best evidence the stereographic branch is right.
- **A comma ends a numeric field and is consumed with it.** The previous log guessed the field ends at the comma.
  Two probes on the reference showed what happens: the next field starts after the comma (`    3,33   12,50` reads
  0.03 and 33.12; the time `19680,06` in `I8` reads 19680 and the next field is `0 60000`). The input is now
  translated, not refused; the variants `comma` and `commai` hold it.
- **Format reversal** restarts at the last group of the outermost parentheses with its repeat count, or at the
  start. `(10F8.3)` on rows of 17, `(1X,2(F8.3))` on a whole array, and `(43F8.2)` already worked before.
- **Binary input** is a record of a length marker, the data and the marker again. A short record is an error with
  `IOSTAT` 5016. Reading gfortran's rules first suggested the statement would go on without branching; the
  reference stopped (the `e_shortrec` and `ai_bin4` cases); a header record that matches the field and the size is skipped; an empty file ends the program
  with no output.
- **The exit statuses.** `EXTCDE` passes the Fortran's `IOSTAT` where a runtime read fails, and the shell sees it
  modulo 256: 146, 152, 142 and 255. The first guess for each of them was checked against the reference by the
  `PREP_ERROR=1` mode of `prep_pair.sh`, which compares the status and the partial file.
- **Type 2 of `ww3_strt` was small.** `EJ5P`, the 1-D spectrum, the directional spreading and the grid floors of
  `SIX` and `SIY` that type 1 already had. The two `**` of `EJ5P` are multiplications in the optimised code
  (`x2*(x*x2)` and `x2*x2`), `FRR**BETA` is `powf`. The first restart, `ww3_tp2.11`, was identical.

## Fortran defects translated

Named in the report: `W3FLDP` overwrites the first address of a fully masked cell with the second point's and keeps
`IX2`, `IY2` at 1 (`w3fldsmd.F90:2028`); `DAT` reads its data with `FORMT`, the time format (`ww3_prep.F90:1185`);
`W3GRMP` takes the stereographic branch on a Cartesian grid (`w3gsrumd.F90:2718`); `NDAT` is read
list-directed whatever `IDFM` is.

## What did not get done, and what went wrong on the way

- Branches not run against the reference: a fully masked cell with exactly one sea neighbour (the counters show
  none in any case: `with one neighbour` is 0 in every row); the two address guards on `IY21` and `IY22`, which
  cannot be reached; `ICLO_TRPL`; the closest-cell search of `W3GFCL`; `DAT` header on a binary file; two data
  files on one unit. `w3prep` therefore stays `translated`.
- Nine of the first inputs were wrong, not Rust: rows that spanned lines in a layout that reads a line a row, a
  `DAT` block one record short, a format reversal on rows that were not laid out in records. The reference
  failed on them (exit 255 or 53), which is how they were found.
- `scripts/interp_faults.sh` left `gridsru.rs` empty once: a perl error under `set -e` after the redirect had
  truncated the file, and the copy was in a temporary directory the exit trap removed. The file was rebuilt from
  `git show HEAD:` and the edits of the day, and every sweep above was run on the rebuilt file. The script now
  writes the faulty file to a copy and replaces the source only when the substitution succeeded.
- Some first faults were equivalent mutants and were replaced: `iy2 = js[3]` against `js[2]` (the same index in
  a rectilinear cell), removing the clamp of `GAMMA` to 1 (invisible, `YLN` is below `EPS` for any value up to 1),
  `f <= fp` against `f < fp` (equal only at one frequency), and `THM` angles for which the two `MOD`s agree. The
  faults and the angles were changed until each fault differed, but for the add-360 loop.
- The ww3_strt type 2 restart of `ww3_tp2.11` is 298 MB; the variants are on a smaller grid, and the two big
  files of `ww3_tp2.11` are deleted after the comparison.
- No timing was taken. Nothing ran on a GPU.
