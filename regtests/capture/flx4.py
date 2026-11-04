# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with FLX4: records W3FLX4's arguments and results on sampled calls.
# Format, little-endian: b"FLX4". Per call: ZWND, U10, U10D, FLX4A0 (f32); then UST, USTD, Z0, CD (f32).
# FLX4A0 is W3GDATMD's pointer to the grid's scaling (the namelist's CDFAC), read at every call.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, inf, run, scalar  # noqa: E402


def read(addr, n):
    return bytes(inf().read_memory(addr, 4 * n))


def header(f):
    f.write(b"FLX4")


# Wind speeds the run never produces, written at the routine's entry in turn (CAPTURE_POKE): none,
# calm, a trace, negative, the cap's edge (50.33 and the float just below it), strong winds past the
# cap, the speed that makes CD zero, one that makes it negative, and a huge one. The cap and the
# polynomial's negative range are what a 12 m/s run cannot reach.
POKE = [None, 0.0, 1.0e-3, -5.0, 50.33, 50.329998, 55.0, 100.0, 1.0e6, 30.0, 60.4375, -20.0, 49.9]


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
    f.write(struct.pack("<f", float(scalar("w3gdatmd::flx4a0"))))

    def done():
        for addr in reg[3:]:
            f.write(read(addr, 1))
        for addr, old in undo:
            inf().write_memory(addr, old)

    Exit(done)


record.calls = 0


run("__w3flx4md_MOD_w3flx4", header, record)
