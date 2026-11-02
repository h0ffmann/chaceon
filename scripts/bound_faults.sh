#!/usr/bin/env bash
# Each fault changes one line of the boundary code, and the case named with it must then differ from the reference: a fault that
# stays identical is a line no case reaches. The sources are restored after each one, and BF_SKIP lists tags to leave out.
# Exit 0 only when every fault was caught.
set -euo pipefail
cd "$(dirname "$0")/.."
i=work/bound_in
k=rust/crates/ww3-kernels/src/pro3.rs
o=rust/crates/ww3-host/src/iobc.rs
c=rust/crates/ww3-host/src/cspc.rs
g=rust/crates/ww3-host/src/gridsru.rs
s=rust/crates/ww3-host/src/shel.rs
b=rust/crates/ww3-host/src/bound.rs
back=work/bound_in/backup; mkdir -p "$back"
bad=0
fault() { # file python-old python-new tag [environment]
  local f="$1" old="$2" new="$3" tag="$4"; shift 4
  case " ${BF_SKIP:-} " in *" $tag "*) return ;; esac
  cp "$f" "$back/$(basename "$f")"
  python3 - "$f" "$old" "$new" <<'P'
import sys
p, a, b = sys.argv[1:]
s = open(p).read()
assert a in s, a
open(p, "w").write(s.replace(a, b, 1))
P
  if out=$(env "$@" bash scripts/bound_pair.sh "f_$tag" 2>&1); then echo "$tag: NOT CAUGHT"; bad=1
  else echo "$tag: caught, $(echo "$out" | grep -c DIFFERENT) comparisons"; fi
  cp "$back/$(basename "$f")" "$f"
}
fault "$k" "dtg * (ntloc - itloc) as Real / ntloc as Real" "dtg * (itloc - 1) as Real / ntloc as Real" subtime BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
fault "$k" "rd2 = 1f32.min(0f32.max(rd1 / rd2));" "rd2 = 1.;" notime BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
fault "$k" "/ t.cg[ik as usize + nk2 * isea as usize]" "/ t.cg[1 + nk2 * isea as usize]" cgik BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
fault "$k" "        bnd.update(p, t, ik, isp, ntloc, itloc, dtg, oq, vq);" "        if itloc == ntloc { bnd.update(p, t, ik, isp, ntloc, itloc, dtg, oq, vq); }" lastsub BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
fault "$o" "v(0) + v(1) + v(2) + v(3)" "v(0) + v(1)" twoweights BP_SHEL=$i/full.sed BP_GRID=$i/gchild.sed BP_NEST=bound/parent/ref/nest01.ww3
fault "$o" "            b0.copy_from_slice(&bn);" "            b0.copy_from_slice(&bn); b0[nspec * 2] = 0.;" shift BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
fault "$o" "(th1i - m.th[0]).abs() > 0.01 * m.dth" "false" nospconv BP_SHEL=$i/full.sed BP_OVERLAY=$i/rot
fault "$c" "sp2[j1 + (j2 - 1) * self.nth2 + n2 * isp] = self.ftl *" "sp2[j1 + (j2 - 1) * self.nth2 + n2 * isp] = 0.5 *" tail BP_SHEL=$i/full.sed BP_GRID=$i/gspec.sed BP_NEST=bound/edge/ref/nest.ww3
fault "$c" "let frac = r2 * self.frq1[i2] * self.xdf1 * r1 * self.dth1;" "let frac = r2 * self.frq1[i2] * self.xdf1 * (r1 * self.dth1);" fracorder BP_SHEL=$i/full.sed BP_GRID=$i/gspec.sed BP_NEST=bound/edge/ref/nest.ww3
fault "$g" "(dmin, ix, iy) = (dd, is[l], js[l]);" "(dmin, ix, iy) = (dd, is[0], js[0]);" gfpt BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
fault "$s" "                    if nbc.read(m, &mapsta) != 1 {
                        nbc.w3ubpt(m, &cg);
                    }" "                    if nbc.read(m, &mapsta) != 1 {
                        nbc.w3ubpt(m, &cg);
                    }
                    break;" onceread BP_SHEL=$i/late.sed BP_OVERLAY=$i/edge
fault "$s" "let bnd = nbc.flbpi.then(|| nbc.at(nspec, time));" "let bnd = (nbc.tbpin[0] != -1).then(|| nbc.at(nspec, time));" noclear BP_SHEL=$i/eof.sed BP_OVERLAY=$i/edge
fault "$s" "f.off = f.last;" "f.off = true;" offearly BP_SHEL=$i/level.sed BP_OVERLAY=$i/edge
fault "$b" "(fq(ifmin + 1) / fq(ifmin))" "(fq(ifmin + 1) / fq(ifmin) + 1.)" xfrtest BP_NORUN=1 BP_OVERLAY=$i/edge
fault "$b" "if (theta[0] - 0.5 * PI).abs() < 0.01 {" "if false {" thfix BP_NORUN=1 BP_OVERLAY=$i/edge
fault "$b" "rdbpo[ip1 + nbo] = (1f32).min((0f32).max(cos1));" "rdbpo[ip1 + nbo] = cos1;" clamp BP_NORUN=1 BP_OVERLAY=$i/two
exit "$bad"
