# Run by gdb inside the reference built with ST6: records W3SPR6's tables once, then its arguments
# and results on sampled calls. CAPTURE_POKE rewrites A at the entry and puts it back at the exit.
# Format, little-endian: b"SPR6", NK NTH NSPEC (i32), DTH FTE FTF FTWN (f32), then SIG, DDEN and DSII
# as common.array writes them. Per call: A(NSPEC), CG(NK), WN(NK); then EMEAN FMEAN WNMEAN AMAX FP.
import math
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
    ints = [int(scalar(G + n)) for n in ("nk", "nth", "nspec")]
    reals = [float(scalar(G + r)) for r in ("dth", "fte", "ftf", "ftwn")]
    f.write(b"SPR6")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", r) for r in reals))
    for name in "sig dden dsii".split():
        f.write(array("w3gdatmd", name))
    header.nk, header.nth, header.nspec = ints
    header.fte = reals[1]
    # DDEN and SIG again, as Python lists, to place a poked spectrum at a chosen Hs.
    header.dden = struct.unpack(f"<{ints[0]}f", array("w3gdatmd", "dden")[8:])


def hs(a, cg, nk, nth):
    """4*sqrt(EMEAN) of W3SPR6 in double precision, to scale a spectrum onto a chosen Hs."""
    eb = [sum(a[ik * nth:(ik + 1) * nth]) * header.dden[ik] / cg[ik] for ik in range(nk)]
    e = sum(eb) + eb[nk - 1] / header.dden[nk - 1] * header.fte
    return 4.0 * math.sqrt(e) if e > 0 else 0.0


# Target Hs for the scaled spectra. HSMIN is 0.05f: the three nearest values straddle it, and the
# smallest ones leave the 1.E-10 and 1.E-7 floors of the tail of section 4 to act.
TARGETS = [0.05 * (1 - 3e-7), 0.05 * (1 - 1e-7), 0.05, 0.05 * (1 + 1e-7), 0.05 * (1 + 3e-7), 0.0499,
           0.0501, 0.06, 0.1, 0.3, 1.0, 3.0, 10.0, 40.0, 1.0e3, 1.0e6, 1.0e10, 1.0e-4, 1.0e-8, 1.0e-20]


def poke(calls, nk, nth, nspec, a, cgaddr):
    """Spectra ww3_ts1 does not produce, so every branch of W3SPR6 runs in the real Fortran. Returns
    the undo."""
    old = bytes(inf().read_memory(a, 4 * nspec))
    v = list(struct.unpack(f"<{nspec}f", old))
    cg = struct.unpack(f"<{nk}f", read(cgaddr, nk))
    kind = calls % (len(TARGETS) + 6)
    if kind < len(TARGETS):  # the whole spectrum scaled onto a chosen Hs
        h = hs(v, cg, nk, nth)
        s = (TARGETS[kind] / h) ** 2 if h > 0 else 1.0  # EMEAN is linear in A, Hs is its root
        new = [x * s for x in v]
    elif kind == len(TARGETS):  # an empty spectrum: EMEAN 0, FP stays 0
        new = [0.0] * nspec
    elif kind == len(TARGETS) + 1:  # one bin in the middle
        new = [0.0] * nspec
        new[nspec // 2] = 3.0e-3
    elif kind == len(TARGETS) + 2:  # one bin at the lowest frequency
        new = [0.0] * nspec
        new[0] = 5.0e-2
    elif kind == len(TARGETS) + 3:  # one bin at the highest frequency, so the tail dominates
        new = [0.0] * nspec
        new[nspec - 1] = 1.0e-2
    elif kind == len(TARGETS) + 4:  # a negative spectrum: EMEAN < 0, SQRT gives NaN
        new = [-x for x in v]
    else:  # one NaN bin
        new = list(v)
        # The spectrum peaks at a low frequency, so a NaN placed first, in the middle or last tells a
        # MAXVAL that restarts after a NaN from one that skips it.
        new[(nspec // 3, 0, nspec - 1, 3 * 24 + 5)[(calls // (len(TARGETS) + 6)) % 4]] = float("nan")
    inf().write_memory(a, struct.pack(f"<{nspec}f", *new))
    return (a, old)


def record(f):
    nk, nth, nspec = header.nk, header.nth, header.nspec
    # A CG WN EMEAN FMEAN WNMEAN AMAX FP in x0-7.
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    undo = None
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, nk, nth, nspec, reg[0], reg[1])
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))

    def done():
        for addr in reg[3:8]:
            f.write(read(addr, 1))
        if undo is not None:
            inf().write_memory(undo[0], undo[1])

    Exit(done)


record.calls = 0
run("__w3src6md_MOD_w3spr6", header, record)
