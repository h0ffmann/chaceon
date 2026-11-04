# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with ST3: records what W3SIN3 reads from W3GDATMD and W3SRC3MD,
# taken as INSIN3 left them at the first call, then W3SIN3's arguments and results on sampled calls.
# Format, little-endian: b"SIN3", NK NTH NSPEC (i32), BBETA AALPHA ZZALP SSINTHP DTH DELUST DELALP
# (f32), then SSWELLF SIG SIG2 DDEN2 ECOS ESIN as common.array writes them, TAUHFT(0:100,0:200) as
# 101*201 f32. Per call: USTAR DRAT USDIR Z0, A(NSPEC), CG(NK), K(NSPEC); then TAUWX TAUWY TAUWNX
# TAUWNY, S(NSPEC), D(NSPEC), LLWS(NSPEC) (i32). Reader: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
INTS = [G + "nk", G + "nth", G + "nspec"]
REALS = [G + r for r in "bbeta aalpha zzalp ssinthp dth".split()]
MODULE = [f"__w3src3md_MOD_{n}" for n in ("delust", "delalp")]


def header(f):
    ints = [int(scalar(i)) for i in INTS]
    f.write(b"SIN3")
    f.write(struct.pack("<3i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    f.write(b"".join(bytes(inf().read_memory(sym(s), 4)) for s in MODULE))
    for name in "sswellf sig sig2 dden2 ecos esin".split():
        f.write(array("w3gdatmd", name))
    f.write(bytes(inf().read_memory(sym("__w3src3md_MOD_tauhft"), 4 * 101 * 201)))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, ustar, z0):
    """Inputs WW3 does not produce on its own, so the table clamps and the floors run.

    MetOcean's W3SRCE floors USTAR at 0.001, and 12 m/s keeps it near 0.5 m/s: IND (:683) reaches
    IUSTAR-1 only here (a calm run reaches 1), and the floors of UST2 (:700) and XJ (:686) never act
    in a run. Z0 reaches J = IALPHA-1 (:687) in a calm run only. The real Fortran executes those
    lines on the input written at its entry and the original is put back at exit. USTAR = 5E-7 is
    below both floors
    (EPS2 = 1E-6 and 1E-5). Every value is finite and positive: USTAR = 0 would give NaN in D.
    """
    kind = calls % 6
    if kind == 5:
        return []
    addr, value = {0: (ustar, 1.0e-5), 1: (ustar, 100.0), 2: (z0, 1.0), 3: (z0, 1.0e-12),
                   4: (ustar, 5.0e-7)}[kind]
    old = bytes(inf().read_memory(addr, 4))
    inf().write_memory(addr, struct.pack("<f", value))
    return [(addr, old)]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # Arguments 9 to 20 are on the stack: Z0, CD, TAUWX, TAUWY, TAUWNX, TAUWNY, ICE, S, D, LLWS, IX, IY.
    z0, _, tx, ty, tnx, tny, _, s, d, llws, _, _ = struct.unpack("<12Q", bytes(inf().read_memory(sp, 96)))
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, reg[4], z0)
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    for addr in (reg[4], reg[5], reg[7], z0):
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nspec))

    def done():
        for addr in (tx, ty, tnx, tny):
            f.write(read(addr, 1))
        f.write(read(s, nspec))
        f.write(read(d, nspec))
        f.write(read(llws, nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0
run("__w3src3md_MOD_w3sin3", header, record)
