# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with ST6 and FLX4 (switch_chaceon_st6): records what W3SIN6 reads from
# W3GDATMD, then its arguments and results on sampled calls. The helpers LFACTOR, TAU_WAVE_ATMOS, IRANGE and
# TAUWINDS are inlined into W3SIN6 or called from it, so one capture covers them all.
# Format, little-endian: b"SIN6", NK NTH NSPEC (i32), DTH XFR SIN6A0 SIN6WS (f32), then SIG2 DDEN2 ECOS ESIN as
# common.array writes them. Per call: UABS USTAR USDIR CD DAIR, A(NSPEC), CG(NK), WN2(NSPEC); then TAUWX TAUWY
# TAUNWX TAUNWY, S(NSPEC), D(NSPEC). Reader: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"
INTS = [G + "nk", G + "nth", G + "nspec"]
REALS = [G + r for r in "dth xfr sin6a0 sin6ws".split()]


def header(f):
    ints = [int(scalar(i)) for i in INTS]
    f.write(b"SIN6")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    for name in "sig2 dden2 ecos esin".split():
        f.write(array("w3gdatmd", name))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, reg, dair):
    """Inputs WW3 does not produce on its own, so every branch of LFACTOR runs.

    ww3_ts1 holds 12 m/s and USTAR near 0.5 m/s, where the wave-supported stress TAUW rarely exceeds
    TAU_TOT, the viscous stress is not capped, and U10 is below the 22 m/s at which -5.E-5*U10 + 1.1E-3
    changes sign. The real Fortran executes those lines on the input written at its entry and the
    original is put back at exit. Arguments: A x0, UABS x3, USTAR x4, USDIR x5, DAIR x7. Every value is
    finite except the two NaN pokes.
    """
    kind = calls % 13
    if kind == 12:
        return []
    ustar, uabs, usdir = reg[4], reg[3], reg[5]
    addr, value = {
        0: (ustar, 0.02),  # TAU_TOT far below the wave stress: the LFACTOR iteration runs long
        1: (ustar, 2.0),  # TAU_TOT far above it: no reduction
        2: (uabs, 30.0),  # above 22 m/s: the viscous stress is zero
        3: (uabs, 0.0),
        4: (ustar, 1.0e-5),  # TAU_TOT tiny against TAU_VIS: the 0.95*TAU_TOT cap
        5: (dair, 1.0),
        6: (usdir, 2.0),
        7: (ustar, 0.1),
        8: (dair, 1.0e7),  # S and TAUW far above TAU_TOT: RTAU starts near 1E5 and LFACTOR runs out of ITERMAX
        # A negative action in one bin, which the model never makes: S < 0 reaches the MAX(0.,S) of LFACTOR
        # (`:937-938`, positive part only). A(102), x0 is A.
        9: (reg[0] + 4 * 101, -0.01),
        # NaN where MAX and MIN meet it: fmaxnm and fminnm return the other operand (W1, W2, TAU_VIS).
        10: (usdir, float('nan')),
        11: (uabs, float('nan')),
    }[kind]
    old = bytes(inf().read_memory(addr, 4))
    inf().write_memory(addr, struct.pack("<f", value))
    return [(addr, old)]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # Arguments 9 to 14 are on the stack: TAUWX, TAUWY, TAUNWX, TAUNWY, S, D.
    tx, ty, tnx, tny, s, d = struct.unpack("<6Q", bytes(inf().read_memory(sp, 48)))
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, reg, reg[7])
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    # UABS USTAR USDIR CD DAIR
    for addr in (reg[3], reg[4], reg[5], reg[6], reg[7]):
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nspec))

    def done():
        for addr in (tx, ty, tnx, tny):
            f.write(read(addr, 1))
        f.write(read(s, nspec))
        f.write(read(d, nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0
run("__w3src6md_MOD_w3sin6", header, record)
