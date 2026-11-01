#!/usr/bin/env bash
# A run with no Fortran in it: Rust's ww3_grid, ww3_strt and ww3_shel in sequence, from the regtest's namelists and
# input files, judged by cmp against the files the reference wrote. Needs the case prepared by tp_pair.sh or
# ts1_pair.sh, for the reference's files and the ww3_shel.nml it ran. Usage: rust_only.sh <regtest> <tag> [suffix].
# RUST_ONLY_SPIRV=1 runs the third step in SPIR-V on lavapipe. Exit 0 only when every file is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
tag="$2"
suffix="${3:-}"
ref="work/pair/$tag"
d="work/rustonly/$tag"
rm -rf "$d"
mkdir -p "$d/work"
cp -R "WW3/regtests/$case_name/input" "$d/input"
cp "$d/input/ww3_grid${suffix:+_$suffix}.nml" "$d/work/ww3_grid.nml"
cp "$d/input/ww3_strt.inp" "$d/work/"
cp "$ref/ww3_shel.nml" "$d/work/"
if [ -f "$ref/points.list" ]; then cp "$ref/points.list" "$d/work/"; fi
# ww3_prep is not ported: a current file is the one input that may still come from the Fortran.
if [ -f "$ref/current.ww3" ]; then cp "$ref/current.ww3" "$d/work/"; fi
bash scripts/rust_in_ref.sh ww3_grid "/work/rustonly/$tag/work" > "$d/grid.log" 2>&1
bash scripts/rust_in_ref.sh ww3_strt "/work/rustonly/$tag/work" > "$d/strt.log" 2>&1
if [ -n "${RUST_ONLY_SPIRV:-}" ]; then
  docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
    --entrypoint /rust/target/linux/release/spirv_shel chaceon-lavapipe /rust/gpu-spirv/out/chaceon.spv \
    "/work/rustonly/$tag/work" > "$d/shel.log" 2>&1
else
  bash scripts/rust_in_ref.sh ww3_shel "/work/rustonly/$tag/work" > "$d/shel.log" 2>&1
fi
bad=0
for f in mod_def out_grd out_pnt restart001 restart; do
  case "$f" in
    mod_def|restart) r="$ref/$f.ww3" ;;
    *) r="$ref/$f.reference" ;;
  esac
  [ -f "$r" ] || continue
  bytes=$(wc -c < "$r" | tr -d ' ')
  if cmp -s "$r" "$d/work/$f.ww3"; then
    echo "$tag $f, Rust only: identical, $bytes bytes"
  else
    echo "$tag $f, Rust only: DIFFERENT, $(cmp -l "$r" "$d/work/$f.ww3" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes"
    bad=1
  fi
done
exit "$bad"
