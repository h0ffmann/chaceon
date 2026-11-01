#!/usr/bin/env bash
# Runs ounp_pair.sh on every case of work/pair that has an out_pnt.reference and a ww3_ounp namelist in its regtest input:
# ww3_ounp.nml, and ww3_ounp_spec.nml and ww3_ounp_par.nml where the regtest has them (ww3_ts1 and ww3_tp2.15 do).
# Usage: ounp_all.sh [variant]. Prints one line per run and a summary; exit 0 only when every run is identical or refused.
# The Rust binary is built once.
set -euo pipefail
cd "$(dirname "$0")/.."
variant="${1:-}"
bash scripts/rust_netcdf.sh --build ww3_ounp
export NETCDF_SKIP_BUILD=1
ok=0; refused=0; bad=0; skipped=0; ncf=0; nbytes=0
failed=""; refusedlist=""; skiplist=""
# The cases of work/pair, and the ones scripts/ounp_src.sh made (ts2).
for d in work/pair/*/ work/ounp_src/*/; do
  [ -d "$d" ] || continue
  c=$(basename "$d")
  [ -f "$d/out_pnt.reference" ] || continue
  src=""
  case "$d" in work/ounp_src/*) src="${d%/}" ;; esac
  case "$c" in
    ww3_*) reg="${c%_leg*}"; reg="${reg%_curv}"; reg="${reg%_x}" ;;
    ts2) reg=ww3_ts2 ;;
    *) reg=ww3_ts1 ;;
  esac
  found=0
  for nml in ww3_ounp.nml ww3_ounp_spec.nml ww3_ounp_par.nml; do
    [ -f "WW3/regtests/$reg/input/$nml" ] || continue
    found=1
    rc=0
    out=$(OUNP_SRC="$src" OUNP_NML="$nml" bash scripts/ounp_pair.sh "$c" ${variant:+"$variant"}) || rc=$?
    echo "$out"
    case "$rc" in
      0) ok=$((ok + 1))
         ncf=$((ncf + $(echo "$out" | sed -n 's/.*identical; \([0-9]*\) nc files.*/\1/p')))
         nbytes=$((nbytes + $(echo "$out" | sed -n 's/.*status ([0-9]*), \([0-9]*\) bytes.*/\1/p'))) ;;
      3) refused=$((refused + 1)); refusedlist="$refusedlist $c/$nml" ;;
      *) bad=$((bad + 1)); failed="$failed $c/$nml" ;;
    esac
  done
  if [ "$found" -eq 0 ]; then skipped=$((skipped + 1)); skiplist="$skiplist $c"; fi
done
echo "ounp-all${variant:+ ($variant)}: $ok runs identical ($ncf NetCDF files, $nbytes bytes with the logs), $refused refused${refusedlist:+ ($refusedlist )}, $bad not identical${failed:+:$failed}; $skipped cases without a ww3_ounp input${skiplist:+ ($skiplist )}"
[ "$bad" -eq 0 ]
