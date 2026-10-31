#!/usr/bin/env bash
# Every propagation regtest the host driver implements, through tp_pair.sh. Stops at the first difference.
set -euo pipefail
cd "$(dirname "$0")"
for c in 1.1 1.2 1.3 1.4 1.5 1.7 1.8 1.9 1.10 2.1 2.3 2.4; do
  bash tp_pair.sh "ww3_tp$c"
done
