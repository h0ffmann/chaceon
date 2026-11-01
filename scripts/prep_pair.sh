#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_prep: the reference runs ww3_grid and ww3_prep on a regtest's input,
# Rust's ww3_prep reads the reference's mod_def.ww3 and the same ww3_prep input, and cmp judges the field file
# it writes (current.ww3, wind.ww3, ice.ww3, ...). The grid is the reference's, so a case whose ww3_grid
# needs a feature the Rust ww3_grid does not have still tests ww3_prep.
# Usage: prep_pair.sh <regtest> <ww3_prep input> [nml-suffix], for example prep_pair.sh ww3_tp1.6 ww3_prep_curr.inp.
# PREP_INP names a replacement input file under work/ (a variant), PREP_TAG a suffix for the work directory
# (a fault is a source edit run with PREP_TAG naming it). The output Rust is to write is not there before it runs.
# Exit 0 only when every file the reference wrote is byte-identical; 3 when Rust refuses the input (status 64)
# although the reference accepts it, 2 when the reference itself fails.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
prep="$2"
suffix="${3:-}"
base="${prep%.inp}"
tag="${case_name}_${base#ww3_prep_}${suffix:+_$suffix}${PREP_TAG:+_$PREP_TAG}"
d="work/prep/$tag"
mkdir -p "$d"
find "$d" -mindepth 1 -delete
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" \
  -v "$PWD/WW3/regtests/$case_name/input:/ww3_input:ro" \
  -e TAG="$tag" -e SUFFIX="$suffix" -e PREP="$prep" -e INP="${PREP_INP:-}" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    cp "../input/ww3_grid${SUFFIX:+_$SUFFIX}.nml" ww3_grid.nml
    if [ -n "$INP" ]; then cp "/work/$INP" ww3_prep.inp; else cp "../input/$PREP" ww3_prep.inp; fi
    ww3_grid > grid.log 2>&1
    rc=0
    ww3_prep > prep.log 2>&1 || rc=$?
    d=/work/prep/$TAG
    mkdir -p $d/work
    cp -R ../input $d/input
    cp mod_def.ww3 ww3_prep.inp $d/work/
    cat prep.log > $d/prep.reference.log
    echo $rc > $d/prep.reference.status
    for f in level current wind ice ice1 ice2 ice3 ice4 ice5 momentum density muddens mudthk mudvisc; do
      if [ -f $f.ww3 ]; then cat $f.ww3 > $d/$f.reference; fi
    done'
if [ "$(cat "$d/prep.reference.status")" != 0 ]; then
  echo "$tag: the reference ww3_prep failed (status $(cat "$d/prep.reference.status")): $(grep -E 'ERROR|\*\*\*' -A2 "$d/prep.reference.log" | tr '\n' ' ' | cut -c1-200)"
  exit 2
fi
rm -f "$d"/work/*.ww3.rust
rust_rc=0
bash scripts/rust_in_ref.sh ww3_prep "/work/prep/$tag/work" > "$d/prep.rust.log" 2>&1 || rust_rc=$?
if [ "$rust_rc" = 64 ]; then
  echo "$tag: REFUSED by Rust, the reference accepts it: $(grep 'not implemented' "$d/prep.rust.log" | head -1)"
  exit 3
fi
branches=$(grep '^ww3_prep:' "$d/prep.rust.log" | head -1 || true)
bad=0
n=0
for ref in "$d"/*.reference; do
  [ -f "$ref" ] || continue
  n=$((n + 1))
  name=$(basename "$ref" .reference)
  ours="$d/work/$name.ww3"
  bytes=$(wc -c < "$ref" | tr -d ' ')
  if [ -f "$ours" ] && cmp -s "$ref" "$ours"; then
    echo "$tag $name.ww3: identical, $bytes bytes; ${branches:-no log line}"
  elif [ -f "$ours" ]; then
    echo "$tag $name.ww3: DIFFERENT, $(cmp -l "$ref" "$ours" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes (Rust wrote $(wc -c < "$ours" | tr -d ' ')); ${branches:-no log line}"
    bad=1
  else
    echo "$tag $name.ww3: NOT WRITTEN by Rust ($(grep -E 'panicked|rror|not implemented' "$d/prep.rust.log" | head -1))"
    bad=1
  fi
done
if [ "$n" = 0 ]; then echo "$tag: the reference wrote no field file"; bad=1; fi
exit "$bad"
