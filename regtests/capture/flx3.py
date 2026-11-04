# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with FLX3 (switches/switch_chaceon_st2flx3): records what W3FLX3 reads
# from W3GDATMD and W3DISPMD, then its arguments and results on sampled calls.
# Format, little-endian: b"FLX3", NITTIN CAP_ID N1MAX (i32), CINXSI CD_MAX DSIE (f32), EWN1(0:121) as 122 f32.
# Per call: ZWIND DEPTH FP U UDIR UST (f32, UST at entry); then UST USTD Z0 CD (f32).
# Reader: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"


def header(f):
    nittin, cap_id = int(scalar(G + "nittin")), int(scalar(G + "cap_id"))
    n1max = struct.unpack("<i", bytes(inf().read_memory(sym("__w3dispmd_MOD_n1max"), 4)))[0]
    f.write(b"FLX3")
    f.write(struct.pack("<3i", nittin, cap_id, n1max))
    f.write(struct.pack("<f", float(scalar(G + "cinxsi"))))
    f.write(struct.pack("<f", float(scalar(G + "cd_max"))))
    f.write(bytes(inf().read_memory(sym("__w3dispmd_MOD_dsie"), 4)))
    f.write(bytes(inf().read_memory(sym("__w3dispmd_MOD_ewn1"), 4 * 122)))


# Inputs the run never produces, written at the routine's entry in turn (CAPTURE_POKE): argument index
# (0 ZWIND, 1 DEPTH, 2 FP, 3 U, 5 UST) and value. They reach the floors and the edges: MAX (FP,0.001) with
# FP zero, negative and NaN; MAX (0.01,U); MAX (UST,0.0001); the table branch of the inlined WAVNU1 in deep
# water and its else branch in shallow water (and the other way round, with a poked FP); a ZWIND where
# LOG gives a negative argument to RDCH's MAX. Most calls keep the run's own inputs.
NAN = float("nan")
POKE = [None, None, (2, 0.0), (2, -1.0), (2, NAN), (2, 5.0), (3, 0.0), (3, -3.0), (3, NAN), (3, 100.0), None,
        (5, 0.0), (5, -1.0), (5, NAN), (5, 5.0), (1, 0.01), (1, 1.0), (1, 200.0), (1, 1.0e5), (0, 0.0),
        (0, 1.0e-3), (0, 100.0), (0, -1.0), (2, 0.05), (3, 0.0005), (1, 0.0), (1, -1.0), None]


def poke(calls, reg):
    value = POKE[calls % len(POKE)]
    if value is None:
        return []
    arg, v = value
    old = bytes(inf().read_memory(reg[arg], 4))
    inf().write_memory(reg[arg], struct.pack("<f", v))
    return [(reg[arg], old)]


def record(f):
    # Nine arguments, all by reference: ZWIND DEPTH FP U UDIR UST in (UST inout); USTD Z0 CD out. The ninth
    # is on the stack at entry.
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    reg.append(struct.unpack("<Q", bytes(inf().read_memory(int(gdb.parse_and_eval("$sp")), 8)))[0])
    undo = []
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo = poke(record.calls, reg)
    for addr in reg[:6]:
        f.write(bytes(inf().read_memory(addr, 4)))

    def done():
        for addr in (reg[5], reg[6], reg[7], reg[8]):
            f.write(bytes(inf().read_memory(addr, 4)))
        if undo:
            # The run goes on with finite values: the poked call's results are replaced by the last
            # unpoked call's, and the poked input is put back. The routine saw the poked input.
            for addr, old in undo:
                inf().write_memory(addr, old)
            for addr, last in zip((reg[5], reg[7], reg[8]), record.last):
                if last is not None:
                    inf().write_memory(addr, last)
        else:
            record.last = [bytes(inf().read_memory(reg[i], 4)) for i in (5, 7, 8)]

    Exit(done)


record.calls = 0
record.last = [None, None, None]
run("__w3flx3md_MOD_w3flx3", header, record)
