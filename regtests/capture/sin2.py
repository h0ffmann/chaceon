# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with ST2: records what W3SIN2 reads from W3GDATMD and W3SRC2MD,
# taken at the first call (INPTAB has run by then), then W3SIN2's arguments and results on sampled calls.
# Format, little-endian: b"SIN2", NK NTH NSPEC (i32), DSIGA DDRAG ZWIND FTE FTTR XFR FPIMIN FACTI1
# FACTI2 FSWELL (f32), then SIG SIG2 DDEN ECOS ESIN as common.array writes them, BETATB(-400:401,21) as
# 802*21 f32. Per call: U UDIR CD Z0, A(NSPEC), CG(NK), K(NSPEC); then FPI, S(NSPEC), D(NSPEC).
# Reader: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
INTS = [G + "nk", G + "nth", G + "nspec"]
REALS = [G + r for r in "zwind fte fttr xfr fpimin facti1 facti2 fswell".split()]
MODULE = [f"__w3src2md_MOD_{n}" for n in ("dsiga", "ddrag")]


def header(f):
    ints = [int(scalar(i)) for i in INTS]
    f.write(b"SIN2")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(bytes(inf().read_memory(sym(s), 4)) for s in MODULE))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    for name in "sig sig2 dden ecos esin".split():
        f.write(array("w3gdatmd", name))
    f.write(bytes(inf().read_memory(sym("__w3src2md_MOD_betatb"), 4 * 802 * 21)))
    header.nk, header.nth, header.nspec = ints[0], ints[1], ints[2]


def poke(calls, u, cd, z0, a):
    """Inputs WW3 does not produce on its own, so the table clamps and the guards run.

    A 12 m/s wind keeps OMA far inside +-NRSIGA*DSIGA = 40 (:469), CLAM/DDRAG between 1 and 20 (:470)
    and M1 above 1E-20 (:503). U = 300 reaches both ends of IOMA, CD = 0.1 reaches ICL = NRDRAG,
    CD = 1E-4 reaches ICL = 1, and U = 1E-6 makes every S negative so that M1 = 0 and NKFILT passes NK.
    Z0 = 20 puts ZWIND/Z0 below 1 and Z0 = 0.1 above e. Kind 7 sets U = 100, A = 1 in the first
    frequency and 0 above it, so that only the lowest bin has input: FPI is then SIG(1), FP1 falls below 0.37
    rad/s and NKFIL2 is negative (:518), which no model state reaches (FPI never drops below about 1
    rad/s in a run). Kind 8 gives CD a negative value, so
    USTAR = U*SQRT(CD) is NaN (a NaN `MAX(FPIMIN, NaN)` is FPIMIN under arm64's fmaxnm, :512, though FP1
    is NaN either way). Kind 9 writes a NaN into A(1), so that S(1) is NaN and `MAX(0.,S)` (:488) has to
    skip it, as fmaxnm does: FPI stays finite. Every other value is finite and positive, A is not negative. The real Fortran executes
    those lines on the input written at its entry; the original is put back at exit.
    """
    kind = calls % 11
    if kind == 10:
        return []
    if kind == 9:
        old = bytes(inf().read_memory(a, 4))
        inf().write_memory(a, struct.pack("<I", 0x7FC00000))
        return [(a, old)]
    if kind == 7:
        n = header.nspec
        old = bytes(inf().read_memory(a, 4 * n))
        olds = [(a, old), (u, bytes(inf().read_memory(u, 4)))]
        inf().write_memory(u, struct.pack("<f", 100.0))
        inf().write_memory(a, struct.pack("<f", 1.0) * header.nth + struct.pack("<f", 0.0) * (n - header.nth))
        return olds
    addr, value = {0: (u, 300.0), 1: (cd, 0.1), 2: (cd, 1.0e-4), 3: (u, 1.0e-6), 4: (z0, 20.0),
                   5: (z0, 0.1), 6: (u, 40.0), 8: (cd, -1.0e-3)}[kind]
    old = bytes(inf().read_memory(addr, 4))
    inf().write_memory(addr, struct.pack("<f", value))
    return [(addr, old)]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    a, cg, k, u, udir, cd, z0, fpi = reg
    # Arguments 9 and 10 are on the stack: S and D.
    s, d = struct.unpack("<2Q", bytes(inf().read_memory(int(gdb.parse_and_eval("$sp")), 16)))
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, u, cd, z0, a)
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    for addr in (u, udir, cd, z0):
        f.write(read(addr, 1))
    f.write(read(a, nspec))
    f.write(read(cg, nk))
    f.write(read(k, nspec))

    def done():
        f.write(read(fpi, 1))
        f.write(read(s, nspec))
        f.write(read(d, nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0
run("__w3src2md_MOD_w3sin2", header, record)
