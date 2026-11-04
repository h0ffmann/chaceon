# Run by gdb inside the reference built with ST1: records W3SPR1's tables once, then its arguments
# and results on sampled calls. CAPTURE_POKE rewrites A at the entry and puts it back at the exit.
# Format, little-endian: b"SPR1", NK NTH NSPEC (i32), FTE FTF FTWN (f32), then SIG, DDEN as
# common.array writes them. Per call: A(NSPEC), CG(NK), WN(NK); then EMEAN FMEAN WNMEAN AMAX.
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
    f.write(b"SPR1")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(G + r))) for r in ("fte", "ftf", "ftwn")))
    for name in "sig dden".split():
        f.write(array("w3gdatmd", name))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, nspec, a):
    """Spectra WW3 does not produce, so the two MAX(1.E-7, ...) floors of section 4 run in the real
    Fortran: the spectrum scaled down, or negated, or with a single nonzero bin. Returns the undo."""
    old = bytes(inf().read_memory(a, 4 * nspec))
    v = struct.unpack(f"<{nspec}f", old)
    kind = calls % 4
    if kind == 0:
        new = [x * 1.0e-12 for x in v]
    elif kind == 1:
        new = [x * 1.0e-4 for x in v]
    elif kind == 2:
        new = [-x for x in v]
    else:
        new = [0.0] * nspec
        new[nspec // 2] = 3.0e-3
    inf().write_memory(a, struct.pack(f"<{nspec}f", *new))
    return (a, old)


def record(f):
    nk, nspec = header.nk, header.nspec
    # A CG WN EMEAN FMEAN WNMEAN AMAX in x0-6.
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(7)]
    undo = None
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, nspec, reg[0])
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))

    def done():
        for addr in reg[3:7]:
            f.write(read(addr, 1))
        if undo is not None:
            inf().write_memory(undo[0], undo[1])

    Exit(done)


record.calls = 0
run("__w3src1md_MOD_w3spr1", header, record)
