# Run by gdb inside the reference built with ST2 and FLX2: records W3FLX2's tables once, then its
# arguments and results on sampled calls.
# Format, little-endian: b"FLX2", NITTIN (i32), CINXSI, DSIE (f32), N1MAX (i32), EWN1(0:121) (f32).
# Per call: ZWIND, DEPTH, FP, U, UDIR, UST (f32, UST in and out); then UST, USTD, Z0, CD (f32).
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
NAR1D = 121


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    f.write(b"FLX2")
    f.write(struct.pack("<i", int(scalar(G + "nittin"))))
    f.write(struct.pack("<f", float(scalar(G + "cinxsi"))))
    f.write(read(sym("__w3dispmd_MOD_dsie"), 1))
    f.write(read(sym("__w3dispmd_MOD_n1max"), 1))
    f.write(read(sym("__w3dispmd_MOD_ewn1"), NAR1D + 1))


# Inputs the run never produces, written at the routine's entry in turn (CAPTURE_POKE), each a list of
# (argument, value): a peak frequency under the 0.001 floor, zero, negative, NaN and infinite; a wind
# under 0.01, negative and very strong (RDCH goes negative and is floored); a friction velocity under
# 0.0001, negative and NaN; depths from a trace to the table's end and past it, zero and negative.
# Argument indices: 0 ZWIND, 1 DEPTH, 2 FP, 3 U, 4 UDIR, 5 UST.
POKE = [
    [],
    [(2, 0.0)], [(2, 1.0e-5)], [(2, -0.3)], [(2, float("nan"))], [(2, float("inf"))], [(2, 0.0005)],
    [(3, 0.0)], [(3, 0.005)], [(3, -3.0)], [(3, 80.0)], [(3, 1000.0)],
    [(5, 0.0)], [(5, -1.0)], [(5, float("nan"))], [(5, 1.0e-5)], [(5, 2.0)],
    [(1, 0.001)], [(1, 0.5)], [(1, 5.0)], [(1, 40.0)], [(1, 1.0e5)], [(1, 0.0)], [(1, -10.0)],
    [(1, float("nan"))], [(1, 3.0), (2, 0.5)], [(1, 20.0), (2, 0.02)],
    [(0, 1.0)], [(0, 10.0), (3, 25.0)], [(0, 100.0)], [(0, 0.0)],
    [],
]


def poke(calls, reg):
    todo = POKE[calls % len(POKE)]
    undo = []
    for arg, value in todo:
        old = bytes(inf().read_memory(reg[arg], 4))
        inf().write_memory(reg[arg], struct.pack("<f", value))
        undo.append((reg[arg], old))
    return undo


def record(f):
    # Nine arguments, all by reference: ZWIND, DEPTH, FP, U, UDIR, UST in; UST, USTD, Z0, CD out.
    # The ninth, CD, is on the stack.
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    reg.append(struct.unpack("<Q", bytes(inf().read_memory(sp, 8)))[0])
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, reg)
    for addr in reg[:6]:
        f.write(read(addr, 1))

    def done():
        for addr in reg[5:]:
            f.write(read(addr, 1))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0


run("__w3flx2md_MOD_w3flx2", header, record)
