# Run by gdb inside the reference built with BT4: records W3SBT4's tables once, then its arguments
# and results on sampled calls, with ERFTABLE as the first call left it.
# Format, little-endian: b"SBT4", NK NTH NSPEC (i32), DELAB (f32), then SBTCX, SIG, DDEN, ECOS, ESIN
# as common.array writes them, FWTABLE(0:300). Per call: DEPTH D50 PSIC, BEDFORM(3), A(NSPEC),
# CG(NK), WN(NK); then S(NSPEC), D(NSPEC), TAUBBL(2), BEDFORM(3), DELXERF, ERFTABLE(0:300).
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    ints = [int(scalar(G + n)) for n in ("nk", "nth", "nspec")]
    f.write(b"SBT4")
    f.write(struct.pack("<3i", *ints))
    f.write(struct.pack("<f", float(scalar("constants::delab"))))
    for name in "sbtcx sig dden ecos esin".split():
        f.write(array("w3gdatmd", name))
    f.write(read(sym("__constants_MOD_fwtable"), 301))
    header.nk, header.nspec = ints[0], ints[2]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # Arguments 9 to 12 are on the stack: S, D, IX, IY.
    s_addr, d_addr = struct.unpack("<2Q", bytes(inf().read_memory(sp, 16)))
    for addr in (reg[3], reg[4], reg[5]):
        f.write(read(addr, 1))
    f.write(read(reg[7], 3))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))
    taubbl, bedform = reg[6], reg[7]

    def done():
        f.write(read(s_addr, nspec))
        f.write(read(d_addr, nspec))
        f.write(read(taubbl, 2))
        f.write(read(bedform, 3))
        f.write(read(sym("__w3sbt4md_MOD_delxerf"), 1))
        f.write(read(sym("__w3sbt4md_MOD_erftable"), 301))

    Exit(done)


run("__w3sbt4md_MOD_w3sbt4", header, record)
