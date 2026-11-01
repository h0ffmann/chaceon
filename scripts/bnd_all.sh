#!/usr/bin/env bash
# Every case judged for W3PART (partition fields of out_grd.ww3) and W3IOBC WRITE (nest files), each through bnd_pair.sh:
# the five partitioning methods of ww3_tpt1.1, ww3_tp2.15 with a changing wind, and ww3_tp2.3 with two boundary-output lines.
# Exit 0 only when every file is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
all='WND HS T01 DIR PHS PTP PLP PDIR PSPR PWS PDP PQP PPE PGW PSW PTM10 PT01 PT02 PEP TWS PNR'
bad=0
for p in 1 2 3 4 5; do
  TP_SHEL=scripts/tpt_dates.sed BND_SUFFIX="PTM$p" BND_FIELDS="$all" bash scripts/bnd_pair.sh ww3_tpt1.1 "tpt_PTM$p" || bad=1
done
BND_FIELDS='HS PHS PTP PLP PDIR PSPR PWS PDP PQP PPE PGW PSW PTM10 PT01 PT02 PEP TWS PNR' TP_SHEL=scripts/tp215_wind.sed \
  bash scripts/bnd_pair.sh ww3_tp2.15 p1_tp215 || bad=1
grid='&OUTBND_COUNT_NML
  OUTBND_COUNT%N_LINE   =  2
/
&OUTBND_LINE_NML
  OUTBND_LINE(1)           =  -300.E3 0. 100.E3 0. 5
  OUTBND_LINE(2)           =  -200.E3 -100.E3 50.E3 50.E3 -4
/'
BND_GRID="$grid" TP_SHEL=scripts/bnd_nest_out.sed bash scripts/bnd_pair.sh ww3_tp2.3 nest_tp23 || bad=1
for n in 01 02; do
  if cmp -s "work/bnd_nest_tp23/nest$n.reference" "work/bnd_nest_tp23/nest$n.ww3"; then
    echo "nest_tp23 nest$n: identical"
  else
    echo "nest_tp23 nest$n: DIFFERENT"; bad=1
  fi
done
exit "$bad"
