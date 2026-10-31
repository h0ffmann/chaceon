#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_strt: the reference runs ww3_grid and ww3_strt on a regtest's input,
# Rust's ww3_strt reads the same mod_def.ww3 and ww3_strt.inp, and cmp judges restart.ww3.
# Usage: strt_pair.sh <regtest> [nml-suffix]. STRT_INP names a replacement ww3_strt.inp under work/ (a fault
# or a variant), STRT_TAG a suffix for the work directory. Rust's restart.ww3 is deleted before it runs.
# Exit 0 only when restart.ww3 is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
suffix="${2:-}"
tag="${case_name}${suffix:+_$suffix}${STRT_TAG:+_$STRT_TAG}"
mkdir -p "work/strt/$tag"
find "work/strt/$tag" -maxdepth 1 -type f -delete
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" \
  -v "$PWD/WW3/regtests/$case_name/input:/ww3_input:ro" \
  -e TAG="$tag" -e SUFFIX="$suffix" -e INP="${STRT_INP:-}" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    cp "../input/ww3_grid${SUFFIX:+_$SUFFIX}.nml" ww3_grid.nml
    if [ -n "$INP" ]; then cp "/work/$INP" ww3_strt.inp; else cp ../input/ww3_strt.inp .; fi
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    d=/work/strt/$TAG
    cat mod_def.ww3 > $d/mod_def.ww3
    cat ww3_strt.inp > $d/ww3_strt.inp
    cat restart.ww3 > $d/restart.reference
    cat strt.log > $d/strt.reference.log'
rm -f "work/strt/$tag/restart.ww3"
bash scripts/rust_in_ref.sh ww3_strt "/work/strt/$tag" > "work/strt/$tag/strt.rust.log" 2>&1 || true
branches=$(grep '^ww3_strt:' "work/strt/$tag/strt.rust.log" || true)
ref="work/strt/$tag/restart.reference"
ours="work/strt/$tag/restart.ww3"
bytes=$(wc -c < "$ref" | tr -d ' ')
if [ -f "$ours" ] && cmp -s "$ref" "$ours"; then
  echo "$tag restart: identical, $bytes bytes; ${branches:-no branch line}"
  exit 0
fi
if [ -f "$ours" ]; then
  echo "$tag restart: DIFFERENT, $(cmp -l "$ref" "$ours" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes; ${branches:-no branch line}"
else
  echo "$tag restart: NOT WRITTEN by Rust ($(grep -E 'panicked|rror' "work/strt/$tag/strt.rust.log" | head -1))"
fi
exit 1
