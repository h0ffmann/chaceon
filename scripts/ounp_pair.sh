#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_ounp: the reference's ww3_ounp and Rust's ww3_ounp read the same mod_def.ww3
# and out_pnt.ww3 (the reference run of a tp_pair.sh or ts1_pair.sh case, kept in work/pair/<case>, linked and never
# written) and the same ww3_ounp.nml, in two directories, and cmp judges every file each wrote (the NetCDF files and
# ww3_ounp.nml.log), the standard output and the exit status.
# Usage: ounp_pair.sh <pair case> [variant]. The regtest input is WW3/regtests/<regtest>/input (ww3_ts1 for the ts1
# cases, ww3_ts2 for the case ts2 of ounp_src.sh, the case name less a _leg/_curv/_x suffix otherwise). OUNP_NML names the namelist file of that input
# (ww3_ounp_par.nml, ww3_ounp_spec.nml; ts1 cases default to the second), OUNP_TAG adds a suffix to the work directory.
# A variant is a name from the table below, or several joined by +: lines added to the end of the groups, the same on both
# sides. Files go to work/ounp/<case>__<nml>[_variant]/{ref,rust}.
# Exit 0 only when every file, the log and the status are identical; exit 3 when Rust refused the input (status 64 and
# "not implemented") and the reference ran; exit 2 when the case has no input.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
variant="${2:-}"
case "$case_name" in
  ww3_*) reg="${case_name%_leg*}"; reg="${reg%_curv}"; reg="${reg%_x}"; nml_default=ww3_ounp.nml ;;
  ts2) reg=ww3_ts2; nml_default=ww3_ounp.nml ;;
  *) reg=ww3_ts1; nml_default=ww3_ounp_spec.nml ;;
esac
nml="${OUNP_NML:-$nml_default}"
stem="${nml%.nml}"
tag="${case_name}__${stem#ww3_}${variant:+_$variant}${OUNP_TAG:+_$OUNP_TAG}"
src="${OUNP_SRC:-work/pair/$case_name}"
[ -f "$src/out_pnt.reference" ] || { echo "$tag: no out_pnt.reference in $src" >&2; exit 2; }
[ -f "WW3/regtests/$reg/input/$nml" ] || { echo "$tag: no $nml in WW3/regtests/$reg/input" >&2; exit 2; }
work="work/ounp/$tag"
mkdir -p "$work"
find "$work" -mindepth 1 -delete
padd=""; fadd=""; sadd=""; qadd=""; oadd=""
add() { local v; v="${!1}"; printf -v "$1" '%s' "${v:+$v; }$2"; }
for one in ${variant//+/ }; do
  case "$one" in
    nc4) add fadd "FILE%NETCDF = 4" ;;
    sep) add padd "POINT%SAMEFILE = F" ;;
    together) add padd "POINT%SAMEFILE = T" ;;
    noorder) add padd "POINT%DIMORDER = F" ;;
    type2) add sadd "SPECTRA%TYPE = 2" ;;
    type3) add sadd "SPECTRA%TYPE = 3" ;;
    type4) add sadd "SPECTRA%TYPE = 4" ;;
    split0) add padd "POINT%TIMESPLIT = 0" ;;
    split1) add padd "POINT%TIMESPLIT = 1" ;;
    split4) add padd "POINT%TIMESPLIT = 4" ;;
    split5) add padd "POINT%TIMESPLIT = 5" ;;
    split7) add padd "POINT%TIMESPLIT = 7" ;;
    split8) add padd "POINT%TIMESPLIT = 8" ;;
    split10) add padd "POINT%TIMESPLIT = 10" ;;
    split12) add padd "POINT%TIMESPLIT = 12" ;;
    split9) add padd "POINT%TIMESPLIT = 9" ;;
    prefix) add fadd "FILE%PREFIX = 'run1_'" ;;
    baddir) add fadd "FILE%PREFIX = 'nodir/ww3.'" ;;
    list312) add padd "POINT%LIST = '3 1 2'" ;;
    list3) add padd "POINT%LIST = '3'" ;;
    listdup) add padd "POINT%LIST = '2 2 1 2'" ;;
    listbig) add padd "POINT%LIST = '1 2 999'" ;;
    list0) add padd "POINT%LIST = '999'" ;;
    count3) add padd "POINT%TIMECOUNT = '3'" ;;
    count1) add padd "POINT%TIMECOUNT = '1'" ;;
    stride0) add padd "POINT%TIMESTRIDE = '0'" ;;
    stride7200) add padd "POINT%TIMESTRIDE = '7200.'" ;;
    stride86400) add padd "POINT%TIMESTRIDE = '86400.'" ;;
    stride21600) add padd "POINT%TIMESTRIDE = '21600.'" ;;
    buf1) add padd "POINT%BUFFER = 1" ;;
    buf2) add padd "POINT%BUFFER = 2" ;;
    buf0) add padd "POINT%BUFFER = 0" ;;
    par1) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 1" ;;
    par2) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 2" ;;
    par3) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 3" ;;
    par4) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 4" ;;
    par5) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 5" ;;
    par6) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 6" ;;
    par7) add padd "POINT%TYPE = 2"; add qadd "PARAM%OUTPUT = 7" ;;
    scale) add sadd "SPECTRA%SCALE_FAC = 2.5"; add sadd "SPECTRA%OUTPUT_FAC = 3.14159" ;;
    source) add oadd "SOURCE%OUTPUT = 2"; add oadd "SOURCE%SCALE_FAC = 0.5"; add oadd "SOURCE%OUTPUT_FAC = 1.25"
            add oadd "SOURCE%TABLE_FAC = 2"; add oadd "SOURCE%BOTTOM = F"; add oadd "SOURCE%ICE = F" ;;
    spec1) add sadd "SPECTRA%OUTPUT = 1" ;;
    spec2) add sadd "SPECTRA%OUTPUT = 2" ;;
    spec4) add sadd "SPECTRA%OUTPUT = 4" ;;
    spec5) add sadd "SPECTRA%OUTPUT = 5" ;;
    type1src) add padd "POINT%TYPE = 3" ;;
    type4pt) add padd "POINT%TYPE = 4" ;;
    nc2) add fadd "FILE%NETCDF = 2" ;;
    start2) add padd "POINT%TIMESTART = '19680607 120000'" ;;
    startlate) add padd "POINT%TIMESTART = '19990101 000000'" ;;
    startoff) add padd "POINT%TIMESTART = '19680606 003000'" ;;
    "" | x_*) ;;   # x_<label>: a label for a run with another OUNP_NML; the namelist is as in the regtest
    *) echo "unknown variant $one" >&2; exit 64 ;;
  esac
done
# Appends the lines before the closing / of the group; a group the file lacks is added.
edit() {
  awk -v pa="$padd" -v fa="$fadd" -v sa="$sadd" -v qa="$qadd" -v oa="$oadd" '
    function emit(txt,   n, a, i) { n = split(txt, a, "; "); for (i = 1; i <= n; i++) if (a[i] != "") print "  " a[i] }
    /^&POINT_NML/ { g = "P" } /^&FILE_NML/ { g = "F" } /^&SPECTRA_NML/ { g = "S" } /^&PARAM_NML/ { g = "Q" } /^&SOURCE_NML/ { g = "O" }
    /^\// && g != "" { emit(g == "P" ? pa : g == "F" ? fa : g == "S" ? sa : g == "Q" ? qa : oa); g = "" }
    { print }' "$1"
}
for side in ref rust; do
  mkdir -p "$work/$side"
  ln -s "/work/${src#work/}/mod_def.ww3" "$work/$side/mod_def.ww3"
  ln -s "/work/${src#work/}/out_pnt.reference" "$work/$side/out_pnt.ww3"
  edit "WW3/regtests/$reg/input/$nml" > "$work/$side/ww3_ounp.nml"
done
[ -z "${OUNP_PREP_ONLY:-}" ] || exit 0
# One retry: now and then the container does not see the directory the host has just filled (a file-sharing delay).
# A run that does not end in OUNP_LIMIT seconds (60) is stopped (status 124): the Fortran has inputs that loop for ever.
limit="${OUNP_LIMIT:-60}"
run_ref() { bash scripts/rust_netcdf.sh --run-ref bash -c "cd /work/ounp/$tag/ref && timeout $limit ww3_ounp > ounp.log 2> ounp.err; echo \$? > exit.code"; }
run_ref || { echo "$tag: the reference run failed, once more" >&2; sleep 3; run_ref; }
set +e
NETCDF_TIMEOUT="$limit" bash scripts/rust_netcdf.sh ww3_ounp "/work/ounp/$tag/rust" > "$work/rust/ounp.log" 2> "$work/rust/ounp.err"
echo $? > "$work/rust/exit.code"
set -e
if [ "$(cat "$work/rust/exit.code")" = 64 ] && grep -q 'not implemented' "$work/rust/ounp.err"; then
  echo "$tag: REFUSED by Rust ($(grep 'not implemented' "$work/rust/ounp.err" | head -1 | sed 's/^ww3_ounp: not implemented: //'))"
  exit 3
fi
if [ "$(cat "$work/ref/exit.code")" = 124 ] && [ "$(cat "$work/rust/exit.code")" = 124 ]; then
  echo "$tag: both stopped at the ${limit} s limit (status 124): the Fortran does not end on this input and Rust does not either"
  exit 4
fi
bad=0; files=0; bytes=0
skip='^(mod_def\.ww3|out_pnt\.ww3|ww3_ounp\.nml|ounp\.err)$'
for f in "$work"/ref/*; do
  b=$(basename "$f")
  [[ "$b" =~ $skip ]] && continue
  files=$((files + 1))
  bytes=$((bytes + $(wc -c < "$f")))
  if [ -f "$work/rust/$b" ] && cmp -s "$f" "$work/rust/$b"; then
    :
  elif [ -f "$work/rust/$b" ]; then
    echo "$tag $b: DIFFERENT, $(cmp -l "$f" "$work/rust/$b" 2>/dev/null | wc -l | tr -d ' ') of $(wc -c < "$f" | tr -d ' ') bytes (Rust wrote $(wc -c < "$work/rust/$b" | tr -d ' '))"
    bad=1
  else
    echo "$tag $b: NOT WRITTEN by Rust ($(grep -E 'panicked|rror' "$work/rust/ounp.err" | head -1))"
    bad=1
  fi
done
for f in "$work"/rust/*; do
  b=$(basename "$f")
  [[ "$b" =~ $skip ]] && continue
  [ -f "$work/ref/$b" ] || { echo "$tag $b: written by Rust only"; bad=1; }
done
[ -s "$work/ref/ounp.err" ] && echo "$tag: the reference wrote to standard error: $(head -1 "$work/ref/ounp.err")"
if [ "$bad" -eq 0 ]; then
  nc=$(find "$work/ref" -name '*.nc' | wc -l | tr -d ' ')
  echo "$tag: identical; $nc nc files + nml log + stdout + status ($(cat "$work/ref/exit.code")), $bytes bytes"
fi
exit "$bad"
