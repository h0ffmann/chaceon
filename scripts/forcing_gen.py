#!/usr/bin/env python3
"""Writes the forcing variants of scripts/forcing_all.sh under work/forcing/: ww3_prep inputs with fields that change in
space and time on the 43 by 42 grid of ww3_tp2.15, and the sed script for ww3_shel.nml of each variant. No regtest on
switch_chaceon_shrd drives water levels from a file, so these inputs are written here. Deterministic: no random numbers."""
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "work/forcing"
SRC = ROOT / "WW3/regtests/ww3_tp2.15/input"
NX, NY = 43, 42
DAY = "20140310"
HOURS = range(0, 7)
LIST = "HS WND ICE WLV DPT T02 DP DIR FP CHA CFX TAW"


def stamp(h):
    return f"{DAY} {h:02d}0000"


def wind(ix, iy, h):
    """A rotating storm: 8 to 24 m/s, its direction turning with position and time."""
    sp = 16 + 8 * math.sin(2 * math.pi * (ix / NX + h / 6))
    th = 2 * math.pi * (iy / NY + h / 12)
    return sp * math.cos(th), sp * math.sin(th)


def level(ix, iy, h, amp):
    """A surge that falls and comes back, tilted along the basin: amp is the largest fall, in metres."""
    return -amp * math.sin(math.pi * h / 6) * (1 + 0.5 * math.sin(2 * math.pi * iy / NY)) + 1.5 * math.cos(math.pi * h / 3) - 1.5


def ice(ix, iy, h):
    """A band that grows from the north and melts back, full cover in its core, partial at its edge."""
    edge = NY * (1 - 0.1 * (3 - abs(h - 3)))
    d = (iy - edge) / 3
    return round(max(0., min(1., 0.5 + d)), 3)


def field(f):
    # One row per line: INA2R reads each row with its own list-directed READ, which drops the rest of the record.
    return "\n".join(" ".join(f"{f(ix, iy):.3f}" for ix in range(1, NX + 1)) for iy in range(1, NY + 1))


def prep(name, idf, fields, times):
    """Free format, read from this file. Each time has one block per component."""
    # W3PREP takes its comment character from the first line.
    t = [f"$ {idf} input written by forcing_gen.py", f"  '{idf}' 'AI' T T", "  'UNIT' 1 1 '(..T..)' '(..F..)'", "  10 'unused'"]
    for h in times:
        t.append(f"  {stamp(h)}")
        t += [field(lambda ix, iy, g=g: g(ix, iy, h)) for g in fields]
    (OUT / "input" / name).write_text("\n".join(t) + "\n")


def seds():
    base = [f"s/TYPE%FIELD%LIST *=.*/TYPE%FIELD%LIST = '{LIST}'/",
            "s/DATE%POINT *=.*/DATE%POINT = '20140310 000000' '900' '20140310 060000'/"]
    homog = "/^&HOMOG_COUNT_NML/a \\  HOMOG_COUNT%N_LEV = 3\\n  HOMOG_COUNT%N_ICE = 3"
    items = [("LEV", 0, 0.0), ("LEV", 2, -9.0), ("LEV", 4, -2.0), ("ICE", 0, 0.0), ("ICE", 2, 0.9), ("ICE", 4, 0.2)]
    block = "\\n".join(f"  HOMOG_INPUT({i})%NAME = '{n}'\\n  HOMOG_INPUT({i})%DATE = '{stamp(h)}'\\n  HOMOG_INPUT({i})%VALUE1 = {v}"
                       for i, (n, h, v) in enumerate(items, 1))
    on = lambda key, v: f"/^&INPUT_NML/a \\  INPUT%FORCING%{key} = '{v}'"
    return {
        "wind": base,
        "level": base + [on("WATER_LEVELS", "T")],
        "ice": base + [on("ICE_CONC", "T")],
        "both": base + [on("WATER_LEVELS", "T"), on("ICE_CONC", "T")],
        "homog": base + [on("WATER_LEVELS", "H"), on("ICE_CONC", "H"), homog, "/^&HOMOG_INPUT_NML/a \\" + block],
    }


def main():
    (OUT / "input").mkdir(parents=True, exist_ok=True)
    for name, text in seds().items():
        (OUT / f"{name}.sed").write_text("\n".join(text) + "\n")
    wx = lambda ix, iy, h: wind(ix, iy, h)[0]
    wy = lambda ix, iy, h: wind(ix, iy, h)[1]
    prep("ww3_prep_wind.inp", "WND", [wx, wy], HOURS)
    prep("ww3_prep_wns.inp", "WNS", [wx, wy, lambda ix, iy, h: 2 * math.sin(ix / 5 + h)], HOURS)
    # 12 m reaches the grid's shallow cells, which dry and wet again; 4 m stays wet and remaps spectra.
    prep("ww3_prep_lev.inp", "LEV", [lambda ix, iy, h: level(ix, iy, h, 12.)], HOURS)
    prep("ww3_prep_levsmall.inp", "LEV", [lambda ix, iy, h: level(ix, iy, h, 4.)], HOURS)
    prep("ww3_prep_levlate.inp", "LEV", [lambda ix, iy, h: level(ix, iy, h, 12.)], range(1, 7))
    prep("ww3_prep_levshort.inp", "LEV", [lambda ix, iy, h: level(ix, iy, h, 12.)], range(0, 3))
    prep("ww3_prep_ice.inp", "ICE", [ice], HOURS)
    prep("ww3_prep_icelate.inp", "ICE", [ice], range(1, 7))
    nml = (SRC / "namelists_ADRIATIC.nml").read_text()
    # CICE0 < CICEN makes W3UTRN's ice transparency act; LICE > 0 takes its exponential branch.
    for name, extra in (("linear", "CICE0 = 0.25, CICEN = 0.75, "), ("expo", "CICE0 = 0.25, CICEN = 0.75, LICE = 20000., ")):
        d = OUT / f"nml_{name}"
        d.mkdir(exist_ok=True)
        (d / "namelists_ADRIATIC.nml").write_text(re.sub(r"&MISC ", "&MISC " + extra, nml))


main()
