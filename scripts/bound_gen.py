#!/usr/bin/env python3
"""Writes the inputs of scripts/bound_all.sh under work/bound_in/: point spectra in WW3's text format for ww3_bound, and sed
scripts for ww3_grid.nml and ww3_shel.nml. ww3_tr1's three stations are outside its grid, so every boundary point takes one
spectrum with weight 1; the stations here stand on the boundary and vary in time. Deterministic: no random numbers."""
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "work/bound_in"
THETA = [(0.5 * math.pi - (i * 2 * math.pi / 24)) % (2 * math.pi) for i in range(24)]


def spc(name, stations, times, fr1=0.0667, xfr=1.1, nk=13, nth=24, theta0=0.):
    freq = [fr1 * xfr**k for k in range(nk)]
    th = [(t + theta0) % (2 * math.pi) for t in (THETA if nth == 24 else [(0.5 * math.pi - i * 2 * math.pi / nth) % (2 * math.pi) for i in range(nth)])]
    for ip, (lat, lon) in enumerate(stations):
        t = [f"'WAVEWATCH III SPECTRA'{nk:6d}{nth:6d}{1:6d} '                       Unknown'"]
        t.append("".join(f"{f:10.3E}" for f in freq))
        t.append("".join(f"{a:11.3E}" for a in th))
        for it, stamp in enumerate(times):
            t.append(stamp)
            t.append(f"'spec{ip + 1:<6d}'{lat:7.2f}{lon:7.2f}{100.0:10.1f}{0.:7.2f}{0.:6.1f}{0.:7.2f}{0.:6.1f}")
            amp = (1 + ip) * (1 + 0.5 * math.sin(it))
            vals = []
            for j in range(nth):
                for k in range(nk):
                    d = math.cos(th[j] - math.pi / 2 - 0.2 * it) ** 2
                    vals.append(amp * math.exp(-((freq[k] - 0.1 - 0.01 * ip) / 0.03) ** 2) * d * d)
            t += ["".join(f"{v:11.3E}" for v in vals[i:i + 7]) for i in range(0, len(vals), 7)]
        (OUT / name).mkdir(parents=True, exist_ok=True)
        (OUT / name / f"bnd{ip + 1}.spc").write_text("\n".join(t) + "\n")


def sed(name, lines):
    (OUT / f"{name}.sed").write_text("\n".join(lines) + "\n")


def date(start, stop):
    return [f"s/DOMAIN%START *=.*/DOMAIN%START = '{start}'/", f"s/DOMAIN%STOP *=.*/DOMAIN%STOP = '{stop}'/",
            f"s/DATE%FIELD *=.*/DATE%FIELD = '{start}' '3600' '{stop}'/",
            f"s/DATE%POINT *=.*/DATE%POINT = '{start}' '3600' '{stop}'\\n  DATE%RESTART = '{stop}' '3600' '{stop}'/",
            "s/TYPE%POINT%FILE *=.*/TYPE%POINT%FILE = '..\\/input_bndin\\/points.list'/"]


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    day = [f"20010101 {h:02d}0000" for h in range(0, 24, 6)] + ["20010102 000000"]
    edge = [(1.0, -8.0), (4.5, -8.0), (8.0, -8.0)]
    spc("edge", edge, day)
    # A station file's list stays ww3_tr1's, three names.
    spc("two", edge[:2] + [(8.0, 40.0)], day)
    spc("lowf", edge, day, fr1=0.0606, nk=13)        # starts 10% under FREQ1: IFMIN
    spc("highf", edge, day, fr1=0.0807, nk=12)       # starts 21% over FREQ1: IFMIN2
    spc("fewf", edge, day, nk=10)                    # fewer frequencies than the grid
    spc("manyf", edge, day, nk=14)                   # NKI > NK: status 68
    spc("wrongx", edge, day, xfr=1.2)                # status 66
    spc("wrongth", edge, day, nth=12)                # status 67
    spc("mixed", edge, day)
    spc("mixed2", edge, day, nk=10)
    (OUT / "mixed" / "bnd2.spc").write_text((OUT / "mixed2" / "bnd2.spc").read_text())       # a file with another NK: STOP
    spc("rot", edge, day, theta0=0.4)                # TH1 of the nest differs: SPCONV with the same bins
    pt = date("20010101 000000", "20010102 000000")
    sed("full", pt)
    sed("early", date("20001231 120000", "20010102 000000"))     # starts before the first spectra
    sed("late", date("20010101 090000", "20010102 000000"))      # W3IOBC reads past two records at the first step
    sed("eof", date("20010101 000000", "20010102 180000"))       # the file ends mid-run, W3IOBC clears FLBPI
    sed("level", pt + ["s/^&INPUT_NML/\\&INPUT_NML\\n  INPUT%FORCING%WATER_LEVELS = 'H'/",
                       "/^&HOMOG_COUNT_NML/a \\  HOMOG_COUNT%N_LEV = 2",
                       "/^&HOMOG_INPUT_NML/a \\  HOMOG_INPUT(1)%NAME = 'LEV'\\n  HOMOG_INPUT(1)%DATE = '20010101 060000'\\n  HOMOG_INPUT(1)%VALUE1 = -60.\\n"
                       "  HOMOG_INPUT(2)%NAME = 'LEV'\\n  HOMOG_INPUT(2)%DATE = '20010101 120000'\\n  HOMOG_INPUT(2)%VALUE1 = -80."])
    sed("parent", pt + ["/DATE%RESTART/a \\  DATE%BOUNDARY = '20010101 000000' '3600' '20010102 000000'"])
    sed("gparent", ["/^&MASK_NML/i \\&OUTBND_COUNT_NML\\n  OUTBND_COUNT%N_LINE = 1\\n/\\n\\n&OUTBND_LINE_NML\\n  OUTBND_LINE(1) = -5. 1.5 0. 1. 8\\n/\\n"])
    sed("gchild", ["s/RECT%X0 *=.*/RECT%X0 = -5.0/", "s/RECT%Y0 *=.*/RECT%Y0 = 0.5/"])
    sed("gspec", ["s/SPECTRUM%NK *=.*/SPECTRUM%NK = 16/", "s/SPECTRUM%NTH *=.*/SPECTRUM%NTH = 36/", "s/SPECTRUM%XFR *=.*/SPECTRUM%XFR = 1.07/",
                  "s/SPECTRUM%FREQ1 *=.*/SPECTRUM%FREQ1 = 0.06/"])
    sed("gcoarse", ["s/SPECTRUM%NK *=.*/SPECTRUM%NK = 10/", "s/SPECTRUM%NTH *=.*/SPECTRUM%NTH = 12/", "s/SPECTRUM%XFR *=.*/SPECTRUM%XFR = 1.15/"])
    sed("gsmall", ["s/TIMESTEPS%DTXY *=.*/TIMESTEPS%DTXY = 600/", "s/TIMESTEPS%DTKTH *=.*/TIMESTEPS%DTKTH = 600/"])
    sed("gnear", ["s/RECT%X0 *=.*/RECT%X0 = -7.95/"])        # the points lie 0.05 of a cell outside the grid: W3GFPT's DCIN
    sed("gfar", ["s/RECT%X0 *=.*/RECT%X0 = 20.0/"])          # no boundary point lies in the grid: status 12
    sed("ginner", ["s/RECT%X0 *=.*/RECT%X0 = -9.0/"])        # they lie on sea points that are not boundary points: status 20
    for tag, extra in (("nml_interp1", "  BOUND%INTERP = 1\n"), ("nml_quiet", "  BOUND%VERBOSE = 0\n"), ("nml_read", "  BOUND%MODE = 'READ'\n"),
                       ("nml_nofile", "  BOUND%FILE = '../input_bndin/absent.list'\n"), ("nml_noname", "  BOUND%FILE = 'spec.bad'\n")):
        (OUT / tag).mkdir(exist_ok=True)
        (OUT / tag / "ww3_bound.nml").write_text(f"&BOUND_NML\n  BOUND%FILE = '../input_bndin/spec.list'\n{extra}/\n")
    (OUT / "nml_noname" / "spec.bad").write_text("")


main()
