# Run by gdb inside the reference built with ST3: records W3SPR3's scalars and TAUT once, then its
# arguments and results on sampled calls. W3SPR3 is called from W3SRCE and W3OUTP.
# Format, little-endian: b"SPR3", NK NTH NSPEC (i32), WWNMEANP WWNMEANPTAIL FTE SSTXFTF SSTXFTFTAIL
# SSTXFTWN ZZWND AALPHA DELTAUW DELU (f32), then SIG and DDEN as common.array writes them, and TAUT
# as (0, 201*201) followed by its data. Per call: U UDIR USTAR USDIR TAUWX TAUWY, A(NSPEC), CG(NK),
# WN(NK), LLWS(NSPEC); then EMEAN FMEAN FMEANS WNMEAN AMAX USTAR USDIR CD Z0 CHARN FMEANWS.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
REALS = [G + r for r in "wwnmeanp wwnmeanptail fte sstxftf sstxftftail sstxftwn zzwnd aalpha".split()]


def header(f):
    ints = [int(scalar(G + i)) for i in ("nk", "nth", "nspec")]
    f.write(b"SPR3")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    for s in ("__w3src3md_MOD_deltauw", "__w3src3md_MOD_delu"):
        f.write(bytes(inf().read_memory(sym(s), 4)))
    for name in "sig dden".split():
        f.write(array("w3gdatmd", name))
    n = 201 * 201
    f.write(struct.pack("<ii", 0, n) + bytes(inf().read_memory(sym("__w3src3md_MOD_taut"), 4 * n)))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, nspec, nk, a, u, tauwx, tauwy, llws):
    """Inputs ww3_ts1 does not produce, so the branches of W3SPR3 and CALC_USTAR run. Returns what
    to undo at exit. The call's outputs reach W3SRCE, so a poked run perturbs the next steps: the
    script stops after CAPTURE_LIMIT calls, and every recorded call is a real evaluation anyway."""
    old = lambda addr, n: (addr, bytes(inf().read_memory(addr, n)))
    put = lambda addr, v: inf().write_memory(addr, struct.pack("<f", v))
    kind = calls % 10
    if kind == 0:  # no wind: USTAR = 0, 0/0 in SQRTCDM1, CHARN = AALPHA
        undo = [old(u, 4)]
        put(u, 0.0)
    elif kind == 1:  # past the last column of TAUT: J and DELJ1 clamp
        undo = [old(u, 4)]
        put(u, 60.0)
    elif kind == 2:  # stress past TAUWMAX: TAUW clamp, IND at its top
        undo = [old(tauwx, 4), old(tauwy, 4)]
        put(tauwx, 3.0)
        put(tauwy, 0.0)
    elif kind == 3:  # an empty spectrum: the 1.E-7 branches of section 4
        undo = [old(a, 4 * nspec)]
        inf().write_memory(a, bytes(4 * nspec))
    elif kind == 4:  # no wind-sea component: EMEANWS < 1.E-7
        undo = [old(llws, 4 * nspec)]
        inf().write_memory(llws, bytes(4 * nspec))
    elif kind == 5:  # a slightly negative wind: DELJ1 < 0, UNZ = 0.01
        undo = [old(u, 4)]
        put(u, -0.1)
    elif kind == 6:  # a spectrum only in the top six bands, so FMEANWS >= 1.E-7 > EMEANWS
        undo = [old(a, 4 * nspec)]
        nth = nspec // nk
        inf().write_memory(a, bytes(4 * (nspec - 6 * nth)) + struct.pack("<f", 2.0e-10) * (6 * nth))
    elif kind == 8 or kind == 9:  # a breeze: USTAR between 0.001 and 0.01, CHARN from Z0
        undo = [old(u, 4)]
        put(u, 0.1 if kind == 8 else 0.03)
    else:  # a gale between two columns of TAUT with a large stress: both DELJ and DELI fractions
        undo = [old(u, 4), old(tauwx, 4), old(tauwy, 4)]
        put(u, 37.3)
        put(tauwx, 1.0)
        put(tauwy, 1.2)
    return undo


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # A CG WN EMEAN FMEAN FMEANS WNMEAN AMAX in x0-7; the stack holds the rest.
    u, udir, ustar, usdir, tauwx, tauwy, cd, z0, charn, llws, fmeanws = struct.unpack(
        "<11Q", bytes(inf().read_memory(sp, 88)))
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, nspec, nk, reg[0], u, tauwx, tauwy, llws)
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    for addr in (u, udir, ustar, usdir, tauwx, tauwy):
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))
    f.write(read(llws, nspec))

    def done():
        for addr in (reg[3], reg[4], reg[5], reg[6], reg[7], ustar, usdir, cd, z0, charn, fmeanws):
            f.write(read(addr, 1))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0
run("__w3src3md_MOD_w3spr3", header, record)
