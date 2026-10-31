# Run by gdb inside the reference: records W3SDS4's tables once, then its arguments on sampled calls.
# Format: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar  # noqa: E402

G = "w3gdatmd::"
INTS = [G + "nk", G + "nth", G + "nspec", "w3src4md::dikcumul", G + "ssdsbrfdf", G + "ssdsiso",
        G + "nkhs", G + "nkd", G + "ndtab"]
REALS = "ssdsbr ssdsbt ssdsp ssdsdth dth aairgb aaircmin".split()


def header(f):
    ints = [int(scalar(i)) for i in INTS]
    f.write(b"SDS4")
    f.write(struct.pack("<9i", *ints))
    flags = [bool(gdb.parse_and_eval(f"w3odatmd::flogrd(5,{i})")) for i in (7, 8)]
    f.write(struct.pack("<2i", *[int(x) for x in flags]))
    f.write(array("w3gdatmd", "ssdsc"))
    f.write(array("w3gdatmd", "ssdsbm"))
    f.write(b"".join(struct.pack("<f", float(scalar(G + r))) for r in REALS))
    for name in "sig dden ec2 es2 esc ecos esin".split():
        f.write(array("w3gdatmd", name))
    for name in "satindices satweights cumulw iktab dcki qbi".split():
        f.write(array("w3gdatmd", name, 2))
    header.nk, header.nspec = ints[0], ints[2]


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    sp = int(gdb.parse_and_eval("$sp"))
    # Arguments 9 to 14 are on the stack: DDIAG, IX, IY, BRLAMBDA, WHITECAP, DLWMEAN.
    ddiag, _, _, brlambda, whitecap, dlwmean = struct.unpack(
        "<6Q", bytes(inf().read_memory(sp, 48)))
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    for addr in (reg[3], reg[4], reg[5], reg[6], dlwmean):
        f.write(read(addr, 1))
    f.write(read(whitecap, 4))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nk))
    srhs = reg[7]

    def done():
        f.write(read(srhs, nspec))
        f.write(read(ddiag, nspec))
        f.write(read(brlambda, nspec))
        f.write(read(whitecap, 4))

    Exit(done)


run("__w3src4md_MOD_w3sds4", header, record)
