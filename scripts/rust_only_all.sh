#!/usr/bin/env bash
# rust_only.sh on every case this repository judges. Each needs work/pair/<tag>: run `just tp-all` and
# `just ts1-pair` first. Exit 0 only when every case is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
bad=0
while read -r case_name tag suffix; do
  # shellcheck disable=SC2086 # suffix is empty for most cases and must then pass no argument
  bash scripts/rust_only.sh "$case_name" "$tag" $suffix || bad=1
done <<'CASES'
ww3_ts1 default_2500m
ww3_tp1.1 ww3_tp1.1
ww3_tp1.2 ww3_tp1.2
ww3_tp1.3 ww3_tp1.3
ww3_tp1.4 ww3_tp1.4
ww3_tp1.5 ww3_tp1.5
ww3_tp1.6 ww3_tp1.6
ww3_tp1.7 ww3_tp1.7
ww3_tp1.8 ww3_tp1.8
ww3_tp1.9 ww3_tp1.9
ww3_tp1.10 ww3_tp1.10
ww3_tp2.1 ww3_tp2.1
ww3_tp2.2 ww3_tp2.2
ww3_tp2.3 ww3_tp2.3
ww3_tp2.4 ww3_tp2.4
ww3_tp2.4 ww3_tp2.4_curv curv
ww3_tp2.15 ww3_tp2.15
CASES
# Water levels and ice: every forcing file from Rust's ww3_prep, then the run. Needs `just forcing-all`.
RUST_ONLY_OVERLAY=work/forcing/ov_both RUST_ONLY_PREP="ww3_prep_wind.inp ww3_prep_lev.inp ww3_prep_ice.inp" \
  bash scripts/rust_only.sh ww3_tp2.15 ww3_tp2.15_both || bad=1
RUST_ONLY_PREP=ww3_prep_ice.inp bash scripts/rust_only.sh ww3_tic2.2 ww3_tic2.2 || bad=1
exit "$bad"
