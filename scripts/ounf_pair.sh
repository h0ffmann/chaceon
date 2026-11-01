#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_ounf: the reference's ww3_ounf and Rust's ww3_ounf read the same mod_def.ww3 and
# out_grd.ww3 (the reference run of a tp_pair.sh case, kept in work/pair/<case>) and the same ww3_ounf.nml, and cmp judges
# every NetCDF file each wrote. Usage: ounf_pair.sh <pair case> [variant]. The regtest input is WW3/regtests/<regtest>/input
# (ww3_ts1 for the ts1 cases, the case name less a _leg/_curv/_x suffix otherwise). OUNF_NML names another namelist file
# of that input (ww3_ounf_flds_hrly.nml), OUNF_TAG adds a suffix to the work directory, OUNF_SRC names another directory
# holding mod_def.ww3 and out_grd.reference (scripts/ounf_extra.sh makes them), OUNF_LIST replaces FIELD%LIST. Files go to
# work/ounf/<case>[_variant]/{ref,rust}.
# A variant is a name from the table below (x_<label> adds nothing), or several joined by +: lines added to the end of the namelist groups, the same on both sides.
# A fault is a source edit run with OUNF_TAG naming it. Exit 0 only when every file is byte-identical and the same set;
# exit 3 when Rust refused the input with a message that says "not implemented" and the reference ran.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
variant="${2:-}"
case "$case_name" in
  ww3_*) reg="${case_name%_leg*}"; reg="${reg%_curv}"; reg="${reg%_x}" ;;
  *) reg=ww3_ts1 ;;
esac
tag="${case_name}${variant:+_$variant}${OUNF_TAG:+_$OUNF_TAG}"
nml="${OUNF_NML:-ww3_ounf.nml}"
src="${OUNF_SRC:-work/pair/$case_name}"
[ -f "$src/out_grd.reference" ] || { echo "$case_name: no out_grd.reference in $src" >&2; exit 2; }
listadd=""
[ -n "${OUNF_LIST:-}" ] && listadd="FIELD%LIST = '${OUNF_LIST}'"
[ -f "WW3/regtests/$reg/input/$nml" ] || { echo "$case_name: no $nml in WW3/regtests/$reg/input" >&2; exit 2; }
mkdir -p "work/ounf/$tag"
find "work/ounf/$tag" -mindepth 1 -delete
fieldadd=""
fileadd=""
# A variant is one name or several joined by +, such as type2+stride.
addfield() { fieldadd="${fieldadd:+$fieldadd; }$1"; }
addfile() { fileadd="${fileadd:+$fileadd; }$1"; }
for one in ${variant//+/ }; do
  case "$one" in
    type2) addfield "FIELD%TYPE = 2" ;;
    type3) addfield "FIELD%TYPE = 3" ;;
    type4) addfield "FIELD%TYPE = 4" ;;
    nc4) addfile "FILE%NETCDF = 4" ;;
    nc4type2) addfile "FILE%NETCDF = 4"; addfield "FIELD%TYPE = 2" ;;
    nc4type4) addfile "FILE%NETCDF = 4"; addfield "FIELD%TYPE = 4" ;;
    sep) addfield "FIELD%SAMEFILE = F" ;;
    sepnc4) addfield "FIELD%SAMEFILE = F"; addfile "FILE%NETCDF = 4" ;;
    split0) addfield "FIELD%TIMESPLIT = 0" ;;
    split4) addfield "FIELD%TIMESPLIT = 4" ;;
    split8) addfield "FIELD%TIMESPLIT = 8" ;;
    split10) addfield "FIELD%TIMESPLIT = 10" ;;
    mag) addfield "FIELD%VECTOR = F" ;;
    noval) addfield "FIELD%NOVAL = -9999." ;;
    nomap) addfield "FIELD%MAPSTA = F" ;;
    secs) addfield "FIELD%TIMEUNIT = 'S'" ;;
    int64) addfield "FIELD%TIMEUNIT = 'S'"; addfield "FIELD%TIMEVAR = 'I'"; addfile "FILE%NETCDF = 4" ;;
    epoch) addfield "FIELD%TIMEEPOCH = '20000101 120000'" ;;
    prefix) addfile "FILE%PREFIX = 'run1_'" ;;
    subset) addfile "FILE%IX0 = 2"; addfile "FILE%IXN = 7"; addfile "FILE%IY0 = 1"; addfile "FILE%IYN = 3" ;;
    part) addfield "FIELD%PARTITION = '0'" ;;
    partmany) addfield "FIELD%PARTITION = '0 1 2 3 4 5 6'" ;;
    stride) addfield "FIELD%TIMESTRIDE = '600.'" ;;
    count) addfield "FIELD%TIMECOUNT = '5'" ;;
    nostride) addfield "FIELD%TIMESTRIDE = '0'" ;;
    "" | x_*) ;;   # x_<label>: a label for a run with OUNF_SRC or OUNF_LIST; the namelist is as in the regtest
    *) echo "unknown variant $one" >&2; exit 64 ;;
  esac
done
# Appends the lines before the closing / of the group; a group the file lacks is added.
edit() {
  awk -v fa="${fieldadd}${listadd:+; $listadd}" -v fi="$fileadd" '
    function emit(txt,   n, a, i) { n = split(txt, a, "; "); for (i = 1; i <= n; i++) print "  " a[i] }
    /^&FIELD_NML/ { g = "F"; seenf = 1 }
    /^&FILE_NML/ { g = "I"; seeni = 1 }
    /^\// && g == "F" { emit(fa); g = "" }
    /^\// && g == "I" { emit(fi); g = "" }
    { print }
    END { if (!seenf && fa != "") { print "&FIELD_NML"; emit(fa); print "/" }
          if (!seeni && fi != "") { print "&FILE_NML"; emit(fi); print "/" } }' "$1"
}
for side in ref rust; do
  mkdir -p "work/ounf/$tag/$side"
  cp "$src/mod_def.ww3" "work/ounf/$tag/$side/mod_def.ww3"
  cp "$src/out_grd.reference" "work/ounf/$tag/$side/out_grd.ww3"
  edit "WW3/regtests/$reg/input/$nml" > "work/ounf/$tag/$side/ww3_ounf.nml"
done
[ -z "${OUNF_PREP_ONLY:-}" ] || exit 0
# One retry: now and then the container does not see the directory the host has just filled (a file-sharing delay).
run_ref() { bash scripts/rust_netcdf.sh --run-ref bash -c "cd /work/ounf/$tag/ref && ww3_ounf > ounf.log 2>&1 || { tail -5 ounf.log; exit 1; }"; }
run_ref || { echo "$tag: the reference run failed, once more" >&2; sleep 3; run_ref; }
bash scripts/rust_netcdf.sh ww3_ounf "/work/ounf/$tag/rust" > "work/ounf/$tag/rust/ounf.log" 2>&1 || true
if ! ls "work/ounf/$tag/rust"/*.nc > /dev/null 2>&1 && grep -qE 'not implemented|are implemented' "work/ounf/$tag/rust/ounf.log"; then
  echo "$tag: REFUSED by Rust ($(grep -E 'not implemented|are implemented' "work/ounf/$tag/rust/ounf.log" | head -1 | sed 's/^thread .main. panicked at [^:]*:[0-9:]*://'))"
  exit 3
fi
branches=$(grep '^ww3_ounf: reached' "work/ounf/$tag/rust/ounf.log" | sed 's/^ww3_ounf: reached: //' || true)
bad=0
n=0
for f in "work/ounf/$tag/ref"/*.nc; do
  [ -f "$f" ] || continue
  b=$(basename "$f")
  n=$((n + 1))
  if [ -f "work/ounf/$tag/rust/$b" ] && cmp -s "$f" "work/ounf/$tag/rust/$b"; then
    echo "$tag $b: identical, $(wc -c < "$f" | tr -d ' ') bytes; ${branches:-no branch line}"
  elif [ -f "work/ounf/$tag/rust/$b" ]; then
    echo "$tag $b: DIFFERENT, $(cmp -l "$f" "work/ounf/$tag/rust/$b" 2>/dev/null | wc -l | tr -d ' ') of $(wc -c < "$f" | tr -d ' ') bytes (Rust wrote $(wc -c < "work/ounf/$tag/rust/$b" | tr -d ' '))"
    bad=1
  else
    echo "$tag $b: NOT WRITTEN by Rust ($(grep -E 'panicked|rror|refus|not implemented' "work/ounf/$tag/rust/ounf.log" | head -1))"
    bad=1
  fi
done
for f in "work/ounf/$tag/rust"/*.nc; do
  [ -f "$f" ] || continue
  [ -f "work/ounf/$tag/ref/$(basename "$f")" ] || { echo "$tag $(basename "$f"): written by Rust only"; bad=1; }
done
[ "$n" -gt 0 ] || { echo "$tag: the reference wrote no file"; bad=1; }
# The program's own log is not the verdict, but it is a second view of the same run.
if diff -q "work/ounf/$tag/ref/ounf.log" <(grep -v '^ww3_ounf: reached' "work/ounf/$tag/rust/ounf.log") > /dev/null 2>&1; then
  echo "$tag log: identical"
else
  echo "$tag log: differs (not the verdict)"
fi
exit "$bad"
