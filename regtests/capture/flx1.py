# Run by gdb inside the reference built with FLX1: records W3FLX1's arguments and results on sampled calls.
# Format, little-endian: b"FLX1". Per call: ZWND, U10, U10D (f32); then UST, USTD, Z0, CD (f32).
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, inf, run  # noqa: E402


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    f.write(b"FLX1")


# Wind speeds the run never produces, written at the routine's entry in turn (CAPTURE_POKE): none,
# calm, a trace, negative, the speed that makes CD zero, and one that makes it negative. W3FLX1 has no
# guard but the ZWND test, so these exercise the square root and the exponential at their edges.
POKE = [None, 0.0, 1.0e-3, -5.0, -12.307692, -20.0, 300.0, 1.0e6]


def poke(calls, u10):
    value = POKE[calls % len(POKE)]
    if value is None:
        return []
    old = bytes(inf().read_memory(u10, 4))
    inf().write_memory(u10, struct.pack("<f", value))
    return [(u10, old)]


def record(f):
    # Seven arguments, all by reference: ZWND, U10, U10D in; UST, USTD, Z0, CD out.
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(7)]
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, reg[1])
    for addr in reg[:3]:
        f.write(read(addr, 1))

    def done():
        for addr in reg[3:]:
            f.write(read(addr, 1))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0


run("__w3flx1md_MOD_w3flx1", header, record)
