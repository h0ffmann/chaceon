#!/usr/bin/env bash
# Every regtest ww3_grid is judged on, through grid_pair.sh: one line per case, bytes and verdict.
set -euo pipefail
cd "$(dirname "$0")"
bad=0
run() { bash grid_pair.sh "$@" || bad=1; }
run ww3_ts1
run ww3_ts1 ST4_T700
for v in t700 filipot strain circle noshelter flatswell sharp romero2; do
  GRID_VARIANT=$v run ww3_ts1
done
GRID_DEPTH=50 run ww3_ts1
GRID_DEPTH=8 GRID_VARIANT=t700 run ww3_ts1
for c in 1.1 1.2 1.3 1.4 1.5 1.6 1.7 1.8 1.9 1.10 2.1 2.2 2.3 2.4 2.15; do
  run "ww3_tp$c"
done
run ww3_tp2.4 curv
exit "$bad"
