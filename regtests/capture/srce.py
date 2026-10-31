# Run by gdb inside the reference: records W3SRCE's scalars once, then its arguments on sampled
# calls. The tables are in the sin4, sds4, snl1 and spr4 captures of the same case.
# Format: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"
REALS = ("dmin dtmax dtmin facti1 facti2 fachfa facp xflt xrel fhmax ffxfm ffxpm ffxfa slnc1 fspm "
         "fshf sbtc1 sdbc1 sdbc2").split()
# 0-based positions in W3SRCE's argument list; the first 8 are in x0-x7, the rest on the stack.
SPEC, WN1, CG1, D_INP, U10ABS, U10DIR, USTAR, USTDIR, ICE = 8, 13, 14, 16, 17, 18, 20, 21, 24
DTDYN, FCUT, DTG = 37, 38, 39
OUT = [USTAR, USTDIR, DTDYN, FCUT] + list(range(40, 52)) + [56]
WHITECAP, TAUBBL, TAUICE, PHICE, TAUOCX, TAUOCY, WNMEAN, DAIR, COEF = 52, 57, 58, 59, 60, 61, 62, 63, 64


def header(f):
    ints = [int(scalar(G + i)) for i in ("nk", "nth", "nspec")]
    f.write(b"SRCE")
    flags = [bool(scalar(G + "fdonly")),
             bool(gdb.parse_and_eval("w3idatmd::inflags2(4)"))]
    f.write(struct.pack("<5i", *ints, *[int(x) for x in flags]))
    f.write(b"".join(struct.pack("<f", float(scalar(G + r))) for r in REALS))
    f.write(array("w3gdatmd", "icescales"))
    header.nk, header.nspec = ints[0], ints[2]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    stack = struct.unpack("<57Q", bytes(inf().read_memory(sp, 57 * 8)))

    def arg(i):
        return reg[i] if i < 8 else stack[i - 8]

    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    f.write(read(arg(0), 1))
    f.write(read(arg(1), 1))
    for i in (D_INP, U10ABS, U10DIR, USTAR, USTDIR, ICE, DTG, DAIR, COEF):
        f.write(read(arg(i), 1))
    f.write(read(arg(SPEC), nspec))
    f.write(read(arg(WN1), nk))
    f.write(read(arg(CG1), nk))

    def done():
        f.write(read(arg(SPEC), nspec))
        for i in OUT:
            f.write(read(arg(i), 1))
        f.write(read(arg(TAUBBL), 2))
        f.write(read(arg(TAUICE), 2))
        for i in (PHICE, TAUOCX, TAUOCY, WNMEAN):
            f.write(read(arg(i), 1))
        f.write(read(arg(WHITECAP), 4))

    Exit(done)


run("__w3srcemd_MOD_w3srce", header, record)
