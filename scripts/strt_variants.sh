#!/usr/bin/env bash
# ww3_strt start type 2 (JONSWAP spectrum, Gaussian in space): the regtests' own inputs (ww3_tp2.11 on a
# rectilinear spherical grid, ww3_tp2.5 on a curvilinear one), and variants of an input on ww3_tp2.2's grid (193 by
# 93, 17 381 sea points) and on ww3_tp2.4's curvilinear one, that reach the branches those two leave out: every
# default, a gamma below 1 and above, a negative and a large mean direction, a negative SIX (the distance form),
# a peak frequency below and above the spectrum, a SIX and SIY under the grid's floor, a spatial scale so small
# that most points fall beyond 40 and sigmas that move the peak. ww3_tp2.11's restart is 298 MB, so its two
# files are deleted after the comparison.
# The inputs are written to regtests/strt/input and staged in work/strt/variants for strt_pair.sh.
# Exit 0 when every restart.ww3 is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=C
v=regtests/strt/input
w=work/strt/variants
mkdir -p "$v" "$w"
# type 2: ALFA FP THM GAMMA SIGA SIGB XM SIX YM SIY
mk() { printf '$ ww3_strt variant made by scripts/strt_variants.sh\n  2\n  %s\n' "$2" > "$v/ww3_strt_$1.inp"; cp "$v/ww3_strt_$1.inp" "$w/$1.inp"; }
mk dflt     '0. 0. 270. 1. 0. 0. 90. 10. 0. 10.'
mk dfltg    '0. 0. 270. 3.3 0. 0. 90. 10. 0. 10.'
mk gamma0   '0.0081 0.1 270. 0.5 0.07 0.09 90. 10. 0. 10.'
mk gamma3   '0.0081 0.1 270. 3.3 0.07 0.09 90. 10. 0. 10.'
mk thmneg   '0.0081 0.1 -123.456 3.3 0.07 0.09 90. 10. 0. 10.'
mk thmbig   '0.0081 0.1 1000.123 3.3 0.07 0.09 90. 10. 0. 10.'
mk thm45    '0.0081 0.1 45. 3.3 0.07 0.09 90. 10. 0. 10.'
mk nosix    '0.0081 0.1 270. 3.3 0.07 0.09 90. -1. 0. 10.'
mk fplow    '0.0081 0.001 270. 3.3 0.07 0.09 90. 10. 0. 10.'
mk fpmid    '0.0081 0.06 270. 3.3 0.07 0.09 90. 10. 0. 10.'
mk fphigh   '0.0081 3.0 270. 3.3 0.07 0.09 90. 10. 0. 10.'
mk floors   '0.0081 0.1 270. 3.3 0.07 0.09 90. 0.0001 0. 0.0001'
mk narrow   '0.0081 0.1 270. 3.3 0.07 0.09 90. 0.2 0. 0.2'
mk sigs     '0.0081 0.1 270. 3.3 0.02 0.3 90. 10. 0. 10.'
# The curvilinear grid of ww3_tp2.4 sits near 214 degrees east and 42 north.
mk c_dflt   '0. 0. 270. 1. 0. 0. 214. 2. 42.5 1.'
mk c_gamma3 '0.0081 0.06 150. 3.3 0.07 0.09 214. 2. 42.5 1.'
mk c_nosix  '0.0081 0.06 150. 1. 0.07 0.09 214. -1.2206 42.5 0.8993'
mk c_floors '0.0081 0.06 150. 3.3 0.07 0.09 214. 0.0001 42.5 0.0001'
cases=(dflt dfltg gamma0 gamma3 thmneg thmbig thm45 nosix fplow fpmid fphigh floors narrow sigs)
bad=0
bash scripts/strt_pair.sh ww3_tp2.5 || bad=1
bash scripts/strt_pair.sh ww3_tp2.11 || bad=1
rm -f work/strt/ww3_tp2.11/restart.reference work/strt/ww3_tp2.11/restart.ww3
for c in "${cases[@]}"; do
  STRT_INP="strt/variants/$c.inp" STRT_TAG="v_$c" bash scripts/strt_pair.sh ww3_tp2.2 || bad=1
done
# The curvilinear grid: the variants that depend on HPFAC and HQFAC.
for c in c_dflt c_gamma3 c_nosix c_floors; do
  STRT_INP="strt/variants/$c.inp" STRT_TAG="v_$c" bash scripts/strt_pair.sh ww3_tp2.4 curv || bad=1
done
exit "$bad"
