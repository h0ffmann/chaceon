#!/usr/bin/env bash
# The host verdict of ADR-0005 for runtime MPI: ww3_shel runs each prepared case under mpirun with R ranks,
# alone and with --threads, and cmp judges every .reference the Fortran wrote. The comparison is against the
# reference's files, never against another rank count. Needs work/pair/<case>/ from tp_pair.sh or ts1_pair.sh.
# Usage: mpi_pair.sh [-n repeats] [case ...]    (default: the eight cases of the proof, 3 repeats)
# Env: MPI_RANKS (default "1 2 3 4"), MPI_THREADS_RANKS (default "2"), the --threads 2 run's R.
# Exit 0 only when every run of every configuration is byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
repeats=3
if [ "${1:-}" = "-n" ]; then repeats="$2"; shift 2; fi
cases=("$@")
if [ "${#cases[@]}" -eq 0 ]; then
  cases=(ww3_tp1.1 ww3_tp1.4 ww3_tp1.6 ww3_tp1.8 ww3_tp2.3 ww3_tp2.15 storm_6m default_2500m)
fi
read -r -a ranks <<< "${MPI_RANKS:-1 2 3 4}"
thread_ranks="${MPI_THREADS_RANKS:-2}"

bash scripts/rust_mpi.sh --build
export MPI_SKIP_BUILD=1

bad=0
printf '%-16s %-14s %5s  %s\n' case config runs verdict
for c in "${cases[@]}"; do
  d="work/pair/$c"
  [ -d "$d" ] || { echo "$c: no $d" >&2; exit 66; }
  configs=()
  for r in "${ranks[@]}"; do configs+=("$r:1"); done
  configs+=("$thread_ranks:2")
  for cfg in "${configs[@]}"; do
    r="${cfg%%:*}"; t="${cfg##*:}"
    ok=0; total=0; why=""
    for ((i = 1; i <= repeats; i++)); do
      # Delete every output that has a reference, so a stale file cannot pass.
      for ref in "$d"/*.reference; do rm -f "${ref%.reference}.ww3"; done
      args=("/work/pair/$c")
      [ "$t" -gt 1 ] && args+=(--threads "$t")
      bash scripts/rust_mpi.sh "$r" "${args[@]}" > "$d/mpi.log" 2>&1 || why="$why exit($?)"
      total=$((total + 1))
      same=1
      for ref in "$d"/*.reference; do
        out="${ref%.reference}.ww3"
        if ! cmp -s "$ref" "$out"; then
          same=0
          why="$why $(basename "$out"):$([ -f "$out" ] && echo "$(cmp -l "$ref" "$out" 2>/dev/null | wc -l | tr -d ' ') bytes" || echo missing)"
        fi
      done
      [ "$same" -eq 1 ] && ok=$((ok + 1))
    done
    label="R=$r"
    [ "$t" -gt 1 ] && label="R=$r,T=$t"
    if [ "$ok" -eq "$total" ]; then
      printf '%-16s %-14s %5s  identical\n' "$c" "$label" "$total"
    else
      printf '%-16s %-14s %5s  DIFFERENT (%s of %s identical;%s)\n' "$c" "$label" "$total" "$ok" "$total" "$why"
      bad=1
    fi
  done
done
exit "$bad"
