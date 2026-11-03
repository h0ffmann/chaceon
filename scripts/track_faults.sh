#!/usr/bin/env bash
# One fault at a time through W3IOTR, ww3_trck and ww3_trnc: the named case must differ from the reference. Needs track_all.sh's cases.
set -euo pipefail
cd "$(dirname "$0")/.."
i=work/track_in
t=rust/crates/ww3-host/src/iotr.rs
k=rust/crates/ww3-host/src/trck.rs
back=$i/backup; mkdir -p "$back"
bad=0
fault() { # file old new tag grid-suffix overlay sed
  local f="$1" old="$2" new="$3" tag="$4" sfx="$5" ov="$6" sd="$7"
  cp "$f" "$back/$(basename "$f")"
  python3 - "$f" "$old" "$new" <<'P'
import sys
p, a, b = sys.argv[1:]
s = open(p).read()
assert a in s, a
open(p, "w").write(s.replace(a, b, 1))
P
  if out=$(TP_TAG="f_$tag" TP_OVERLAY="$ov" TP_SHEL="$sd" bash scripts/tp_pair.sh ww3_tp2.2 ${sfx:+"$sfx"} 2>&1); then echo "$tag: NOT CAUGHT"; bad=1
  else echo "$tag: caught, $(echo "$out" | grep -c DIFFERENT) comparisons"; fi
  cp "$back/$(basename "$f")" "$f"
}
fault "$t" "ixx.swap(2, 3);" "" swap "" $i/many $i/t22.sed
fault "$t" "if rd < RDCHCK {
                        ixx[1] = ixx[0];" "if rd < 0. {
                        ixx[1] = ixx[0];" cmprx "" $i/many $i/t22.sed
fault "$t" "iyy[0] = iyy[2];" "iyy[0] = iyy[0];" cmpry "" $i/many $i/t22.sed
fault "$t" "let mut mask1 = self.mask2.clone();" "let mut mask1 = vec![false; self.mask2.len()];" carry "" $i/many $i/t22.sed
fault "$t" "self.peek = Some(e);" "" backspace "" $i/many $i/t22.sed
fault "$t" "dsec21(timeb, e.time) < 0." "false" early "" $i/many $i/t22.sed
fault "$t" "if m.cmprtrck {" "if true {" nocmpr nocmpr $i/many $i/t22.sed
fault "$t" "if dsec21(timee, tolast) < 0. {" "if false {" tolast "" $i/many $i/t22w.sed
# ww3_trck and ww3_trnc, on an output the reference already wrote.
tool() { # file old new tag
  local f="$1" old="$2" new="$3" tag="$4"
  cp "$f" "$back/$(basename "$f")"
  python3 - "$f" "$old" "$new" <<'P'
import sys
p, a, b = sys.argv[1:]
s = open(p).read()
assert a in s, a
open(p, "w").write(s.replace(a, b, 1))
P
  case "$f" in
    *trck.rs)
      docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry -e CARGO_TARGET_DIR=/rust/target/linux -w /rust rust:1.90-bookworm cargo build --release -q --bin ww3_trck
      d=work/trck/ww3_tp2.2_many
      docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" --entrypoint /rust/target/linux/release/ww3_trck chaceon-ref:switch_chaceon_shrd /work/trck/ww3_tp2.2_many/rust > /dev/null
      if cmp -s "$d/ref/track.ww3" "$d/rust/track.ww3"; then echo "$tag: NOT CAUGHT"; bad=1; else echo "$tag: caught"; fi ;;
    *)
      bash scripts/rust_netcdf.sh --build ww3_trnc
      d=work/trnc/many__base
      rm -f "$d"/rust/*.nc
      NETCDF_SKIP_BUILD=1 bash scripts/rust_netcdf.sh ww3_trnc /work/trnc/many__base/rust > /dev/null
      if cmp -s "$d"/ref/*.nc "$d"/rust/*.nc; then echo "$tag: NOT CAUGHT"; bad=1; else echo "$tag: caught"; fi ;;
  esac
  cp "$back/$(basename "$f")" "$f"
}
n=rust/tools/ww3_trnc/src/main.rs
tool "$k" 'format!(" {n}*0")' 'format!(" {}*0", n + 1)' zeros
tool "$k" "if self.s.len() + b.len() > LINELN {" "if self.s.len() + b.len() > LINELN + 2 {" lineln
tool "$k" "if part.as_bytes()[(10 - iwdth) as usize] == b' ' {" "if false {" width
tool "$n" "t[2] + (153 * c + 2) / 5 + 365 * b" "t[2] + (153 * c + 2) / 5 + 365 * b + b / 4" leap
tool "$n" "let wdo = if wao > 1.0e-7 { (270. - atan2f(wy, wx) * RADE) % 360. }" "let wdo = if wao > 1.0e-7 { 270. - atan2f(wy, wx) * RADE }" wdomod
tool "$n" "(rth0 + i as Real)) % 360.)" "(rth0 + i as Real)))" dirmod
tool "$n" "t.push(r.spec[ik + ith * mk]);" "t.push(r.spec[ik * mth + ith]);" transpose
exit "$bad"
