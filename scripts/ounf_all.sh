#!/usr/bin/env bash
# Runs ounf_pair.sh on every case in work/pair that has an out_grd.reference and a ww3_ounf.nml in its regtest input.
# Usage: ounf_all.sh [variant]. Prints one line per file and a summary; exit 0 only when every case is identical.
set -euo pipefail
cd "$(dirname "$0")/.."
variant="${1:-}"
bad=0
refused=0
total=0
failed=""
refusedlist=""
for d in work/pair/*/; do
  c=$(basename "$d")
  [ -f "$d/out_grd.reference" ] || continue
  total=$((total + 1))
  rc=0
  bash scripts/ounf_pair.sh "$c" ${variant:+"$variant"} || rc=$?
  if [ "$rc" -eq 3 ]; then
    refused=$((refused + 1))
    refusedlist="$refusedlist $c"
  elif [ "$rc" -ne 0 ]; then
    bad=$((bad + 1))
    failed="$failed $c"
  fi
done
echo "ounf-all${variant:+ ($variant)}: $((total - bad - refused)) of $total cases identical, $refused refused${refusedlist:+ ($refusedlist )}${failed:+; NOT identical:$failed}"
[ "$bad" -eq 0 ]
