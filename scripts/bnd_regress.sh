#!/usr/bin/env bash
# Rust's ww3_shel again on every case tp_all.sh and ts1_pair.sh left in work/pair, without writing there (the directory is
# shared): each case is copied to work/bndreg/<tag>, Rust runs in the copy, and cmp judges every .reference file against the
# copy's output. Exit 0 only when every file is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf work/bndreg
mkdir -p work/bndreg
bad=0
n=0
for src in work/pair/*/; do
  tag=$(basename "$src")
  [ -f "$src/ww3_shel.nml" ] && [ -f "$src/mod_def.ww3" ] || continue
  mkdir "work/bndreg/$tag"
  cp "$src"/* "work/bndreg/$tag/"
  rm -f "work/bndreg/$tag"/out_grd.ww3 "work/bndreg/$tag"/out_pnt.ww3 "work/bndreg/$tag"/restart001.ww3
  bash scripts/rust_in_ref.sh ww3_shel "/work/bndreg/$tag" > "work/bndreg/$tag/shel.log" 2>&1 || true
  for f in out_grd out_pnt restart001; do
    ref="work/bndreg/$tag/$f.reference"
    [ -f "$ref" ] || continue
    n=$((n + 1))
    if cmp -s "$ref" "work/bndreg/$tag/$f.ww3"; then echo "$tag $f: identical"; else echo "$tag $f: DIFFERENT"; bad=1; fi
  done
done
echo "$n files compared"
exit "$bad"
