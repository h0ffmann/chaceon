#!/usr/bin/env bash
# Runs WW3's ww3_ts1 in the reference under gdb and records one routine.
# Usage: capture.sh <routine> <variant> [depth] [stride]. Writes work/capture/<routine>_<variant>_<depth>m.bin.
# A variant only changes the case's namelists, never WW3: ww3_ts1 at 2500 m reaches one branch of
# W3SDS4, and the rest need other settings.
set -euo pipefail
cd "$(dirname "$0")/.."
routine="$1"
variant="$2"
depth="${3:-2500}"
stride="${4:-1}"
case "$variant" in
  default) sds4='' ;;
  t700) sds4='&SDS4 SDSBCHOICE=3, SDSC2=-3.8, SDSBR=0.005, SDSSTRAIN=0., SDSSTRAIN2=0., FXFM3=20., SDSFACMTF=400., SDSCUM=0., SDSC5=0. /' ;;
  filipot) sds4='&SDS4 SDSBCHOICE=2, SDSC2=0.0, SDSBCK=0.185, SDSHCK=1.5 /' ;;
  strain) sds4='&SDS4 SDSSTRAIN=2., SDSSTRAIN2=0.5, SDSBRFDF=2, SDSISO=1, SDSBM0=0., SDSBM1=0.3, SDSBM2=0.3, SDSBM3=0.2, SDSBM4=0.1 /' ;;
  circle) sds4='&SDS4 SDSDTH=180. /' ;;
  romero2) sds4='&SDS4 SDSBCHOICE=3, SDSC2=-3.8, SDSBR=0.005, SDSSTRAIN=2., SDSSTRAIN2=0.5, FXFM3=20., SDSFACMTF=400., SDSCUM=0., SDSC5=0. /' ;;
  *) echo "unknown variant $variant" >&2; exit 64 ;;
esac
out="${routine}_${variant}_${depth}m.bin"
mkdir -p work/capture
docker run --rm --platform linux/arm64 --security-opt seccomp=unconfined --cap-add SYS_PTRACE \
  -v "$PWD/work:/work" -v "$PWD/regtests/capture:/capture:ro" \
  -v "$PWD/WW3/regtests/ww3_ts1/input:/ww3_input:ro" \
  -e CAPTURE_OUT="/work/capture/$out" -e CAPTURE_STRIDE="$stride" -e DEPTH="$depth" -e SDS4="$sds4" \
  --entrypoint sh chaceon-ref-capture:switch_chaceon_shrd -c "
    set -e
    mkdir -p /run/ts1/work && cp -R /ww3_input /run/ts1/input && cd /run/ts1/work
    cp ../input/ww3_grid.nml ../input/ww3_strt.inp ../input/ww3_shel.nml .
    sed -i \"s/DEPTH%SF        =  -2500\\\\./DEPTH%SF        =  -\$DEPTH./\" ww3_grid.nml
    grep -q \"DEPTH%SF        =  -\$DEPTH\\\\.\" ww3_grid.nml
    if [ -n \"\$SDS4\" ]; then
      sed -i \"/END OF NAMELISTS/i \$SDS4\" ../input/namelists_HOMOGENEOUS.nml
    fi
    ww3_grid > grid.log 2>&1
    ww3_strt > strt.log 2>&1
    gdb -q -nx -batch -x /capture/$routine.py --args ww3_shel 2>&1 | grep -E 'capture:|rror' | tail -3"
ls -l "work/capture/$out"
if [ -f "work/capture/$out.error" ]; then tail -4 "work/capture/$out.error"; exit 1; fi
