# Run by gdb inside the reference: records W3SIN4's tables, what INSIN4 needs and makes, then W3SIN4's
# arguments on sampled calls. Format: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
INTS = [G + "nk", G + "nth", G + "nspec", G + "nkhs", G + "nkd", G + "ndtab", G + "ssdsbrfdf",
        "w3src4md::dikcumul"]
REALS = [G + r for r in ("bbeta zzwnd aalpha zzalp ttauwshelter ssinthp zz0rat dth zz0max fachfe "
                         "xfr ssdscos ssdsbrf1 ssdsbint ssdspbk ssdsabk ssdshck ssdsbck "
                         "ssdsdth").split()]
MODULE = [f"__{m}_MOD_{n}" for m, n in (("w3src4md", "delust"), ("w3src4md", "deltail"),
                                        ("w3src4md", "deltauw"), ("w3src4md", "delu"),
                                        ("w3src4md", "delalp"), ("constants", "delab"))]


def header(f):
    ints = [int(scalar(i)) for i in INTS]
    f.write(b"SIN4")
    f.write(struct.pack(f"<{len(ints)}i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    f.write(b"".join(bytes(inf().read_memory(sym(s), 4)) for s in MODULE))
    for name in "sswellf ssdsc sig sig2 dden dden2 ecos esin th dsip".split():
        f.write(array("w3gdatmd", name))
    for name, n in (("constants_MOD_fwtable", 301), ("w3src4md_MOD_taut", 201 * 201),
                    ("w3src4md_MOD_tauhft", 101 * 201), ("w3src4md_MOD_tauhft2", 101 * 201 * 51)):
        f.write(struct.pack("<ii", 0, n) + bytes(inf().read_memory(sym(f"__{name}"), 4 * n)))
    for name in "satindices satweights cumulw iktab dcki qbi".split():
        f.write(array("w3gdatmd", name, 2))
    header.nk, header.nspec = ints[0], ints[2]


def poke(calls, nspec, a, ustar):
    """Inputs WW3 does not produce on its own, so its guards run. Returns what to undo at exit.

    MetOcean floors USTAR in W3SRCE, so W3SIN4's own floor and ISNAN guards see no such value in a
    run. The real Fortran still executes them here, on the input written at its entry.
    """
    kind = calls % 5
    if kind == 0 or kind == 1:
        old = bytes(inf().read_memory(ustar, 4))
        inf().write_memory(ustar, struct.pack("<f", 1.0e-5 if kind == 0 else 0.0))
        return [(ustar, old)], None
    # A NaN mid-spectrum reaches TEMP2 (:780), one in the last band reaches TEMP (:835) too.
    i = nspec // 2 if kind == 2 else nspec - 5
    old = bytes(inf().read_memory(a + 4 * i, 4))
    inf().write_memory(a + 4 * i, struct.pack("<I", 0x7fc00000 if kind != 4 else 0x7f800000))
    return [(a + 4 * i, old)], i


def record(f):
    nk, nspec = header.nk, header.nspec
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(8)]
    undo, spoiled = [], None
    if os.environ.get("CAPTURE_POKE"):
        record.calls += 1
        undo, spoiled = poke(record.calls, nspec, reg[0], reg[4])
    sp = int(gdb.parse_and_eval("$sp"))
    # Arguments 9 to 20 are on the stack: Z0, CD, TAUWX, TAUWY, TAUWNX, TAUWNY, S, D, LLWS, IX, IY,
    # BRLAMBDA.
    z0, cd, tx, ty, tnx, tny, s, d, llws, _, _, brl = struct.unpack(
        "<12Q", bytes(inf().read_memory(sp, 96)))
    read = lambda addr, n: bytes(inf().read_memory(addr, 4 * n))
    for addr in (reg[3], reg[4], reg[5], reg[6], reg[7], z0, cd):
        f.write(read(addr, 1))
    f.write(read(reg[0], nspec))
    f.write(read(reg[1], nk))
    f.write(read(reg[2], nspec))
    f.write(read(brl, nspec))

    def done():
        for addr in (tx, ty, tnx, tny):
            f.write(read(addr, 1))
        f.write(read(s, nspec))
        f.write(read(d, nspec))
        f.write(read(llws, nspec))
        for addr, old in undo:
            inf().write_memory(addr, old)
        # S is D*A, so the poked bin is NaN or Inf: keep it out of the integration.
        if spoiled is not None:
            inf().write_memory(s + 4 * spoiled, struct.pack("<f", 0.0))

    Exit(done)


record.calls = 0
run("__w3src4md_MOD_w3sin4", header, record)
