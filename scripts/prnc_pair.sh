#!/usr/bin/env bash
# The host verdict of ADR-0005 on ww3_prnc: the reference runs ww3_grid (once per grid, cached) and ww3_prnc on a case of
# regtests/prnc_inputs/<case>/, Rust's ww3_prnc reads the same mod_def.ww3, the same input and the same NetCDF file, and
# cmp judges every file the program wrote, its log (standard output), its standard error and its exit status.
# The reference's NetCDF inputs of the regtests are not in this repository, so each case holds a CDL file that ncgen
# turns into the NetCDF file (scripts/prnc_cdl_gen.py writes them). case.conf names the regtest whose grid is used, and
# NCGEN_KIND the file format ncgen writes (classic, 64-bit-offset or nc4; classic when absent).
# Usage: prnc_pair.sh <case>. PRNC_CASES names another directory of cases (default regtests/prnc_inputs); PRNC_TAG adds a suffix to the work directory (a fault run uses it); PRNC_SKIP_BUILD skips
# the Rust build. Work goes to work/prnc/<case>/{input,ref,ref2,rs} and work/prnc_grid/.
# The reference runs twice (ref, ref2) and the two must agree byte for byte first: a file that differs between two runs
# of the Fortran cannot judge Rust. Exit 0 when every file, the log, the error output and the status are identical;
# 3 when Rust refused (status 64) and the reference ran; 2 when the reference runs differ from each other.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="$1"
src="${PRNC_CASES:-regtests/prnc_inputs}/$case_name"
[ -f "$src/case.conf" ] || { echo "$case_name: no $src/case.conf" >&2; exit 2; }
# shellcheck source=/dev/null
. "$src/case.conf"
tag="${case_name}${PRNC_TAG:+_$PRNC_TAG}"
d="work/prnc/$tag"
gid="${REGTEST}_${GRID_NML%.nml}"
g="work/prnc_grid/$gid"
mkdir -p "$d" work/prnc_grid
find "$d" -mindepth 1 -delete

# The grid, once: ww3_grid in the reference image on the regtest's own namelist and input.
if [ ! -f "$g/mod_def.ww3" ]; then
  mkdir -p "$g"
  docker run --rm --platform linux/arm64 -v "$PWD/work:/work" -v "$PWD/WW3/regtests/$REGTEST/input:/ww3_input:ro" \
    -e GID="$gid" -e GNML="$GRID_NML" --entrypoint bash chaceon-ref-netcdf:switch_chaceon_shrd -c '
      set -e
      mkdir -p /run/c/work && cp -R /ww3_input /run/c/input && cd /run/c/work
      cp "../input/$GNML" ww3_grid.nml
      ww3_grid > grid.log 2>&1
      cp mod_def.ww3 grid.log /work/prnc_grid/$GID/'
fi

# The case: regtest input (for the grid files, as the regtest has them), then the NetCDF files from the CDL.
mkdir -p "$d/input" "$d/ref" "$d/ref2" "$d/rs"
cp -R "WW3/regtests/$REGTEST/input/." "$d/input/"
for cdl in "$src"/*.cdl; do
  [ -f "$cdl" ] || continue
  cp "$cdl" "$d/input/$(basename "$cdl")"
done
for r in ref ref2 rs; do
  cp "$g/mod_def.ww3" "$d/$r/"
  for f in ww3_prnc.nml ww3_prnc.inp; do
    [ -f "$src/$f" ] && cp "$src/$f" "$d/$r/"
  done
done
for extra in "$src"/*.dat; do
  [ -f "$extra" ] && cp "$extra" "$d/input/"
done
docker run --rm --platform linux/arm64 -v "$PWD/work:/work" -e T="$tag" -e NCK="${NCGEN_KIND:-classic}" --entrypoint bash chaceon-ref-netcdf:switch_chaceon_shrd -c '
  d=/work/prnc/$T
  for cdl in $d/input/*.cdl; do
    [ -f "$cdl" ] || continue
    ncgen -k "$NCK" -o "${cdl%.cdl}.nc" "$cdl"
  done
  for r in ref ref2; do
    cd $d/$r
    rc=0
    ww3_prnc > ../$r.stdout 2> ../$r.stderr || rc=$?
    echo $rc > ../$r.status
  done'
for ext in stdout stderr status; do
  cmp -s "$d/ref.$ext" "$d/ref2.$ext" || { echo "$tag: the reference differs from itself in its $ext"; exit 2; }
done
for f in "$d"/ref/*; do
  n=$(basename "$f")
  [ "$n" = mod_def.ww3 ] && continue
  cmp -s "$f" "$d/ref2/$n" || { echo "$tag: the reference differs from itself in $n"; exit 2; }
done

# Rust, in the reference image with its libnetcdf.
export NETCDF_SKIP_BUILD="${PRNC_SKIP_BUILD:-}"
rust_rc=0
bash scripts/rust_netcdf.sh ww3_prnc "/work/prnc/$tag/rs" > "$d/rs.stdout" 2> "$d/rs.stderr" || rust_rc=$?
echo "$rust_rc" > "$d/rs.status"
if [ "$rust_rc" = 64 ] && [ "$(cat "$d/ref.status")" != 64 ]; then
  echo "$tag: REFUSED by Rust, the reference accepts it (status $(cat "$d/ref.status")): $(grep 'not implemented' "$d/rs.stderr" | head -1)"
  exit 3
fi
bad=0
n=0
# The status, the log and the error output first, then every file the reference wrote besides its inputs.
for ext in status stdout stderr; do
  n=$((n + 1))
  if cmp -s "$d/ref.$ext" "$d/rs.$ext"; then
    echo "$tag $ext: identical, $(wc -c < "$d/ref.$ext" | tr -d ' ') bytes"
  else
    echo "$tag $ext: DIFFERENT, $(cmp -l "$d/ref.$ext" "$d/rs.$ext" 2>/dev/null | wc -l | tr -d ' ') bytes of $(wc -c < "$d/ref.$ext" | tr -d ' ') (Rust wrote $(wc -c < "$d/rs.$ext" | tr -d ' '))"
    bad=1
  fi
done
for f in "$d"/ref/*; do
  name=$(basename "$f")
  case "$name" in mod_def.ww3 | ww3_prnc.nml | ww3_prnc.inp) continue ;; esac
  n=$((n + 1))
  bytes=$(wc -c < "$f" | tr -d ' ')
  if [ -f "$d/rs/$name" ] && cmp -s "$f" "$d/rs/$name"; then
    echo "$tag $name: identical, $bytes bytes"
  elif [ -f "$d/rs/$name" ]; then
    echo "$tag $name: DIFFERENT, $(cmp -l "$f" "$d/rs/$name" 2>/dev/null | wc -l | tr -d ' ') of $bytes bytes (Rust wrote $(wc -c < "$d/rs/$name" | tr -d ' '))"
    bad=1
  else
    echo "$tag $name: NOT WRITTEN by Rust"
    bad=1
  fi
done
for f in "$d"/rs/*; do
  name=$(basename "$f")
  case "$name" in mod_def.ww3 | ww3_prnc.nml | ww3_prnc.inp) continue ;; esac
  [ -f "$d/ref/$name" ] || { echo "$tag $name: WRITTEN by Rust only"; bad=1; }
done
echo "$tag: $n comparisons, status $(cat "$d/ref.status")"
exit "$bad"
