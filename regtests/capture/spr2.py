# Run by gdb inside the reference built with ST2: records W3SPR2's grid scalars and tables once, then
# its arguments and results on sampled calls. CAPTURE_POKE rewrites inputs at the entry for the guards
# ww3_ts1 does not reach, and puts them back at the exit.
# Format, little-endian: b"SPR2", NK NTH NSPEC (i32), DTH FTE FTF FTWN (f32), then SIG, DDEN as
# common.array writes them. Per call: DEPTH FPI U USTAR, A(NSPEC), CG(NK), WN(NK); then
# EMEAN FMEAN WNMEAN AMAX FP, ALFA(NK).
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
    f.write(b"SPR2")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(G + r))) for r in ("dth", "fte", "ftf", "ftwn")))
    for name in "sig dden".split():
        f.write(array("w3gdatmd", name))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, nspec, a, fpi, ustar):
    """Inputs ww3_ts1 does not produce, so every guard of W3SPR2 runs in the real Fortran: the floor of
    USTAR (0.0001), the floor of FPISTR (0.008) and the two MAX(1.E-7, ...) floors of the mean
    frequency and wavenumber. Returns what to put back at the exit."""
    put = lambda addr, v: inf().write_memory(addr, struct.pack("<f", v))
    kind = calls % 6
    if kind == 0:  # USTAR under its floor
        undo = [(ustar, read(ustar, 1))]
        put(ustar, 5.0e-5)
    elif kind == 1:  # FPI so small that FPISTR is under 0.008
        undo = [(fpi, read(fpi, 1))]
        put(fpi, 0.01)
    elif kind == 2:  # a spectrum under both 1.E-7 floors
        undo = [(a, read(a, nspec))]
        v = struct.unpack(f"<{nspec}f", read(a, nspec))
        inf().write_memory(a, struct.pack(f"<{nspec}f", *[x * 1.0e-12 for x in v]))
    elif kind == 3:  # a negated spectrum: -0. bins and AMAX staying at +0
        undo = [(a, read(a, nspec))]
        v = struct.unpack(f"<{nspec}f", read(a, nspec))
        inf().write_memory(a, struct.pack(f"<{nspec}f", *[-x for x in v]))
    elif kind == 4:  # an empty spectrum
        undo = [(a, read(a, nspec))]
        inf().write_memory(a, bytes(4 * nspec))
    else:  # no friction velocity and no input frequency: both floors at once
        undo = [(ustar, read(ustar, 1)), (fpi, read(fpi, 1))]
        put(ustar, 0.0)
        put(fpi, 0.0)
    return undo


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # A CG WN DEPTH FPI U USTAR EMEAN in x0-7; the stack holds FMEAN WNMEAN AMAX ALFA FP.
    fmean, wnmean, amax, alfa, fp = struct.unpack("<5Q", bytes(inf().read_memory(sp, 40)))
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, nspec, reg[0], reg[4], reg[6])
    for addr in reg[3:7]:
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))

    def done():
        for addr in (reg[7], fmean, wnmean, amax, fp):
            f.write(read(addr, 1))
        f.write(read(alfa, nk))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0
run("__w3src2md_MOD_w3spr2", header, record)
