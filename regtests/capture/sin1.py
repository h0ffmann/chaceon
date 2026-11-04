# Run by gdb inside the reference built with ST1 (switch_chaceon_st1): records W3SIN1's arguments and
# results on sampled calls.
# Format, little-endian: b"SIN1", NK NTH NSPEC (i32), SINC1 (f32), then SIG2, ECOS, ESIN as
# common.array writes them. Per call: USTAR, USDIR, A(NSPEC), K(NSPEC); then S(NSPEC), D(NSPEC).
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
    f.write(b"SIN1")
    f.write(struct.pack("<3i", *ints))
    f.write(struct.pack("<f", float(scalar(G + "sinc1"))))
    for name in "sig2 ecos esin".split():
        f.write(array("w3gdatmd", name))
    header.nspec = ints[2]


def record(f):
    nspec = header.nspec
    # A, K, USTAR, USDIR, S, D: all six arguments are in registers.
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(6)]
    f.write(read(reg[2], 1))
    f.write(read(reg[3], 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nspec))
    s, d = reg[4], reg[5]

    def done():
        f.write(read(s, nspec))
        f.write(read(d, nspec))

    Exit(done)


run("__w3src1md_MOD_w3sin1", header, record)
