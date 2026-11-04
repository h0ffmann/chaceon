#!/usr/bin/env bash
# Creates or refreshes the git worktree of each OpenRig seat (rigs/), one per seat under .worktrees/, on a
# branch of its own from BASE, with WW3/ at the pin and a private scratch directory.
# Usage: rig_worktrees.sh BASE SEAT...   e.g. rig_worktrees.sh goal/abcd1234-1234-batch port-1 port-2 audit
# A worktree's submodule is empty, and git refuses the file transport a local submodule clone needs, so WW3/
# is a plain clone of the main checkout's WW3/ at the pin. A seat with uncommitted work is left alone.
set -euo pipefail
cd "$(dirname "$0")/.."
root="$PWD"
base="${1:?base branch}"
shift
pin="$(git ls-tree HEAD WW3 | awk '{print $3}')"
for seat in "$@"; do
  wt=".worktrees/$seat"
  br="rig/$seat"
  if [ -d "$wt" ]; then
    if [ -n "$(git -C "$wt" status --porcelain --ignore-submodules=all)" ]; then
      echo "$seat: uncommitted work, left as is" >&2
      continue
    fi
    git -C "$wt" checkout -q -B "$br" "$base"
  else
    git worktree add -q -B "$br" "$wt" "$base"
  fi
  if [ ! -e "$wt/WW3/model/src" ]; then
    rmdir "$wt/WW3" 2>/dev/null || true
    git clone -q --no-checkout "$root/WW3" "$wt/WW3"
  fi
  git -C "$wt/WW3" checkout -q "$pin"
  mkdir -p "$wt/work/capture" "$wt/work/scratch"
  echo "$seat: $wt on $br at $(git -C "$wt" rev-parse --short HEAD), WW3 at ${pin:0:8}"
done
