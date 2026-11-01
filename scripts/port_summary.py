#!/usr/bin/env python3
"""The port's size by WW3 area and its Rust lines by evidence, from the source at the pin and rust/ports.tsv.
Writes the blocks between the port-badges and port-summary markers of README.md; --check fails when one is stale."""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "WW3/model/src"
BEGIN, END = "<!-- port-summary:begin -->", "<!-- port-summary:end -->"
BADGES_BEGIN, BADGES_END = "<!-- port-badges:begin -->", "<!-- port-badges:end -->"

# Every .F90 of model/src falls in the first area that matches. A file in none stops the script.
AREAS = [
    ("Source terms", r"w3(src\d|snl\w*|sln\d|sbt\d|sdb\d|str\d|sbs\d|sic\d|sis\d|ref\d|swld|uost|flx\d|gke|gig\d|srce)md"),
    ("Propagation, structured grids", r"w3(pro[123]|uqck|uno2|psmc|smco)md"),
    ("Propagation, unstructured grids", r"w3(profsmd(_pdlib)?|triamd|wavset)|pdlib_field_vec"),
    ("Time loop, updates, state", r"w3(wave|init|updt|wdat|adat|idat|gdat|odat|disp|time|serv|arry|parall|meminfo|tide|cspc|strk|part|wdas)(md)?$|constants"),
    ("File I/O in the model", r"w3(iogr|iogo|iopo|iors|iobc|iosf|iotr|flds|fld[12])md"),
    ("Grid search and metrics", r"w3gsrumd"),
    ("Drivers", r"ww3_(shel|multi)|ww3_multi_esmf|wm(final|fin|ini|io|grid|mdat|scrp|unit|updt|wave)\w*"),
    ("Preprocessors", r"ww3_(grid|strt|prep|prnc|prtide|bound|bounc|gspl|gint|uprstr)|w3gridmd|w3nml\w+|w3canomd"),
    ("Postprocessors", r"ww3_(ounf|ounp|outf|outp|trnc|trck|grib|systrk|sbs1)|gx_\w+|w3(bull|meta|ounfmeta)md"),
    ("Coupling and other", r".*"),
]
# What still needs a Fortran program for a run, and what it writes.
PREPROCESSORS = ["ww3_prep"]


# Rust evidence, weakest last. A real GPU has no verdict yet, so its count is a constant until one exists.
EVIDENCE = ["spirv", "host", "routine", "translated"]
# Rust files the ledger has no row for. The first prefix that matches wins; a file in none stops the script.
# None means a verification tool, which is not port code and is counted apart.
UNLEDGERED = [
    ("rust/crates/ww3-host/src/capture.rs", None),
    ("rust/crates/ww3-host/", "host"),
    ("rust/crates/ww3-kernels/", "spirv"),
    ("rust/crates/ww3-libm/", "spirv"),
    ("rust/gpu-spirv/shader/", "spirv"),
    ("rust/gpu-spirv/builder/", None),
    ("rust/gpu-cuda/builder/", None),
    # Compiled to PTX and never executed.
    ("rust/gpu-cuda/kernels/", "translated"),
    ("rust/tools/spirv_shel/", "spirv"),
    ("rust/tools/ww3_strt/", "host"),
    ("rust/tools/", None),
]


def code_lines(text):
    """Lines that are neither blank nor a Fortran comment. Preprocessor lines count as not code."""
    return sum(1 for l in text.splitlines() if l.strip() and not l.lstrip().startswith(("!", "#")))


def area_of(stem):
    for name, pattern in AREAS:
        if re.fullmatch(pattern, stem) or (name != "Coupling and other" and re.match(pattern, stem)):
            return name
    sys.exit(f"{stem} is in no area")


def fmt(n):
    return f"{n:,}".replace(",", " ")


def pct(a, b):
    return f"{100 * a / b:.0f}%" if b else "n/a"


def rust_lines():
    groups = {
        "`ww3-kernels`, runs on the device": ["rust/crates/ww3-kernels"],
        "`ww3-libm`, glibc's bits in plain Rust": ["rust/crates/ww3-libm"],
        "`ww3-host`, the host side": ["rust/crates/ww3-host"],
        "GPU entry points and builders": ["rust/gpu-spirv/shader", "rust/gpu-spirv/builder", "rust/gpu-cuda/kernels", "rust/gpu-cuda/builder"],
        "Tools": ["rust/tools"],
    }
    out = []
    for name, dirs in groups.items():
        n = 0
        for d in dirs:
            for f in (ROOT / d).rglob("*.rs"):
                if "target" not in f.parts:
                    n += sum(1 for _ in f.open())
        out.append((name, n))
    return out


def rust_code_lines(path):
    """Lines that are neither blank nor a // comment, the Rust counterpart of code_lines."""
    return sum(1 for l in path.read_text().splitlines() if l.strip() and not l.lstrip().startswith("//"))


def rust_by_evidence():
    """Rust code lines per evidence. A file counts at the weakest ledger row that points at it."""
    rows = {}
    for line in (ROOT / "rust/ports.tsv").read_text().splitlines():
        c = line.split("\t")
        if not line.startswith("#") and c[0] != "routine":
            rows.setdefault(c[5], []).append(c[3])
    out = dict.fromkeys(EVIDENCE, 0)
    tools = 0
    for d in ("crates", "gpu-spirv", "gpu-cuda", "tools"):
        for f in sorted((ROOT / "rust" / d).rglob("*.rs")):
            if "target" in f.parts:
                continue
            rel = str(f.relative_to(ROOT))
            if rel in rows:
                out[max(rows[rel], key=EVIDENCE.index)] += rust_code_lines(f)
                continue
            status = next(((s,) for p, s in UNLEDGERED if rel.startswith(p)), None)
            if status is None:
                sys.exit(f"{rel} has no ledger row and no line in UNLEDGERED")
            if status[0] is None:
                tools += rust_code_lines(f)
            else:
                out[status[0]] += rust_code_lines(f)
    return out, tools


def badge(label, value, colour):
    # shields.io splits on a single hyphen, so a hyphen in the text is written twice.
    def esc(t):
        return t.replace("-", "--").replace(" ", "%20")
    return f"![{label}: {value}](https://img.shields.io/badge/{esc(label)}-{esc(value)}-{colour})"


def build_badges(fortran, ev):
    ported = sum(ev.values())
    cpu = ev["spirv"] + ev["host"]
    return "\n".join([BADGES_BEGIN,
                      badge("Fortran target", f"{fmt(fortran)} lines", "lightgrey"),
                      badge("Rust ported", f"{fmt(ported)} lines", "blue"),
                      badge("Rust CPU validated", f"{fmt(cpu)} lines", "brightgreen"),
                      badge("Rust SPIR-V on CPU Vulkan", f"{fmt(ev['spirv'])} lines", "yellowgreen"),
                      badge("Rust GPU validated", "0 lines", "red"),
                      BADGES_END])


def build():
    files = sorted(SRC.glob("*.F90"))
    area = {name: {"files": 0, "lines": 0, "code": 0, "done": 0, "partial": 0} for name, _ in AREAS}
    stems = {}
    for f in files:
        text = f.read_text(errors="replace")
        a = area[area_of(f.stem)]
        a["files"] += 1
        a["lines"] += len(text.splitlines())
        a["code"] += code_lines(text)
        stems[f.stem] = (area_of(f.stem), len(text.splitlines()))
    done = {"translated": 0, "routine": 0, "host": 0, "spirv": 0}
    for line in (ROOT / "rust/ports.tsv").read_text().splitlines():
        c = line.split("\t")
        if line.startswith("#") or c[0] == "routine":
            continue
        stem = Path(c[1].split(":")[0]).stem
        n, status = int(c[2]), c[3]
        done[status] += n
        key = "partial" if status == "translated" else "done"
        area[area_of(stem)][key] += n

    total = {k: sum(a[k] for a in area.values()) for k in ("files", "lines", "code", "done", "partial")}
    rust = rust_lines()
    ev, tools = rust_by_evidence()
    rtotal = sum(n for _, n in rust)
    head = (ROOT / "WW3").resolve()
    pin = subprocess.run(["git", "-C", str(ROOT), "ls-files", "-s", "WW3"], capture_output=True, text=True).stdout.split()[1][:8]
    prep = sum(stems[s][1] for s in PREPROCESSORS)
    _ = head

    o = [BEGIN, "", f"Generated by `just port-summary` from `WW3/model/src` at `{pin}` and `rust/ports.tsv`.", ""]
    o += ["| WW3 area | Files | Fortran lines | Not comments | Routines with Rust, lines | Share | Partial |", "|---|---:|---:|---:|---:|---:|---:|"]
    for name, a in area.items():
        o.append(f"| {name} | {a['files']} | {fmt(a['lines'])} | {fmt(a['code'])} | {fmt(a['done'])} | {pct(a['done'], a['lines'])} | {fmt(a['partial'])} |")
    o.append(f"| **All of `model/src/*.F90`** | **{total['files']}** | **{fmt(total['lines'])}** | **{fmt(total['code'])}** | **{fmt(total['done'])}** | **{pct(total['done'], total['lines'])}** | **{fmt(total['partial'])}** |")
    o += ["", "*Routines with Rust* sums the whole Fortran routine for each row of `rust/ports.tsv` with a verdict on the "
          "code reached, comments included, so a share is an upper bound on what was exercised, and it counts only "
          "the switches of `switch_chaceon_shrd`. *Partial* is `translated`: Rust exists and is not fully reached.", ""]
    o += ["| Rust by evidence, code lines | Lines |", "|---|---:|",
          "| Real GPU: byte-identical whole runs | 0 |",
          f"| SPIR-V on CPU Vulkan (lavapipe); its kernels also natively on the CPU | {fmt(ev['spirv'])} |",
          f"| Native on the CPU only | {fmt(ev['host'])} |",
          f"| Every captured call identical, no whole-run verdict | {fmt(ev['routine'])} |",
          f"| Partly reached, or compiled and never run | {fmt(ev['translated'])} |",
          f"| **Rust ported** | **{fmt(sum(ev.values()))}** |",
          f"| Verification tools, not port code | {fmt(tools)} |", ""]
    o += [f"Badges compare code lines on both sides: {fmt(total['code'])} Fortran lines that are not comments or "
          "preprocessor lines, every switch of `model/src` included, against Rust lines that are not blank or `//` "
          "comments. The two languages' lines are not the same unit, so the ratio is not a percentage of the port. "
          "*CPU validated* is the first two rows. **SPIR-V on CPU Vulkan is compiled for a GPU API but runs on a CPU, "
          "so it does not count as GPU validated.** A Rust file counts at the weakest ledger row pointing at it, "
          "which never overclaims and understates `shel.rs` and `gridpre.rs`, partly reached only through "
          "`W3WAVE` and `W3NMLGRID`.", ""]
    o += ["| Rust, all lines | Lines |", "|---|---:|"]
    o += [f"| {n} | {fmt(l)} |" for n, l in rust]
    o += [f"| **All** | **{fmt(rtotal)}** |", ""]
    o += ["| Verdict, lines of Fortran routines | Lines |", "|---|---:|",
          f"| `spirv`: byte-identical whole runs on CPU Vulkan | {fmt(done['spirv'])} |",
          f"| `host`: byte-identical whole runs on the CPU | {fmt(done['host'])} |",
          f"| `routine`: every captured call identical | {fmt(done['routine'])} |",
          f"| `translated`: exists, not fully reached | {fmt(done['translated'])} |", ""]
    o += ["**What still depends on Fortran.** No Rust binary links Fortran. A run still reads files that Fortran "
          "programs wrote where Rust stops: `current.ww3` from `ww3_prep` when a case has a current file, and "
          "`mod_def.ww3` from `ww3_grid` for unstructured and SMC grids, formatted or unformatted data files (`IDFM` 2 "
          "and 3), the old `ww3_grid.inp` and tripole closure. `restart.ww3` can come from Rust's `ww3_strt`, except "
          f"for start type 2. `ww3_prep` is {fmt(prep)} lines, {pct(prep, total['lines'])} of `model/src`. Every verdict "
          "also needs the Fortran reference, by design: it is what Rust is compared with.", "", END]
    return "\n".join(o), build_badges(total["code"], ev)


def main():
    readme = ROOT / "README.md"
    s = readme.read_text()
    block, badges = build()
    new = re.sub(re.escape(BEGIN) + ".*?" + re.escape(END), lambda _: block, s, flags=re.S)
    new = re.sub(re.escape(BADGES_BEGIN) + ".*?" + re.escape(BADGES_END), lambda _: badges, new, flags=re.S)
    if "--check" in sys.argv:
        sys.exit(0 if new == s else "README.md: run just port-summary")
    readme.write_text(new)


main()
