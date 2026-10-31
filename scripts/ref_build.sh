#!/usr/bin/env bash
# Builds the Fortran reference, metocean/WW3 at the pin, for linux/arm64. Usage: ref_build.sh [switch-name]
set -euo pipefail
cd "$(dirname "$0")/.."
name="${1:-switch_chaceon_shrd}"
test -f "switches/$name"
docker build --platform linux/arm64 -f docker/Dockerfile.reference \
  --target dist --build-arg "WW3_SWITCH_FILE=$name" -t "chaceon-ref:$name" .
docker build --platform linux/arm64 -f docker/Dockerfile.reference \
  --target capture --build-arg "WW3_SWITCH_FILE=$name" -t "chaceon-ref-capture:$name" .
docker run --rm --platform linux/arm64 --entrypoint cat "chaceon-ref:$name" /usr/local/share/ww3/BUILD_INFO
