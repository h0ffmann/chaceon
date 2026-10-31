#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_ts1: the reference and Rust run the same case, and cmp judges the
# output file. Usage: ts1_pair.sh <variant> [depth]. Rust reads the mod_def.ww3 the reference wrote.
# Exit 0 only when out_pnt.ww3 is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
variant="$1"
depth="${2:-2500}"
# shellcheck source=scripts/ts1_variants.sh
source scripts/ts1_variants.sh
tag="${variant}_${depth}m"
mkdir -p "work/pair/$tag"
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" \
  -v "$PWD/WW3/regtests/ww3_ts1/input:/ww3_input:ro" \
  -e DEPTH="$depth" -e SDS4="$sds4" -e WIND="$wind" -e TAG="$tag" \
  --entrypoint sh chaceon-ref:switch_chaceon_shrd -c "
    set -e
    mkdir -p /run/ts1/work && cp -R /ww3_input /run/ts1/input && cd /run/ts1/work
    cp ../input/ww3_grid.nml ../input/ww3_strt.inp ../input/ww3_shel.nml .
    sed -i \"s/DEPTH%SF        =  -2500\\\\./DEPTH%SF        =  -\$DEPTH./\" ww3_grid.nml
    sed -i \"s/VALUE1      = 12\\\\./VALUE1      = \$WIND./\" ww3_shel.nml
    if [ -n \"\$SDS4\" ]; then
      sed -i \"/END OF NAMELISTS/i \$SDS4\" ../input/namelists_HOMOGENEOUS.nml
    fi
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    ww3_shel > shel.log 2>&1
    d=/work/pair/\$TAG
    cp mod_def.ww3 ww3_shel.nml \$d/
    cp ../input/points.list \$d/
    sed -i \"s#../input/points.list#points.list#\" \$d/ww3_shel.nml
    cp restart.ww3 \$d/
    mv out_pnt.ww3 \$d/out_pnt.reference
    rm -f \$d/out_pnt.ww3"
bash scripts/rust_in_ref.sh ww3_shel "/work/pair/$tag" 2>&1 | tail -1 | grep -E 'panicked|rror' || true
ref="work/pair/$tag/out_pnt.reference"
ours="work/pair/$tag/out_pnt.ww3"
bytes=$(wc -c < "$ref" | tr -d ' ')
if cmp -s "$ref" "$ours"; then
  echo "$tag: identical, $bytes bytes"
else
  echo "$tag: DIFFERENT, $(cmp -l "$ref" "$ours" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes"
  exit 1
fi
