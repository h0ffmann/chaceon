#!/usr/bin/env bash
# Counts the double-precision declarations and D-exponent literals of WW3 at the pin, per file.
# The table in docs/FLOAT_REPRESENTATION_202610.md is this script's output.
set -euo pipefail
cd "$(dirname "$0")/../WW3/model/src"
decl='^[^!]*\b(DOUBLE PRECISION|REAL[[:space:]]*\*[[:space:]]*8|REAL[[:space:]]*\([[:space:]]*(KIND[[:space:]]*=[[:space:]]*)?8[[:space:]]*\))'
lit='^[^!]*[0-9.][dD][+-]?[0-9]'
printf 'file\tdecl\tliteral\n'
for f in *.F90; do
  d=$(grep -icE "$decl" "$f" || true)
  l=$(grep -cE "$lit" "$f" || true)
  if [ "$d" -gt 0 ] || [ "$l" -gt 0 ]; then printf '%s\t%s\t%s\n' "$f" "$d" "$l"; fi
done | sort -t"$(printf '\t')" -k2,2nr -k3,3nr
