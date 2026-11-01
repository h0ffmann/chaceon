#!/usr/bin/env python3
"""Translates DEFAULT_META of model/src/w3ounfmetamd.F90 (the pinned WW3) into Rust, statement by statement.

The Fortran is the source; the Rust is its transcription for the switch file switch_chaceon_shrd: the
blocks under `#ifdef` (BT4, IS2) are left out because that switch does not define them, and group 10
(user defined fields) is refused at run time. Usage: ounf_meta_gen.py [--check]. Without --check it writes
rust/crates/ww3-host/src/ounfmeta.rs; with --check it exits 1 when that file differs from a fresh run.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "WW3/model/src/w3ounfmetamd.F90"
OUT = ROOT / "rust/crates/ww3-host/src/ounfmeta.rs"

LIMITS = {"units": 24, "ename": 50, "varnm": 80, "varnl": 80, "varns": 120, "varng": 120, "varnd": 120, "varnc": 512}
STRING_FIELDS = set(LIMITS)
REAL_FIELDS = {"fsc", "vmin", "vmax"}
TOKENS = {"IPART_TOKEN": "IPART", "SPART_TOKEN": "SPART", "SPART_TOKEN_": "SPART_", "PARTCOM": "ctx.partcom.as_str()", "DIRCOM": "ctx.dircom.as_str()"}


def logical_lines(text):
    """Strips comments, joins `&` continuations."""
    out, buf = [], ""
    for raw in text.splitlines():
        line = ""
        quote = None
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
        if line.endswith("&"):
            buf += line[:-1]
            continue
        buf += line
        if buf.strip():
            out.append(buf.strip())
        buf = ""
    return out


def real_lit(tok):
    t = tok.strip()
    m = re.fullmatch(r"([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?", t)
    if not m or not (m.group(2) or m.group(3)):
        raise ValueError(f"not a number: {tok}")
    sign, ip, fp, ex = m.groups()
    s = f"{sign}{ip or '0'}.{fp or '0'}"
    if ex:
        s += f"e{ex}"
    return s + "_f32"


def string_expr(rhs):
    """Parts of a `'a' // TOKEN // 'b'` expression, as Rust &str pieces."""
    parts, i = [], 0
    rhs = rhs.strip()
    while i < len(rhs):
        if rhs[i] == "'":
            j = rhs.index("'", i + 1)
            parts.append('"' + rhs[i + 1 : j].replace("\\", "\\\\").replace('"', '\\"') + '"')
            i = j + 1
        elif rhs.startswith("//", i):
            i += 2
        elif rhs[i].isspace():
            i += 1
        else:
            m = re.match(r"META\(\d\)%[A-Z]+|[A-Z_]+", rhs[i:])
            if not m:
                raise ValueError(f"cannot parse {rhs!r} at {i}")
            w = m.group(0)
            if w.startswith("META("):
                k = int(w[5]) - 1
                parts.append(f"m[{k}].{w.split('%')[1].lower()}.as_str()")
            else:
                parts.append(TOKENS[w])
            i += len(w)
    return parts


def literal_check(field, parts):
    """A pure literal longer than the Fortran CHARACTER length is cut by the assignment: say which."""
    if all(p.startswith('"') for p in parts):
        n = sum(len(eval(p)) for p in parts)  # noqa: S307 - own output
        if n > LIMITS[field]:
            print(f"note: {field} literal of {n} characters is cut to {LIMITS[field]}", file=sys.stderr)


def translate():
    text = SRC.read_text()
    body = text[text.index("SUBROUTINE DEFAULT_META()") : text.index("END SUBROUTINE DEFAULT_META")]
    lines = logical_lines(body)
    out = []
    ind = 1
    ifdef = 0
    skip_group = False
    cur = None

    def emit(s):
        out.append("    " * ind + s)

    for ln in lines:
        if ln.startswith("#ifdef") or ln.startswith("#if "):
            ifdef += 1
            continue
        if ln.startswith("#endif"):
            ifdef -= 1
            continue
        if ln.startswith("#"):
            raise ValueError(ln)
        if ifdef:
            continue
        up = ln.upper()
        if up.startswith(("SUBROUTINE", "IMPLICIT", "TYPE(META_T)")):
            continue
        m = re.fullmatch(r"META => GROUP\((\d+)\)%FIELD\((\d+)\)%META", ln)
        if m:
            g, f = int(m.group(1)), int(m.group(2))
            skip_group = g == 10
            cur = (g, f)
            if not skip_group:
                emit(f"let m = g.field({g}, {f});")
            continue
        if skip_group:
            continue
        m = re.fullmatch(r"IF ?\((.*)\) THEN", ln, re.I)
        if m:
            cond = m.group(1).strip().upper()
            rust = {"VECTOR": "ctx.vector", "NCVARTYPE.LE.3": "ctx.ncvartype <= 3", "NCVARTYPE.EQ.2": "ctx.ncvartype == 2"}[cond]
            emit(f"if {rust} {{")
            ind += 1
            continue
        if re.fullmatch(r"ELSE", ln, re.I):
            ind -= 1
            emit("} else {")
            ind += 1
            continue
        if re.fullmatch(r"END ?IF", ln, re.I):
            ind -= 1
            emit("}")
            continue
        m = re.fullmatch(r"META\((\d)\) = META\((\d)\)", ln)
        if m:
            emit(f"m[{int(m.group(1)) - 1}] = m[{int(m.group(2)) - 1}].clone();")
            continue
        if re.fullmatch(r"WRITE\(META\((\d)\)%VARNC,'\(A,F8\.4,A,F8\.4,A\)'\).*", ln):
            k = int(re.match(r"WRITE\(META\((\d)\)", ln).group(1)) - 1
            emit(f"m[{k}].varnc = freq_range(ctx);")
            continue
        m = re.fullmatch(r"META\((\d)\)%([A-Z]+) *= *(.*)", ln)
        if m:
            k, fld, rhs = int(m.group(1)) - 1, m.group(2).lower(), m.group(3)
            if fld in REAL_FIELDS:
                emit(f"m[{k}].{fld} = {real_lit(rhs)};")
            elif fld in STRING_FIELDS:
                parts = string_expr(rhs)
                literal_check(fld, parts)
                if len(parts) == 1:
                    emit(f"m[{k}].{fld} = fit({parts[0]}, {LIMITS[fld]});")
                else:
                    emit(f"m[{k}].{fld} = fit(&[{', '.join(parts)}].concat(), {LIMITS[fld]});")
            else:
                raise ValueError(ln)
            continue
        raise ValueError(f"unhandled line: {ln}")
    assert ind == 1, ind
    return "\n".join(out)


HEAD = """// SPDX-License-Identifier: MPL-2.0
// Generated by scripts/ounf_meta_gen.py from DEFAULT_META in model/src/w3ounfmetamd.F90 at 92e2cf66, for
// switch_chaceon_shrd (no BT4, no IS2). Do not edit: change the generator or the pin and run it again.
//! The default netCDF metadata of `ww3_ounf`, field by field and component by component.

use std::collections::BTreeMap;

pub const UNSETC: &str = "unset";
/// `HUGE(1.0)`.
pub const UNSETR: f32 = f32::MAX;

/// `META_T` without the user-defined list, which this port refuses (`ounfmeta.inp`).
#[derive(Clone)]
pub struct Meta {
    pub fsc: f32,
    pub vmin: f32,
    pub vmax: f32,
    pub units: String,
    pub ename: String,
    pub varnm: String,
    pub varnl: String,
    pub varns: String,
    pub varng: String,
    pub varnd: String,
    pub varnc: String,
}

impl Default for Meta {
    fn default() -> Self {
        let u = || UNSETC.to_string();
        Self { fsc: 0., vmin: 0., vmax: UNSETR, units: u(), ename: u(), varnm: u(), varnl: u(), varns: u(), varng: u(), varnd: u(), varnc: u() }
    }
}

/// What `DEFAULT_META` reads from the rest of the program.
pub struct Ctx {
    /// `VECTOR`.
    pub vector: bool,
    /// `NCVARTYPE`, as `INIT_META` sees it: the namelist's `FIELD%TYPE`.
    pub ncvartype: i32,
    /// `DIRCOM`, `PARTCOM`.
    pub dircom: String,
    pub partcom: String,
    /// `SIG(1)*TPIINV` and `SIG(NK)*TPIINV`, in `REAL`.
    pub f1: f32,
    pub fnk: f32,
}

/// `F8.4` of a `REAL`: the exact value of the `f32` rounded to four decimals, right-justified.
fn f84(v: f32) -> String {
    format!("{:>8.4}", f64::from(v))
}

fn freq_range(ctx: &Ctx) -> String {
    format!("Frequency range {} to {} Hz", f84(ctx.f1), f84(ctx.fnk))
}

/// A Fortran assignment to `CHARACTER(LEN=n)` keeps the first `n` characters.
pub fn fit(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub const IPART: &str = "<IPART>";
pub const SPART: &str = "<SPART>";
pub const SPART_: &str = "<SPART_>";

/// `GROUP(i)%FIELD(j)%META(1:3)`.
#[derive(Default)]
pub struct Metas(BTreeMap<(usize, usize), [Meta; 3]>);

impl Metas {
    fn field(&mut self, i: usize, j: usize) -> &mut [Meta; 3] {
        self.0.entry((i, j)).or_default()
    }
    pub fn get(&self, i: usize, j: usize) -> &[Meta; 3] {
        self.0.get(&(i, j)).unwrap_or_else(|| panic!("no default metadata for group {i} field {j}"))
    }
}

#[allow(clippy::too_many_lines)]
pub fn default_meta(ctx: &Ctx) -> Metas {
    let mut g = Metas::default();
"""


def main():
    body = translate()
    text = HEAD + body + "\n    g\n}\n"
    if "--check" in sys.argv:
        sys.exit(0 if OUT.exists() and OUT.read_text() == text else 1)
    OUT.write_text(text)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(text.splitlines())} lines")


if __name__ == "__main__":
    main()
