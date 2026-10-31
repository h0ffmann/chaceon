#!/usr/bin/env bash
# The host verdict of ADR-0005 on a propagation regtest: the reference and Rust run the same case, and
# cmp judges out_pnt.ww3 and a restart at the last time step, which holds every sea point's spectrum.
# Usage: tp_pair.sh <regtest> [nml-suffix] [extra ww3_grid lines]. Rust reads the mod_def.ww3 and
# restart.ww3 the reference's preprocessors wrote. Exit 0 only when every file is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
suffix="${2:-}"
tag="${case_name}${suffix:+_$suffix}${TP_TAG:+_$TP_TAG}"
mkdir -p "work/pair/$tag"
rm -f "work/pair/$tag"/*
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" \
  -v "$PWD/WW3/regtests/$case_name/input:/ww3_input:ro" \
  -e TAG="$tag" -e SUFFIX="$suffix" -e RUN_EXTRA="${TP_RUN:-}" -e SHEL_EXTRA="${TP_SHEL:-}" -e STOP_AT="${TP_STOP:-}" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    grid=../input/ww3_grid${SUFFIX:+_$SUFFIX}.nml
    cp "$grid" ww3_grid.nml
    cp ../input/ww3_strt.inp ../input/ww3_shel.nml .
    if [ -n "$RUN_EXTRA" ]; then
      sed -i "/^&RUN_NML/a $RUN_EXTRA" ww3_grid.nml
    fi
    if [ -n "$STOP_AT" ]; then
      sed -i "/DOMAIN%STOP/d; /^&DOMAIN_NML/a \  DOMAIN%STOP = \x27$STOP_AT\x27" ww3_shel.nml
    fi
    stop=$(sed -n "s/.*DOMAIN%STOP *= *.\([0-9]* [0-9]*\).*/\1/p" ww3_shel.nml)
    stop=${stop:-19680607 000000}
    sed -i "/^&OUTPUT_DATE_NML/a \  DATE%RESTART = \x27$stop\x27 \x273600\x27 \x27$stop\x27" ww3_shel.nml
    if [ -n "$SHEL_EXTRA" ]; then
      sed -i "$SHEL_EXTRA" ww3_shel.nml
    fi
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    ww3_shel > shel.log 2>&1
    d=/work/pair/$TAG
    cat mod_def.ww3 > $d/mod_def.ww3
    cat restart.ww3 > $d/restart.ww3
    sed "s#\.\./input/##" ww3_shel.nml > $d/ww3_shel.nml
    if [ -f ../input/points.list ]; then cp ../input/points.list $d/; fi
    if [ -f out_pnt.ww3 ]; then mv out_pnt.ww3 $d/out_pnt.reference; fi
    mv restart001.ww3 $d/restart001.reference'
bash scripts/rust_in_ref.sh ww3_shel "/work/pair/$tag" 2>&1 | tail -2 | grep -E 'panicked|rror' || true
bad=0
for f in out_pnt restart001; do
  ref="work/pair/$tag/$f.reference"
  ours="work/pair/$tag/$f.ww3"
  [ -f "$ref" ] || continue
  bytes=$(wc -c < "$ref" | tr -d ' ')
  if cmp -s "$ref" "$ours"; then
    echo "$tag $f: identical, $bytes bytes"
  else
    echo "$tag $f: DIFFERENT, $(cmp -l "$ref" "$ours" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes"
    bad=1
  fi
done
exit "$bad"
