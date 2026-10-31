#!/usr/bin/env bash
# Every regtest the host driver implements, through tp_pair.sh. A case that needs a change to run here says
# so in its line: ww3_tp2.15's wind.nc is not in the repository, and ww3_tp2.2's track output is not ported.
set -euo pipefail
cd "$(dirname "$0")"
bad=0
for c in 1.1 1.2 1.3 1.4 1.5 1.7 1.8 1.9 1.10 2.1 2.3 2.4; do
  bash tp_pair.sh "ww3_tp$c" || bad=1
done
bash tp_pair.sh ww3_tp2.4 curv || bad=1
TP_PREP=ww3_prep_curr.inp bash tp_pair.sh ww3_tp1.6 || bad=1
TP_SHEL=scripts/tp22_no_track.sed bash tp_pair.sh ww3_tp2.2 || bad=1
TP_SHEL=scripts/tp215_wind.sed bash tp_pair.sh ww3_tp2.15 || bad=1
# A full restart: the second leg starts from the first leg's Fortran restart.
TP_TAG=leg1 TP_SHEL=scripts/tp215_wind.sed TP_STOP='20140310 020000' bash tp_pair.sh ww3_tp2.15 || bad=1
TP_TAG=leg2 TP_SHEL=scripts/tp215_wind.sed TP_START='20140310 020000' \
  TP_RESUME=pair/ww3_tp2.15_leg1/restart001.reference bash tp_pair.sh ww3_tp2.15 || bad=1
exit "$bad"
