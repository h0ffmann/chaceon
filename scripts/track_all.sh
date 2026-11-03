#!/usr/bin/env bash
# Track output: track_o.ww3 of ww3_shel with the rest of each run's files, by tp_pair.sh, then ww3_trck on every track_o.ww3, its
# track.ww3, log and status by cmp. Needs scripts/forcing_gen.py's overlay for the ice and level case. Exit 0 only when all match.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/forcing_gen.py
python3 scripts/track_gen.py
i=work/track_in
bad=0
pair() { # tag regtest overlay sed [grid suffix] [prep]
  env TP_TAG="$1" TP_OVERLAY="$3" TP_SHEL="$4" TP_PREP="${6:-}" bash scripts/tp_pair.sh "$2" ${5:+"$5"} || bad=1
}
pair many ww3_tp2.2 $i/many $i/t22.sed
pair many ww3_tp2.2 $i/many $i/t22.sed nocmpr
pair bin ww3_tp2.2 $i/bin $i/t22b.sed
pair win ww3_tp2.2 $i/many $i/t22w.sed
pair short ww3_tp2.2 $i/short $i/t22.sed
pair wrongid ww3_tp2.2 $i/wrongid $i/t22.sed
pair empty ww3_tp2.2 $i/empty $i/t22.sed
pair cart ww3_tp2.3 $i/cart $i/t23.sed
o=$i/ov_curv; rm -rf "$o"; mkdir -p "$o"; cp work/forcing/input/* work/forcing/nml_linear/* $i/curv/track_i.ww3 "$o/"
pair curv ww3_tp2.15 "$o" $i/t215.sed "" "ww3_prep_wind.inp ww3_prep_lev.inp ww3_prep_ice.inp"
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/rust/target/linux -w /rust rust:1.90-bookworm cargo build --release -q --bin ww3_trck
# The ww3_trck.inp each grid needs: the case's NK and NTH.
for t in ww3_tp2.2 ww3_tp2.2_many ww3_tp2.2_nocmpr_many ww3_tp2.2_bin ww3_tp2.2_win ww3_tp2.2_short ww3_tp2.3_cart ww3_tp2.15_curv; do
  p="work/pair/$t"; [ -f "$p/track_o.reference" ] || continue
  d="work/trck/$t"; rm -rf "$d"; mkdir -p "$d/ref" "$d/rust"
  read -r nk nth < <(python3 - "$p/track_o.reference" <<'P'
import struct, sys
b = open(sys.argv[1], "rb").read(60)
print(*struct.unpack(">ii", b[42:50]))
P
)
  for s in ref rust; do cp "$p/track_o.reference" "$d/$s/track_o.ww3"; printf '$ track\n  %s %s\n' "$nk" "$nth" > "$d/$s/ww3_trck.inp"; done
  docker run --rm --platform linux/arm64 -v "$PWD/work:/work" -w "/work/trck/$t/ref" --entrypoint bash chaceon-ref:switch_chaceon_shrd -c 'ww3_trck > log 2> err; echo $? > status' || bad=1
  docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" --entrypoint bash chaceon-ref:switch_chaceon_shrd -c "/rust/target/linux/release/ww3_trck /work/trck/$t/rust > /work/trck/$t/rust/log 2> /work/trck/$t/rust/err; echo \$? > /work/trck/$t/rust/status" || bad=1
  for f in track.ww3 log status; do
    if cmp -s "$d/ref/$f" "$d/rust/$f"; then echo "$t ww3_trck $f: identical, $(wc -c < "$d/ref/$f" | tr -d ' ') bytes"
    else echo "$t ww3_trck $f: DIFFERENT"; bad=1; fi
  done
done
# ww3_trnc: every file it wrote, its log and status.
bash scripts/rust_netcdf.sh --build ww3_trnc
trnc() { # case tag track_o [namelist lines, or inp:<text>]
  local t="$1" v="$2" o="$3" extra="${4:-}" d="work/trnc/${1}__$2" s
  rm -rf "$d"; mkdir -p "$d/ref" "$d/rust"
  for s in ref rust; do
    if [ -n "$o" ]; then cp "$o" "$d/$s/track_o.ww3"; fi
    case "$extra" in
      inp:*) printf '%b' "${extra#inp:}" > "$d/$s/ww3_trnc.inp" ;;
      *) printf '&TRACK_NML\n  TRACK%%TIMESTART = '"'"'19000101 000000'"'"'\n  TRACK%%TIMESTRIDE = '"'"'3600.'"'"'\n%b/\n&FILE_NML\n/\n' "$extra" > "$d/$s/ww3_trnc.nml" ;;
    esac
  done
  bash scripts/rust_netcdf.sh --run-ref bash -c "cd /work/trnc/${t}__$v/ref; ww3_trnc > log 2> err; echo \$? > status; rm -f err"
  NETCDF_SKIP_BUILD=1 bash scripts/rust_netcdf.sh ww3_trnc "/work/trnc/${t}__$v/rust" > "$d/rust/log" 2> "$d/rust/err" && st=0 || st=$?
  echo "$st" > "$d/rust/status"; rm -f "$d/rust/err"
  if diff -rq "$d/ref" "$d/rust" > "$d/diff"; then echo "$t $v ww3_trnc: identical, $(find "$d/ref" -type f | wc -l | tr -d ' ') files"
  else echo "$t $v ww3_trnc: DIFFERENT"; cat "$d/diff"; bad=1; fi
}
m=work/pair/ww3_tp2.2_many/track_o.reference
trnc many base $m
trnc nocmpr base work/pair/ww3_tp2.2_nocmpr_many/track_o.reference
trnc cart base work/pair/ww3_tp2.3_cart/track_o.reference
trnc curv base work/pair/ww3_tp2.15_curv/track_o.reference
trnc cart nc4 work/pair/ww3_tp2.3_cart/track_o.reference 'inp:$ track\n  19680606 000000  21600.  1000\n 4\n ww3.\n 6\n'
for s in 0 4 6 8 10 11; do trnc cart "split$s" work/pair/ww3_tp2.3_cart/track_o.reference "  TRACK%TIMESPLIT = $s\n"; done
trnc many count $m "  TRACK%TIMECOUNT = '7'\n"
trnc many inp $m 'inp:$ track\n  19680606 000000  3600.  100000\n 4\n ww3.\n 6\n'
trnc many prefix $m 'inp:$ track\n  19680606 030000  1800.  10\n 3\n trk_\n 8\n'
trnc many badtype $m 'inp:$ track\n  19680606 000000  3600.  10\n 2\n ww3.\n 6\n'
trnc many nofile "" ""
trnc many noinp $m 'inp:$ track\n'
exit "$bad"
