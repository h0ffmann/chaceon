# Run by gdb inside the reference built with ST1: records W3SDS1's scalars once, then its arguments
# and results on sampled calls.
# Format, little-endian: b"SDS1", NK NTH (i32), SDSC1 (f32). Per call: EMEAN FMEAN WNMEAN, A(NSPEC),
# K(NSPEC); then S(NSPEC), D(NSPEC).
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
    nk, nth = int(scalar(G + "nk")), int(scalar(G + "nth"))
    f.write(b"SDS1")
    f.write(struct.pack("<2i", nk, nth))
    f.write(struct.pack("<f", float(scalar(G + "sdsc1"))))
    header.nspec = nk * nth


def record(f):
    nspec = header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(7)]
    # W3SDS1 (A, K, EMEAN, FMEAN, WNMEAN, S, D): all seven arguments are in x0-x6.
    for addr in reg[2:5]:
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nspec))
    s_addr, d_addr = reg[5], reg[6]

    def done():
        f.write(read(s_addr, nspec))
        f.write(read(d_addr, nspec))

    Exit(done)


run("__w3src1md_MOD_w3sds1", header, record)
