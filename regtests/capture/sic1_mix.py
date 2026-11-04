# Joins the calls of several W3SIC1 captures of one grid into one file, so that a batch replay puts
# points with different CG and A in one buffer: ww3_ts1 gives every call of a case the same CG, and a
# wrong CG offset hides there. Usage: sic1_mix.py <out> <capture>...  (plain Python, not gdb)
import struct
import sys

out, ins = sys.argv[1], sys.argv[2:]
first = open(ins[0], "rb").read()
nk, nth, nspec, _ = struct.unpack_from("<4i", first, 4)
# Header: b"SIC1", 4 ints, then MAPWN as (extent, count, count ints).
count = struct.unpack_from("<2i", first, 20)[1]
head = 28 + 4 * count
data = first[:head]
for p in ins:
    b = open(p, "rb").read()
    assert b[:head] == first[:head], p
    data += b[head:]
open(out, "wb").write(data)
