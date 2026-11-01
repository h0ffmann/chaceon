#!/usr/bin/env bash
# tp_pair.sh with two more knobs, for the partition fields of out_grd.ww3: the reference and Rust run the same regtest,
# and cmp judges out_grd.ww3, out_pnt.ww3 and the last restart. Usage: bnd_pair.sh <regtest> <tag>.
#   BND_SUFFIX  picks ww3_grid_<suffix>.nml, as the second argument of tp_pair.sh does
#   BND_GRID    text appended to ww3_grid.nml (OUTBND lines)
#   BND_FIELDS  replaces TYPE%FIELD%LIST of ww3_shel.nml
#   BND_MISC    keys added to &MISC of the regtest's namelists file (PTM = 2, ...), the same on both sides
#   TP_SHEL, TP_STOP, TP_PREP  as in tp_pair.sh
# Files go to work/bnd_<tag>/. Exit 0 only when every file is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
tag="$2"
d="work/bnd_$tag"
mkdir -p "$d"
find "$d" -maxdepth 1 -type f -delete
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" \
  -v "$PWD/WW3/regtests/$case_name/input:/ww3_input:ro" \
  -v "$PWD/${TP_SHEL:-scripts/bnd_pair.sh}:/ww3_sed:ro" \
  -e TAG="bnd_$tag" -e SUFFIX="${BND_SUFFIX:-}" -e FIELDS="${BND_FIELDS:-}" -e MISC="${BND_MISC:-}" -e GRIDADD="${BND_GRID:-}" -e SHEL_EXTRA="${TP_SHEL:-}" -e STOP_AT="${TP_STOP:-}" -e PREP="${TP_PREP:-}" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    cp "../input/ww3_grid${SUFFIX:+_$SUFFIX}.nml" ww3_grid.nml
    if [ -n "$GRIDADD" ]; then printf "%s\n" "$GRIDADD" >> ww3_grid.nml; fi
    cp ../input/ww3_strt.inp ../input/ww3_shel.nml .
    if [ -n "$MISC" ]; then
      for f in ../input/namelists*.nml; do
        if grep -q "^&MISC" "$f"; then sed -i "s#^&MISC #\&MISC $MISC, #" "$f"; else echo "&MISC $MISC /" >> "$f"; fi
      done
    fi
    if [ -n "$FIELDS" ]; then
      sed -i "s#^\( *TYPE%FIELD%LIST *= *\).*#\1\x27$FIELDS\x27#" ww3_shel.nml
    fi
    if [ -n "$STOP_AT" ]; then
      sed -i "/DOMAIN%STOP/d; /^&DOMAIN_NML/a \  DOMAIN%STOP = \x27$STOP_AT\x27" ww3_shel.nml
    fi
    stop=$(sed -n "s/.*DOMAIN%STOP *= *.\([0-9]* [0-9]*\).*/\1/p" ww3_shel.nml)
    stop=${stop:-19680607 000000}
    sed -i "/DATE%RESTART/d" ww3_shel.nml
    sed -i "/^&OUTPUT_DATE_NML/a \  DATE%RESTART = \x27$stop\x27 \x273600\x27 \x27$stop\x27" ww3_shel.nml
    if [ -n "$SHEL_EXTRA" ]; then
      sed -i -f /ww3_sed ww3_shel.nml
    fi
    ww3_grid > grid.log 2>&1
    if [ -n "$PREP" ]; then
      cp "../input/$PREP" ww3_prep.inp
      ww3_prep > prep.log 2>&1
    fi
    ww3_strt > strt.log 2>&1
    ww3_shel > shel.log 2>&1
    o=/work/$TAG
    cat mod_def.ww3 > $o/mod_def.ww3
    cat restart.ww3 > $o/restart.ww3
    sed "s#\.\./input/##" ww3_shel.nml > $o/ww3_shel.nml
    for f in current.ww3 wind.ww3; do if [ -f $f ]; then cat $f > $o/$f; fi; done
    if [ -f ../input/points.list ]; then cp ../input/points.list $o/; fi
    if [ -f out_pnt.ww3 ]; then mv out_pnt.ww3 $o/out_pnt.reference; fi
    if [ -f out_grd.ww3 ]; then cat out_grd.ww3 > $o/out_grd.reference; fi
    for f in nest*.ww3; do if [ -f "$f" ]; then cat "$f" > "$o/${f%.ww3}.reference"; fi; done
    mv restart001.ww3 $o/restart001.reference'
bash scripts/rust_in_ref.sh ww3_shel "/work/bnd_$tag" 2>&1 | tail -2 | grep -E 'panicked|rror' || true
bad=0
for f in out_grd out_pnt restart001 nest01; do
  ref="$d/$f.reference"
  ours="$d/$f.ww3"
  if [ "$f" = nest01 ]; then ours="$d/nest01.ww3"; fi
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
