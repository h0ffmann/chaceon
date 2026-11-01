#!/usr/bin/env bash
# Every ww3_prep input of a single-grid regtest that runs on switch_chaceon_shrd, through prep_pair.sh: one line
# per output file, bytes and verdict. The mww3_test_* inputs need ww3_multi and are not here.
set -euo pipefail
cd "$(dirname "$0")"
bad=0
run() { bash prep_pair.sh "$@" || bad=1; }
run ww3_tp1.6 ww3_prep_curr.inp
for i in mudd mudt mudv; do run ww3_tbt1.1 "ww3_prep_$i.inp"; done
for i in conc mud1 mud2 mud3; do run ww3_tbt2.1 "ww3_prep_$i.inp"; done
for i in ice ice5; do run ww3_tic2.2 "ww3_prep_$i.inp"; done
for i in ice ice1 ice5; do run ww3_tic2.3 "ww3_prep_$i.inp" scat; done
run ww3_ts3 ww3_prep_wind.inp
exit "$bad"
