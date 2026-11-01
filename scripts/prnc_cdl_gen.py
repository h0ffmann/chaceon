#!/usr/bin/env python3
"""Writes the cases of regtests/prnc_inputs/: for each, a CDL file that ncgen turns into the NetCDF input, the
ww3_prnc.nml or ww3_prnc.inp, and case.conf (the regtest whose grid the case runs on).

The NetCDF inputs of WW3's own regtests (wind.nc, current.nc, ice.nc, ...) are not in the WW3 copy of this
repository: the regtests fetch them from outside. These cases rebuild their *shapes* (the variable and dimension
names, the layouts and the namelists of WW3/regtests/*/input/ww3_prnc*) with synthetic values; none of them is a
regtest's own data. The values are deterministic: re-running this script rewrites the same bytes.

Usage: python3 scripts/prnc_cdl_gen.py   (from the repository root)
"""
import os
import sys

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "regtests", "prnc_inputs")

# The grids: regtest, the ww3_grid namelist of its input directory.
IROISE = ("ww3_tp2.8", "ww3_grid.nml")  # RECT, spherical, 103 x 119, x -6.119 .. -4.181, y 47.6375 .. 49.1125
GLO5 = ("ww3_ufs1.1", "ww3_grid_glo_5deg.nml")  # RECT, spherical, closure SMPL, 72 x 35, x 0 .. 355, y -85 .. 85
ADRI = ("ww3_tp2.15", "ww3_grid.nml")  # CURV, spherical, 43 x 42
CART = ("ww3_tic1.4", "ww3_grid_1000m.nml")  # RECT, Cartesian, 153 x 3, x -1000 m .. 151000 m


def num(v, kind):
    s = f"{v:.3f}".rstrip("0").rstrip(".")
    if "." not in s:
        s += ".0" if kind in ("float", "double") else ""
    return s + {"float": "f", "double": "", "short": "s", "int": ""}[kind]


def val(t, j, i, seed):
    """A deterministic field, in 0.25 steps from -4 to about 6."""
    return ((i * 3 + j * 5 + t * 7 + seed) % 41) * 0.25 - 4.0


def cdl(name, *, nlon, nlat, lon0, dlon, lat0, dlat, times, tunits, vars, lonname="longitude", latname="latitude",
        dimlon="longitude", dimlat="latitude", tname="time", tdim="time", cal="standard", coord2d=False, coordtype="float",
        ttype="double", levels=0, dimlev="level", holes=(), nan=(), negzero=(), rot=0.0, lonlat_dims=None, extra_dims=""):
    """vars: dicts name, type ('float', 'double', 'short'), fill ('_FillValue' | 'missing_value' | None),
    fillvalue, scale, offset, seed. Dimension order (time, [level,] lat, lon), as the regtests' files have."""
    nt = len(times)
    L = [f"netcdf {name} {{", "dimensions:", f"\t{dimlon} = {nlon} ;", f"\t{dimlat} = {nlat} ;", f"\t{tdim} = UNLIMITED ; // ({nt} currently)"]
    if levels:
        L.append(f"\t{dimlev} = {levels} ;")
    L.append("variables:")
    L.append(f"\t{ttype} {tname}({tdim}) ;")
    if tunits is not None:
        L.append(f"\t\t{tname}:units = \"{tunits}\" ;")
    if cal is not None:
        L.append(f"\t\t{tname}:calendar = \"{cal}\" ;")
    if coord2d:
        L.append(f"\t{coordtype} {lonname}({dimlat}, {dimlon}) ;")
        L.append(f"\t{coordtype} {latname}({dimlat}, {dimlon}) ;")
    else:
        L.append(f"\t{coordtype} {lonname}({dimlon}) ;")
        L.append(f"\t{coordtype} {latname}({dimlat}) ;")
    shape = f"{tdim}" + (f", {dimlev}" if levels else "") + f", {dimlat}, {dimlon}"
    for v in vars:
        L.append(f"\t{v['type']} {v['name']}({shape}) ;")
        if v.get("fill"):
            L.append(f"\t\t{v['name']}:{v['fill']} = {num(v['fillvalue'], v['type'])} ;")
        if v.get("scale") is not None:
            L.append(f"\t\t{v['name']}:scale_factor = {v['scale']} ;")
            L.append(f"\t\t{v['name']}:add_offset = {v['offset']} ;")
    L.append("data:")
    L.append(" " + tname + " = " + ", ".join(num(t, "double" if ttype == "double" else "int") for t in times) + " ;")
    if coord2d:
        lo, la = [], []
        for j in range(nlat):
            for i in range(nlon):
                lo.append(num(lon0 + i * dlon + rot * j, coordtype))
                la.append(num(lat0 + j * dlat + rot * i, coordtype))
        L.append(" " + lonname + " = " + ", ".join(lo) + " ;")
        L.append(" " + latname + " = " + ", ".join(la) + " ;")
    else:
        L.append(" " + lonname + " = " + ", ".join(num(lon0 + i * dlon, coordtype) for i in range(nlon)) + " ;")
        L.append(" " + latname + " = " + ", ".join(num(lat0 + j * dlat, coordtype) for j in range(nlat)) + " ;")
    for v in vars:
        vals = []
        for t in range(nt):
            for lv in range(max(levels, 1)):
                for j in range(nlat):
                    for i in range(nlon):
                        x = val(t, j + lv, i, v.get("seed", 0))
                        fv = v.get("fillvalue")
                        if (t, j, i) in holes and v.get("fill"):
                            vals.append(num(fv, v["type"]))
                        elif v["type"] == "short":
                            # packed: x = stored * scale + offset
                            vals.append(num(round((x - v["offset"]) / v["scale"]), "short"))
                        elif (t, j, i) in nan:
                            vals.append("NaNf" if v["type"] == "float" else "NaN")
                        elif (t, j, i) in negzero:
                            vals.append("-0.0f")
                        else:
                            vals.append(num(x, v["type"]))
        L.append(" " + v["name"] + " = " + ", ".join(vals) + " ;")
    L.append("}")
    return "\n".join(L) + "\n"


def fv(name, seed=0, **kw):
    d = dict(name=name, type="float", fill="_FillValue", fillvalue=-999.0, seed=seed)
    d.update(kw)
    return d


NML_FIELD = dict(
    WINDS="FORCING%FIELD%WINDS", WINDS_AST="FORCING%FIELD%WINDS_AST", CURRENTS="FORCING%FIELD%CURRENTS",
    WATER_LEVELS="FORCING%FIELD%WATER_LEVELS", ICE_CONC="FORCING%FIELD%ICE_CONC", ICE_BERG="FORCING%FIELD%ICE_BERG",
    ICE_PARAM1="FORCING%FIELD%ICE_PARAM1", ICE_PARAM2="FORCING%FIELD%ICE_PARAM2", ICE_PARAM3="FORCING%FIELD%ICE_PARAM3",
    ICE_PARAM4="FORCING%FIELD%ICE_PARAM4", ICE_PARAM5="FORCING%FIELD%ICE_PARAM5", MUD_DENSITY="FORCING%FIELD%MUD_DENSITY",
    MUD_THICKNESS="FORCING%FIELD%MUD_THICKNESS", MUD_VISCOSITY="FORCING%FIELD%MUD_VISCOSITY",
    ATM_MOMENTUM="FORCING%FIELD%ATM_MOMENTUM", AIR_DENSITY="FORCING%FIELD%AIR_DENSITY", DATA_ASSIM="FORCING%FIELD%DATA_ASSIM",
)


def nml(field, grid, filename, lon, lat, vars, *, tstart=None, tstop=None, shift=None, tidal=None, field2=None, raw_extra=""):
    f = ["&FORCING_NML"]
    if tstart:
        f.append(f"  FORCING%TIMESTART            = '{tstart}'")
    if tstop:
        f.append(f"  FORCING%TIMESTOP             = '{tstop}'")
    f.append(f"  {NML_FIELD[field]} = T")
    if field2:
        f.append(f"  {NML_FIELD[field2]} = T")
    if grid:
        f.append(f"  FORCING%GRID%{grid} = T")
    if tidal:
        f.append(f"  FORCING%TIDAL = '{tidal}'")
    f.append("/")
    f.append("&FILE_NML")
    f.append(f"  FILE%FILENAME      = '{filename}'")
    f.append(f"  FILE%LONGITUDE     = '{lon}'")
    f.append(f"  FILE%LATITUDE      = '{lat}'")
    for k, v in enumerate(vars):
        f.append(f"  FILE%VAR({k + 1})        = '{v}'")
    if shift:
        f.append(f"  FILE%TIMESHIFT     = '{shift}'")
    if raw_extra:
        f.append(raw_extra)
    f.append("/")
    return "\n".join(f) + "\n"


def inp(idfld, idtype, fltime, flhdr, dims, vars, filename, *, shift=None, tail=""):
    L = ["$ ww3_prnc.inp written by scripts/prnc_cdl_gen.py", f"  '{idfld}' '{idtype}' {'T' if fltime else 'F'} {'T' if flhdr else 'F'}",
         "$", f"  {dims}", "$", f"  {vars}"]
    if shift:
        L += ["$", f"  {shift}"]
    L += ["$", f"  '{filename}'"]
    if tail:
        L.append(tail)
    return "\n".join(L) + "\n"


CASES = {}


# The cases whose namelist is the shape of one in WW3/regtests/*/input/ww3_prnc*: prnc_all.sh runs these, prnc_variants.sh the rest.
REGTEST_SHAPED = {
    "cart_ice_ll": "ww3_tic1.4 ww3_prnc_ice.nml", "cart_ice_param1": "ww3_tic1.4 ww3_prnc_ice1.nml", "cart_ice_param5": "ww3_tic1.4 ww3_prnc_ice5.nml",
    "adri_wind_ll": "ww3_tp2.15 ww3_prnc.nml", "ll_currents": "ww3_tp2.8 ww3_prnc.nml",
    "ai_one_dim_nodes": "ww3_tp2.17 ww3_prnc_current.nml", "ai_one_dim_level": "ww3_tp2.17 ww3_prnc_level.nml", "ai_one_dim_wind": "ww3_tp2.17 ww3_prnc_wind.nml",
    "glo_wind_0_360": "ww3_ufs1.1 ww3_prnc_glo_5deg_wind.nml", "glo_current_0_360": "ww3_ufs1.1 ww3_prnc_glo_5deg_current.nml",
    "glo_ice_conc": "ww3_ufs1.1 ww3_prnc_glo_5deg_ice.nml", "refuse_tidal_at": "ww3_tp2.18 ww3_prnc.nml",
}


# The NetCDF format ncgen writes, when it is not the classic one.
NCGEN_KIND = {"ll_wind_nc4": "nc4", "ll_wind_64bit_offset": "64-bit-offset"}


def case(name, grid, files):
    """files: name -> text."""
    CASES[name] = (grid, files)


def wind(name, **kw):
    return cdl(name, vars=[fv("U", 0), fv("V", 5)], **kw)


# ---- the geometry of the inputs ------------------------------------------------------------------------------------------
IR = dict(nlon=24, nlat=18, lon0=-6.5, dlon=0.1, lat0=47.5, dlat=0.1)  # covers x -6.5 .. -4.2, y 47.5 .. 49.2 (the whole model grid)
T3 = dict(times=[0, 3, 6], tunits="hours since 1968-06-06 00:00:00")

# ---- LL, regular model grid (ww3_tp2.8) ------------------------------------------------------------------------------------
case("ll_wind_1d", IROISE, {"in.cdl": wind("in", **IR, **T3, holes={(1, 4, 5)}), "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
case("ll_cur_2d_packed", IROISE, {
    "in.cdl": cdl("in", nlon=22, nlat=17, lon0=-6.45, dlon=0.105, lat0=47.55, dlat=0.098, rot=0.004, coord2d=True, coordtype="double",
                  lonname="lon", latname="lat", dimlon="x", dimlat="y", times=[0, 21600, 43200], tunits="seconds since 1970-01-01 00:00:00",
                  vars=[fv("U", 1, type="short", fillvalue=-32768, scale=0.01, offset=0.5), fv("V", 2, type="short", fillvalue=-32768, scale=0.01, offset=-0.25)],
                  holes={(0, 3, 3), (2, 9, 9)}),
    "ww3_prnc.nml": nml("CURRENTS", "LATLON", "../input/in.nc", "x", "y", ["U", "V"])})
case("ll_wns_shift", IROISE, {
    "in.cdl": cdl("in", **IR, **T3, vars=[fv("uwnd", 0), fv("vwnd", 5), fv("dT", 9)]),
    "ww3_prnc.nml": nml("WINDS_AST", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd", "dT"], tstart="19680606 010000", tstop="19680606 050000", shift="19680607 000000")})
case("ll_wns_shift_all", IROISE, {
    # scale_factor and add_offset on the third variable: the Fortran never reads them for it
    "in.cdl": cdl("in", **IR, **T3, vars=[fv("uwnd", 0), fv("vwnd", 5), fv("dT", 9, scale=0.5, offset=1.0)]),
    "ww3_prnc.nml": nml("WINDS_AST", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd", "dT"], shift="19680606 013000")})
for fld, vs in [("WATER_LEVELS", ["wlv"]), ("ICE_CONC", ["ice"]), ("ICE_BERG", ["ice", "berg"]), ("ICE_PARAM1", ["h"]), ("ICE_PARAM2", ["h"]), ("ICE_PARAM3", ["h"]),
                ("ICE_PARAM4", ["h"]), ("ICE_PARAM5", ["h"]), ("MUD_DENSITY", ["h"]), ("MUD_THICKNESS", ["h"]), ("MUD_VISCOSITY", ["h"]),
                ("ATM_MOMENTUM", ["tx", "ty"]), ("AIR_DENSITY", ["rho"])]:
    case("ll_" + fld.lower(), IROISE, {
        "in.cdl": cdl("in", **IR, **T3, vars=[fv(v, k) for k, v in enumerate(vs)]),
        "ww3_prnc.nml": nml(fld, "LATLON", "../input/in.nc", "longitude", "latitude", vs)})
case("ll_4d_missing_value", IROISE, {
    "in.cdl": cdl("in", **IR, **T3, levels=2, dimlev="depth", vars=[fv("U", 0, fill="missing_value", fillvalue=-9999.0), fv("V", 4, fill="missing_value", fillvalue=-9999.0)],
                  holes={(0, 2, 2), (1, 7, 8), (2, 0, 0)}, nan={(2, 5, 5)}),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
case("ll_days_units_lon_names", IROISE, {
    "in.cdl": cdl("in", **IR, times=[0, 0.125, 0.25], tunits="days since 1968-06-06 00:00:00", lonname="lon", latname="lat", dimlon="lon", dimlat="lat", vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "lon", "lat", ["U", "V"])})
case("ll_minutes_short_units_MT", IROISE, {
    "in.cdl": cdl("in", **IR, times=[0, 90, 180], tunits="minutes since 1968-6-6 0:0:0", tname="MT", tdim="MT", lonname="Longitude", latname="Latitude",
                  dimlon="x", dimlat="y", cal=None, vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "x", "y", ["U", "V"])})
case("ll_hours_short_hms_calendar", IROISE, {
    "in.cdl": cdl("in", **IR, times=[0, 3, 6], tunits="hours since 1968-6-6 03:00:00", cal="noleap", lonname="X", latname="Y", dimlon="x", dimlat="y", vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "x", "y", ["U", "V"])})
case("ll_int_time", IROISE, {
    "in.cdl": cdl("in", **IR, times=[0, 6, 12], tunits="hours since 1968-06-06 00:00:00", ttype="int", vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"], tstart="19680606 030000", tstop="19680606 090000")})
case("ll_partial_cover", IROISE, {
    "in.cdl": cdl("in", nlon=10, nlat=9, lon0=-6.0, dlon=0.1, lat0=48.0, dlat=0.1, **T3, vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
# the old input, ww3_prnc.inp, for the same forms
case("inp_ll_cur", IROISE, {
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.inp": inp("CUR", "LL", True, True, "longitude latitude", "U V", "../input/in.nc")})
case("inp_ll_wns_notime_nohdr", IROISE, {
    "in.cdl": cdl("in", **IR, **T3, vars=[fv("uwnd", 0), fv("vwnd", 5), fv("dT", 9)]),
    "ww3_prnc.inp": inp("WNS", "LL", False, False, "longitude latitude", "uwnd vwnd dT", "../input/in.nc", shift="19680606 053000")})
case("inp_ll_lev", IROISE, {
    "in.cdl": cdl("in", **IR, **T3, vars=[fv("wlv")]),
    "ww3_prnc.inp": inp("LEV", "LL", True, True, "longitude latitude", "wlv", "../input/in.nc")})

# ---- AI, the field already on the model grid (ww3_tp2.8 grid 103 x 119) ------------------------------------------------
AIG = dict(nlon=103, nlat=119, lon0=0.0, dlon=1.0, lat0=0.0, dlat=1.0, **dict(times=[0, 3], tunits="hours since 1968-06-06 00:00:00"))
case("ai_xy_wind", IROISE, {
    "in.cdl": cdl("in", **AIG, dimlon="x", dimlat="y", lonname="x", latname="y", vars=[fv("uwnd", 0), fv("vwnd", 5)], holes={(0, 3, 3), (1, 50, 60)}, nan={(1, 7, 7)}),
    "ww3_prnc.nml": nml("WINDS", "ASIS", "../input/in.nc", "x", "y", ["uwnd", "vwnd"])})
case("ai_xy_larger_file", IROISE, {
    "in.cdl": cdl("in", nlon=110, nlat=125, lon0=0.0, dlon=1.0, lat0=0.0, dlat=1.0, times=[0, 3], tunits="hours since 1968-06-06 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y",
                  vars=[fv("wlv", 3, scale=0.5, offset=2.0)]),
    "ww3_prnc.nml": nml("WATER_LEVELS", "ASIS", "../input/in.nc", "x", "y", ["wlv"])})
case("ai_one_dim_nodes", IROISE, {
    # LONGITUDE = 'time', LATITUDE = '': the form of ww3_tp2.17 (unstructured grids). On a structured grid the Fortran
    # reads one row of NX values, and the other rows of XC stay at zero.
    "in.cdl": "\n".join([
        "netcdf in {", "dimensions:", "\tnode = 103 ;", "\ttime = UNLIMITED ; // (2 currently)", "variables:",
        "\tdouble time(time) ;", "\t\ttime:units = \"days since 1968-06-06 00:00:00\" ;", "\t\ttime:calendar = \"standard\" ;",
        "\tfloat ucur(time, node) ;", "\t\tucur:_FillValue = -999.f ;", "\tfloat vcur(time, node) ;", "\t\tvcur:_FillValue = -999.f ;", "data:",
        " time = 0, 0.25 ;",
        " ucur = " + ", ".join(num(val(t, 0, i, 0), "float") for t in range(2) for i in range(103)) + " ;",
        " vcur = " + ", ".join(num(val(t, 0, i, 7), "float") if i != 5 else "-999.f" for t in range(2) for i in range(103)) + " ;", "}"]) + "\n",
    "ww3_prnc.nml": nml("CURRENTS", "ASIS", "../input/in.nc", "time", "", ["ucur", "vcur"])})
case("inp_ai_one_dim", IROISE, {
    "in.cdl": "\n".join([
        "netcdf in {", "dimensions:", "\tnode = 103 ;", "\ttime = UNLIMITED ; // (2 currently)", "variables:",
        "\tdouble time(time) ;", "\t\ttime:units = \"days since 1968-06-06 00:00:00\" ;",
        "\tfloat ucur(time, node) ;", "\t\tucur:_FillValue = -999.f ;", "\tfloat vcur(time, node) ;", "\t\tvcur:_FillValue = -999.f ;", "data:",
        " time = 0, 0.25 ;",
        " ucur = " + ", ".join(num(val(t, 0, i, 0), "float") for t in range(2) for i in range(103)) + " ;",
        " vcur = " + ", ".join(num(val(t, 0, i, 7), "float") for t in range(2) for i in range(103)) + " ;", "}"]) + "\n",
    "ww3_prnc.inp": inp("CUR", "AI", True, True, "time", "ucur vcur", "../input/in.nc")})
case("ai_xy_ice_berg_4d", IROISE, {
    "in.cdl": cdl("in", **AIG, levels=1, dimlev="z", dimlon="x", dimlat="y", lonname="x", latname="y", vars=[fv("ice", 0), fv("berg", 5)]),
    "ww3_prnc.nml": nml("ICE_BERG", "ASIS", "../input/in.nc", "x", "y", ["ice", "berg"])})
case("ai_xy_rho", IROISE, {
    "in.cdl": cdl("in", **AIG, dimlon="x", dimlat="y", lonname="x", latname="y", vars=[fv("rho", 0)]),
    "ww3_prnc.nml": nml("AIR_DENSITY", "ASIS", "../input/in.nc", "x", "y", ["rho"])})

# ---- global grid, closure SMPL (ww3_ufs1.1 glo_5deg 72 x 35) -----------------------------------------------------------
GT = dict(times=[0, 3], tunits="seconds since 1970-01-01 00:00:00")
GT = dict(times=[0, 10800], tunits="seconds since 1970-01-01 00:00:00")
case("glo_wind_0_360", GLO5, {
    "in.cdl": cdl("in", nlon=180, nlat=91, lon0=0.0, dlon=2.0, lat0=-90.0, dlat=2.0, **GT, vars=[fv("uwnd", 0), fv("vwnd", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"])})
case("glo_wind_neg180", GLO5, {
    "in.cdl": cdl("in", nlon=180, nlat=91, lon0=-180.0, dlon=2.0, lat0=-90.0, dlat=2.0, **GT, vars=[fv("uwnd", 0), fv("vwnd", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"])})
case("glo_current_10deg_noclose", GLO5, {
    "in.cdl": cdl("in", nlon=19, nlat=19, lon0=0.0, dlon=10.0, lat0=-90.0, dlat=10.0, **GT, vars=[fv("ucur", 3), fv("vcur", 8)], holes={(0, 4, 4)}),
    "ww3_prnc.nml": nml("CURRENTS", "LATLON", "../input/in.nc", "longitude", "latitude", ["ucur", "vcur"])})
case("glo_ice_conc", GLO5, {
    "in.cdl": cdl("in", nlon=72, nlat=36, lon0=0.0, dlon=5.0, lat0=-87.5, dlat=5.0, **GT, vars=[fv("ice", 2)]),
    "ww3_prnc.nml": nml("ICE_CONC", "LATLON", "../input/in.nc", "longitude", "latitude", ["ice"])})

# Model nodes that coincide with input nodes (5 degrees, from 0 and from -90), 35 % of the input nodes missing: the weight of a
# missing corner is then the whole weight, and INTERP falls back on the other corners, one by one, or on zero.
_holes = {(0, j, i) for j in range(37) for i in range(72) if (i * 7 + j * 13) % 20 < 7}
case("glo_nodes_holes", GLO5, {
    "in.cdl": cdl("in", nlon=72, nlat=37, lon0=0.0, dlon=5.0, lat0=-90.0, dlat=5.0, times=[0], tunits="seconds since 1970-01-01 00:00:00", vars=[fv("uwnd", 0), fv("vwnd", 5)], holes=_holes),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"])})
case("glo_nodes_holes_ice", GLO5, {
    "in.cdl": cdl("in", nlon=72, nlat=37, lon0=0.0, dlon=5.0, lat0=-90.0, dlat=5.0, times=[0], tunits="seconds since 1970-01-01 00:00:00", vars=[fv("ice", 4)], holes=_holes),
    "ww3_prnc.nml": nml("ICE_CONC", "LATLON", "../input/in.nc", "longitude", "latitude", ["ice"])})

# The same nodes with 60 % of the input missing, chosen by a fixed-seed generator, and in the first column (the model nodes at
# longitude 0, which lie in the first cell of their search bucket as its corner 3) three holes before a valid corner: INTERP then
# reaches its third and fourth fallback and its all-missing exit.
import random

_rng = random.Random(20261101)
_dense = {(0, j, i) for j in range(37) for i in range(72) if _rng.random() < 0.6}
for _j in range(4, 36, 4):
    _dense |= {(0, _j - 1, 0), (0, _j - 1, 1), (0, _j, 0)}
    _dense.discard((0, _j, 1))
case("glo_nodes_holes_dense", GLO5, {
    "in.cdl": cdl("in", nlon=72, nlat=37, lon0=0.0, dlon=5.0, lat0=-90.0, dlat=5.0, times=[0], tunits="seconds since 1970-01-01 00:00:00", vars=[fv("uwnd", 0), fv("vwnd", 5)], holes=_dense),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"])})

# 36 input longitudes 10 degrees apart, 0 to 350: NXI * SXI is 360, so the input is closed in longitude, and the model nodes at
# 355 lie in the cell between the last column and the first.
case("glo_wrap_10deg", GLO5, {
    "in.cdl": cdl("in", nlon=36, nlat=19, lon0=0.0, dlon=10.0, lat0=-90.0, dlat=10.0, times=[0], tunits="seconds since 1970-01-01 00:00:00", vars=[fv("uwnd", 0), fv("vwnd", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"])})
case("glo_wrap_10deg_neg", GLO5, {
    "in.cdl": cdl("in", nlon=36, nlat=19, lon0=-180.0, dlon=10.0, lat0=-90.0, dlat=10.0, times=[0], tunits="seconds since 1970-01-01 00:00:00", vars=[fv("uwnd", 0), fv("vwnd", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"])})

# ---- curvilinear model grid (ww3_tp2.15, the Adriatic 15 km grid, 43 x 42) ------------------------------------------------
case("adri_wind_ll", ADRI, {
    "in.cdl": cdl("in", nlon=61, nlat=49, lon0=8.0, dlon=0.25, lat0=37.0, dlat=0.25, times=[0, 3], tunits="hours since 2010-01-01 00:00:00", vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
case("adri_wind_ll_2d_curved", ADRI, {
    "in.cdl": cdl("in", nlon=40, nlat=34, lon0=9.0, dlon=0.35, lat0=38.0, dlat=0.3, rot=0.01, coord2d=True, times=[0, 3], tunits="hours since 2010-01-01 00:00:00",
                  vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
case("adri_ai", ADRI, {
    "in.cdl": cdl("in", nlon=43, nlat=42, lon0=0, dlon=1, lat0=0, dlat=1, times=[0, 3], tunits="hours since 2010-01-01 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y",
                  vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "ASIS", "../input/in.nc", "x", "y", ["U", "V"])})

# ---- Cartesian model grid (ww3_tic1.4, 153 x 3, metres) ---------------------------------------------------------------------
case("cart_ice_ll", CART, {
    "in.cdl": cdl("in", nlon=40, nlat=5, lon0=-2000.0, dlon=4000.0, lat0=-2000.0, dlat=1000.0, times=[0, 6], tunits="hours since 1968-06-06 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y",
                  vars=[fv("ice", 1)]),
    "ww3_prnc.nml": nml("ICE_CONC", "LATLON", "../input/in.nc", "x", "y", ["ice"])})
case("cart_ice_ll_inp", CART, {
    "in.cdl": cdl("in", nlon=40, nlat=5, lon0=-2000.0, dlon=4000.0, lat0=-2000.0, dlat=1000.0, times=[0, 6], tunits="hours since 1968-06-06 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y",
                  vars=[fv("ice", 1)]),
    "ww3_prnc.inp": inp("IC1", "LL", True, True, "x y", "ice", "../input/in.nc")})

# ---- more forms of the namelist and of the time ---------------------------------------------------------------------------------
case("ll_nml_forms", IROISE, {
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.nml": "! a comment line\n&forcing_nml forcing%field%winds=.true., forcing%grid%latlon = t, ! trailing comment\n  forcing%timestart = '19680606 000000' /\n"
                    "&file_nml\n  file%filename = \"../input/in.nc\", file%longitude = 'longitude', file%latitude = 'latitude'\n  file%var = 'U', 'V'\n/\n"})
case("ll_shift_back", IROISE, {
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"], shift="19680605 120000")})
case("adri_wind_ll_lat_desc", ADRI, {
    "in.cdl": cdl("in", nlon=61, nlat=49, lon0=8.0, dlon=0.25, lat0=49.0, dlat=-0.25, times=[0, 3], tunits="hours since 2010-01-01 00:00:00", vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
case("inp_comment_char", IROISE, {
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.inp": inp("WND", "LL", True, True, "longitude latitude", "U V", "../input/in.nc").replace("$", "#")})
case("inp_ai_xy", IROISE, {
    "in.cdl": cdl("in", **AIG, dimlon="x", dimlat="y", lonname="x", latname="y", vars=[fv("uwnd", 0), fv("vwnd", 5)]),
    "ww3_prnc.inp": inp("WND", "AI", False, True, "x y", "uwnd vwnd", "../input/in.nc", shift="19680606 120000")})

case("ll_currents", IROISE, {
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.nml": nml("CURRENTS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
for k, par in [("param1", "ICE_PARAM1"), ("param5", "ICE_PARAM5")]:
    case("cart_ice_" + k, CART, {
        "in.cdl": cdl("in", nlon=40, nlat=5, lon0=-2000.0, dlon=4000.0, lat0=-2000.0, dlat=1000.0, times=[0, 6], tunits="hours since 1968-06-06 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y",
                      vars=[fv("ice", 1)]),
        "ww3_prnc.nml": nml(par, "LATLON", "../input/in.nc", "x", "y", ["ice"])})
for nm, fld, vs in [("level", "WATER_LEVELS", ["wlv"]), ("wind", "WINDS", ["uwnd", "vwnd"])]:
    case("ai_one_dim_" + nm, IROISE, {
        "in.cdl": "\n".join([
            "netcdf in {", "dimensions:", "\tnode = 103 ;", "\ttime = UNLIMITED ; // (2 currently)", "variables:",
            "\tdouble time(time) ;", "\t\ttime:units = \"days since 1968-06-06 00:00:00\" ;", "\t\ttime:calendar = \"standard\" ;"]
            + [x for v in vs for x in (f"\tfloat {v}(time, node) ;", f"\t\t{v}:_FillValue = -999.f ;") + ((f"\t\t{v}:scale_factor = 0.5f ;", f"\t\t{v}:add_offset = 2.0f ;") if nm == "level" else ())] + ["data:", " time = 0, 0.25 ;"]
            + [f" {v} = " + ", ".join(num(val(t, 0, i, 3 * k), "float") for t in range(2) for i in range(103)) + " ;" for k, v in enumerate(vs)] + ["}"]) + "\n",
        "ww3_prnc.nml": nml(fld, "ASIS", "../input/in.nc", "time", "", vs)})
case("glo_current_0_360", GLO5, {
    "in.cdl": cdl("in", nlon=180, nlat=91, lon0=0.0, dlon=2.0, lat0=-90.0, dlat=2.0, **GT, vars=[fv("ucur", 3), fv("vcur", 8)]),
    "ww3_prnc.nml": nml("CURRENTS", "LATLON", "../input/in.nc", "longitude", "latitude", ["ucur", "vcur"])})

case("ll_two_fillvalues", IROISE, {
    # U has _FillValue -999 and V has -888: the Fortran keeps the one it read last for both fields.
    "in.cdl": cdl("in", **IR, **T3, vars=[fv("U", 0), fv("V", 5, fillvalue=-888.0)], holes={(0, 3, 3), (1, 4, 4), (2, 10, 10)}),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})
case("ll_negative_zero_and_fill_zero", IROISE, {
    # a _FillValue of 0 and a negative zero in the data
    "in.cdl": cdl("in", **IR, **T3, vars=[fv("U", 0, fillvalue=0.0), fv("V", 5, fillvalue=0.0)], negzero={(0, 3, 3), (1, 5, 5), (2, 8, 8)}),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})

case("nml_wins_over_inp", IROISE, {
    # both input files present: the namelist is read and the old file is never opened
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"]),
    "ww3_prnc.inp": "this is not an input file\n"})

case("ll_shift_year", IROISE, {
    # a delay of about 31 million seconds: DSEC21 returns a REAL, which holds integers exactly only up to 2**24
    "in.cdl": wind("in", **IR, **T3),
    "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"], shift="19690607 013000")})

for _n in NCGEN_KIND:
    case(_n, IROISE, {
        "in.cdl": cdl("in", **IR, **T3, vars=[fv("U", 0, type="short", fillvalue=-32768, scale=0.01, offset=0.5), fv("V", 5)], holes={(1, 4, 5)}),
        "ww3_prnc.nml": nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])})

# ---- errors the reference reports and Rust reports the same way ------------------------------------------------------------
base_cdl = wind("in", **IR, **T3)
base_nml = nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"])
case("err_nofile", IROISE, {"ww3_prnc.nml": base_nml.replace("in.nc", "nofile.nc")})
case("err_novar", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": base_nml.replace("'V'", "'W'")})
case("err_nofill", IROISE, {"in.cdl": cdl("in", **IR, **T3, vars=[fv("U", 0, fill=None), fv("V", 5)]), "ww3_prnc.nml": base_nml})
case("err_units", IROISE, {"in.cdl": cdl("in", **IR, times=[0, 3], tunits="furlongs since 1968-06-06", vars=[fv("U", 0), fv("V", 5)]), "ww3_prnc.nml": base_nml})
case("err_inp_dims_not_found", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("WND", "LL", True, True, "longitude lat2", "U V", "../input/in.nc")})
case("err_lat_reversed", IROISE, {"in.cdl": cdl("in", **{**IR, "lat0": 49.2, "dlat": -0.1}, **T3, vars=[fv("U", 0), fv("V", 5)]), "ww3_prnc.nml": base_nml})
case("err_small_grid", IROISE, {"in.cdl": cdl("in", **{**IR, "nlon": 1}, **T3, vars=[fv("U", 0), fv("V", 5)]), "ww3_prnc.nml": base_nml})
case("err_no_field", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": base_nml.replace("FORCING%FIELD%WINDS = T", "FORCING%GRID%ASIS = F")})
case("err_no_grid", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": base_nml.replace("FORCING%GRID%LATLON = T", "FORCING%GRID%ASIS = F")})
case("err_tidal_on_wind", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": base_nml.replace("FORCING%GRID%LATLON = T", "FORCING%GRID%ASIS = T\n  FORCING%TIDAL = 'M2 S2'")})
case("err_tidal_not_asis", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": nml("CURRENTS", "LATLON", "../input/in.nc", "longitude", "latitude", ["U", "V"], tidal="M2 S2")})
case("err_inp_field_id", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("XXX", "LL", True, True, "longitude latitude", "U V", "../input/in.nc")})
case("err_inp_type_id", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("WND", "ZZ", True, True, "longitude latitude", "U V", "../input/in.nc")})
case("err_inp_illegal_time", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("WND", "LL", False, True, "longitude latitude", "U V", "../input/in.nc", shift="1968 53000")})
case("err_inp_short", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": "$\n  'WND' 'LL' T T\n  longitude latitude\n  U V\n"})
case("err_inp_bad_logical", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("WND", "LL", True, True, "longitude latitude", "U V", "../input/in.nc").replace("T T", "T x")})
case("err_edge_ai_small_file", IROISE, {
    "in.cdl": cdl("in", nlon=50, nlat=119, lon0=0.0, dlon=1.0, lat0=0.0, dlat=1.0, times=[0, 3], tunits="hours since 1968-06-06 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y", vars=[fv("U", 0), fv("V", 5)]),
    "ww3_prnc.nml": nml("WINDS", "ASIS", "../input/in.nc", "x", "y", ["U", "V"])})
case("err_no_time_var", IROISE, {
    "in.cdl": "netcdf in {\ndimensions:\n\tx = 3 ;\nvariables:\n\tfloat U(x) ;\ndata:\n U = 1, 2, 3 ;\n}\n",
    "ww3_prnc.nml": base_nml})

# ---- forms Rust refuses (status 64) while the reference runs them ----------------------------------------------------------------
case("refuse_tidal_at", CART, {
    "in.cdl": cdl("in", nlon=153, nlat=3, lon0=0, dlon=1, lat0=0, dlat=1, times=[0, 1, 2], tunits="hours since 1968-06-06 00:00:00", dimlon="x", dimlat="y", lonname="x", latname="y",
                  vars=[fv("wlv", 0)]),
    "ww3_prnc.nml": nml("WATER_LEVELS", "ASIS", "../input/in.nc", "x", "y", ["wlv"], tidal="M2 S2")})
case("refuse_dat_nml", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": nml("DATA_ASSIM", "LATLON", "../input/in.nc", "longitude", "latitude", ["U"])})
case("refuse_inp_at", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("LEV", "AT", True, True, "longitude latitude", "U", "../input/in.nc").replace("$\n  longitude", "$\n  M2 S2\n$\n  longitude", 1)})
case("refuse_inp_f1", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("WND", "F1", True, True, "longitude latitude", "U V", "../input/in.nc")})
case("refuse_inp_f2", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("LEV", "F2", True, True, "longitude latitude", "U", "../input/in.nc")})
case("refuse_inp_dat", IROISE, {"in.cdl": base_cdl, "ww3_prnc.inp": inp("DAT", "AI", True, True, "longitude latitude", "U", "../input/in.nc")})
case("refuse_nml_unknown_key", IROISE, {"in.cdl": base_cdl, "ww3_prnc.nml": base_nml.replace("FORCING%GRID%LATLON = T", "FORCING%GRID%LATLON = T\n  FORCING%NOPE = T")})
case("refuse_time_units_digits", IROISE, {"in.cdl": cdl("in", **IR, times=[0, 3], tunits="hours since 19x8-06-06 00:00:00", vars=[fv("U", 0), fv("V", 5)]), "ww3_prnc.nml": base_nml})


def big(outdir):
    """A global 0.25 degree wind file on the 1 degree model grid of ww3_tp2.21: too large to commit, so it is written
    to outdir (work/prnc_big) and run with PRNC_CASES=outdir."""
    d = os.path.join(outdir, "glo1_wind_025deg")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "case.conf"), "w") as f:
        f.write("REGTEST=ww3_tp2.21\nGRID_NML=ww3_grid_glo_1deg.nml\nKIND=big\n")
    with open(os.path.join(d, "in.cdl"), "w") as f:
        f.write(cdl("in", nlon=1440, nlat=721, lon0=0.0, dlon=0.25, lat0=-90.0, dlat=0.25, times=[0, 10800], tunits="seconds since 1970-01-01 00:00:00",
                    vars=[fv("uwnd", 0), fv("vwnd", 5)]))
    with open(os.path.join(d, "ww3_prnc.nml"), "w") as f:
        f.write(nml("WINDS", "LATLON", "../input/in.nc", "longitude", "latitude", ["uwnd", "vwnd"]))
    print(f"written to {d}")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--big":
        big(sys.argv[2] if len(sys.argv) > 2 else os.path.join(OUT, "..", "..", "work", "prnc_big"))
        return
    only = sys.argv[1:]
    n = 0
    for name, (grid, files) in CASES.items():
        if only and name not in only:
            continue
        d = os.path.join(OUT, name)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "case.conf"), "w") as f:
            kind = "regtest" if name in REGTEST_SHAPED and not name.startswith("refuse_") else "variant"
            f.write(f"REGTEST={grid[0]}\nGRID_NML={grid[1]}\nKIND={kind}\n")
            if name in REGTEST_SHAPED:
                f.write(f"SHAPE_OF=\"{REGTEST_SHAPED[name]}\"\n")
            if name in NCGEN_KIND:
                f.write(f"NCGEN_KIND={NCGEN_KIND[name]}\n")
        for fn, text in files.items():
            with open(os.path.join(d, fn), "w") as f:
                f.write(text)
        n += 1
    print(f"{n} cases written to regtests/prnc_inputs/")


if __name__ == "__main__":
    main()
