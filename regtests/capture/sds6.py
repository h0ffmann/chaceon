# Run by gdb inside the reference built with ST6: records W3SDS6's scalars and tables once, then
# its arguments and results on sampled calls.
# Format, little-endian: b"SDS6", NK NTH SDS6ET SDS6P1 SDS6P2 (i32), SDS6A1 SDS6A2 DTH XFR (f32),
# SIG2 and DSII each as (extent, count: i32) then the data. Per call: A(NSPEC), CG(NK), WN(NK);
# then S(NSPEC), D(NSPEC).
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
    ints = [int(scalar(G + n)) for n in ("nk", "nth", "sds6et", "sds6p1", "sds6p2")]
    f.write(b"SDS6")
    f.write(struct.pack("<5i", *ints))
    f.write(struct.pack("<4f", *[float(scalar(G + n)) for n in ("sds6a1", "sds6a2", "dth", "xfr")]))
    f.write(array("w3gdatmd", "sig2"))
    f.write(array("w3gdatmd", "dsii"))
    header.nk, header.nspec = ints[0], ints[0] * ints[1]
    header.nth = ints[1]


def poke(calls, addrs):
    """Inputs the running case never makes. The real Fortran still executes on what is written at its
    entry. Cycle of ten calls: unchanged; a zero spectrum (EDENSMAX is 0 and ALL(EDENS > 0) is
    false); one band far above the rest (the DO loop of the ELSE branch, the others below EDENSMAX);
    a NaN in A; a NaN in WN (ETDENS, NEXDENS, T1 and T2 become NaN, which MAX(0.0, T) turns into 0);
    CG = 0 in one band; WN = -1 in one band (NEXDENS below 0); CG = -1 in one band;
    a flat spectrum of 1 (ALL is true) and a flat one of 1.E4.
    Returns what to restore: (address, bytes) pairs."""
    nk, nth, nspec = header.nk, header.nth, header.nspec
    a_addr, cg_addr, wn_addr = addrs
    kind, band = calls % 10, (calls // 10) % nk
    saved = [(a_addr, read(a_addr, nspec)), (cg_addr, read(cg_addr, nk)), (wn_addr, read(wn_addr, nk))]
    a = list(struct.unpack(f"<{nspec}f", saved[0][1]))
    cg = list(struct.unpack(f"<{nk}f", saved[1][1]))
    wn = list(struct.unpack(f"<{nk}f", saved[2][1]))
    if kind == 1:
        a = [0.0] * nspec
    elif kind == 2:
        # 1.E6 puts EDENS above ETDENS in the lowest bands, whose threshold is the highest; 1.E3
        # does so only higher up.
        amp = 1.0e6 if (calls // 10) % 2 == 0 else 1.0e3
        a = [1.0e-12] * nspec
        for ith in range(nth):
            a[ith + band * nth] = amp
    elif kind == 3:
        a[(calls // 10) % nspec] = float("nan")
    elif kind == 4:
        wn[band] = float("nan")
    elif kind == 5:
        cg[band] = 0.0
    elif kind == 6:
        wn[band] = -1.0
    elif kind == 7:
        cg[band] = -1.0
    elif kind == 8:
        a = [1.0] * nspec  # every EDENS within 1E5 of the largest: ALL(EDENS > EDENSMAX) is true
    elif kind == 9:
        a = [1.0e4] * nspec
    inf().write_memory(a_addr, struct.pack(f"<{nspec}f", *a))
    inf().write_memory(cg_addr, struct.pack(f"<{nk}f", *cg))
    inf().write_memory(wn_addr, struct.pack(f"<{nk}f", *wn))
    return saved


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    s_addr, d_addr = reg[3], reg[4]
    old = None
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        old = poke(record.calls, (reg[0], reg[1], reg[2]))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))

    def done():
        f.write(read(s_addr, nspec))
        f.write(read(d_addr, nspec))
        if old is not None:
            for addr, data in old:
                inf().write_memory(addr, data)
            # S and D of a poked call may be NaN: keep them out of the integration.
            inf().write_memory(s_addr, bytes(4 * nspec))
            inf().write_memory(d_addr, bytes(4 * nspec))

    Exit(done)


record.calls = 0
run("__w3src6md_MOD_w3sds6", header, record)
