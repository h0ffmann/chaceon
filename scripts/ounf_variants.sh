#!/usr/bin/env bash
# Runs ounf_pair.sh on a set of cases with each namelist variant of that script, and prints a summary.
# Usage: ounf_variants.sh [case ...]. Without arguments it takes the cases below, which together request every
# field group the cases of work/pair reach. The log of every run is work/ounf/variants.log.
set -euo pipefail
cd "$(dirname "$0")/.."
cases=("$@")
[ ${#cases[@]} -gt 0 ] || cases=(ww3_tp1.6 ww3_tp1.7 ww3_tp1.8 ww3_tp2.4_curv ww3_tp2.15 ww3_tp2.3 ww3_tp1.3 default_2500m)
variants=(type2 type3 type4 nc4 nc4type2 nc4type4 sep sepnc4 split0 split4 split8 split10 mag noval nomap secs int64 epoch prefix subset part)
log=work/ounf/variants.log
: > "$log"
ok=0
refused=0
bad=0
for c in "${cases[@]}"; do
  for v in "${variants[@]}"; do
    rc=0
    bash scripts/ounf_pair.sh "$c" "$v" >> "$log" 2>&1 || rc=$?
    case "$rc" in
      0) ok=$((ok + 1)) ;;
      3) refused=$((refused + 1)) ;;
      *) bad=$((bad + 1)); echo "NOT identical: $c $v" ;;
    esac
  done
done
echo "ounf-variants: $ok identical, $refused refused, $bad not identical, of $((ok + refused + bad)) runs; log in $log"
[ "$bad" -eq 0 ]
