# Run by gdb inside the reference built with IS1: records W3SIS1's arguments and results on sampled
# calls. Format, little-endian: b"SIS1", NK NTH NSPEC (i32), DTMIN IS1C1 IS1C2 (f32). Per call:
# ICE, A(NSPEC); then S(NSPEC). With CAPTURE_POKE, rare inputs are written at the routine's entry.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    ints = [int(scalar(G + n)) for n in ("nk", "nth", "nspec")]
    f.write(b"SIS1")
    f.write(struct.pack("<3i", *ints))
    f.write(struct.pack("<3f", *[float(scalar(G + n)) for n in ("dtmin", "is1c1", "is1c2")]))
    header.nspec = ints[2]


def poke(calls, nspec, a, ice):
    """Inputs the run does not produce, so the guards of :186 and :191-196 run on both sides.

    W3SRCE calls W3SIS1 only for ICE > 0, and a spectrum has no negative or NaN bin. The caller's
    ICE and A are put back at exit, so the integration goes on as it was.
    """
    kind = calls % 8
    if kind < 3:
        old = read(ice, 1)
        inf().write_memory(ice, struct.pack("<I", (0x00000000, 0xbf000000, 0x7fc00000)[kind]))
        return [(ice, old)]
    i = {3: nspec // 2, 4: 3, 5: nspec - 1, 6: 0, 7: 7}[kind]
    old = read(a + 4 * i, 1)
    # A negative bin, a NaN bin, negative zero (the GE test passes it), a bin of one, and an infinite
    # bin: with ALPHA = 0 only the guard of :191 keeps its product with ALPHA (NaN) out of S.
    inf().write_memory(a + 4 * i, struct.pack("<I", (0, 0, 0, 0xb8d1b717, 0x7fc00000, 0x80000000, 0x3f800000, 0x7f800000)[kind]))
    return [(a + 4 * i, old)]


def record(f):
    nspec = header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(3)]
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, nspec, reg[0], reg[1])
    f.write(read(reg[1], 1))
    f.write(read(reg[0], nspec))
    spoiled = bool(undo) and os.environ.get("CAPTURE_POKE") and record.calls % 8 == 7

    def done():
        f.write(read(reg[2], nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)
        # An infinite bin makes S infinite or NaN, and the caller's EXP(ICE*S*DTG) would carry it on.
        if spoiled:
            inf().write_memory(reg[2], bytes(4 * nspec))

    Exit(done)


record.calls = 0
run("__w3sis1md_MOD_w3sis1", header, record)
