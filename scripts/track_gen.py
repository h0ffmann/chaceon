#!/usr/bin/env python3
"""Writes the inputs of scripts/track_all.sh under work/track_in/: track_i.ww3 variants as overlays of a regtest's input, and sed
scripts for ww3_shel.nml. ww3_tp2.2 is the only regtest on switch_chaceon_shrd with a track, and it stays on open sea with
five points. These reach land, ice and dry cells, cells the compression narrows, points out of the grid and out of time, and the
binary input. Deterministic: no random numbers."""
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "work/track_in"
ID = "WAVEWATCH III TRACK LOCATIONS DATA"


def stamp(day, h, m=0):
    return f"{day} {h:02d}{m:02d}00"


def text(name, rows, head=ID):
    d = OUT / name
    d.mkdir(parents=True, exist_ok=True)
    (d / "track_i.ww3").write_text(head + " \n" + "".join(" ".join(str(v) for v in r) + "\n" for r in rows))


def binary(name, rows):
    def rec(b):
        n = struct.pack(">i", len(b))
        return n + b + n
    d = OUT / name
    d.mkdir(parents=True, exist_ok=True)
    out = rec(ID.encode())
    for day, hms, x, y, *rest in rows:
        out += rec(struct.pack(">iiff", int(day), int(hms), x, y) + (rest[0] if rest else "").ljust(32).encode())
    (d / "track_i.ww3").write_bytes(out)


def walk(day, hours, x0, y0, dx, dy, step=30, name="W"):
    """A track every `step` minutes, with fractions that land near cell edges for the compression."""
    rows = []
    n = 0
    for t in range(0, hours * 60 + 1, step):
        rows.append((day, f"{t // 60:02d}{t % 60:02d}00", round(x0 + n * dx, 3), round(y0 + n * dy, 3), f"{name}{n}"))
        n += 1
    return rows


def sed(name, lines):
    (OUT / f"{name}.sed").write_text("\n".join(lines) + "\n")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    day = "19680606"
    rows = walk(day, 12, 0.02, 0.97, 1.03, 0.31)
    # Out of the grid, too early, a repeat of a time, no name, a name longer than 32, land beyond the grid's north.
    rows = [("19680605", "120000", 1.5, 1.5, "EARLY")] + rows[:6] + [(day, "030000", 400.0, 0.0, "OFFGRID"), (day, "030000", 3.5, 4.5)] + rows[6:]
    rows += [(day, "060000", 20.25, 3.5, "N" * 40), ("19680607", "000000", 2.5, 2.5, "LATE")]
    text("many", sorted(rows, key=lambda r: r[0] + r[1]))
    binary("bin", [(r[0], r[1], float(r[2]), float(r[3]), *r[4:]) for r in sorted(rows, key=lambda r: r[0] + r[1])])
    text("wrongid", rows, head="WAVEWATCH III TRACK LOCATIONS XXXX")
    (OUT / "empty").mkdir(exist_ok=True)
    (OUT / "empty" / "track_i.ww3").write_text("")
    # Ends before the run does: STOP.
    text("short", walk(day, 3, 0.5, 0.5, 1.0, 0.0))
    # ww3_tp2.3, Cartesian, in metres.
    text("cart", walk(day, 96, -500.0e3, -500.0e3, 33.3e3, 19.7e3, step=360))
    # ww3_tp2.15, curvilinear, over the cells ice and low water take.
    # The rows of the north, where forcing_gen.py's ice grows and low water dries the shallow cells: every third cell, hourly.
    src = ROOT / "WW3/regtests/ww3_tp2.15/input"
    lon, lat = ([float(v) for v in (src / f).read_text().split()] for f in ("lon_ste_adri_15km.dat", "lat_ste_adri_15km.dat"))
    nx = 43
    north = [("20140310", f"{h:02d}0000", round(lon[ix + iy * nx] * 0.5 + lon[ix + 1 + (iy + 1) * nx] * 0.5, 3),
              round(lat[ix + iy * nx] * 0.5 + lat[ix + 1 + (iy + 1) * nx] * 0.5, 3), f"N{iy}_{ix}")
             for h in range(0, 7) for iy in range(30, 41, 2) for ix in range(0, 42, 3)]
    rows = walk("20140310", 6, 12.31, 42.1, 0.17, 0.09, step=15) + walk("20140310", 6, 15.2, 44.9, 0.05, -0.02, step=15, name="Q") + north
    text("curv", sorted(rows, key=lambda r: r[1]))
    track = "/^&OUTPUT_DATE_NML/a \\  DATE%TRACK = '{}' '{}' '{}'"
    sed("t22", ["/DATE%TRACK/d", track.format("19680606 000000", "3600", "19680606 120000")])
    sed("t22b", ["/DATE%TRACK/d", track.format("19680606 000000", "3600", "19680606 120000"), "/^&OUTPUT_TYPE_NML/a \\  TYPE%TRACK%FORMAT = F"])
    sed("t22w", ["/DATE%TRACK/d", track.format("19680606 013000", "5400", "19680606 103000")])
    sed("t23", [track.format("19680606 000000", "21600", "19680610 000000")])
    both = (ROOT / "work/forcing/both.sed").read_text().splitlines() if (ROOT / "work/forcing/both.sed").exists() else []
    sed("t215", both + [track.format("20140310 000000", "1800", "20140310 060000")])


main()
