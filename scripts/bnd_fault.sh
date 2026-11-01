#!/usr/bin/env bash
# Fault run for bnd_pair.sh: applies a one-line edit to a source file, reruns Rust on the work/bnd_<tag> directories
# that bnd_pair.sh left (the reference files stay), compares, and restores the file. Usage: bnd_fault.sh <file> <from> <to> <tag>...
set -euo pipefail
cd "$(dirname "$0")/.."
file="$1"; from="$2"; to="$3"; shift 3
cp "$file" "$file.orig"
trap 'mv "$file.orig" "$file"' EXIT
python3 -I -c 'import sys
p,a,b=sys.argv[1:4]
s=open(p).read()
assert a in s, "fault pattern not found"
open(p,"w").write(s.replace(a,b,1))' "$file" "$from" "$to"
for tag in "$@"; do
  d="work/bnd_$tag"
  rm -f "$d"/out_grd.ww3 "$d"/out_pnt.ww3 "$d"/restart001.ww3
  bash scripts/rust_in_ref.sh ww3_shel "/work/bnd_$tag" > /dev/null 2>&1 || true
  for f in out_grd out_pnt; do
    if [ -f "$d/$f.ww3" ] && cmp -s "$d/$f.reference" "$d/$f.ww3"; then echo "$tag $f: identical (fault NOT caught)"; else echo "$tag $f: differs or missing (caught)"; fi
  done
done
