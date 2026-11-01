#!/usr/bin/env bash
# Measures whether the reference's ww3_ounp writes the same bytes twice: runs it two times on one input, two seconds
# apart, and cmp's every file it wrote (the NetCDF files, ww3_ounp.nml.log), its standard output and its status. This is
# what decides the verdict of scripts/ounp_pair.sh (cmp of the file) against a comparison of contents.
# Usage: ounp_selfcmp.sh <pair case> [variant] (as in ounp_pair.sh), or --all (the inputs behind the claim).
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" = "--all" ]; then
  rc=0
  while read -r c v; do
    out=$(bash "$0" "$c" ${v:+"$v"} 2>&1 || true)
    echo "$out" | grep -E "identical to itself|DIFFERS|files$" || true
    ! echo "$out" | grep -q DIFFERS || rc=1
    echo "$out" | grep -q "identical to itself" || { echo "$c ${v:-}: no comparison was made"; rc=1; }
  done <<'LIST'
ww3_tp1.1
ww3_tp1.1 nc4
ww3_tp1.1 sep
ww3_tp1.1 sep+nc4
ww3_tp1.7
ww3_tp1.7 nc4+type3
ww3_tp1.8 split8
ww3_tp2.2 list312
default_2500m
LIST
  # ww3_tp2.15 has no ww3_ounp.nml: its namelists are the _par and _spec ones.
  while read -r nml v; do
    out=$(OUNP_NML=$nml bash "$0" ww3_tp2.15 ${v:+"$v"} 2>&1 || true)
    echo "$out" | grep -E "identical to itself|DIFFERS|files$" || true
    ! echo "$out" | grep -q DIFFERS || rc=1
    echo "$out" | grep -q "identical to itself" || { echo "ww3_tp2.15 $nml ${v:-}: no comparison was made"; rc=1; }
  done <<'LIST'
ww3_ounp_par.nml
ww3_ounp_par.nml par5+nc4
ww3_ounp_par.nml par1+sep
ww3_ounp_par.nml par6
ww3_ounp_spec.nml nc4
LIST
  exit "$rc"
fi
case_name="$1"
variant="${2:-}"
# ounp_pair.sh prepares the two directories; the reference then runs in both of them, two seconds apart.
OUNP_TAG="selfcmp" OUNP_PREP_ONLY=1 bash scripts/ounp_pair.sh "$case_name" ${variant:+"$variant"}
case "$case_name" in
  ww3_*) reg="${case_name%_leg*}"; reg="${reg%_curv}"; reg="${reg%_x}"; nml_default=ww3_ounp.nml ;;
  *) nml_default=ww3_ounp_spec.nml ;;
esac
nml="${OUNP_NML:-$nml_default}"
tag="${case_name}__${nml%.nml}"; tag="${tag/__ww3_/__}${variant:+_$variant}_selfcmp"
work="work/ounp/$tag"
[ -d "$work/ref" ] || { echo "$tag: not prepared" >&2; exit 2; }
bash scripts/rust_netcdf.sh --run-ref bash -c "cd /work/ounp/$tag/ref && ww3_ounp > ounp.log 2> ounp.err; echo \$? > exit.code; sleep 2; cd ../rust && ww3_ounp > ounp.log 2> ounp.err; echo \$? > exit.code"
bad=0; n=0
skip='^(mod_def\.ww3|out_pnt\.ww3|ww3_ounp\.nml|ounp\.err)$'
for f in "$work"/ref/*; do
  b=$(basename "$f")
  [[ "$b" =~ $skip ]] && continue
  n=$((n + 1))
  if cmp -s "$f" "$work/rust/$b"; then
    echo "$tag $b: the reference is identical to itself, $(wc -c < "$f" | tr -d ' ') bytes"
  else
    echo "$tag $b: the reference DIFFERS from itself"; bad=1
  fi
done
echo "$tag: $n files"
exit "$bad"
