#!/usr/bin/env bash
# Runs WW3's ww3_ts1 in the reference under gdb and writes work/capture/snl1_ts1_<depth>m.bin.
# Usage: capture_snl1.sh [stride] [depth-in-metres]. The case is 2500 m deep, where W3SNL1's EXP term
# underflows; a shallow depth, set in the case's namelist, exercises it.
set -euo pipefail
cd "$(dirname "$0")/.."
name=switch_chaceon_shrd
stride="${1:-50}"
depth="${2:-2500}"
out="snl1_ts1_${depth}m.bin"
mkdir -p work/capture
docker run --rm --platform linux/arm64 --security-opt seccomp=unconfined --cap-add SYS_PTRACE \
  -v "$PWD/work:/work" -v "$PWD/regtests/capture:/capture:ro" \
  -v "$PWD/WW3/regtests/ww3_ts1/input:/ww3_input:ro" \
  -e CAPTURE_OUT="/work/capture/$out" -e CAPTURE_STRIDE="$stride" -e DEPTH="$depth" \
  --entrypoint sh "chaceon-ref-capture:$name" -c '
    set -e
    mkdir -p /run/ts1/work && cp -R /ww3_input /run/ts1/input && cd /run/ts1/work
    cp ../input/ww3_grid.nml ../input/ww3_strt.inp ../input/ww3_shel.nml .
    sed -i "s/DEPTH%SF        =  -2500\./DEPTH%SF        =  -$DEPTH./" ww3_grid.nml
    grep -q "DEPTH%SF        =  -$DEPTH\." ww3_grid.nml
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    gdb -q -nx -batch -x /capture/snl1.py --args ww3_shel 2>&1 | grep -v "^\[" | tail -3
    tail -2 shel.log 2>/dev/null || true'
ls -l "work/capture/$out"
