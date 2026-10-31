#!/usr/bin/env bash
# How much of WW3 each batch owns (rust/port_files.tsv) and how far the port is (rust/ports.tsv).
set -euo pipefail
cd "$(dirname "$0")/../rust"
awk -F'\t' '!/^#/ && $1 != "file" {
  n[$3]++; l[$3] += $2; t += $2; c++
} END {
  for (b in n) printf "%s\t%d\t%d\n", b, n[b], l[b]
  printf "all\t%d\t%d\n", c, t
}' port_files.tsv | sort | { printf 'batch\tfiles\tlines\n'; cat; }
echo
awk -F'\t' '!/^#/ && $1 != "routine" { s[$4]++ } END {
  printf "status\troutines\n"
  for (k in s) printf "%s\t%d\n", k, s[k]
}' ports.tsv
