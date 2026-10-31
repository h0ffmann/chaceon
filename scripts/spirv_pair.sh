#!/usr/bin/env bash
# Runs a case already prepared by tp_pair.sh with W3XYP3, W3KTP3 and W3SRCE in compiled SPIR-V on lavapipe, cmp
# judging its files against the reference's. Usage: spirv_pair.sh <directory under work/pair/>.
set -euo pipefail
cd "$(dirname "$0")/.."
tag="$1"
docker build --platform linux/arm64 -q -f docker/Dockerfile.lavapipe -t chaceon-lavapipe docker >/dev/null
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/rust/target/linux -w /rust rust:1.90-bookworm cargo build --release -q --bin spirv_shel
rm -f work/pair/"$tag"/out_grd.ww3 work/pair/"$tag"/out_pnt.ww3 work/pair/"$tag"/restart001.ww3
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
  --entrypoint /rust/target/linux/release/spirv_shel chaceon-lavapipe /rust/gpu-spirv/out/chaceon.spv "/work/pair/$tag" 2>&1 | tail -4
bad=0
for f in out_grd out_pnt restart001; do
  ref="work/pair/$tag/$f.reference"
  ours="work/pair/$tag/$f.ww3"
  [ -f "$ref" ] || continue
  bytes=$(wc -c < "$ref" | tr -d ' ')
  if cmp -s "$ref" "$ours"; then
    echo "$tag $f on SPIR-V: identical, $bytes bytes"
  else
    echo "$tag $f on SPIR-V: DIFFERENT, $(cmp -l "$ref" "$ours" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes"
    bad=1
  fi
done
exit "$bad"
