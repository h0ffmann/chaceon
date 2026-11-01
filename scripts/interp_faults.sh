#!/usr/bin/env bash
# A comparison that cannot fail proves nothing: one fault in each stage of the interpolating ww3_prep and of
# ww3_strt type 2 goes through the pair that should catch it, and is then restored. Each fault is a perl
# substitution on one source file, so the script edits rust/crates/ww3-host/src while it runs and puts the file
# back from a copy (checked by cmp) after every fault. Do not run another pair at the same time: its build
# would carry the fault. Exit 0 when every fault is caught (the pair is DIFFERENT, or the exit status differs).
# Usage: interp_faults.sh [name-pattern]
set -euo pipefail
cd "$(dirname "$0")/.."
pat="${1:-.}"
src=rust/crates/ww3-host/src
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
bad=0
# nth FILE N OLD NEW: replace the Nth occurrence of the literal OLD with NEW.
# shellcheck disable=SC2016 # the perl program is single-quoted on purpose
nth() { printf 'my $n = 0; s~(\\Q%s\\E)~++$n == %s ? q(%s) : $1~ge' "$3" "$2" "$4"; }
# fault NAME FILE PERL-EXPRESSION COMMAND...: the command is a pair script with its arguments.
fault() {
  local name="$1" file="$2" expr="$3"
  shift 3
  [[ "$name" =~ $pat ]] || return 0
  cp "$file" "$tmp/orig"
  # The faulty file is written to a copy first: a perl error must not leave the source empty.
  if ! perl -0777 -pe "$expr" "$tmp/orig" > "$tmp/faulty"; then
    echo "$name: the perl substitution failed"
    bad=1
    return
  fi
  if cmp -s "$tmp/faulty" "$tmp/orig"; then
    echo "$name: the edit changed nothing"
    bad=1
    return
  fi
  cp "$tmp/faulty" "$file"
  local rc=0
  env PREP_TAG="fault_$name" STRT_TAG="fault_$name" "$@" > "$tmp/out" 2>&1 || rc=$?
  cp "$tmp/orig" "$file"
  cmp -s "$file" "$tmp/orig" || { echo "$name: the file was not restored"; exit 2; }
  local line
  line=$(grep -E 'DIFFERENT|STATUS DIFFERS|stops with status|REFUSED|NOT WRITTEN' "$tmp/out" | head -1 | sed 's/; ww3_.*//' | cut -c1-140 || true)
  if [ "$rc" = 0 ]; then
    echo "$name: NOT CAUGHT ($*)"
    bad=1
  else
    echo "$name: caught, ${line:-exit $rc}"
  fi
}
P=$src/prep.rs
G=$src/gridsru.rs
S=$src/strt.rs
OVER=(env PREP_OVERLAY=regtests/prep/input bash scripts/prep_pair.sh)

# ---- ww3_prep: the input arrays
fault bin_le "$P" "$(nth x 1 'f32::from_be_bytes(w)' 'f32::from_le_bytes(w)')" "${OVER[@]}" ww3_ts3 ww3_prep_ai_bin2.inp
fault hdr_skip "$P" "$(nth x 1 'Ok(rec) if rec.len() >= 24 &&' 'Ok(rec) if false && rec.len() >= 24 &&')" "${OVER[@]}" ww3_ts3 ww3_prep_ai_ww3hdr.inp
fault rev_start "$P" "$(nth x 1 'at = fmt.rev;' 'at = 0;')" "${OVER[@]}" ww3_ts3 ww3_prep_ai_revgrp.inp
fault comma_width "$P" "$(nth x 1 '*pos += k + 1;' '*pos += w;')" env PREP_INP=prep/variants/comma.inp bash scripts/prep_pair.sh ww3_ts3 ww3_prep_wind.inp
fault rows_idla3 "$P" "$(nth x 1 '(0..ny).rev().collect() };' '(0..ny).collect() };')" "${OVER[@]}" ww3_ts3 ww3_prep_ai_bin3.inp
fault dat_nd "$P" "$(nth x 1 '[time[0], time[1], ndat as i32]' '[time[0], time[1], 0]')" "${OVER[@]}" ww3_ts3 ww3_prep_dat_0.inp
# ---- ww3_prep: the weights
fault ll_rd12 "$P" "$(nth x 1 '(rd.rd11[k], rd.rd21[k], rd.rd12[k], rd.rd22[k]) = (g.rw[0], g.rw[1], g.rw[3], g.rw[2])' '(rd.rd11[k], rd.rd21[k], rd.rd12[k], rd.rd22[k]) = (g.rw[0], g.rw[1], g.rw[2], g.rw[3])')" "${OVER[@]}" ww3_tp2.2 ww3_prep_ll_wnd.inp sph
fault ll_iy2 "$P" "$(nth x 1 '(rd.ix1[k], rd.ix2[k], rd.iy1[k], rd.iy2[k]) = (g.is[0], g.is[1], g.js[0], g.js[3])' '(rd.ix1[k], rd.ix2[k], rd.iy1[k], rd.iy2[k]) = (g.is[0], g.is[1], g.js[0], g.js[0])')" "${OVER[@]}" ww3_tp2.2 ww3_prep_ll_wnd.inp sph
fault ll_closure "$P" "$(nth x 1 'iclo = 2;' 'iclo = -1;')" "${OVER[@]}" ww3_tp2.2 ww3_prep_llg_wnd.inp glob
fault ll_outside "$P" "$(nth x 1 'reached.outside += 1;' 'reached.outside += 1; rd.rd11[k] = 1.;')" "${OVER[@]}" ww3_tp2.2 ww3_prep_ll_outside.inp sph
fault factor_cap "$P" "$(nth x 1 'let factor = if r < 1.5 { r } else { 1.5 };' 'let factor = if r < 1.5 { r } else { 1.4 };')" "${OVER[@]}" ww3_tp2.2 ww3_prep_ll_chk.inp sph
fault f1_partial "$G" "$(nth x 1 '*d /= dsum;' '*d *= 1.;')" "${OVER[@]}" ww3_tp2.2 ww3_prep_f1_ice.inp sph
fault f1_nn_ix2 "$P" "$(nth x 1 'w.ix1[k] = g.is[1];' 'w.ix2[k] = g.is[1];')" "${OVER[@]}" ww3_tp2.2 ww3_prep_f1_ice.inp sph
fault f1_nn_sort "$G" "$(nth x 1 'for k in 0..ns.saturating_sub(1) {' 'for k in 0..0 {')" "${OVER[@]}" ww3_tp2.2 ww3_prep_f1_bigland.inp sph
fault f1_dist "$G" "$(nth x 1 'R2D * 2. * argd.asin()' 'R2D * 2. * argd')" "${OVER[@]}" ww3_tp2.2 ww3_prep_f1_bigland.inp sph
fault f1_cart_dist "$G" "$(nth x 1 '(dx * dx + dy * dy).sqrt()' '(dx * dx + dy * dx).sqrt()')" "${OVER[@]}" ww3_ts3 ww3_prep_f1c_bigland.inp
fault f2_mean "$P" "$(nth x 1 'rd.rd11[k] *= factor;' 'rd.rd11[k] *= 1.;')" "${OVER[@]}" ww3_tp2.2 ww3_prep_f2_ice.inp sph
fault f2_second "$P" "$(nth x 1 'xd.rd22[k] *= factor;' 'xd.rd22[k] *= 1.;')" "${OVER[@]}" ww3_tp2.2 ww3_prep_f2_lev.inp sph
# ---- ww3_strt type 2
fault j_f5 "$S" "$(nth x 1 'let f5 = f2 * (f * f2);' 'let f5 = (f2 * f2) * f;')" env STRT_INP=strt/variants/gamma3.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_fphigh "$S" "$(nth x 1 'fp = (TPIINV * sig(nk)).min(fp);' 'fp = fp;')" env STRT_INP=strt/variants/fphigh.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_fplow "$S" "$(nth x 2 'fp = (0.5 * TPIINV * sig(1)).max(fp);' 'fp = fp;')" env STRT_INP=strt/variants/fplow.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_sigb "$S" "$(nth x 1 'let sig = if f <= fp { siga } else { sigb };' 'let sig = if f <= fp { sigb } else { siga };')" env STRT_INP=strt/variants/sigs.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_beta "$S" "$(nth x 1 'let beta: Real = if fr < fp { 4.06 } else { -2.34 };' 'let beta: Real = if fr < fp { -2.34 } else { 4.06 };')" env STRT_INP=strt/variants/gamma3.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_thm "$S" "$(nth x 2 'thm = (630. - thm) % 360. * DERA;' 'thm = (630. - thm) * DERA;')" env STRT_INP=strt/variants/thm45.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_thmmod "$S" "$(nth x 2 'thm %= 360.;' 'thm = thm;')" env STRT_INP=strt/variants/thmbig.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_thmneg "$S" "$(nth x 2 'while thm < 0. {' 'while thm < -1000. {')" env STRT_INP=strt/variants/thmneg.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_yln "$S" "$(nth x 1 'let yln = logf(gamma);' 'let yln = logf(gamma) * 1.0000001;')" env STRT_INP=strt/variants/gamma3.inp bash scripts/strt_pair.sh ww3_tp2.2
fault j_default "$S" "$(nth x 1 'siga = 0.07;' 'siga = 0.08;')" env STRT_INP=strt/variants/dfltg.inp bash scripts/strt_pair.sh ww3_tp2.2
exit "$bad"
