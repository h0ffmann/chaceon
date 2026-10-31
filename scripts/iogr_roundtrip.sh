#!/usr/bin/env bash
# The writer alone: Rust reads a mod_def.ww3 the Fortran ww3_grid wrote, writes it again, and cmp judges.
# Usage: iogr_roundtrip.sh <work/grid/<tag>> ... (each holds mod_def.reference). Exit 0 only when every file is identical.
set -euo pipefail
cd "$(dirname "$0")/.."
bad=0
for d in "$@"; do
  rm -f "$d/roundtrip.ww3"
  bash scripts/rust_in_ref.sh ww3_grid --roundtrip "/work/${d#work/}/mod_def.reference" "/work/${d#work/}/roundtrip.ww3" \
    > "$d/roundtrip.log" 2>&1 || true
  bytes=$(wc -c < "$d/mod_def.reference" | tr -d ' ')
  if [ -f "$d/roundtrip.ww3" ] && cmp -s "$d/mod_def.reference" "$d/roundtrip.ww3"; then
    echo "$(basename "$d") round trip: identical, $bytes bytes"
  else
    echo "$(basename "$d") round trip: DIFFERENT or not written, $bytes bytes ($(grep -E 'panicked|rror' "$d/roundtrip.log" | head -1))"
    bad=1
  fi
done
exit "$bad"
