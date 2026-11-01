#!/usr/bin/env bash
# The cases of regtests/prnc_inputs/ of one KIND (case.conf) through prnc_pair.sh: one line per file, the log, the
# error output and the status. KIND=regtest cases have the namelist of a ww3_prnc input of WW3's regtests (the
# shape; the data are synthetic, see prnc_pair.sh), KIND=variant cases are the other forms, the errors and the
# refusals (scripts/prnc_variants.sh). A case named refuse_* must end in a refusal (prnc_pair.sh status 3);
# every other case must be identical. Usage: prnc_all.sh [case ...]; with no argument every case of PRNC_KIND
# (default regtest) runs. PRNC_CASES names another directory of cases. Exit 0 only when every case ended as it must.
set -euo pipefail
cd "$(dirname "$0")/.."
dir="${PRNC_CASES:-regtests/prnc_inputs}"
kind="${PRNC_KIND:-regtest}"
if [ "$#" -gt 0 ]; then
  cases=("$@")
else
  cases=()
  for d in "$dir"/*/; do
    # shellcheck source=/dev/null
    k=$(. "$d/case.conf" && echo "${KIND:-variant}")
    if [ "$k" = "$kind" ]; then cases+=("$(basename "$d")"); fi
  done
fi
bash scripts/rust_netcdf.sh --build ww3_prnc
export PRNC_SKIP_BUILD=1
bad=0
n=0
for c in "${cases[@]}"; do
  rc=0
  bash scripts/prnc_pair.sh "$c" || rc=$?
  n=$((n + 1))
  case "$c" in
    refuse_*) [ "$rc" = 3 ] || { echo "$c: expected a refusal (status 3), got $rc"; bad=1; } ;;
    *) [ "$rc" = 0 ] || { echo "$c: expected identical (status 0), got $rc"; bad=1; } ;;
  esac
done
echo "prnc_all: $n cases of kind $kind, $([ "$bad" = 0 ] && echo all as expected || echo FAILURES)"
exit "$bad"
