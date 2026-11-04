# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with ST2: records what W3SDS2 reads from W3GDATMD once, then its
# arguments and results on sampled calls. W3SDS2 is called from W3SRCE, W3EXPO and GXEXPO.
# Format, little-endian: b"SDS2", NK NTH (i32), FTE DTH FPIMIN FACTI1 FACTI2 XFH XF1 XF2 SDSALN CDSA0 CDSA1
# CDSA2 CDSB0 CDSB1 CDSB2 CDSB3 (f32), then SIG(0:NK+1) and DDEN(NK) as common.array writes them. Per call:
# FPI USTAR, A(NSPEC), CG(NK), K(NK), ALFA(NK); then S(NSPEC), D(NSPEC). Reader:
# rust/crates/ww3-host/src/capture.rs, Sds2Capture.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"
REALS = [G + r for r in "fte dth fpimin facti1 facti2 xfh xf1 xf2 sdsaln cdsa0 cdsa1 cdsa2 cdsb0 cdsb1 cdsb2 cdsb3".split()]


def header(f):
    ints = [int(scalar(G + n)) for n in ("nk", "nth")]
    f.write(b"SDS2")
    f.write(struct.pack("<2i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    sig = array("w3gdatmd", "sig")
    f.write(sig)
    f.write(array("w3gdatmd", "dden"))
    header.nk, header.nspec = ints[0], ints[0] * ints[1]
    header.sig = struct.unpack(f"<{header.nk + 2}f", sig[8:])
    header.fpimin = float(scalar(G + "fpimin"))


def poke(calls, fpi, ustar, a):
    """Inputs ww3_ts1 does not produce, so every branch of W3SDS2 runs on the real Fortran. FHW is
    2*FPI, XHW its position among the bands, and SIG(NK+1) is where the high-frequency branch starts
    (:719). The call's outputs reach W3SRCE: they are zeroed at exit, and the script stops after
    CAPTURE_LIMIT calls. Returns what to restore."""
    nk, sig = header.nk, header.sig
    old = lambda addr, n: (addr, bytes(inf().read_memory(addr, n)))
    put = lambda addr, v: inf().write_memory(addr, struct.pack("<f", v))
    kind = calls % 12
    undo = []
    if kind in (0, 1, 2, 3, 4, 5, 8, 9):
        undo.append(old(fpi, 4))
        v = {
            0: sig[nk + 1] / 2 * 0.99,  # just below the switch: low branch, IKHW = NK
            1: sig[nk + 1] / 2,  # FHW = SIG(NK+1) exactly: the high-frequency branch starts
            2: sig[nk + 1] * 0.7,  # deep in the high-frequency branch
            3: sig[nk] / 2 * 1.03,  # XHW + 0.5 just over NK: MAX(0., XHW-0.5) clamps to 0
            4: sig[nk] / 2 * 0.99,  # IKHW = NK with a fraction that XHW-0.5 keeps
            5: sig[1] / 2 * 0.97,  # IKHW = 1
            8: sig[1] / 2 * 1.2,  # IKHW = 2
            9: sig[nk // 2] / 2,  # the middle of the table
        }[kind]
        put(fpi, v)
    elif kind in (6, 7, 10):
        undo.append(old(ustar, 4))
        # no friction: CDIST = -0 and POW capped; a gale: FPIT above FPIMIN; a tiny USTAR
        put(ustar, {6: 0.0, 7: 1.8, 10: 1.0e-4}[kind])
    else:  # an empty spectrum
        undo.append(old(a, 4 * header.nspec))
        inf().write_memory(a, bytes(4 * header.nspec))
    return undo


def record(f):
    nk, nspec = header.nk, header.nspec
    a, cg, k, fpi, ustar, alfa, s_addr, d_addr = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, fpi, ustar, a)
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    f.write(read(fpi, 1))
    f.write(read(ustar, 1))
    f.write(read(a, nspec))
    for addr in (cg, k, alfa):
        f.write(read(addr, nk))

    def done():
        f.write(read(s_addr, nspec))
        f.write(read(d_addr, nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)
        if undo:
            inf().write_memory(s_addr, bytes(4 * nspec))
            inf().write_memory(d_addr, bytes(4 * nspec))

    Exit(done)


record.calls = 0
run("__w3src2md_MOD_w3sds2", header, record)
