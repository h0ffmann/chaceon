# Run by gdb inside the reference: records INSIN3's inputs when it is called, and the tables it has set
# when it returns, then stops the run. Format: rust/crates/ww3-host/src/capture.rs.
import os
import struct
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, array, inf, run, scalar, sym  # noqa: E402

G = "w3gdatmd::"
MODULE = [f"__w3src3md_MOD_{n}" for n in ("deltauw", "delu", "delust", "delalp")]


class Stop(Exit):
    """Records on return, then stops the run, so that gdb's `continue` comes back here."""

    def stop(self):
        self.done()
        return True


def header(f):
    f.write(b"IN3 ")


def record(f):
    nk = int(scalar(G + "nk"))
    f.write(struct.pack("<i", nk))
    for name in ("zzwnd", "aalpha", "bbeta", "zzalp", "zz0max"):
        f.write(struct.pack("<f", float(scalar(G + name))))
    sig = array("w3gdatmd", "sig")  # SIG(0:NK+1): first extent, count, data
    extent, count = struct.unpack_from("<ii", sig, 0)
    assert count == nk + 2, (count, nk)
    f.write(sig[8 + 4 * nk : 8 + 4 * nk + 4])

    def done():
        for s in MODULE:
            f.write(bytes(inf().read_memory(sym(s), 4)))
        f.write(bytes(inf().read_memory(sym("__w3src3md_MOD_taut"), 4 * 201 * 201)))
        f.write(bytes(inf().read_memory(sym("__w3src3md_MOD_tauhft"), 4 * 101 * 201)))

    Stop(done)


run("__w3src3md_MOD_insin3", header, record)
