# Run by gdb inside the reference: records W3SPR4's scalars once, then its arguments on sampled calls.
# TAUT is in the sin4 capture. Format: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
REALS = [G + r for r in "wwnmeanp wwnmeanptail fte ftf sstxftftail sstxftwn zzwnd aalpha".split()]


def header(f):
    ints = [int(scalar(G + i)) for i in ("nk", "nth", "nspec")]
    f.write(b"SPR4")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    for s in ("__w3src4md_MOD_deltauw", "__w3src4md_MOD_delu"):
        f.write(bytes(inf().read_memory(sym(s), 4)))
    for name in "sig dden ecos esin".split():
        f.write(array("w3gdatmd", name))
    n = 201 * 201
    f.write(struct.pack("<ii", 0, n) + bytes(inf().read_memory(sym("__w3src4md_MOD_taut"), 4 * n)))
    header.nk, header.nspec = ints[0], ints[2]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # A CG WN EMEAN FMEAN FMEAN1 WNMEAN AMAX in x0-7; the stack holds the rest.
    u, udir, ustar, usdir, tauwx, tauwy, cd, z0, charn, llws, fmeanws, dlwmean = struct.unpack(
        "<12Q", bytes(inf().read_memory(sp, 96)))
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    for addr in (u, udir, ustar, usdir, tauwx, tauwy):
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))
    f.write(read(llws, nspec))

    def done():
        for addr in (reg[3], reg[4], reg[5], reg[6], reg[7], ustar, usdir, cd, z0, charn, fmeanws, dlwmean):
            f.write(read(addr, 1))

    Exit(done)


run("__w3src4md_MOD_w3spr4", header, record)
