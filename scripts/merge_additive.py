#!/usr/bin/env python3
"""Three-way merge for branches that only add, as parallel ports do: applies base->theirs onto ours.
Inserts land after the ours line that maps to the base line before them. A line both sides changed is
merged the same way on tokens. Anything else is reported, never guessed.
Usage, after a merge stops on conflicts: git diff --name-only --diff-filter=U | xargs python3 scripts/merge_additive.py
It reads stages 1, 2 and 3 from the git index, writes each file and prints its status; exit 1 if one needs a hand merge.
Used by the OpenRig lead (rigs/) to merge porter branches that all append to the same lists.
"""
import difflib, re, subprocess, sys


def stage(n, f):
    return subprocess.run(["git", "show", f":{n}:{f}"], capture_output=True, text=True, check=True).stdout


def mapping(base, ours):
    """base index -> ours index for lines equal in both; None where ours changed them."""
    m = [None] * (len(base) + 1)
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(None, base, ours, autojunk=False).get_opcodes():
        if op == "equal":
            for k in range(i2 - i1):
                m[i1 + k] = j1 + k
    m[len(base)] = len(ours)
    return m


def anchor(m, i, ours_len):
    """Where in ours to insert something that sits before base line i."""
    k = i - 1
    while k >= 0 and m[k] is None:
        k -= 1
    return 0 if k < 0 else m[k] + 1


def merge(ours, base, theirs, tokens=False):
    m = mapping(base, ours)
    edits = []  # (ours position, ours lines to drop, new lines)
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(None, base, theirs, autojunk=False).get_opcodes():
        if op == "equal":
            continue
        new = theirs[j1:j2]
        if op == "insert":
            edits.append((anchor(m, i1, len(ours)), 0, new))
        elif all(m[i] is not None for i in range(i1, i2)) and m[i2 - 1] - m[i1] == i2 - i1 - 1:
            edits.append((m[i1], i2 - i1, new))
        elif not tokens and i2 - i1 == 1 and j2 - j1 == 1:
            # ours changed this line too: find ours' version of it and merge tokens
            a = anchor(m, i1, len(ours))
            if a >= len(ours):
                return None
            tok = lambda s: re.findall(r"\w+|\s+|\W", s)
            r = merge(tok(ours[a]), tok(base[i1]), tok(theirs[j1]), tokens=True)
            if r is None:
                return None
            edits.append((a, 1, ["".join(r)]))
        else:
            return None
    out = list(ours)
    for pos, drop, new in sorted(edits, key=lambda e: e[0], reverse=True):
        out[pos:pos + drop] = new
    return out


bad = 0
for f in sys.argv[1:]:
    o, b, t = (stage(n, f).splitlines(True) for n in (2, 1, 3))
    r = merge(o, b, t)
    if r is None:
        print(f, "NEEDS HAND MERGE")
        bad = 1
        continue
    open(f, "w").write("".join(r))
    print(f, "merged")
sys.exit(bad)
