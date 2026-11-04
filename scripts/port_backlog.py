#!/usr/bin/env python3
"""The routines h0ffmann/ww3-gpu has ported to Kokkos and chaceon has not, grouped by WW3 module: the backlog
the OpenRig lead (rigs/) picks from. A routine already in rust/ports.tsv or claimed in rust/port_claims.tsv is left out.
Each row gives the module's lines at our pin when the routine is found there, since ww3-gpu's are at NOAA's pin.
Usage: port_backlog.py [--ww3-gpu DIR] [--module NAME] [--tsv]
The ww3-gpu clone defaults to $WW3_GPU_DIR, else to ww3-gpu beside the main checkout of this repository."""
import argparse, os, re, subprocess, sys
from collections import defaultdict


def rows(path):
    out = []
    for line in open(path):
        if line.startswith("#") or not line.strip():
            continue
        f = line.rstrip("\n").split("\t")
        if f[0] in ("routine",):
            continue
        out.append(f)
    return out


def main_checkout():
    common = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
                            capture_output=True, text=True, check=True).stdout.strip()
    return os.path.dirname(common)


def find_at_pin(src, module, routine):
    """model/src/<file>.F90:<line> of SUBROUTINE or FUNCTION <routine> in the module's file at our pin, or None."""
    name = module.split("_mod_")[0]
    path = os.path.join(src, name + ".F90")
    if not os.path.exists(path):
        return None
    pat = re.compile(r"^\s*(?:[A-Z ]*\s)?(?:SUBROUTINE|FUNCTION)\s+" + re.escape(routine) + r"\b", re.I)
    for n, line in enumerate(open(path, errors="replace"), 1):
        if pat.match(line):
            return f"model/src/{name}.F90:{n}"
    return f"model/src/{name}.F90"


def main():
    root = main_checkout()
    ap = argparse.ArgumentParser()
    ap.add_argument("--ww3-gpu", default=os.environ.get("WW3_GPU_DIR", os.path.join(os.path.dirname(root), "ww3-gpu")))
    ap.add_argument("--module")
    ap.add_argument("--tsv", action="store_true")
    a = ap.parse_args()
    theirs = os.path.join(a.ww3_gpu, "kokkos", "ports.tsv")
    if not os.path.exists(theirs):
        sys.exit(f"no {theirs}: clone h0ffmann/ww3-gpu there or pass --ww3-gpu")
    here = os.path.join(root, "rust")
    done = {r[0].lower() for r in rows(os.path.join(here, "ports.tsv"))}
    claims = os.path.join(here, "port_claims.tsv")
    claimed = {r[0].lower() for r in rows(claims)} if os.path.exists(claims) else set()
    src = os.path.join(root, "WW3", "model", "src")
    groups = defaultdict(list)
    for r in rows(theirs):
        key = r[0].lower()
        if key in done or key in claimed or ":" not in key:
            continue
        module, routine = key.split(":", 1)
        if a.module and module != a.module.lower():
            continue
        groups[module].append((routine, r[2] if len(r) > 2 else "", r[3] if len(r) > 3 else "",
                               find_at_pin(src, module, routine)))
    total = sum(len(v) for v in groups.values())
    if a.tsv:
        print("routine\tww3_gpu_lines\tww3_gpu_status\tat_our_pin")
    for module in sorted(groups, key=lambda m: (-len(groups[m]), m)):
        if not a.tsv:
            print(f"{module} ({len(groups[module])})")
        for routine, lines, status, at in groups[module]:
            if a.tsv:
                print(f"{module}:{routine}\t{lines}\t{status}\t{at or 'not at our pin'}")
            else:
                print(f"  {routine:<34} {lines:>5} lines in ww3-gpu ({status}); {at or 'not at our pin'}")
    if not a.tsv:
        print(f"{total} routines in {len(groups)} modules, not in rust/ports.tsv nor claimed")


main()
