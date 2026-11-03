#!/usr/bin/env bash
# A run with no Fortran in it: Rust's ww3_grid, ww3_strt, ww3_prep (when the case has a forcing file) and ww3_shel
# in sequence, from the regtest's namelists and input files, judged by cmp against the files the reference wrote. Needs the case prepared by tp_pair.sh or
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
if [ -f "$ref/track_i.ww3" ]; then cp "$ref/track_i.ww3" "$d/work/"; fi
# A case whose reference run has a forcing file has one ww3_prep input in its directory, or RUST_ONLY_PREP names
# them in order, one per forcing file. RUST_ONLY_OVERLAY is a directory laid over the input, as TP_OVERLAY is.
if [ -n "${RUST_ONLY_OVERLAY:-}" ]; then cp "$RUST_ONLY_OVERLAY"/* "$d/input/"; cp "$d/input/ww3_grid${suffix:+_$suffix}.nml" "$d/work/ww3_grid.nml"; fi
prep=""
for f in current wind level ice; do
  if [ -f "$ref/$f.ww3" ]; then
    prep="${RUST_ONLY_PREP:-$(cd "$d/input" && ls ww3_prep*.inp)}"
    [ -n "${RUST_ONLY_PREP:-}" ] || [ "$(echo "$prep" | wc -w | tr -d ' ')" = 1 ] || { echo "$tag: set RUST_ONLY_PREP to one of: $prep"; exit 2; }
  fi
done
bash scripts/rust_in_ref.sh ww3_grid "/work/rustonly/$tag/work" > "$d/grid.log" 2>&1
bash scripts/rust_in_ref.sh ww3_strt "/work/rustonly/$tag/work" > "$d/strt.log" 2>&1
for p in $prep; do
  cp "$d/input/$p" "$d/work/ww3_prep.inp"
  bash scripts/rust_in_ref.sh ww3_prep "/work/rustonly/$tag/work" >> "$d/prep.log" 2>&1
done
if [ -n "${RUST_ONLY_SPIRV:-}" ]; then
  docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
    --entrypoint /rust/target/linux/release/spirv_shel chaceon-lavapipe /rust/gpu-spirv/out/chaceon.spv \
    "/work/rustonly/$tag/work" > "$d/shel.log" 2>&1
else
  bash scripts/rust_in_ref.sh ww3_shel "/work/rustonly/$tag/work" > "$d/shel.log" 2>&1
fi
bad=0
for f in mod_def current wind level ice out_grd out_pnt restart001 track_o restart; do
  case "$f" in
    mod_def|restart|current|wind|level|ice) r="$ref/$f.ww3" ;;
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
