#!/usr/bin/env python3
"""Cross-tool build benchmark: Mirzam against Marp and Typst/Touying.

Generates the same deck in three source languages, builds it with each tool,
and reports wall time, peak memory and output size. The point is not to crown a
winner - the tools target different outputs - but to keep an honest number
beside the claim in docs/roadmap.md that Mirzam stays fast as decks grow.

  python3 scripts/bench-compare.py --sizes 20,120,500 --reps 3

Tools are skipped, not faked, when they are not installed:
  mirzam  target/release/mirzam (or --mirzam), built from this checkout
  marp    npm install -g @marp-team/marp-cli
  typst   cargo install typst-cli, plus touying in the package cache

PDF runs need a Chromium for Mirzam and Marp; point at one with MIRZAM_CHROMIUM
and CHROME_PATH, or pass --chromium.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

# Runs the real command in a child of its own, so peak RSS is that command's
# and not the maximum over every child this script has ever waited for. stdin is
# closed on purpose: marp reads a deck from a pipe when one is open, so an
# inherited stdin leaves it waiting instead of building the file it was given.
WRAPPER = r"""
import json, resource, subprocess, sys, time
t = time.perf_counter()
p = subprocess.run(sys.argv[1:], stdin=subprocess.DEVNULL,
                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
elapsed = time.perf_counter() - t
ru = resource.getrusage(resource.RUSAGE_CHILDREN)
print(json.dumps({
    "ms": elapsed * 1000.0,
    "maxrss_kb": ru.ru_maxrss,
    "code": p.returncode,
    "stderr": p.stderr.decode("utf-8", "replace")[-4000:],
}))
"""


def measure(cmd, cwd=None, env=None):
    """Run cmd once and return (ms, peak RSS in MiB, returncode, stderr)."""
    proc = subprocess.run(
        [sys.executable, "-c", WRAPPER, *cmd],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return None, None, proc.returncode, proc.stdout + proc.stderr
    data = json.loads(proc.stdout.strip().splitlines()[-1])
    return data["ms"], data["maxrss_kb"] / 1024.0, data["code"], data["stderr"]


# --------------------------------------------------------------------------
# Deck generators. The three sources must say the same thing; a difference in
# content is a difference in the measurement.
# --------------------------------------------------------------------------

def mirzam_deck(slides: int, profile: str) -> str:
    out = ["---", "title: Benchmark", "---", ""]
    for i in range(1, slides + 1):
        if i > 1:
            out += ["---", ""]
        out.append(f"## Section {i}")
        out.append("")
        if profile == "plain":
            out += [
                f"Body text for slide {i} with **emphasis** and `code`.",
                "",
                "- First point",
                "- Second point",
                "- Third point",
                "",
                "| Key | Value |",
                "|---|---:|",
                "| a | 1 |",
                "| b | 2 |",
                "",
            ]
        else:
            for j in range(1, 6):
                out += [f"Paragraph {j}: $\\alpha_{{{i}}}^{{{j}}} + \\frac{{x}}{{y}}$", ""]
            for j in range(1, 4):
                out += [f"$$\\int_0^{{{j}}} e^{{-x^2}} dx$$", ""]
    return "\n".join(out) + "\n"


def marp_deck(slides: int, profile: str) -> str:
    out = ["---", "marp: true", "math: katex", "---", ""]
    for i in range(1, slides + 1):
        if i > 1:
            out += ["---", ""]
        out.append(f"## Section {i}")
        out.append("")
        if profile == "plain":
            out += [
                f"Body text for slide {i} with **emphasis** and `code`.",
                "",
                "- First point",
                "- Second point",
                "- Third point",
                "",
                "| Key | Value |",
                "|---|---:|",
                "| a | 1 |",
                "| b | 2 |",
                "",
            ]
        else:
            for j in range(1, 6):
                out += [f"Paragraph {j}: $\\alpha_{{{i}}}^{{{j}}} + \\frac{{x}}{{y}}$", ""]
            for j in range(1, 4):
                out += [f"$$\\int_0^{{{j}}} e^{{-x^2}} dx$$", ""]
    return "\n".join(out) + "\n"


def typst_deck(slides: int, profile: str) -> str:
    out = [
        '#import "@preview/touying:0.7.1": *',
        "#import themes.simple: *",
        "",
        '#show: simple-theme.with(aspect-ratio: "16-9")',
        "",
    ]
    for i in range(1, slides + 1):
        out.append(f"== Section {i}")
        out.append("")
        if profile == "plain":
            out += [
                f"Body text for slide {i} with *emphasis* and `code`.",
                "",
                "- First point",
                "- Second point",
                "- Third point",
                "",
                "#table(",
                "  columns: (auto, auto),",
                "  [Key], [Value],",
                "  [a], [1],",
                "  [b], [2],",
                ")",
                "",
            ]
        else:
            for j in range(1, 6):
                out += [f"Paragraph {j}: $alpha_({i})^({j}) + x/y$", ""]
            for j in range(1, 4):
                out += [f"$ integral_0^{j} e^(-x^2) dif x $", ""]
    return "\n".join(out) + "\n"


GENERATORS = {"mirzam": mirzam_deck, "marp": marp_deck, "typst": typst_deck}
EXTENSIONS = {"mirzam": ".md", "marp": ".md", "typst": ".typ"}



# --------------------------------------------------------------------------
# Output verification. A build that quietly dropped half the deck would be the
# fastest one in the table, so every run is counted before it is believed.
# --------------------------------------------------------------------------

def pdf_pages(path: Path) -> int:
    """Page count, reading through Flate streams so compressed PDFs count too."""
    raw = path.read_bytes()
    blobs = [raw]
    for m in re.finditer(rb"stream\r?\n", raw):
        start = m.end()
        end = raw.find(b"endstream", start)
        if end < 0:
            continue
        try:
            blobs.append(zlib.decompress(raw[start:end]))
        except zlib.error:
            pass
    counts = [int(n) for n in re.findall(rb"/Count\s+(\d+)", b"".join(blobs))]
    return max(counts) if counts else 0


def html_slides(path: Path, tool: str) -> int:
    # Both patterns are deliberately narrow: each deck embeds a viewer script
    # that mentions `<section class="slide">` in a string, and a bare `<section`
    # would count that too.
    text = path.read_text(encoding="utf-8", errors="replace")
    if tool == "mirzam":
        return len(re.findall(r'<section class="slide" data-index=', text))
    return len(re.findall(r"<section id=", text))


def count_output(tool: str, target: str, path: Path) -> int:
    if not path.exists():
        return 0
    return pdf_pages(path) if target == "pdf" else html_slides(path, tool)


# --------------------------------------------------------------------------
# Jobs
# --------------------------------------------------------------------------

def build_jobs(tool: str, src: Path, outdir: Path, mirzam: str, chromium: str | None):
    """Return {target: (argv, output path)} for one tool."""
    jobs = {}
    if tool == "mirzam":
        jobs["html"] = ([mirzam, "build", str(src), "-o", str(outdir / "html")],
                        outdir / "html" / "index.html")
        pdf = outdir / "deck.pdf"
        cmd = [mirzam, "export", "pdf", str(src), "-o", str(pdf)]
        if chromium:
            cmd += ["--chromium", chromium]
        jobs["pdf"] = (cmd, pdf)
    elif tool == "marp":
        html = outdir / "deck.html"
        jobs["html"] = (["marp", "--html", str(src), "-o", str(html)], html)
        pdf = outdir / "deck.pdf"
        jobs["pdf"] = (["marp", "--pdf", str(src), "-o", str(pdf)], pdf)
    elif tool == "typst":
        pdf = outdir / "deck.pdf"
        jobs["pdf"] = (["typst", "compile", str(src), str(pdf)], pdf)
    return jobs


def tool_env(chromium: str | None):
    env = dict(os.environ)
    if chromium:
        env["MIRZAM_CHROMIUM"] = chromium
        env["CHROME_PATH"] = chromium
    # Chromium refuses its sandbox as root, which is how CI containers run.
    env.setdefault("CHROME_NO_SANDBOX", "1")
    return env


def available(tool: str, mirzam: str) -> bool:
    if tool == "mirzam":
        return Path(mirzam).exists() or shutil.which(mirzam) is not None
    return shutil.which(tool) is not None


def run_case(tool, target, slides, profile, reps, workdir, mirzam, chromium, env):
    src = workdir / f"{tool}-{profile}-{slides}{EXTENSIONS[tool]}"
    src.write_text(GENERATORS[tool](slides, profile), encoding="utf-8")
    outdir = workdir / f"out-{tool}-{target}-{profile}-{slides}"
    jobs = build_jobs(tool, src, outdir, mirzam, chromium)
    if target not in jobs:
        return None
    cmd, produced = jobs[target]

    times, mems = [], []
    for rep in range(reps + 1):  # one warm-up, then `reps` measured runs
        if outdir.exists():
            shutil.rmtree(outdir)
        outdir.mkdir(parents=True, exist_ok=True)
        ms, mem, code, err = measure(cmd, cwd=workdir, env=env)
        if ms is None or code != 0:
            return {"tool": tool, "target": target, "slides": slides,
                    "profile": profile, "error": (err or "").strip()[-500:]}
        if rep:
            times.append(ms)
            mems.append(mem)
    size = produced.stat().st_size if produced.exists() else 0
    produced_slides = count_output(tool, target, produced)
    return {
        "tool": tool, "target": target, "slides": slides, "profile": profile,
        "src_bytes": src.stat().st_size, "slides_out": produced_slides,
        "median_ms": statistics.median(times), "min_ms": min(times),
        "max_ms": max(times), "peak_mib": max(mems), "out_bytes": size,
    }


def fmt_ms(v):
    return f"{v:,.0f}" if v >= 100 else f"{v:.1f}"


def fmt_bytes(n):
    if n >= 1024 * 1024:
        return f"{n / 1024 / 1024:.1f} MB"
    if n >= 1024:
        return f"{n / 1024:.0f} KB"
    return f"{n} B"


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--sizes", default="20,120,500")
    ap.add_argument("--profiles", default="plain,math")
    ap.add_argument("--targets", default="html,pdf")
    ap.add_argument("--tools", default="mirzam,marp,typst")
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--mirzam", default="target/release/mirzam")
    ap.add_argument("--chromium", default=os.environ.get("MIRZAM_CHROMIUM"))
    ap.add_argument("--json", help="also write the raw rows here")
    ap.add_argument("--keep", action="store_true", help="keep the generated decks")
    args = ap.parse_args()

    sizes = [int(s) for s in args.sizes.split(",") if s]
    profiles = [p for p in args.profiles.split(",") if p]
    targets = [t for t in args.targets.split(",") if t]
    tools = [t for t in args.tools.split(",") if t]

    mirzam = str(Path(args.mirzam).resolve()) if Path(args.mirzam).exists() else args.mirzam
    env = tool_env(args.chromium)

    usable = [t for t in tools if available(t, mirzam)]
    for t in tools:
        if t not in usable:
            print(f"skipping {t}: not installed", file=sys.stderr)
    if not usable:
        sys.exit("no tool to benchmark")

    workdir = Path(tempfile.mkdtemp(prefix="mirzam-compare-"))
    rows = []
    try:
        for profile in profiles:
            for slides in sizes:
                for target in targets:
                    for tool in usable:
                        row = run_case(tool, target, slides, profile, args.reps,
                                       workdir, mirzam, args.chromium, env)
                        if row is None:
                            continue
                        rows.append(row)
                        if "error" in row:
                            print(f"  {tool:7} {target:4} {profile:5} "
                                  f"{slides:>3}: FAILED {row['error'][:120]}",
                                  file=sys.stderr)
                        else:
                            warn = "" if row["slides_out"] == slides else \
                                f"  <-- produced {row['slides_out']} slides"
                            print(f"  {tool:7} {target:4} {profile:5} {slides:>3}: "
                                  f"{fmt_ms(row['median_ms'])} ms, "
                                  f"{row['peak_mib']:.0f} MiB, "
                                  f"{fmt_bytes(row['out_bytes'])}{warn}", file=sys.stderr)
    finally:
        if not args.keep:
            shutil.rmtree(workdir, ignore_errors=True)
        else:
            print(f"decks kept in {workdir}", file=sys.stderr)

    if args.json:
        Path(args.json).write_text(json.dumps(rows, indent=2), encoding="utf-8")

    for target in targets:
        for profile in profiles:
            subset = [r for r in rows if r["target"] == target and r["profile"] == profile]
            if not subset:
                continue
            print(f"\n### {target.upper()} - {profile} deck\n")
            print("| Slides | " + " | ".join(
                f"{t} (ms)" for t in usable) + " | " + " | ".join(
                f"{t} (MiB)" for t in usable) + " |")
            print("|---" * (1 + 2 * len(usable)) + "|")
            for slides in sizes:
                cells, mems = [], []
                for tool in usable:
                    r = next((x for x in subset
                              if x["tool"] == tool and x["slides"] == slides), None)
                    if r is None:
                        cells.append("-")
                        mems.append("-")
                    elif "error" in r:
                        cells.append("fail")
                        mems.append("fail")
                    else:
                        cells.append(fmt_ms(r["median_ms"]))
                        mems.append(f"{r['peak_mib']:.0f}")
                print(f"| {slides} | " + " | ".join(cells) + " | " + " | ".join(mems) + " |")


if __name__ == "__main__":
    main()
