#!/usr/bin/env bash
# Runs Lean 4 in Docker (never on the host). Usage: lean.sh [build|check|lake <args>|shell].
# The mathlib cache and .lake live in named volumes, so only the first run downloads.
set -euo pipefail
cd "$(dirname "$0")/.."
image=chaceon-lean
mode="${1:-check}"; [ $# -gt 0 ] && shift
docker build -q --platform linux/arm64 -f docker/Dockerfile.lean -t "$image" lean >/dev/null
run() {
  docker run --rm --platform linux/arm64 -v "$PWD/lean:/proj/lean" \
    -v chaceon-lean-lake:/proj/lean/.lake -v chaceon-lean-cache:/cache/mathlib "$image" "$@"
}
case "$mode" in
  build)
    run bash -c 'lake exe cache get && lake build' ;;
  check)
    # build, then refuse a sorry and print the axioms each theorem uses
    run bash -c 'lake exe cache get >/dev/null && lake build 2>&1 | tee /tmp/b.log \
      && ! grep -q "declaration uses" /tmp/b.log && ! grep -rnw sorry Chaceon.lean Chaceon/ \
      && lake env lean Chaceon/Axioms.lean' ;;
  lake) run lake "$@" ;;
  shell) docker run --rm -it --platform linux/arm64 -v "$PWD/lean:/proj/lean" \
    -v chaceon-lean-lake:/proj/lean/.lake -v chaceon-lean-cache:/cache/mathlib "$image" bash ;;
  *) echo "usage: lean.sh [build|check|lake <args>|shell]" >&2; exit 64 ;;
esac
