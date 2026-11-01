#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_prep: the reference runs ww3_grid and ww3_prep on a regtest's input,
# Rust's ww3_prep reads the reference's mod_def.ww3 and the same ww3_prep input, and cmp judges the field file
# it writes (current.ww3, wind.ww3, ice.ww3, ...). The grid is the reference's, so a case whose ww3_grid
# needs a feature the Rust ww3_grid does not have still tests ww3_prep.
# Usage: prep_pair.sh <regtest> <ww3_prep input> [nml-suffix], for example prep_pair.sh ww3_tp1.6 ww3_prep_curr.inp.
# PREP_INP names a replacement input file under work/ (a variant), PREP_TAG a suffix for the work directory
# (a fault is a source edit run with PREP_TAG naming it). The output Rust is to write is not there before it runs.
# PREP_OVERLAY names a directory whose files are copied over the regtest's input (a custom grid namelist, data
# files); fort.<n> files in it are copied to the working directory, where an unnamed OPEN finds them.
# PREP_ERROR=1 is for an input the reference stops on: Rust must stop with the same exit status, and whatever
# field file the reference had written when it stopped must be byte-identical too.
# Exit 0 only when every file the reference wrote is byte-identical; 3 when Rust refuses the input (status 64)
# although the reference accepts it, 2 when the reference itself fails (unless PREP_ERROR is set).
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
  ${PREP_OVERLAY:+-v "$PWD/$PREP_OVERLAY:/overlay:ro"} \
  -e TAG="$tag" -e SUFFIX="$suffix" -e PREP="$prep" -e INP="${PREP_INP:-}" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    if [ -d /overlay ]; then cp -R /overlay/. /run/c/input/; cp /run/c/input/fort.* . 2>/dev/null || true; fi
    cp "../input/ww3_grid${SUFFIX:+_$SUFFIX}.nml" ww3_grid.nml
    if [ -n "$INP" ]; then cp "/work/$INP" ww3_prep.inp; else cp "../input/$PREP" ww3_prep.inp; fi
    ww3_grid > grid.log 2>&1
    rc=0
    ww3_prep > prep.log 2>&1 || rc=$?
    d=/work/prep/$TAG
    mkdir -p $d/work
    cp -R ../input $d/input
    cp mod_def.ww3 ww3_prep.inp $d/work/
    cp fort.* $d/work/ 2>/dev/null || true
    cat prep.log > $d/prep.reference.log
    echo $rc > $d/prep.reference.status
    for f in level current wind ice ice1 ice2 ice3 ice4 ice5 momentum density muddens mudthk mudvisc data0 data1 data2; do
      if [ -f $f.ww3 ]; then cat $f.ww3 > $d/$f.reference; fi
    done'
ref_status=$(cat "$d/prep.reference.status")
if [ "$ref_status" != 0 ] && [ -z "${PREP_ERROR:-}" ]; then
  echo "$tag: the reference ww3_prep failed (status $(cat "$d/prep.reference.status")): $(grep -E 'ERROR|\*\*\*' -A2 "$d/prep.reference.log" | tr '\n' ' ' | cut -c1-200)"
  exit 2
fi
rm -f "$d"/work/*.ww3.rust
rust_rc=0
bash scripts/rust_in_ref.sh ww3_prep "/work/prep/$tag/work" > "$d/prep.rust.log" 2>&1 || rust_rc=$?
if [ "$rust_rc" = 64 ] && [ "$ref_status" = 0 ]; then
  echo "$tag: REFUSED by Rust, the reference accepts it: $(grep 'not implemented' "$d/prep.rust.log" | head -1)"
  exit 3
fi
branches=$(grep '^ww3_prep:' "$d/prep.rust.log" | head -1 || true)
bad=0
n=0
if [ "$ref_status" != 0 ]; then
  if [ "$rust_rc" = "$ref_status" ] || [ "$rust_rc" = $(( (ref_status % 256 + 256) % 256 )) ]; then
    echo "$tag: both stop with status $ref_status"
  else
    echo "$tag: STATUS DIFFERS, reference $ref_status, Rust $rust_rc: $(grep -v '^$' "$d/prep.rust.log" | tail -2 | tr '\n' ' ' | cut -c1-160)"
    bad=1
  fi
elif [ "$rust_rc" != 0 ]; then
  echo "$tag: Rust stops with status $rust_rc where the reference does not: $(grep -v '^$' "$d/prep.rust.log" | tail -2 | tr '\n' ' ' | cut -c1-160)"
  bad=1
fi
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
if [ "$n" = 0 ] && [ "$ref_status" = 0 ]; then
  # The reference ended normally without a field file (an empty binary input): Rust must write none either.
  wrote=""
  for f in level current wind ice ice1 ice2 ice3 ice4 ice5 momentum density muddens mudthk mudvisc data0 data1 data2; do
    if [ -f "$d/work/$f.ww3" ]; then wrote="$wrote $f.ww3"; fi
  done
  if [ -n "$wrote" ]; then
    echo "$tag: the reference wrote no field file and Rust wrote$wrote"; bad=1
  else
    echo "$tag: neither the reference nor Rust wrote a field file; ${branches:-no log line}"
  fi
fi
exit "$bad"
