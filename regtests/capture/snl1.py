# Run by gdb inside the reference: records W3SNL1's tables once, then its real arguments on sampled
# calls. Nothing is compiled or linked into WW3. Format: rust/tools/snl1_cmp/src/main.rs.
import os
import struct

import gdb

OUT = os.environ["CAPTURE_OUT"]
STRIDE = int(os.environ.get("CAPTURE_STRIDE", "50"))
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


def inf():
    return gdb.selected_inferior()


def sym(name):
    text = gdb.execute(f"info address {name}", to_string=True)
    return int(text.split(" at ")[1].split()[0].rstrip("."), 16)


def desc(module, name):
    """A gfortran rank-1 pointer descriptor: base at +0, lbound and ubound at +48 and +56."""
    raw = bytes(inf().read_memory(sym(f"__{module}_MOD_{name}"), 64))
    base = struct.unpack_from("<Q", raw, 0)[0]
    lb, ub = struct.unpack_from("<qq", raw, 48)
    return base, lb, ub


def scalar(expr):
    """WW3 keeps most grid scalars as Fortran pointers into GRIDS, WADATS and MPARS."""
    value = gdb.parse_and_eval(expr)
    if value.type.strip_typedefs().code == gdb.TYPE_CODE_PTR:
        value = value.dereference()
    return value


def tables(f):
    ints = [int(scalar(("w3gdatmd::" if n in ("nk", "nth", "nspec") else "w3adatmd::") + n)) for n in INTS]
    nk, nspec, nspecx = ints[0], ints[2], ints[6]
    f.write(b"SNL1")
    f.write(struct.pack("<8i", *ints))
    f.write(b"".join(struct.pack("<f", float(scalar(r))) for r in REALS))
    base, lb, ub = desc("w3gdatmd", "sig")
    assert (lb, ub) == (0, nk + 1), (lb, ub)
    f.write(bytes(inf().read_memory(base, 4 * (nk + 2))))
    base, lb, ub = desc("w3adatmd", "af11")
    assert ub - lb + 1 >= nspecx, (lb, ub, nspecx)
    f.write(bytes(inf().read_memory(base, 4 * nspecx)))
    for name, n in [(p, nspecx) for p in PM] + [(c, nspec) for c in IC]:
        base, lb, ub = desc("w3adatmd", name)
        assert ub - lb + 1 >= n, (name, lb, ub, n)
        f.write(bytes(inf().read_memory(base, 4 * n)))
    return ints


class Exit(gdb.Breakpoint):
    def __init__(self, f, s, d, nspec):
        super().__init__(f"*{int(gdb.parse_and_eval('$x30'))}", internal=True)
        self.f, self.s, self.d, self.nspec = f, s, d, nspec

    def stop(self):
        self.f.write(bytes(inf().read_memory(self.s, 4 * self.nspec)))
        self.f.write(bytes(inf().read_memory(self.d, 4 * self.nspec)))
        self.enabled = False
        return False


class Entry(gdb.Breakpoint):
    def __init__(self, f, addr):
        super().__init__(f"*{addr}", internal=True)
        self.f, self.ints, self.calls, self.kept = f, None, 0, 0

    def stop(self):
        try:
            return self.record()
        except Exception as e:
            import traceback
            open(OUT + ".error", "w").write(traceback.format_exc())
            return True

    def record(self):
        if self.ints is None:
            self.ints = tables(self.f)
        self.calls += 1
        if (self.calls - 1) % STRIDE == 0:
            nk, nspec = self.ints[0], self.ints[2]
            reg = [int(gdb.parse_and_eval(f"$x{i}")) for i in range(5)]
            self.f.write(bytes(inf().read_memory(reg[2], 4)))
            self.f.write(bytes(inf().read_memory(reg[0], 4 * nspec)))
            self.f.write(bytes(inf().read_memory(reg[1], 4 * nk)))
            Exit(self.f, reg[3], reg[4], nspec)
            self.kept += 1
        return False


gdb.execute("set python print-stack full")
gdb.execute("set pagination off")
gdb.execute("set confirm off")
gdb.execute("starti > /dev/null", to_string=True)
with open(OUT, "wb") as out:
    entry = Entry(out, sym("__w3snl1md_MOD_w3snl1"))
    gdb.execute("continue", to_string=True)
    print(f"capture: {entry.calls} W3SNL1 calls, {entry.kept} kept")
gdb.execute("quit")
