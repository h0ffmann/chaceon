# Run by gdb inside the reference built with IC1: records W3SIC1's MAPWN once, then its arguments and
# results on sampled calls. ICEP1(IX,IY) is read from the input field at the call, since a homogeneous
# ice parameter changes with the date.
# Format, little-endian: b"SIC1", NK NTH NSPEC INFLAGS2(-7) (i32), MAPWN as common.array writes it.
# Per call: ICEP1(IX,IY) DEPTH (f32), IX IY (i32), A(NSPEC), CG(NK); then S(NSPEC), D(NSPEC).
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, desc, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    ints = [int(scalar(G + n)) for n in ("nk", "nth", "nspec")]
    flag = int(bool(gdb.parse_and_eval("w3idatmd::inflags2(-7)")))
    f.write(b"SIC1")
    f.write(struct.pack("<4i", *ints, flag))
    f.write(array("w3gdatmd", "mapwn"))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, nspec, a, icep1_addr):
    """Inputs the model does not produce, so every branch of W3SIC1 runs in the real Fortran.

    A run has a fixed ICEP1 per date and a finite spectrum. Returns what to undo at exit, and the
    bin of S that the poke made NaN or Inf (the integration must not see it).
    """
    kind = calls % 7
    f4 = lambda v: struct.pack("<f", v)
    inf_, nan = 0x7f800000, 0x7fc00000
    old_i = bytes(inf().read_memory(icep1_addr, 4))
    if kind in (0, 1, 2, 6):
        new = {0: struct.pack("<I", nan), 1: f4(-1.0e-5), 2: struct.pack("<I", inf_), 6: f4(1.0e-40)}[kind]
        inf().write_memory(icep1_addr, new)
        return [(icep1_addr, old_i)], None
    if kind == 5:
        # No ice, so D = 0 and S = 0*A: an Inf in A makes S NaN.
        inf().write_memory(icep1_addr, f4(0.0))
        undo = [(icep1_addr, old_i)]
        i = 3
    else:
        undo = []
        i = nspec // 2 if kind == 3 else nspec - 1
    old = bytes(inf().read_memory(a + 4 * i, 4))
    inf().write_memory(a + 4 * i, struct.pack("<I", nan if kind == 3 else inf_))
    undo.append((a + 4 * i, old))
    return undo, i


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(7)]
    # A, DEPTH, CG, IX, IY, S, D are x0 to x6.
    ix, iy = struct.unpack("<2i", read(reg[3], 1) + read(reg[4], 1))
    base, dims = desc("w3idatmd", "icep1", 2)
    (s1, l1, _), (s2, l2, _) = dims
    icep1_addr = base + 4 * ((ix - l1) * s1 + (iy - l2) * s2)
    undo, spoiled = [], None
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo, spoiled = poke(record.calls, nspec, reg[0], icep1_addr)
    icep1 = read(icep1_addr, 1)
    f.write(icep1)
    f.write(read(reg[1], 1))
    f.write(struct.pack("<2i", ix, iy))
    f.write(read(reg[0], nspec))
    f.write(read(reg[2], nk))

    def done():
        f.write(read(reg[5], nspec))
        f.write(read(reg[6], nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)
        # A poked ICEP1 or A leaves NaN, Inf or growth in S and D: keep them out of the integration.
        if undo:
            zero = struct.pack("<f", 0.0) * nspec
            inf().write_memory(reg[5], zero)
            inf().write_memory(reg[6], zero)

    Exit(done)


record.calls = 0
run("__w3sic1md_MOD_w3sic1", header, record)
