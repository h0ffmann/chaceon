#!/usr/bin/env bash
# Makes the input of ounp_pair.sh for a regtest that work/pair does not hold: the reference's ww3_grid, ww3_strt and
# ww3_shel run on the regtest's own input in the reference image (switch_chaceon_shrd), and mod_def.ww3 and
# out_pnt.reference are kept in work/ounp_src/<tag>. Nothing outside work/ounp_src is written.
# Usage: ounp_src.sh <regtest> [tag]. Then: OUNP_SRC=work/ounp_src/<tag> ounp_pair.sh <regtest> (see ounp_pair.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
reg="$1"
tag="${2:-$reg}"
d="work/ounp_src/$tag"
mkdir -p "$d"
find "$d" -mindepth 1 -delete
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" -v "$PWD/WW3/regtests/$reg/input:/ww3_input:ro" \
  -e TAG="$tag" --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -e
    mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
    cp ../input/ww3_grid.nml ../input/ww3_strt.inp ../input/ww3_shel.nml .
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    ww3_shel > shel.log 2>&1
    d=/work/ounp_src/$TAG
    cat mod_def.ww3 > $d/mod_def.ww3
    cat out_pnt.ww3 > $d/out_pnt.reference'
ls -l "$d"
