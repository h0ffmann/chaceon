#!/usr/bin/env bash
# Runs scripts/outp_pair.sh on every ww3_outp*.inp of WW3's regtests whose case has an out_pnt.reference under
# work/pair/ (ww3_ts1 is the pair directory default_2500m), plus the inputs written here under
# regtests/outpf_inputs/ (a first line "$ pair: <case>" names the pair case).
# Prints one line per run; exit 0 only when every run is identical or refused (refusals are listed).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
bad=0
for f in WW3/regtests/*/input/ww3_outp*.inp; do
  reg=$(echo "$f" | cut -d/ -f3)
  case "$reg" in ww3_ts1) pair=default_2500m;; *) pair=$reg;; esac
  [ -f "work/pair/$pair/out_pnt.reference" ] || continue
  bash scripts/outp_pair.sh "$pair" "${f#WW3/regtests/}" "${reg}__$(basename "$f" .inp)" || { rc=$?; [ "$rc" = 2 ] || bad=1; }
done
for f in regtests/outpf_inputs/ww3_outp*.inp; do
  [ -f "$f" ] || continue
  pair=$(sed -n 's/^\$ pair: *//p' "$f" | head -1)
  bash scripts/outp_pair.sh "$pair" "$(basename "$f")" "$(basename "$f" .inp)" || { rc=$?; [ "$rc" = 2 ] || bad=1; }
done
exit $bad
