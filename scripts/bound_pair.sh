#!/usr/bin/env bash
# ww3_bound and a run with boundary input, the reference and Rust on the same files, by cmp: nest.ww3, the log and the
# status of ww3_bound, then out_grd.ww3, out_pnt.ww3 and restart001.ww3 of ww3_shel.
# Usage: bound_pair.sh <tag>. BP_GRID and BP_SHEL name sed scripts for ww3_grid.nml and ww3_shel.nml, BP_OVERLAY a
# directory laid over ww3_tr1/input_bndin, BP_INP=1 removes ww3_bound.nml so ww3_bound.inp is read, BP_NORUN=1 stops
# after ww3_bound, BP_NONEST=1 runs ww3_shel with no nest.ww3, BP_NEST names a file under work/ that replaces nest.ww3 before
# the run, BP_THREADS passes --threads to Rust. nest01.ww3 is compared when the grid writes one.
# Exit 0 only when every file, log and status is identical.
set -euo pipefail
cd "$(dirname "$0")/.."
tag="$1"
d="work/bound/$tag"
rm -rf "$d"; mkdir -p "$d/ref" "$d/rust/work" "$d/rust/input_bndin"
docker run --rm --platform linux/arm64 -v "$PWD/WW3/regtests/ww3_tr1:/t:ro" -v "$PWD/work:/work" -v "$PWD/regtests/bound/tr1:/base:ro" \
  ${BP_GRID:+-v "$PWD/$BP_GRID:/grid.sed:ro"} ${BP_SHEL:+-v "$PWD/$BP_SHEL:/shel.sed:ro"} ${BP_OVERLAY:+-v "$PWD/$BP_OVERLAY:/overlay:ro"} \
  -e TAG="$tag" -e BP_INP="${BP_INP:-}" -e BP_NORUN="${BP_NORUN:-}" -e BP_NONEST="${BP_NONEST:-}" -e BP_NEST="${BP_NEST:-}" \
  --entrypoint bash chaceon-ref:switch_chaceon_shrd -c '
    set -u
    mkdir -p /run/c/work; cp -R /t/input_bndin /run/c/input_bndin; cp /base/* /run/c/input_bndin/
    if [ -d /overlay ]; then cp -R /overlay/. /run/c/input_bndin/; fi
    cd /run/c/work
    cp ../input_bndin/ww3_grid.nml ../input_bndin/ww3_bound.nml ../input_bndin/ww3_bound.inp ../input_bndin/ww3_strt.inp ../input_bndin/ww3_shel.nml .
    if [ -f /grid.sed ]; then sed -i -f /grid.sed ww3_grid.nml; fi
    if [ -f /shel.sed ]; then sed -i -f /shel.sed ww3_shel.nml; fi
    if [ -n "$BP_INP" ]; then rm ww3_bound.nml; fi
    ww3_grid > grid.log 2>&1 || { echo "ww3_grid failed"; tail -5 grid.log; exit 3; }
    ww3_strt > strt.log 2>&1
    D=/work/bound/$TAG/ref
    cp mod_def.ww3 restart.ww3 $D/
    cp ww3_bound.* $D/ 2>/dev/null
    cp -R ../input_bndin/. /work/bound/$TAG/rust/input_bndin/
    ww3_bound > bound.log 2> bound.err; echo $? > $D/bound.status
    cp bound.log $D/; cp ww3_bound.nml.log $D/ 2>/dev/null
    cp ww3_shel.nml $D/
    if [ -f nest.ww3 ]; then cp nest.ww3 $D/; fi
    if [ -z "$BP_NORUN" ]; then
      if [ -n "$BP_NONEST" ]; then rm -f nest.ww3; fi
      if [ -n "$BP_NEST" ]; then cp "/work/$BP_NEST" nest.ww3; fi
      ww3_shel > shel.log 2>&1; echo $? > $D/shel.status
      for f in out_grd.ww3 out_pnt.ww3 restart001.ww3 nest01.ww3; do if [ -f $f ]; then cp $f $D/; fi; done
      cp shel.log $D/
    fi
  ' || exit 3
r="$d/rust/work"
cp "$d/ref/mod_def.ww3" "$d/ref/restart.ww3" "$r/"
for f in ww3_bound.nml ww3_bound.inp; do if [ -f "$d/ref/$f" ]; then cp "$d/ref/$f" "$r/"; fi; done
bin=/rust/target/linux/release
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/rust/target/linux -w /rust rust:1.90-bookworm cargo build --release -q --bin ww3_bound --bin ww3_shel
in_ref() { docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" --entrypoint "$bin/$1" chaceon-ref:switch_chaceon_shrd "${@:2}"; }
set +e
in_ref ww3_bound "/work/bound/$tag/rust/work" > "$d/rust/bound.log" 2> "$d/rust/bound.err"
echo $? > "$d/rust/bound.status"
set -e
bad=0
same() { # what reference rust
  if [ ! -f "$2" ] && [ ! -f "$3" ]; then return; fi
  if cmp -s "$2" "$3"; then echo "$tag $1: identical, $(wc -c < "$2" | tr -d ' ') bytes"
  else echo "$tag $1: DIFFERENT"; bad=1; fi
}
same "ww3_bound status" "$d/ref/bound.status" "$d/rust/bound.status"
same "ww3_bound log" "$d/ref/bound.log" "$d/rust/bound.log"
same nest.ww3 "$d/ref/nest.ww3" "$r/nest.ww3"
same ww3_bound.nml.log "$d/ref/ww3_bound.nml.log" "$r/ww3_bound.nml.log"
if [ -z "${BP_NORUN:-}" ]; then
  cp "$d/ref/ww3_shel.nml" "$r/"
  if [ -n "${BP_NONEST:-}" ]; then rm -f "$r/nest.ww3"; fi
  if [ -n "${BP_NEST:-}" ]; then cp "work/$BP_NEST" "$r/nest.ww3"; fi
  set +e
  in_ref ww3_shel "/work/bound/$tag/rust/work" ${BP_THREADS:+--threads "$BP_THREADS"} > "$d/rust/shel.log" 2>&1
  st=$?
  set -e
  echo "$st" > "$d/rust/shel.status"
  same "ww3_shel status" "$d/ref/shel.status" "$d/rust/shel.status"
  # A run that stopped leaves a partial out_grd.ww3 behind: its status is the verdict.
  if [ "$st" = 0 ]; then for f in out_grd out_pnt restart001 nest01; do same "$f" "$d/ref/$f.ww3" "$r/$f.ww3"; done; fi
fi
exit "$bad"
