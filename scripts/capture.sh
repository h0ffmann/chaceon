#!/usr/bin/env bash
# Runs WW3's ww3_ts1 in the reference under gdb and records one routine.
# Usage: capture.sh <routine> <variant> [depth] [stride]. Writes work/capture/<routine>_<variant>_<depth>m.bin.
# CAPTURE_SWITCH picks the reference build: switch_chaceon_shrd by default, switch_chaceon_bt4 for sbt4
# and for every variant named bt4*, so that all captures of one tag come from one build.
# A variant only changes the case's namelists, never WW3: ww3_ts1 at 2500 m reaches one branch of
# W3SDS4, and the rest need other settings. calm, light and storm change the wind: at 12 m/s USTAR
# never reaches MetOcean's floors, and shallow water never reaches depth-limited breaking. poke writes rare inputs at the real routine's entry (sin4.py).
set -euo pipefail
cd "$(dirname "$0")/.."
routine="$1"
variant="$2"
depth="${3:-2500}"
stride="${4:-1}"
switch="${CAPTURE_SWITCH:-switch_chaceon_shrd}"
case "$routine/$variant" in sbt4/* | */bt4*) switch="${CAPTURE_SWITCH:-switch_chaceon_bt4}" ;; esac
case "$routine/$variant" in insin3/* | */st3*) switch="${CAPTURE_SWITCH:-switch_chaceon_st3}" ;; esac
# shellcheck source=scripts/ts1_variants.sh
source scripts/ts1_variants.sh
out="${routine}_${variant}_${depth}m.bin"
mkdir -p work/capture
docker run --rm --platform linux/arm64 --security-opt seccomp=unconfined --cap-add SYS_PTRACE \
  -v "$PWD/work:/work" -v "$PWD/regtests/capture:/capture:ro" \
  -v "$PWD/WW3/regtests/ww3_ts1/input:/ww3_input:ro" \
  -e CAPTURE_OUT="/work/capture/$out" -e CAPTURE_STRIDE="$stride" -e DEPTH="$depth" -e SDS4="$sds4" -e WIND="$wind" -e CAPTURE_POKE="$poke" -e CAPTURE_LIMIT="$limit" -e GRID="$grid" \
  --entrypoint sh "chaceon-ref-capture:$switch" -c "
    set -e
    mkdir -p /run/ts1/work && cp -R /ww3_input /run/ts1/input && cd /run/ts1/work
    cp ../input/ww3_grid.nml ../input/ww3_strt.inp ../input/ww3_shel.nml .
    sed -i \"s/DEPTH%SF        =  -2500\\\\./DEPTH%SF        =  -\$DEPTH./\" ww3_grid.nml
    grep -q \"DEPTH%SF        =  -\$DEPTH\\\\.\" ww3_grid.nml
    sed -i \"s/VALUE1      = 12\\./VALUE1      = \$WIND./\" ww3_shel.nml
    if [ -n \"\$GRID\" ]; then sed -i \"\$GRID\" ww3_grid.nml; fi
    if [ -n \"\$SDS4\" ]; then
      sed -i \"/END OF NAMELISTS/i \$SDS4\" ../input/namelists_HOMOGENEOUS.nml
    fi
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    gdb -q -nx -batch -x /capture/$routine.py --args ww3_shel 2>&1 | grep -E 'capture:|rror' | tail -3"
ls -l "work/capture/$out"
if [ -f "work/capture/$out.error" ]; then tail -4 "work/capture/$out.error"; exit 1; fi
