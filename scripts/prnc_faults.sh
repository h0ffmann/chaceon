#!/usr/bin/env bash
# A comparison that cannot fail proves nothing: one fault per stage of ww3_prnc goes into the Rust source, the pair runs on
# a case that exercises the stage, and the comparison must say DIFFERENT. Each fault is a perl substitution of the first
# match in one source file; the source is restored from a copy afterwards and compared with cmp. The faulty build goes to
# its own target directory (rust/target/linux-netcdf-fault), so the build of the other scripts is not touched.
# Usage: prnc_faults.sh [fault ...]; with no argument every fault runs. Exit 0 when every fault was caught.
set -euo pipefail
cd "$(dirname "$0")/.."
host=rust/crates/ww3-host/src
# name | case | file | perl substitution (first match)
faults=(
  "header|adri_wind_ll|prnc.rs|s/m\\.gtype, 0, 0, 0/m.gtype + 1, 0, 0, 0/"
  "time_second|ll_wns_shift|prnc.rs|s/second \\+= 0\\.0;/second += 1.0;/"
  "weights|adri_wind_ll|prnc.rs|s/rd12\\[k\\] = rw\\[3\\];/rd12[k] = rw[2];/"
  "velocity_clamp|adri_wind_ll|prnc.rs|s/1\\.5f32\\.min\\(/1.4f32.min(/"
  "scale_offset|ll_cur_2d_packed|prnc.rs|s/\\*x = \\*x \\* xcfac \\+ xcoff;/*x = *x * xcfac;/"
  "closure|glo_wrap_10deg|prnc.rs|s/iclo = 2; \\/\\/ ICLOSE_SMPL/iclo = -1;/"
  "pole_branch|glo_wind_0_360|gridsru.rs|s/if D90 - lat0\\.abs\\(\\) > NEAR_POLE \\{/if true {/"
  "fill_to_zero|ai_xy_wind|prnc.rs|s/xc\\[k\\] = 0\\.;/xc[k] = 1.;/"
  "nan_to_fill|ai_xy_wind|prnc.rs|s/if x\\.is_nan\\(\\) \\{/if false {/"
  "time_shift_sign|ll_shift_back|prnc.rs|s/timedelay = dsec21\\(time, timeshift\\);/timedelay = -dsec21(time, timeshift);/"
  "log_text|ll_wind_1d|prnc.rs|s/           Time : \\{idtime\\}/           Tim  : {idtime}/"
  "hours_unit|ll_wind_1d|prnc.rs|s/curjulday \\/= 24\\.;/curjulday \\/= 24.5;/"
  "interp_corner1|glo_nodes_holes_dense|prnc.rs|s/fa\\[k\\] = at\\(a, c\\);/fa[k] = at(a, c) + 1.0;/"
  "interp_corner2|glo_nodes_holes_dense|prnc.rs|s/fa\\[k\\] = at\\(b, c\\);/fa[k] = at(b, c) + 1.0;/"
  "interp_corner3|glo_nodes_holes_dense|prnc.rs|s/fa\\[k\\] = at\\(a, d\\);/fa[k] = at(a, d) + 1.0;/"
  "interp_corner4|glo_nodes_holes_dense|prnc.rs|s/fa\\[k\\] = at\\(b, d\\);/fa[k] = at(b, d) + 1.0;/"
  "interp_all_fill|glo_nodes_holes_dense|prnc.rs|s/\\} else \\{\\s+fa\\[k\\] = 0\\.0;/} else { fa[k] = 1.0;/"
  "one_fill_value|ll_two_fillvalues|prnc.rs|s/Ok\\(v\\) => fillvalue = v,/Ok(v) if i == 0 => fillvalue = v,\n            Ok(_) => {}/"
)
want=("$@")
bad=0
for f in "${faults[@]}"; do
  IFS='|' read -r name cs file sub <<< "$f"
  if [ "${#want[@]}" -gt 0 ]; then
    hit=0
    for w in "${want[@]}"; do [ "$w" = "$name" ] && hit=1; done
    [ "$hit" = 1 ] || continue
  fi
  cp "$host/$file" "$host/$file.faultsave"
  restore() { cp "$host/$file.faultsave" "$host/$file"; rm -f "$host/$file.faultsave"; }
  perl -0pi -e "$sub" "$host/$file"
  if cmp -s "$host/$file" "$host/$file.faultsave"; then
    restore
    echo "$name: the substitution matched nothing"
    bad=1
    continue
  fi
  built=0
  NETCDF_TARGET=linux-netcdf-fault bash scripts/rust_netcdf.sh --build ww3_prnc > /dev/null 2>&1 && built=1
  restore
  if [ "$built" = 0 ]; then
    echo "$name: the faulty source did not build"
    bad=1
    continue
  fi
  out=$(NETCDF_TARGET=linux-netcdf-fault PRNC_SKIP_BUILD=1 PRNC_TAG="fault_$name" bash scripts/prnc_pair.sh "$cs" 2>&1 || true)
  diffs=$(echo "$out" | grep -E 'DIFFERENT|NOT WRITTEN|WRITTEN by' | tr '\n' ';' | sed 's/;$//' || true)
  if [ -n "$diffs" ]; then
    echo "$name ($cs): caught: $diffs"
  else
    echo "$name ($cs): NOT CAUGHT"
    bad=1
  fi
done
exit "$bad"
