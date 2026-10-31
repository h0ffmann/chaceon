#!/usr/bin/env bash
# Every case tp_all.sh prepared, with W3XYP3 and W3KTP3 in compiled SPIR-V on lavapipe.
set -euo pipefail
cd "$(dirname "$0")"
bad=0
for t in "$@"; do
  bash spirv_pair.sh "$t" || bad=1
done
exit "$bad"
