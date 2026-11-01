#!/usr/bin/env bash
# Every field the grid of a case can hold, through ww3_ounf: ounf_extra.sh makes an out_grd.ww3 with the full field list
# (the reference's ww3_shel, on the pair case's own inputs) and ounf_pair.sh compares the NetCDF files, plain and with
# namelist variants. The log is work/ounf/extra.log. Usage: ounf_extra_all.sh [case ...] (default: the cases below). Every run takes every Nth output time (stride) to keep it short..
set -euo pipefail
cd "$(dirname "$0")/.."
cases=("$@")
[ ${#cases[@]} -gt 0 ] || cases=(ww3_tp1.7 ww3_tp2.3 ww3_tp2.4_curv default_2500m ww3_tp1.8)
list=$(cat work/ounf/all.list)
variants=(stride type2+stride nc4type4+stride sep+stride part+stride partmany+stride mag+noval+stride secs+stride nomap+subset+stride)
log=work/ounf/extra.log
: > "$log"
ok=0
bad=0
for c in "${cases[@]}"; do
  [ -f "work/ounf/src_all_$c/out_grd.reference" ] || bash scripts/ounf_extra.sh "$c" "all_$c" "$list" >> "$log" 2>&1
  for v in "${variants[@]}"; do
    rc=0
    OUNF_SRC="work/ounf/src_all_$c" OUNF_LIST="$list" bash scripts/ounf_pair.sh "$c" "$v" >> "$log" 2>&1 || rc=$?
    if [ "$rc" -eq 0 ]; then ok=$((ok + 1)); else bad=$((bad + 1)); echo "NOT identical: $c ${v:-plain}"; fi
  done
done
echo "ounf-extra: $ok identical, $bad not identical; log in $log"
[ "$bad" -eq 0 ]
