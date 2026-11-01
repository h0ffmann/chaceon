#!/usr/bin/env bash
# Makes an out_grd.ww3 that holds fields the regtest's own ww3_shel.nml does not ask for: the reference's ww3_shel
# runs a pair case again with another TYPE%FIELD%LIST, and its out_grd.ww3 and mod_def.ww3 go to work/ounf/src_<tag>.
# ounf_pair.sh reads them with OUNF_SRC=work/ounf/src_<tag>, and OUNF_LIST sets FIELD%LIST of ww3_ounf.nml.
# Usage: ounf_extra.sh <pair case> <tag> '<field list>'. Only the Fortran runs here; Rust is not involved.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
tag="$2"
list="$3"
src="work/pair/$case_name"
out="work/ounf/src_$tag"
rm -rf "$out"
mkdir -p "$out/run"
for f in mod_def.ww3 restart.ww3 ww3_shel.nml points.list current.ww3 wind.ww3; do
  [ -f "$src/$f" ] && cp "$src/$f" "$out/run/$f"
done
awk -v l="$list" '/TYPE%FIELD%LIST/ { print "  TYPE%FIELD%LIST          = \x27" l "\x27"; next } { print }' "$src/ww3_shel.nml" > "$out/run/ww3_shel.nml"
bash scripts/rust_netcdf.sh --run-ref bash -c "cd /work/ounf/src_$tag/run && ww3_shel > shel.log 2>&1 || { tail -20 shel.log; exit 1; }"
mv "$out/run/out_grd.ww3" "$out/out_grd.reference"
cp "$out/run/mod_def.ww3" "$out/mod_def.ww3"
echo "$tag: out_grd.reference $(wc -c < "$out/out_grd.reference" | tr -d ' ') bytes, fields: $list"
