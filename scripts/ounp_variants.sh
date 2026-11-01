#!/usr/bin/env bash
# Runs ounp_pair.sh on a set of cases with each namelist variant of that script and prints a summary. The variants cover
# the types and flags the regtests do not reach: FILE%NETCDF 3 and 4, SAMEFILE, DIMORDER, SPECTRA%TYPE 2/3/4,
# TIMESPLIT 0..12, the point list (order, duplicates, out of range, none), TIMECOUNT, TIMESTRIDE, TIMESTART, BUFFER,
# POINT%TYPE 2 with PARAM%OUTPUT 1/2/5/6, and the inputs the program refuses or stops on (exit 20, 43, 46).
# Usage: ounp_variants.sh [case:nml:full|core ...]; "full" is every variant below, "core" a third of them. The log of every run is work/ounp/variants.log.
set -euo pipefail
cd "$(dirname "$0")/.."
cases=("$@")
[ ${#cases[@]} -gt 0 ] || cases=(ww3_tp1.1:ww3_ounp.nml:full ww3_tp2.15:ww3_ounp_spec.nml:full ww3_tp1.4:ww3_ounp.nml:core ww3_tp1.7:ww3_ounp.nml:core
  ww3_tp1.8:ww3_ounp.nml:core ww3_tp1.10:ww3_ounp.nml:core ww3_tp2.2:ww3_ounp.nml:core default_2500m:ww3_ounp_spec.nml:core
  ts2:ww3_ounp.nml:core)
full=(nc4 together together+nc4 sep sep+nc4 noorder noorder+nc4 together+noorder+nc4 sep+noorder+nc4
  type2 type3 type4 type3+nc4 type2+together+nc4 scale source
  split0 split1 split4 split5 split7 split8 split10 split12 split8+sep split10+together
  prefix list312 list3 listdup listbig list0 together+list0 together+list0+nc4 together+list312 sep+list312
  count1 count3 stride0 stride7200 stride86400 stride21600 buf1 buf2 sep+buf1 sep+buf2 sep+buf2+nc4
  start2 startoff startlate
  par1 par2 par5 par6 par1+together par2+together par5+together+nc4 par6+sep par2+sep+nc4 par2+noorder+nc4
  par2+together+noorder+nc4 par1+together+list312 par5+together+list312 par6+together+listdup par2+sep+buf2
  par1+nc4+prefix par5+split8 par2+count3 par2+stride0
  split9 buf0 par3 par4 par7 spec1 spec2 spec4 spec5 type1src type4pt nc2 baddir par2+baddir)
core=(nc4 sep sep+nc4 noorder noorder+nc4 type2 type3+nc4 scale source split8 list312 list3 listdup count3 stride0 buf2 sep+buf2
  par1 par2 par2+together par5+together+nc4 par6+sep par2+noorder+nc4 par3 spec2 baddir)
log=work/ounp/variants.log
mkdir -p work/ounp
: > "$log"
bash scripts/rust_netcdf.sh --build ww3_ounp
export NETCDF_SKIP_BUILD=1
ok=0; refused=0; looped=0; bad=0; ncf=0; nbytes=0
for cn in "${cases[@]}"; do
  IFS=: read -r c nml set <<< "$cn"
  if [ "${set:-core}" = full ]; then variants=("${full[@]}"); else variants=("${core[@]}"); fi
  for v in "${variants[@]}"; do
    rc=0
    src=""; [ "$c" = ts2 ] && src=work/ounp_src/ts2
    out=$(OUNP_SRC="$src" OUNP_NML="$nml" bash scripts/ounp_pair.sh "$c" "$v" 2>&1) || rc=$?
    echo "$out" >> "$log"
    case "$rc" in
      0) ok=$((ok + 1))
         ncf=$((ncf + $(echo "$out" | sed -n 's/.*identical; \([0-9]*\) nc files.*/\1/p')))
         nbytes=$((nbytes + $(echo "$out" | sed -n 's/.*status ([0-9]*), \([0-9]*\) bytes.*/\1/p'))) ;;
      3) refused=$((refused + 1)) ;;
      4) looped=$((looped + 1)) ;;
      *) bad=$((bad + 1)); echo "NOT identical: $c $nml $v" ;;
    esac
  done
done
echo "ounp-variants: $ok identical ($ncf NetCDF files, $nbytes bytes with the logs), $refused refused, $looped looped in both, $bad not identical, of $((ok + refused + bad)) runs; log in $log"
[ "$bad" -eq 0 ]
