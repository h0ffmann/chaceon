# Run by gdb inside the reference: records W3SNL1's tables once, then its real arguments on sampled
# calls. Nothing is compiled or linked into WW3. Format: rust/tools/snl1_cmp/src/main.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, desc, inf, run, scalar  # noqa: E402

INTS = "nk nth nspec nfr nfrhgh nfrchg nspecx nspecy".split()
REALS = (
    "w3gdatmd::lam w3gdatmd::xfr w3gdatmd::dth w3gdatmd::fachfe w3gdatmd::kdcon w3gdatmd::kdmn "
    "w3gdatmd::snlc1 w3gdatmd::snls1 w3gdatmd::snls2 w3gdatmd::snls3 "
    "w3adatmd::dal1 w3adatmd::dal2 w3adatmd::dal3 "
    + " ".join(f"w3adatmd::awg{i}" for i in range(1, 9)) + " "
    + " ".join(f"w3adatmd::swg{i}" for i in range(1, 9))
).split()
PM = ["ip1" + str(i) for i in range(1, 5)] + ["im1" + str(i) for i in range(1, 5)] \
    + ["ip2" + str(i) for i in range(1, 5)] + ["im2" + str(i) for i in range(1, 5)]
IC = [f"ic{i}{j}" for i in range(1, 9) for j in (1, 2)]


def tables(f):
    ints = [int(scalar(("w3gdatmd::" if n in ("nk", "nth", "nspec") else "w3adatmd::") + n)) for n in INTS]
    nk, nspec, nspecx = ints[0], ints[2], ints[6]
    f.write(b"SNL1")
    f.write(struct.pack("<8i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    base, ((_, lb, ub),) = desc("w3gdatmd", "sig")
    assert (lb, ub) == (0, nk + 1), (lb, ub)
    f.write(bytes(inf().read_memory(base, 4 * (nk + 2))))
    base, ((_, lb, ub),) = desc("w3adatmd", "af11")
    assert ub - lb + 1 >= nspecx, (lb, ub, nspecx)
    f.write(bytes(inf().read_memory(base, 4 * nspecx)))
    for name, n in [(p, nspecx) for p in PM] + [(c, nspec) for c in IC]:
        base, ((_, lb, ub),) = desc("w3adatmd", name)
        assert ub - lb + 1 >= n, (name, lb, ub, n)
        f.write(bytes(inf().read_memory(base, 4 * n)))
    return ints


def record(f):
    nk, nspec = tables.ints[0], tables.ints[2]
    reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(5)]
    f.write(bytes(inf().read_memory(reg[2], 4)))
    f.write(bytes(inf().read_memory(reg[0], 4 * nspec)))
    f.write(bytes(inf().read_memory(reg[1], 4 * nk)))

    def done():
        f.write(bytes(inf().read_memory(reg[3], 4 * nspec)))
        f.write(bytes(inf().read_memory(reg[4], 4 * nspec)))

    Exit(done)


def header(f):
    tables.ints = tables(f)


run("__w3snl1md_MOD_w3snl1", header, record)
