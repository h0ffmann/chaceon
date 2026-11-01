#!/usr/bin/env bash
# Measures whether the reference's ww3_ounf writes the same bytes twice: runs it two times on one input, two seconds
# apart, and cmp's every NetCDF file. This is what decides the verdict of scripts/ounf_pair.sh (cmp of the file) against
# a comparison of the file's contents. Usage: ounf_selfcmp.sh <pair case> [variant] (variants as in ounf_pair.sh), or --all.
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" = "--all" ]; then
  # The inputs behind the claim: classic and HDF5, types 2 and 4, one file per field, split names, a curvilinear grid.
  rc=0
  while read -r c v; do
    out=$(bash "$0" "$c" ${v:+"$v"} 2>&1 || true)
    echo "$out" | grep -E "identical to itself|DIFFERS" | sed 's/^selfcmp_//' || true
    ! echo "$out" | grep -q DIFFERS || rc=1
  done <<'LIST'
ww3_tp2.4
ww3_tp2.4 nc4
ww3_tp2.4_curv
ww3_tp2.4_curv nc4type2
default_2500m
default_2500m nc4
ww3_tp1.8
ww3_tp1.8 nc4type4
ww3_tp2.3 sep
ww3_tp1.7
ww3_tp1.7 nc4
ww3_tp1.7 nc4type2
ww3_tp1.7 sep
ww3_tp1.7 sepnc4
ww3_tp1.7 split10
LIST
  exit "$rc"
fi
case_name="$1"
variant="${2:-}"
tag="selfcmp_${case_name}${variant:+_$variant}"
# Reuse ounf_pair.sh's input preparation, then run the reference twice in two directories.
case "$case_name" in
  ww3_*) reg="${case_name%_leg*}"; reg="${reg%_curv}"; reg="${reg%_x}" ;;
  *) reg=ww3_ts1 ;;
esac
src="work/pair/$case_name"
rm -rf "work/ounf/$tag"
mkdir -p "work/ounf/$tag/a" "work/ounf/$tag/b"
# The variant text comes from ounf_pair.sh's table by running its edit on the regtest namelist.
OUNF_PREP_ONLY=1 bash scripts/ounf_pair.sh "$case_name" ${variant:+"$variant"}
pairdir="work/ounf/${case_name}${variant:+_$variant}/ref"
for r in a b; do
  cp "$src/mod_def.ww3" "work/ounf/$tag/$r/"
  cp "$src/out_grd.reference" "work/ounf/$tag/$r/out_grd.ww3"
  cp "$pairdir/ww3_ounf.nml" "work/ounf/$tag/$r/"
done
bash scripts/rust_netcdf.sh --run-ref bash -c "cd /work/ounf/$tag/a && ww3_ounf > ounf.log 2>&1; sleep 2; cd ../b && ww3_ounf > ounf.log 2>&1"
bad=0
n=0
for f in "work/ounf/$tag/a"/*.nc; do
  b=$(basename "$f")
  n=$((n + 1))
  if cmp -s "$f" "work/ounf/$tag/b/$b"; then echo "$tag $b: the reference is identical to itself, $(wc -c < "$f" | tr -d ' ') bytes"; else echo "$tag $b: the reference DIFFERS from itself"; bad=1; fi
done
echo "$tag: $n files"
exit "$bad"
