# Run by gdb inside the reference: records W3SRCE's scalars once, then its arguments on sampled
# calls. The tables are in the sin4, sds4, snl1 and spr4 captures of the same case, and with BT4 in
# its sbt4 capture. Format "SRCB", whose header also names the build's BT switch:
# rust/crates/ww3-host/src/capture.rs. Files written before D50,
# PSIC and BEDFORM were recorded start "SRCE" and lack them.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
REALS = ("dmin dtmax dtmin facti1 facti2 fachfa facp xflt xrel fhmax ffxfm ffxpm ffxfa slnc1 fspm "
         "fshf sbtc1 sdbc1 sdbc2").split()
# 0-based positions in W3SRCE's argument list; the first 8 are in x0-x7, the rest on the stack.
SPEC, WN1, CG1, D_INP, U10ABS, U10DIR, USTAR, USTDIR, ICE = 8, 13, 14, 16, 17, 18, 20, 21, 24
DTDYN, FCUT, DTG = 37, 38, 39
OUT = [USTAR, USTDIR, DTDYN, FCUT] + list(range(40, 52)) + [56]
WHITECAP, TAUBBL, TAUICE, PHICE, TAUOCX, TAUOCY, WNMEAN, DAIR, COEF = 52, 57, 58, 59, 60, 61, 62, 63, 64
D50, PSIC, BEDFORM = 53, 54, 55


def real(name):
    # SBTC1 exists only in a BT1 build (w3gdatmd.F90:953); W3SRCE never reads it otherwise.
    try:
        return float(scalar(G + name))
    except gdb.error:
        return 0.0 if name == "sbtc1" else float("nan")


def btopt():
    # The build's bottom friction: 4 when W3SBT4MD is linked in, else 1.
    try:
        sym("__w3sbt4md_MOD_erftable")
        return 4
    except (gdb.error, IndexError):
        return 1


def header(f):
    ints = [int(scalar(G + i)) for i in ("nk", "nth", "nspec")]
    f.write(b"SRCB")
    flags = [bool(scalar(G + "fdonly")),
             bool(gdb.parse_and_eval("w3idatmd::inflags2(4)"))]
    f.write(struct.pack("<6i", *ints, *[int(x) for x in flags], btopt()))
    f.write(b"".join(struct.pack("<f", real(r)) for r in REALS))
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
    for i in (D_INP, U10ABS, U10DIR, USTAR, USTDIR, ICE, DTG, DAIR, COEF, D50, PSIC):
        f.write(read(arg(i), 1))
    f.write(read(arg(BEDFORM), 3))
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
        f.write(read(arg(BEDFORM), 3))

    Exit(done)


run("__w3srcemd_MOD_w3srce", header, record)
