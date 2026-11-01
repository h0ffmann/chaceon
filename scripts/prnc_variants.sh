#!/usr/bin/env bash
# The ww3_prnc cases no regtest has: other fields, units, layouts, the old ww3_prnc.inp, the errors the reference
# reports (Rust must report the same bytes and status) and the forms Rust refuses. See prnc_all.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
PRNC_KIND=variant exec bash scripts/prnc_all.sh "$@"
