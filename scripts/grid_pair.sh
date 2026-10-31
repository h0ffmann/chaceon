#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_grid: the reference runs ww3_grid on a regtest's input, Rust's ww3_grid
# runs on the same input and namelist, and cmp judges mod_def.ww3.
# Usage: grid_pair.sh <regtest> [nml-suffix]. For ww3_ts1, GRID_VARIANT names a namelist variant of
# scripts/ts1_variants.sh and GRID_DEPTH replaces the depth (m). GRID_TAG adds a suffix to the work directory.
# Rust's mod_def.ww3 is deleted before it runs; a fault is a source edit run with GRID_TAG naming it.
# Exit 0 only when mod_def.ww3 is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
suffix="${2:-}"
variant="${GRID_VARIANT:-default}"
# shellcheck source=scripts/ts1_variants.sh
source scripts/ts1_variants.sh
depth="${GRID_DEPTH:-}"
tag="${case_name}${suffix:+_$suffix}${GRID_VARIANT:+_$variant}${depth:+_${depth}m}${GRID_TAG:+_$GRID_TAG}"
mkdir -p "work/grid/$tag"
find "work/grid/$tag" -mindepth 1 -delete
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" \
  -v "$PWD/WW3/regtests/$case_name/input:/ww3_input:ro" \
  -e TAG="$tag" -e SUFFIX="$suffix" -e SDS4="$sds4" -e DEPTH="$depth" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    cp "../input/ww3_grid${SUFFIX:+_$SUFFIX}.nml" ww3_grid.nml
    if [ -n "$DEPTH" ]; then sed -i "s/DEPTH%SF        =  -2500\./DEPTH%SF        =  -$DEPTH./" ww3_grid.nml; fi
    if [ -n "$SDS4" ]; then sed -i "/END OF NAMELISTS/i $SDS4" ../input/namelists_HOMOGENEOUS.nml; fi
    ww3_grid > grid.log 2>&1
    d=/work/grid/$TAG
    mkdir -p $d/work
    cp -R ../input $d/input
    cp ww3_grid.nml $d/work/
    cat mod_def.ww3 > $d/mod_def.reference
    cat grid.log > $d/grid.reference.log'
rm -f "work/grid/$tag/work/mod_def.ww3"
bash scripts/rust_in_ref.sh ww3_grid "/work/grid/$tag/work" > "work/grid/$tag/grid.rust.log" 2>&1 || true
branches=$(grep '^ww3_grid: reached' "work/grid/$tag/grid.rust.log" | sed 's/^ww3_grid: reached: //' || true)
ref="work/grid/$tag/mod_def.reference"
ours="work/grid/$tag/work/mod_def.ww3"
bytes=$(wc -c < "$ref" | tr -d ' ')
if [ -f "$ours" ] && cmp -s "$ref" "$ours"; then
  echo "$tag mod_def.ww3: identical, $bytes bytes; ${branches:-no branch line}"
  exit 0
fi
if [ -f "$ours" ]; then
  echo "$tag mod_def.ww3: DIFFERENT, $(cmp -l "$ref" "$ours" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes (Rust wrote $(wc -c < "$ours" | tr -d ' ')); ${branches:-no branch line}"
else
  echo "$tag mod_def.ww3: NOT WRITTEN by Rust ($(grep -E 'panicked|rror|refus' "work/grid/$tag/grid.rust.log" | head -1))"
fi
exit 1
