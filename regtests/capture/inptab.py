# SPDX-License-Identifier: MPL-2.0
# Run by gdb inside the reference built with ST2: stops at INPTAB's return and records the module
# variables it set, DSIGA, DDRAG and BETATB(-400:401,21), then stops the run. Nothing is an input:
# W3BETA reads only constants, so the table is the same in every run.
# Format, little-endian: b"INPT", DSIGA DDRAG (f32), BETATB as 802*21 f32. Reader:
# rust/crates/ww3-host/src/capture.rs.
import os
import sys

import gdb

sys.path.insert(0, os.path.dirname(__file__))
from common import Exit, inf, run, sym  # noqa: E402


class Stop(Exit):
    """Records on return, then stops the run, so that gdb's `continue` comes back here."""

    def stop(self):
        self.done()
        return True


def header(f):
    f.write(b"INPT")


def record(f):
    def done():
        for name in ("dsiga", "ddrag"):
            f.write(bytes(inf().read_memory(sym(f"__w3src2md_MOD_{name}"), 4)))
        f.write(bytes(inf().read_memory(sym("__w3src2md_MOD_betatb"), 4 * 802 * 21)))

    Stop(done)


run("__w3src2md_MOD_inptab", header, record)
