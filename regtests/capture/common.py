# Shared by the capture scripts, which gdb runs inside the reference.
import os
import struct

import gdb

OUT = os.environ["CAPTURE_OUT"]
STRIDE = int(os.environ.get("CAPTURE_STRIDE", "50"))


def inf():
    return gdb.selected_inferior()


def sym(name):
    text = gdb.execute(f"info address {name}", to_string=True)
    return int(text.split(" at ")[1].split()[0].rstrip("."), 16)


def desc(module, name, rank=1):
    """A gfortran descriptor: base at +0, then stride, lbound, ubound per dimension from +40."""
    raw = bytes(inf().read_memory(sym(f"__{module}_MOD_{name}"), 40 + 24 * rank))
    base = struct.unpack_from("<Q", raw, 0)[0]
    dims = [struct.unpack_from("<qqq", raw, 40 + 24 * d) for d in range(rank)]
    return base, dims


def scalar(expr):
    """WW3 keeps most grid scalars as Fortran pointers into GRIDS, WADATS and MPARS."""
    value = gdb.parse_and_eval(expr)
    if value.type.strip_typedefs().code == gdb.TYPE_CODE_PTR:
        value = value.dereference()
    return value


def array(module, name, rank=1):
    """Writes as `first extent, count, data`. Unallocated arrays have count 0."""
    base, dims = desc(module, name, rank)
    extents = [ub - lb + 1 for _, lb, ub in dims]
    count = 1
    expected = dims[0][0]
    for (stride, _, _), extent in zip(dims, extents):
        assert stride == expected, (name, dims)
        expected *= extent
        count *= extent
    if base == 0 or count <= 0:
        return struct.pack("<ii", 0, 0)
    return struct.pack("<ii", extents[0], count) + bytes(inf().read_memory(base, 4 * count))


class Exit(gdb.Breakpoint):
    """Runs `done` when the call returns. A breakpoint that stop() declines is not deleted, so it
    disables itself."""

    def __init__(self, done):
        super().__init__(f"*{int(gdb.parse_and_eval('$x30'))}", internal=True)
        self.done = done

    def stop(self):
        self.done()
        self.enabled = False
        return False


class Entry(gdb.Breakpoint):
    def __init__(self, out, symbol, header, record):
        super().__init__(f"*{sym(symbol)}", internal=True)
        self.out, self.header, self.record = out, header, record
        self.started, self.calls, self.kept = False, 0, 0

    def stop(self):
        try:
            if not self.started:
                self.header(self.out)
                self.started = True
            self.calls += 1
            if (self.calls - 1) % STRIDE == 0:
                self.record(self.out)
                self.kept += 1
            return False
        except Exception:
            import traceback

            open(OUT + ".error", "w").write(traceback.format_exc())
            return True


def run(symbol, header, record):
    gdb.execute("set python print-stack full")
    gdb.execute("set pagination off")
    gdb.execute("set confirm off")
    gdb.execute("starti > /dev/null", to_string=True)
    with open(OUT, "wb") as out:
        entry = Entry(out, symbol, header, record)
        gdb.execute("continue", to_string=True)
        print(f"capture: {entry.calls} {symbol} calls, {entry.kept} kept")
    gdb.execute("quit")
