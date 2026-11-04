# Run by gdb inside the reference built with ST3: records W3SDS3's scalars once, then its arguments
# and results on sampled calls.
# Format, little-endian: b"SDS3", NK NTH (i32), SSDSC1 DDELTA1 DDELTA2 (f32). Per call: EMEAN FMEAN
# WNMEAN, A(NSPEC), K(NK); then S(NSPEC), D(NSPEC).
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
    ints = [int(scalar(G + n)) for n in ("nk", "nth")]
    f.write(b"SDS3")
    f.write(struct.pack("<2i", *ints))
    f.write(struct.pack("<3f", *[float(scalar(G + n)) for n in ("ssdsc1", "ddelta1", "ddelta2")]))
    header.nk, header.nspec = ints[0], ints[0] * ints[1]


def poke(calls, wnmean):
    """WNMEAN below the 1.E-10 floor of `:1382`, which no run of ww3_ts1 reaches. The real Fortran
    still executes its MAX on the input written at its entry. Returns the bytes to restore."""
    old = bytes(inf().read_memory(wnmean, 4))
    value = (0.0, 1.0e-12, -0.5, float("nan"))[calls % 4]
    inf().write_memory(wnmean, struct.pack("<f", value))
    return old


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # Arguments 9 to 13 are on the stack: DEPTH, S, D, IX, IY.
    _, s_addr, d_addr = struct.unpack("<3Q", bytes(inf().read_memory(sp, 24)))
    old = None
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        old = poke(record.calls, reg[5])
    for addr in (reg[3], reg[4], reg[5]):
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))

    def done():
        f.write(read(s_addr, nspec))
        f.write(read(d_addr, nspec))
        if old is not None:
            inf().write_memory(reg[5], old)
            # S and D of a poked call may be NaN: keep them out of the integration.
            inf().write_memory(s_addr, bytes(4 * nspec))
            inf().write_memory(d_addr, bytes(4 * nspec))

    Exit(done)


record.calls = 0
run("__w3src3md_MOD_w3sds3", header, record)
