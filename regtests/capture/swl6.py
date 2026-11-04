# Run by gdb inside the reference built with ST6: records W3SWL6's grid once, then its arguments and
# results on sampled calls.
# Format, little-endian: b"SWL6", NK NTH SWL6CSTB1 (i32), DTH SWL6B1 (f32), then SIG(0:NK+1) and
# DDEN(NK) as common.array writes them. Per call: A(NSPEC), CG(NK), WN(NK); then S(NSPEC), D(NSPEC).
# W3SWL6 (A, CG, WN, S, D) takes five arrays by reference: x0 to x4.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    nk, nth = [int(scalar(G + n)) for n in ("nk", "nth")]
    cstb1 = int(scalar(G + "swl6cstb1"))
    f.write(b"SWL6")
    f.write(struct.pack("<3i", nk, nth, cstb1))
    f.write(struct.pack("<2f", float(scalar(G + "dth")), float(scalar(G + "swl6b1"))))
    f.write(array("w3gdatmd", "sig"))
    f.write(array("w3gdatmd", "dden"))
    header.nk, header.nspec = nk, nk * nth


# Inputs the run never produces, written at the routine's entry in turn (CAPTURE_POKE). A column of
# zeros and a tiny spectrum reach the ABAND > 1.E-30 test, a negative WN or CG makes BN or the
# energy sum negative (the square roots give NaN, whose sign the Fortran decides), NaN and Inf
# reach MAXVAL and MAXLOC (kind 9 is every element NaN: MAXLOC gives 1), and a constant spectrum
# makes every ABAND tie.
def poke(calls, a, cg, wn, nk, nth):
    kind = calls % 10
    if kind == 0:
        return []
    saved = [(a, read(a, nk * nth)), (cg, read(cg, nk)), (wn, read(wn, nk))]
    av = list(struct.unpack(f"<{nk * nth}f", saved[0][1]))
    cv = list(struct.unpack(f"<{nk}f", saved[1][1]))
    wv = list(struct.unpack(f"<{nk}f", saved[2][1]))
    nan, inf_ = float("nan"), float("inf")
    if kind == 1:
        av = [0.0] * len(av)
    elif kind == 2:
        av = [x * 1.0e-25 for x in av]
    elif kind == 3:
        wv = [-x if i % 3 == 0 else x for i, x in enumerate(wv)]
    elif kind == 4:
        cv = [-x for x in cv]
    elif kind == 5:
        av[0] = nan
        av[nth + 2] = nan
    elif kind == 6:
        for ith in range(nth):
            av[2 * nth + ith] = nan
        av[nth * 4 + 1] = inf_
        av[nth * 5 + 1] = -inf_
    elif kind == 7:
        av = [1.0] * len(av)
    elif kind == 9:
        av = [nan] * len(av)
    elif kind == 8:
        for ith in range(nth):
            av[ith] = -1.0
        av[nth] = -0.0
    inf().write_memory(a, struct.pack(f"<{len(av)}f", *av))
    inf().write_memory(cg, struct.pack(f"<{nk}f", *cv))
    inf().write_memory(wn, struct.pack(f"<{nk}f", *wv))
    return saved


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(5)]
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, reg[0], reg[1], reg[2], nk, nspec // nk)
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))

    def done():
        f.write(read(reg[3], nspec))
        f.write(read(reg[4], nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)
        if undo:
            # S and D of a poked call may be NaN: keep them out of the integration.
            inf().write_memory(reg[3], bytes(4 * nspec))
            inf().write_memory(reg[4], bytes(4 * nspec))

    Exit(done)


record.calls = 0
run("__w3swldmd_MOD_w3swl6", header, record)
