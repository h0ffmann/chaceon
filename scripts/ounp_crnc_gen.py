#!/usr/bin/env python3
"""Translates W3CRNC of model/src/ww3_ounp.F90 (the pinned WW3) into Rust, statement by statement.

W3CRNC defines the NetCDF dimensions, variables and attributes of one output file. The Fortran is the source; the
Rust is its transcription for switch_chaceon_shrd: the blocks under `#ifdef W3_RTD` and `W3_SETUP` are left out
because that switch defines neither, and only the (ITYPE, OTYPE) pairs in IMPLEMENTED are emitted; the program
refuses the others before it creates a file. Usage: ounp_crnc_gen.py [--check]. Without --check it writes
rust/tools/ww3_ounp/src/crnc.rs; with --check it exits 1 when that file differs from a fresh run.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "WW3/model/src/ww3_ounp.F90"
OUT = ROOT / "rust/tools/ww3_ounp/src/crnc.rs"

IMPLEMENTED = [(1, 3), (2, 1), (2, 2), (2, 5), (2, 6)]
OFF = {"W3_RTD", "W3_SETUP"}
FILL = {"NF90_FILL_FLOAT": "att_float(FILL_FLOAT)", "NF90_FILL_SHORT": "att_short(FILL_SHORT)", "NF90_FILL_INT": "att_int(FILL_INT)"}
TYPES = {"NF90_DOUBLE": "NC_DOUBLE", "NF90_FLOAT": "NC_FLOAT", "NF90_INT": "NC_INT", "NF90_SHORT": "NC_SHORT", "NF90_CHAR": "NC_CHAR"}


def logical_lines(lines):
    """Strips comments and the cpp blocks of switches this build leaves off, joins `&` continuations. A line is
    (number of its first physical line, text)."""
    out, buf, first, skip, taint = [], "", None, 0, False
    for no, raw in lines:
        s = raw.strip()
        m = re.match(r"#\s*(ifdef|ifndef|if|else|endif)\s*(\S*)", s)
        if m:
            kind, sym = m.groups()
            if skip:
                if kind in ("ifdef", "ifndef", "if"):
                    skip += 1
                elif kind == "endif":
                    skip -= 1
                continue
            if kind == "ifdef" and sym in OFF:
                skip = 1
                continue
            if kind == "endif":
                continue
            raise SystemExit(f"{no}: cpp line {s!r} is not one the generator knows")
        if skip:
            continue
        line, quote = "", None
        if taint:
            line = raw
        else:
            for ch in raw:
                if quote:
                    line += ch
                    if ch == quote:
                        quote = None
                elif ch in "'\"":
                    quote = ch
                    line += ch
                elif ch == "!":
                    break
                else:
                    line += ch
        line = line.rstrip()
        if quote or taint:
            # A string that runs across lines keeps the blanks of the next line: not translated, and a rule
            # that meets it stops the generator.
            taint = line.endswith("&")
            buf += "@UNSUPPORTED "
            if not taint:
                buf += line
                out.append((first or no, buf.strip()))
                buf, first = "", None
                continue
            if first is None:
                first = no
            continue
        if line.endswith("&"):
            if first is None:
                first = no
            buf += line[:-1]
            continue
        buf += line.lstrip() if buf else line
        if buf.strip():
            out.append((first or no, buf.strip()))
        buf, first = "", None
    return out


def lit(tok):
    """A PUT_ATT value: its Rust call and argument."""
    t = tok.strip()
    if t in FILL:
        return FILL[t]
    if t.upper() == "TRIM(CALTYPE)":
        return "text(c.caltype)"
    if t.startswith("'"):
        return "text(" + rstr(t) + ")"
    if re.fullmatch(r"[+-]?\d+", t):
        return f"att_int({int(t)})"
    m = re.fullmatch(r"([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?", t)
    if not m or not (m.group(2) or m.group(3)):
        raise SystemExit(f"not a literal: {tok}")
    sign, ip, fp, ex = m.groups()
    s = f"{sign}{ip or '0'}.{fp or '0'}" + (f"e{ex}" if ex else "")
    return f"att_float({s}f32)"


def rstr(t):
    body = t[1:-1].replace("''", "'")
    return '"' + body.replace("\\", "\\\\").replace('"', '\\"') + '"'


def cond(c):
    c = c.strip()
    for a, b in ((".AND.", " && "), (".OR.", " || "), (".EQ.", " == "), (".NE.", " != "), (".LE.", " <= "), (".GE.", " >= ")):
        c = c.replace(a, b)
    c = re.sub(r"\bFLSRCE\((\d)\)", lambda m: f"c.flsrce[{int(m.group(1)) - 1}]", c)
    c = c.replace("ITYPE", "c.itype").replace("OTYPE", "c.otype").replace("NCTYPE", "c.nctype")
    c = c.replace("NCVARTYPE", "c.ncvartype").replace("ISCALE", "c.iscale").replace("FLWW3", "c.flww3")
    c = c.replace("FLAGLL", "c.flagll")
    c = re.sub(r"c\.c\.", "c.", c)
    c = c.replace("c.nc c.ncvartype", "c.ncvartype")
    return re.sub(r"\s+", " ", c).strip()


def dimlist(s):
    s = s.strip()
    m = re.fullmatch(r"\(/(.*)/\)", s)
    parts = m.group(1).split(",") if m else [s]
    out = []
    for p in parts:
        mm = re.fullmatch(r"\s*dimid\((\w+)\)\s*", p, re.I)
        if not mm:
            raise SystemExit(f"dimension list {s!r}")
        k = mm.group(1).upper()
        out.append({"ONE": "dimid[c.one]", "TWO": "dimid[c.two]"}.get(k, f"dimid[{k}]"))
    return "&[" + ", ".join(out) + "]"


def split_args(s):
    """Top-level comma split."""
    out, depth, cur, quote = [], 0, "", None
    for ch in s:
        if quote:
            cur += ch
            if ch == quote:
                quote = None
            continue
        if ch in "'\"":
            quote = ch
        elif ch in "(":
            depth += 1
        elif ch in ")":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    out.append(cur)
    return out


def stmt(no, text, ind, state):
    """One Fortran statement as Rust lines."""
    pad = "    " * ind
    t = text.strip()
    if "@UNSUPPORTED" in t:
        raise SystemExit(f"{no}: a string runs across lines, which the generator does not translate")
    if re.match(r"(FREQ1?2?|DIR)\(1:N\w+\)\s*=", t) or re.match(r"FREQ[12]\((1|NK)\)\s*=", t):
        return []
    m = re.fullmatch(r"IF \((.*)\) (IRET\s*=.*)", t, re.I)
    if m:
        body = stmt(no, m.group(2), ind + 1, state)
        return [f"{pad}if {cond(m.group(1))} {{"] + body + [f"{pad}}}"]
    m = re.fullmatch(r"IRET\s*=\s*(\w+)\((.*)\)", t, re.I)
    if m:
        fn, args = m.group(1).upper(), split_args(m.group(2))
        args = [a.strip() for a in args]
        v = lambda a: re.fullmatch(r"VARID\((\d+)\)", a, re.I).group(1)
        if fn == "NF90_DEF_DIM":
            d = re.fullmatch(r"DIMLN\((\d)\)", args[2]).group(1)
            k = re.fullmatch(r"DIMID\((\d)\)", args[3]).group(1)
            return [f"{pad}iret = nc::def_dim(ncid, {rstr(args[1])}, c.dimln[{d}], &mut dimid[{k}]);"]
        if fn == "NF90_DEF_VAR":
            return [f"{pad}iret = nc::def_var(ncid, {rstr(args[1])}, nc::{TYPES[args[2]]}, {dimlist(args[3])}, &mut varid[{v(args[4])}]);"]
        if fn == "NF90_DEF_VAR_DEFLATE":
            return [f"{pad}iret = nc::def_var_deflate(ncid, varid[{v(args[1])}], 1, 1, DEFLATE);"]
        if fn == "NF90_PUT_ATT":
            vid = "nc::NC_GLOBAL" if args[1].upper() == "NF90_GLOBAL" else f"varid[{v(args[1])}]"
            val = lit(args[3])
            if val.startswith("text("):
                return [f"{pad}iret = nc::put_att_text(ncid, {vid}, {rstr(args[2])}, {val[5:-1]});"]
            fnn, a = val.split("(", 1)
            return [f"{pad}iret = nc::put_{fnn}(ncid, {vid}, {rstr(args[2])}, {a[:-1]});"]
        if fn == "NF90_ENDDEF":
            return [f"{pad}iret = nc::enddef(ncid);"]
        if fn == "NF90_PUT_VAR":
            name = re.fullmatch(r"(FREQ1?2?|DIR)\(1:(NK|NTH)\)", args[2]).group(1).lower()
            return [f"{pad}iret = nc::put_real(ncid, varid[{v(args[1])}], &[1], None, &[{name}.len()], {name});"]
    m = re.fullmatch(r"CALL CHECK_ERR\(IRET,(\d+)\)", t, re.I)
    if m:
        return [f"{pad}ck(iret, {m.group(1)});"]
    m = re.fullmatch(r"IF ?\((.*)\) THEN", t, re.I)
    if m:
        return [f"{pad}if {cond(m.group(1))} {{"]
    m = re.fullmatch(r"ELSE ?IF ?\((.*)\) ?THEN", t, re.I)
    if m:
        return [f"{pad}}} else if {cond(m.group(1))} {{"]
    if re.fullmatch(r"ELSE", t, re.I):
        return [f"{pad}}} else {{"]
    if re.fullmatch(r"END ?IF", t, re.I):
        return [f"{pad}}}"]
    raise SystemExit(f"{no}: no rule for {t!r}")


def translate(lines, ind):
    out = []
    for no, text in lines:
        r = stmt(no, text, ind, None)
        if r:
            out += r
    return out


def main():
    src = SRC.read_text().splitlines()
    start = next(i for i, l in enumerate(src) if re.match(r"\s*SUBROUTINE W3CRNC", l))
    end = next(i for i, l in enumerate(src) if re.match(r"\s*END SUBROUTINE W3CRNC", l))
    body = [(i + 1, l) for i, l in enumerate(src[start:end], start)]
    ll = logical_lines(body)
    # General part: from the first DEF_DIM to the blocks of ITYPE and OTYPE.
    first = next(k for k, (_, t) in enumerate(ll) if "NF90_DEF_DIM(NCID, 'time'" in t)
    blocks_at = next(k for k, (_, t) in enumerate(ll) if re.match(r"IF \(ITYPE\.EQ\.1 \.AND\. OTYPE\.EQ\.2\) THEN", t))
    general = ll[first:blocks_at]
    rest = ll[blocks_at:]
    # Drop the time long_name SELECT CASE, which the generated code writes by hand for the standard calendar.
    g2 = []
    skip = False
    for no, t in general:
        if re.match(r"SELECT CASE", t):
            skip = True
            g2.append((no, "@CALTYPE"))
            continue
        if re.match(r"END SELECT", t):
            skip = False
            continue
        if not skip:
            g2.append((no, t))
    out = []
    out.append("// SPDX-License-Identifier: MPL-2.0")
    out.append("//! W3CRNC of `model/src/ww3_ounp.F90:3068-5543` for `switches/switch_chaceon_shrd`: the NetCDF dimensions, variables and")
    out.append("//! attributes of one file, in the Fortran's order of calls. Generated by `scripts/ounp_crnc_gen.py` (`--check` finds")
    out.append("//! it stale); do not edit. The pairs (ITYPE, OTYPE) it holds are " + ", ".join(map(str, IMPLEMENTED)) + ".")
    out.append("")
    out.append("use crate::nc::{self, DEFLATE, FILL_FLOAT, FILL_INT, FILL_SHORT};")
    out.append("")
    out.append("/// What W3CRNC reads from the main program and the grid.")
    out.append("pub struct Crnc<'a> {")
    out.append("    pub itype: i32,")
    out.append("    pub otype: i32,")
    out.append("    pub nctype: i32,")
    out.append("    pub ncfile: &'a str,")
    out.append("    /// `DIMLN(1:5)` at index 1..=5; `NF90_UNLIMITED` is 0.")
    out.append("    pub dimln: [usize; 6],")
    out.append("    /// `ONE` and `TWO`, which of the dimensions time and station comes first.")
    out.append("    pub one: usize,")
    out.append("    pub two: usize,")
    out.append("    pub ncvartype: i32,")
    out.append("    pub flww3: i32,")
    out.append("    pub flagll: bool,")
    out.append("    pub caltype: &'a str,")
    out.append("    pub freq: &'a [f32],")
    out.append("    pub freq1: &'a [f32],")
    out.append("    pub freq2: &'a [f32],")
    out.append("    pub dir: &'a [f32],")
    out.append("}")
    out.append("")
    out.append("/// Creates the file and defines everything; returns `NCID` and `VARID(1:28)` (index 0 unused). `ck` is `CHECK_ERR`.")
    out.append("#[allow(unused_assignments, clippy::too_many_lines)]")
    out.append("pub fn w3crnc(c: &Crnc, ck: &mut dyn FnMut(i32, i32)) -> (i32, [i32; 29]) {")
    out.append("    let mut dimid = [0i32; 8];")
    out.append("    let mut varid = [0i32; 29];")
    out.append("    let mut iret;")
    out.append("    let (freq, freq1, freq2, dir) = (c.freq, c.freq1, c.freq2, c.dir);")
    out.append("    let mut ncid = 0;")
    out.append("    iret = if c.nctype == 3 { nc::create(c.ncfile, 0, &mut ncid) } else { nc::create(c.ncfile, nc::NC_NETCDF4, &mut ncid) };")
    out.append("    ck(iret, 20);")
    for no, t in g2:
        if t == "@CALTYPE":
            out.append("    iret = nc::put_att_text(ncid, varid[1], \"long_name\", \"julian day (UT)\");")
            continue
        out += stmt(no, t, 1, None) or []
    out.append("")
    # The (ITYPE, OTYPE) blocks.
    blk = []
    cur = None
    for no, t in rest:
        m = re.match(r"(?:ELSE )?IF \(ITYPE\.EQ\.(\d) \.AND\. OTYPE\.EQ\.(\d)\) THEN", t)
        if m:
            cur = (int(m.group(1)), int(m.group(2)))
            blk.append((cur, []))
            continue
        if cur is not None:
            blk[-1][1].append((no, t))
    first_branch = True
    for key, lines in blk:
        # The last block's closing END IF and RETURN stay out of the list.
        while lines and lines[-1][1] in ("END IF", "RETURN"):
            lines.pop()
        if key not in IMPLEMENTED:
            continue
        kw = "if" if first_branch else "} else if"
        first_branch = False
        out.append(f"    {kw} c.itype == {key[0]} && c.otype == {key[1]} {{")
        out += translate(lines, 2)
    out.append("    } else {")
    out.append("        unreachable!(\"the program refuses the other (ITYPE, OTYPE) before W3CRNC\");")
    out.append("    }")
    out.append("    (ncid, varid)")
    out.append("}")
    # Indent by brace depth, so nested IF blocks read as they do in the Fortran.
    depth, res = 0, []
    for l in out:
        t = l.strip()
        if t.startswith("}"):
            depth -= 1
        res.append(("    " * depth + t) if t else "")
        if t.endswith("{"):
            depth += 1
    return "\n".join(res) + "\n"


if __name__ == "__main__":
    text = main()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"{OUT} is stale: run scripts/ounp_crnc_gen.py", file=sys.stderr)
            sys.exit(1)
        print(f"{OUT.name} is current")
    else:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(text)
        print(f"wrote {OUT}")
