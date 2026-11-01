#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_outf: the reference runs ww3_outf on a regtest input and the out_grd.ww3 of a
# case under work/pair/, Rust's ww3_outf runs on the same files, and cmp judges the standard output log, the exit
# code and every file the program wrote (tabNN.ww3, ww3.*.<field>, ...).
# Usage: outf_pair.sh <case under work/pair> <input> [name]. <input> is a path under WW3/regtests (an input of
# WW3) or a file under regtests/outpf_inputs (an input written here). OUTF_TAG adds a suffix to the work
# directory (a fault run names itself). Exit 0 when everything is byte-identical, 1 when something differs,
# 2 when Rust refused the option (a listed line, never silent).
set -euo pipefail
cd "$(dirname "$0")/.."
tag="$1"
inp="$2"
stem=$(basename "$inp" .inp)
name="${3:-${tag}__${stem}}${OUTF_TAG:+_$OUTF_TAG}"
if [ -f "regtests/outpf_inputs/$inp" ]; then src="$PWD/regtests/outpf_inputs/$inp"; else src="$PWD/WW3/regtests/$inp"; fi
[ -f "$src" ] || { echo "$name: no input $inp" >&2; exit 64; }
[ -f "work/pair/$tag/out_grd.reference" ] || { echo "$name: work/pair/$tag/out_grd.reference is missing" >&2; exit 64; }
d="work/outf/$name"
mkdir -p "$d/ref" "$d/rust"
find "$d" -mindepth 2 -delete
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" -v "$src:/inp:ro" \
  -e OUT="/work/outf/$name/ref" -e TAG="$tag" --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
  mkdir -p /run/c && cd /run/c
  ln -s /work/pair/$TAG/mod_def.ww3 mod_def.ww3
  ln -s /work/pair/$TAG/out_grd.reference out_grd.ww3
  cp /inp ww3_outf.inp
  set +e; ww3_outf > outf.log 2>&1; echo $? > exit.code; set -e
  for f in *; do [ -L "$f" ] || [ "$f" = ww3_outf.inp ] || cp "$f" "$OUT/"; done'
ln -s "/work/pair/$tag/mod_def.ww3" "$d/rust/mod_def.ww3"
ln -s "/work/pair/$tag/out_grd.reference" "$d/rust/out_grd.ww3"
cp "$src" "$d/rust/ww3_outf.inp"
set +e
bash scripts/rust_in_ref.sh ww3_outf "/work/outf/$name/rust" > "$d/rust/outf.log" 2> "$d/rust.err"
echo $? > "$d/rust/exit.code"
set -e
if [ "$(cat "$d/rust/exit.code")" = 64 ] && grep -q 'not implemented' "$d/rust.err"; then
  echo "$name: REFUSED, $(grep 'not implemented' "$d/rust.err" | head -1 | sed 's/^ww3_outf: //')"
  exit 2
fi
bad=0; files=0; bytes=0; list=""
for f in "$d"/ref/*; do
  b=$(basename "$f")
  files=$((files + 1))
  bytes=$((bytes + $(wc -c < "$f")))
  if [ -f "$d/rust/$b" ] && cmp -s "$f" "$d/rust/$b"; then
    list="$list $b"
  else
    if [ -f "$d/rust/$b" ]; then
      echo "$name $b: DIFFERENT, $(cmp -l "$f" "$d/rust/$b" 2>/dev/null | wc -l | tr -d ' ') of $(wc -c < "$f" | tr -d ' ') bytes (Rust wrote $(wc -c < "$d/rust/$b" | tr -d ' '))"
    else
      echo "$name $b: NOT WRITTEN by Rust ($(grep -E 'panicked|rror' "$d/rust.err" | head -1))"
    fi
    bad=1
  fi
done
for f in "$d"/rust/*; do
  b=$(basename "$f")
  case "$b" in ww3_outf.inp|mod_def.ww3|out_grd.ww3) continue;; esac
  [ -f "$d/ref/$b" ] || { echo "$name $b: WRITTEN by Rust only"; bad=1; }
done
if [ "$bad" = 0 ]; then
  echo "$name: identical, $files files ($(echo "$list" | tr -s " " "," | sed "s/^,//"); outf.log is the stdout log, exit.code the exit status), $bytes bytes"
  exit 0
fi
exit 1
