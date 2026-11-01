#!/usr/bin/env bash
# ww3_prep inputs no regtest has, built from ww3_ts3's wind input (two 43 x 43 arrays, inline) and judged by
# prep_pair.sh on ww3_ts3's grid: the other field ids, the header off, several times, other layouts and
# formatted input, a comma inside a fixed-width field, LL on the model's own grid and DAT. The interpolating
# types, binary input and format reversal are in prep_interp.sh.
# Exit 0 when every variant is identical.
set -euo pipefail
cd "$(dirname "$0")/.."
src=WW3/regtests/ww3_ts3/input/ww3_prep_wind.inp
v=work/prep/variants
mkdir -p "$v"
rm -f "$v"/*.inp
# The two arrays of the input, 1849 values each (field u is lines 1 to 185 of the data, v is lines 186 to 370:
# the Fortran drops the rest of the last line it reads): field u, then field v, one value per line.
tail -n +9 "$src" > "$v/data.txt"
[ "$(wc -l < "$v/data.txt" | tr -d ' ')" = 370 ]
head -n 185 "$v/data.txt" | tr -s ' ' '\n' | grep -v '^$' | head -n 1849 > "$v/u.txt"
tail -n 185 "$v/data.txt" | tr -s ' ' '\n' | grep -v '^$' | head -n 1849 > "$v/v.txt"
[ "$(wc -l < "$v/u.txt" | tr -d ' ')" = 1849 ] && [ "$(wc -l < "$v/v.txt" | tr -d ' ')" = 1849 ]
# arrays <name>...: 10 values per line, as the original. rows <name>...: 43 per line. frows <name> <scale>
# <printf>: 43 fixed-width fields per line, each value times the scale, formatted with the printf format.
arrays() { for f in "$@"; do xargs -n 10 < "$v/$f.txt" | sed 's/^/  /'; done; }
rows() { for f in "$@"; do xargs -n 43 < "$v/$f.txt" | sed 's/^/  /'; done; }
frows() { LC_ALL=C awk -v s="$2" -v fmt="$3" '{ printf fmt, $1 * s; if (NR % 43 == 0) printf "\n" }' "$v/$1.txt"; }
unit() { printf "  'UNIT' %s %s '%s' '%s'\n  10 'dummy_name'\n" "$1" "$2" "${3:-(..T..)}" "${4:-(..F..)}"; }
t1='  19680606 000000'

{ echo '$ ww3_prep variant'; echo "  'LEV' 'AI' F T"; echo "$t1"; unit 2 1; arrays u; } > "$v/lev.inp"
{ echo '$ ww3_prep variant'; echo "  'WNS' 'AI' F T"; echo "$t1"; unit 2 1; arrays u v u; } > "$v/wns.inp"
{ echo '$ ww3_prep variant'; echo "  'TAU' 'AI' F T"; echo "$t1"; unit 2 1; arrays u v; } > "$v/tau.inp"
{ echo '$ ww3_prep variant'; echo "  'RHO' 'AI' F T"; echo "$t1"; unit 2 1; arrays u v; } > "$v/rho.inp"
{ echo '$ ww3_prep variant'; echo "  'ISI' 'AI' F T"; echo "$t1"; unit 2 1; arrays u v; } > "$v/isi.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' F F"; echo "$t1"; unit 2 1; arrays u v; } > "$v/nohdr.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' F T"; echo "$t1"; unit 1 1; rows u v; } > "$v/idla1.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' F T"; echo "$t1"; unit 4 1; rows u v; } > "$v/idla4.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' T T"; unit 2 1; echo "$t1"; arrays u v; echo '  19680606 030000'; arrays v u; echo; echo '  19680607 000000'; arrays u u; } > "$v/times.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' F T"; echo "$t1"; unit 1 2 '(I8,1X,I6)' '(1000(F8.2))'; frows u 1 '%8.2f'; frows v 1 '%8.2f'; } > "$v/fmtf.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' F T"; echo "$t1"; unit 1 2 '(I8,1X,I6)' '(43F8.2)'; frows u 100 '%8d'; frows v 100 '%8d'; } > "$v/fmtimp.inp"
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' T T"; unit 1 2 '(I8,1X,I6)' '(1000E12.4)'; echo '19680606 000000'; frows u 1 '%12.4E'; frows v 1 '%12.4E'
  echo '19680606 060000'; frows v 1 '%12.4E'; frows u 1 '%12.4E'; } > "$v/fmte.inp"
# A comma ends a fixed-width numeric field and is consumed with it: the next field starts after the comma.
# Every decimal point of the data is a comma here, so each field is cut at its comma, and the columns shift.
sed '6,$ s/\./,/g' "$v/fmtf.inp" > "$v/comma.inp"
# A comma in an integer field: the time 19680,06 is the integer 19680 and the next field starts after the comma.
{ echo '$ ww3_prep variant'; echo "  'WND' 'AI' T T"; unit 1 2 '(I8,1X,I6)' '(1000(F8.2))'; echo '19680,06 000000'; frows u 1 '%8.2f'; frows v 1 '%8.2f'; } > "$v/commai.inp"
# The input types that were refused before: LL on the model's own grid, and DAT.
{ echo '$ ww3_prep variant'; echo "  'WND' 'LL' F T"; echo "$t1"; echo "  -525.E3 525.E3 43 -525.E3 525.E3 43"; unit 1 1; rows u v; } > "$v/ll_same.inp"
{ echo '$ ww3_prep variant'; echo "  'DAT' 'AI' F T"; echo "$t1"; echo "  0 3 -999."; unit 1 1; echo "  2"; echo "  1. 2. 3."; echo "  4. 5. 6."; } > "$v/dat_one.inp"

bad=0
for f in lev wns tau rho isi nohdr idla1 idla4 times fmtf fmtimp fmte comma commai ll_same dat_one; do
  PREP_INP="prep/variants/$f.inp" PREP_TAG="v_$f" bash scripts/prep_pair.sh ww3_ts3 ww3_prep_wind.inp || bad=1
done
exit "$bad"
