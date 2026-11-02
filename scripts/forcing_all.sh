#!/usr/bin/env bash
# ww3_shel with file and homogeneous winds, water levels and ice, by cmp against the reference on out_grd.ww3, out_pnt.ww3
# and the last restart: forcing_gen.py's variants on ww3_tp2.15's grid and physics, and the regtests that take them.
# Exit 0 only when every file of every run is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/forcing_gen.py
bad=0
f=work/forcing
# Each overlay is the generated inputs plus, for ice, a namelist that separates CICE0 from CICEN.
overlay() { d="$f/ov_$1"; rm -rf "$d"; mkdir -p "$d"; cp "$f"/input/* "$d/"; if [ -n "${2:-}" ]; then cp "$f/nml_$2"/* "$d/"; fi; echo "$d"; }
run() { # tag sed prep nml [extra environment as NAME=value]
  local tag="$1" sed="$2" prep="$3" nml="${4:-}"; shift 4
  env TP_TAG="$tag" TP_SHEL="$f/$sed.sed" TP_PREP="$prep" TP_OVERLAY="$(overlay "$tag" "$nml")" "$@" bash scripts/tp_pair.sh ww3_tp2.15 || bad=1
}
run wind wind ww3_prep_wind.inp ""
run wns wind ww3_prep_wns.inp ""
run level level "ww3_prep_wind.inp ww3_prep_lev.inp" ""
run levsmall level "ww3_prep_wind.inp ww3_prep_levsmall.inp" ""
run levlate level "ww3_prep_wind.inp ww3_prep_levlate.inp" ""
run levshort level "ww3_prep_wind.inp ww3_prep_levshort.inp" ""
run ice ice "ww3_prep_wind.inp ww3_prep_ice.inp" ""
run icelin ice "ww3_prep_wind.inp ww3_prep_ice.inp" linear
run iceexp ice "ww3_prep_wind.inp ww3_prep_ice.inp" expo
run icelate ice "ww3_prep_wind.inp ww3_prep_icelate.inp" linear
run both both "ww3_prep_wind.inp ww3_prep_lev.inp ww3_prep_ice.inp" linear
run homog homog "ww3_prep_wind.inp" linear
# A restart in mid-run carries TLEV, TICE, WLV, ICE and the dry and ice maps.
run fleg1 both "ww3_prep_wind.inp ww3_prep_lev.inp ww3_prep_ice.inp" linear TP_STOP='20140310 030000'
run fleg2 both "ww3_prep_wind.inp ww3_prep_lev.inp ww3_prep_ice.inp" linear TP_START='20140310 030000' TP_RESUME=pair/ww3_tp2.15_fleg1/restart001.reference
# The regtests: their ICE_PARAM fields ask for IC2 and IS2, which this switch has not, so the concentration alone runs.
TP_SHEL=scripts/no_ice_param.sed TP_PREP=ww3_prep_ice.inp bash scripts/tp_pair.sh ww3_tic2.2 || bad=1
TP_SHEL=scripts/ts3_no_mov.sed TP_PREP=ww3_prep_wind.inp TP_OVERLAY=regtests/forcing/ts3 bash scripts/tp_pair.sh ww3_ts3 || bad=1
exit "$bad"
