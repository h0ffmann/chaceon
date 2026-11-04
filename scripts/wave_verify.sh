#!/usr/bin/env bash
# Reruns every gate of a wave on the merged integration branch, one line per gate in work/log/verify/summary.txt.
# Usage: wave_verify.sh ROUTINE...   (the <x> of each new <x>-cmp recipe; a host-only routine such as a table
# builder, which has no kernel, is written host:<x>). Captures are work/capture/<x>_*.bin.
# Every kernel spirv_replay knows is replayed too, on its first capture, because the shader module changed.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
export PATH="$HOME/.cargo/bin:$PATH"
L=work/log/verify
mkdir -p "$L"
S=$L/summary.txt
: > "$S"
run() {
  local name=$1
  shift
  if "$@" > "$L/$name.log" 2>&1; then echo "PASS $name" >> "$S"; else echo "FAIL $name ($?)" >> "$S"; fi
}
caps() { (cd work && ls capture/"$1"_*.bin 2>/dev/null); }

run check just check
run lint just lint-sh
run spirv just spirv
run ptx just ptx
new=()
for arg in "$@"; do
  r=${arg#host:}
  list=$(caps "$r" | sed 's|^|/work/|' | tr '\n' ' ')
  if [ -z "$list" ]; then echo "FAIL cmp_$r (no capture)" >> "$S"; continue; fi
  # shellcheck disable=SC2086
  run "cmp_$r" just "$r-cmp" $list
  [ "$arg" = "$r" ] && new+=("$r")
done
for r in "${new[@]}"; do
  for c in $(caps "$r"); do run "replay_$(basename "$c" .bin)" bash scripts/spirv_replay.sh "$r" "$c"; done
done
for k in $(grep -oE '^\s+"[a-z0-9]+" =>' rust/tools/spirv_replay/src/main.rs | tr -d ' "=>'); do
  [ "$k" = libm ] && continue
  case " ${new[*]} " in *" $k "*) continue ;; esac
  c=$(caps "$k" | head -1)
  if [ -n "$c" ]; then run "replay_$(basename "$c" .bin)" bash scripts/spirv_replay.sh "$k" "$c"; fi
done
run srce_cmp_default_8m just srce-cmp default_8m
run srce_cmp_bt4_8m just srce-cmp bt4_8m
pass=$(grep -c '^PASS' "$S")
fail=$(grep -c '^FAIL' "$S")
echo "DONE $pass PASS $fail FAIL" >> "$S"
tail -1 "$S"
[ "$fail" -eq 0 ]
