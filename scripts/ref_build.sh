#!/usr/bin/env bash
# Builds the Fortran reference, metocean/WW3 at the pin, for x86_64. Usage: ref_build.sh [switch-name]
set -euo pipefail
cd "$(dirname "$0")/.."
name="${1:-switch_chaceon_shrd}"
test -f "switches/$name"
docker build --platform linux/amd64 -f docker/Dockerfile.reference \
  --build-arg "WW3_SWITCH_FILE=$name" -t "chaceon-ref:$name" .
docker run --rm --platform linux/amd64 --entrypoint cat "chaceon-ref:$name" /usr/local/share/ww3/BUILD_INFO
