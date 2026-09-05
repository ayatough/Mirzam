#!/usr/bin/env python3
"""Context-cost benchmark: `mirzam check --format json` against screenshots.

An agent fixing a deck's layout runs a feedback loop: change the deck, look at
the result, fix the next problem. This script quantifies what one loop costs in
LLM context tokens under two feedback channels: reading the JSON document that
`mirzam check --format json` prints, or inspecting one screenshot per slide.
The JSON side is measured for real - synthetic decks with seeded defects are
checked at every "k defects remaining" state, defects down to zero, and the
stdout payload is what an agent would read each round. The screenshot side is
modelled arithmetically with the Claude vision formula (an image costs
ceil(w/28) * ceil(h/28) tokens after downscaling to the tier's long-edge and
token caps); no screenshot is actually taken.

  MIRZAM_CHROMIUM=/path/to/chromium python3 scripts/bench-agent-context.py --sizes 10,30,100

Text token counts are ESTIMATES: tokens = ceil(bytes / 4). The exact count is
tokenizer-specific and this script calls no API. Image token counts are exact
under the published formula for the two tiers modelled (standard: 1568 px long
edge, 1568 tokens; high-resolution: 2576 px, 4784 tokens).

Needs a mirzam binary (target/release/mirzam, target/debug/mirzam, or --mirzam)
and a browser for the layout pass (MIRZAM_CHROMIUM if none is on PATH).
"""

from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# --------------------------------------------------------------------------
# Defect recipes. Each returns the slide body at its defective or fixed state.
# The defective forms are verified to produce these diagnostics with mirzam
# 0.10.0: clipped -> layout.clipped + layout.overlap errors, connector ->
# build.connect warning + layout.connector error, annotate -> build.annotate
# warning, shape -> build.shape warning. The fixed forms produce none.
# --------------------------------------------------------------------------

PANE_GRID = """```pane
+------------------+
|  head            |
+------------------+
|                  |
|  main            |
|                  |
+------------------+
```"""


def clipped(n: int, fixed: bool) -> str:
    if fixed:
        head = "# Short heading"
    else:
        head = ("# A very long heading that keeps going and going far beyond "
                "what a single band row can hold on one line of layout\n"
                "Three or four full sentences of extra prose here so the band "
                "overflows by a comfortable margin and the checker reports it.")
    return (f"{PANE_GRID}\n\n::: pane head\n{head}\n:::\n\n"
            "::: pane main\nBody text.\n:::")


def connector(n: int, fixed: bool) -> str:
    if fixed:
        return (f"The [session cache]{{#cache-label-{n}}} sits in front of the "
                f"[object store]{{#store-label-{n}}}.\n\n"
                f"```connect\n#cache-label-{n} -> #store-label-{n}\n```")
    return (f"The [session cache]{{#cache-label-{n}}} sits in front of the "
            "database.\n\n"
            f"```connect\n#cache-label-{n} -> #storelabel-{n}\n```")


def annotation(n: int, fixed: bool) -> str:
    target = f"#anchor-{n}" if fixed else "#does-not-exist"
    return (f"## Anchor slide {n}\n\n"
            f"A [key phrase]{{#anchor-{n}}} worth marking.\n\n"
            f"```annotate\ncircle {target} : pad=6\n```")


def shape(n: int, fixed: bool) -> str:
    kind = "rect" if fixed else "boxx"
    return (f"## Shape slide {n}\n\n"
            f"```shape\n{kind} #tier-{n} at(30%, 40%) size(36%, 20%) "
            'label="Cache tier"\n```')


# clipped and connector first, so the sanity assertion below always has both.
RECIPES = [clipped, connector, annotation, shape]


def clean_slide(i: int) -> str:
    return (f"## Section {i}\n\n"
            f"Body text for slide {i} with **emphasis** and `code`, a sentence "
            "of prose that says something plausible about the section.\n\n"
            "- First point\n- Second point\n- Third point")


def make_deck(slides: int, defects: int, fixed_count: int) -> str:
    """The deck with the first `fixed_count` of `defects` seeded sites fixed."""
    # Spread defect sites evenly across the deck, cycling through the recipes.
    sites = {round((j + 1) * slides / (defects + 1)): j for j in range(defects)}
    if len(sites) != defects:
        sys.exit(f"cannot spread {defects} defect sites over {slides} slides")
    bodies = []
    for i in range(1, slides + 1):
        if i - 1 in sites:
            j = sites[i - 1]
            bodies.append(RECIPES[j % len(RECIPES)](j, fixed=j < fixed_count))
        else:
            bodies.append(clean_slide(i))
    front = "---\ntitle: Agent context benchmark\n---\n\n"
    return front + "\n\n---\n\n".join(bodies) + "\n"


# --------------------------------------------------------------------------
# The JSON side: run the real checker and weigh its stdout.
# --------------------------------------------------------------------------

def run_check(mirzam: str, deck: Path, env: dict) -> tuple[bytes, dict]:
    """Run `mirzam check --format json`; a non-zero exit is a finding, not a
    failure - the checker exits 1 when layout errors exist."""
    proc = subprocess.run([mirzam, "check", str(deck), "--format", "json"],
                         capture_output=True, env=env)
    try:
        doc = json.loads(proc.stdout)
    except json.JSONDecodeError:
        sys.exit(f"mirzam check produced no JSON for {deck}:\n"
                 + proc.stderr.decode("utf-8", "replace")[-2000:])
    if doc.get("schema") != "mirzam-check":
        sys.exit(f"unexpected schema in checker output: {doc.get('schema')!r}")
    return proc.stdout, doc


def text_tokens(nbytes: int) -> int:
    """ESTIMATE: 4 bytes per token. Model-specific; no API is called."""
    return math.ceil(nbytes / 4)


# --------------------------------------------------------------------------
# The screenshot side: the Claude vision formula, computed, not photographed.
# An image costs ceil(w/28) * ceil(h/28) tokens; before that it is downscaled
# (aspect preserved) to the largest size fitting the tier's long-edge cap and
# token cap.
# --------------------------------------------------------------------------

TIERS = {"standard": (1568, 1568), "high-res": (2576, 4784)}
VIEWPORTS = [(1440, 810), (1920, 1080)]


def grid_tokens(w: float, h: float) -> int:
    return math.ceil(w / 28) * math.ceil(h / 28)


def image_tokens(w: int, h: int, tier: str) -> int:
    max_edge, max_tokens = TIERS[tier]
    scale = min(1.0, max_edge / max(w, h))
    sw, sh = w * scale, h * scale
    if grid_tokens(sw, sh) <= max_tokens:
        return grid_tokens(sw, sh)
    # Over the token cap: find the largest width (aspect preserved) that fits.
    for width in range(int(sw), 0, -1):
        if grid_tokens(width, width * h / w) <= max_tokens:
            return grid_tokens(width, width * h / w)
    return 1


# --------------------------------------------------------------------------
# The benchmark proper.
# --------------------------------------------------------------------------

def bench_size(slides: int, defects: int, mirzam: str, workdir: Path,
               env: dict) -> dict:
    rounds = []
    for fixed_count in range(defects + 1):
        remaining = defects - fixed_count
        deck = workdir / f"deck-{slides}-k{remaining}.md"
        deck.write_text(make_deck(slides, defects, fixed_count), encoding="utf-8")
        payload, doc = run_check(mirzam, deck, env)
        kinds = [d["kind"] for d in doc["diagnostics"]]
        if remaining == defects:
            # Sanity: the seeded deck must actually be broken the way we claim.
            for must in ("layout.clipped", "layout.connector"):
                if must not in kinds:
                    sys.exit(f"generated deck ({slides} slides, {defects} defects) "
                             f"did not produce a {must} diagnostic; the recipes "
                             "no longer reproduce - fix the generator before "
                             "trusting any number here")
        if doc["slides"] != slides:
            sys.exit(f"deck rendered {doc['slides']} slides, expected {slides}")
        rounds.append({
            "defects_remaining": remaining,
            "json_bytes": len(payload),
            "json_tokens_est": text_tokens(len(payload)),
            "diagnostics": len(kinds),
            "kinds": sorted(set(kinds)),
        })
        print(f"  {slides:>4} slides, {remaining} defect(s) left: "
              f"{len(payload)} B json, {len(kinds)} diagnostics", file=sys.stderr)

    nrounds = len(rounds)  # defects + 1: one review round per fix, plus the clean pass
    json_total = sum(r["json_tokens_est"] for r in rounds)
    shots = {}
    for w, h in VIEWPORTS:
        for tier in TIERS:
            per_image = image_tokens(w, h, tier)
            shots[f"{w}x{h}/{tier}"] = {
                "per_image_tokens": per_image,
                "per_round_full_sweep": per_image * slides,
                "loop_full_sweep": per_image * slides * nrounds,
                # Cheaper policy: full sweep once, then only the changed slide.
                "loop_changed_only": per_image * (slides + (nrounds - 1)),
            }
    return {
        "slides": slides, "defects": defects, "rounds": nrounds,
        "json_tokens_per_round_avg": round(json_total / nrounds, 1),
        "json_tokens_loop_total": json_total,
        "per_round": rounds, "screenshots": shots,
    }


def print_report(results: list[dict], out) -> None:
    def p(line=""):
        print(line, file=out)

    p("### Context tokens per review round (full-sweep screenshots)")
    p()
    p("Text tokens are an estimate (ceil(bytes/4), tokenizer-specific); image")
    p("tokens are exact under the Claude vision formula for each tier.")
    p()
    keys = [f"{w}x{h}/{t}" for w, h in VIEWPORTS for t in TIERS]
    p("| Slides | json tokens/round (avg) | " +
      " | ".join(f"shots {k}" for k in keys) + " | ratio (1440x810/standard : json) |")
    p("|---" * (3 + len(keys)) + "|")
    for r in results:
        cells = [str(r["screenshots"][k]["per_round_full_sweep"]) for k in keys]
        ratio = r["screenshots"]["1440x810/standard"]["per_round_full_sweep"] \
            / r["json_tokens_per_round_avg"]
        p(f"| {r['slides']} | {r['json_tokens_per_round_avg']} | "
          + " | ".join(cells) + f" | {ratio:,.0f}x |")
    p()
    p(f"### Totals per converged loop ({results[0]['rounds']} rounds: "
      "one per fix, plus the clean pass)")
    p()
    p("| Slides | json loop | " +
      " | ".join(f"shots {k} full" for k in keys) + " | " +
      " | ".join(f"shots {k} changed-only" for k in keys) + " |")
    p("|---" * (2 + 2 * len(keys)) + "|")
    for r in results:
        full = [f"{r['screenshots'][k]['loop_full_sweep']:,}" for k in keys]
        chg = [f"{r['screenshots'][k]['loop_changed_only']:,}" for k in keys]
        p(f"| {r['slides']} | {r['json_tokens_loop_total']:,} | "
          + " | ".join(full) + " | " + " | ".join(chg) + " |")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--sizes", default="10,30,100",
                    help="comma-separated deck sizes in slides")
    ap.add_argument("--defects", type=int, default=5,
                    help="seeded defect sites per deck (min 2: the sanity check "
                         "needs one clipped band and one dangling connector)")
    ap.add_argument("--mirzam", default=None,
                    help="mirzam binary (default: target/release/mirzam, then "
                         "target/debug/mirzam)")
    ap.add_argument("--out", default="bench-agent-context-out",
                    help="directory for results.md and results.json")
    ap.add_argument("--keep", action="store_true", help="keep the generated decks")
    args = ap.parse_args()

    sizes = [int(s) for s in args.sizes.split(",") if s]
    if args.defects < 2:
        sys.exit("--defects must be at least 2 (see --help)")
    candidates = [args.mirzam] if args.mirzam else \
        ["target/release/mirzam", "target/debug/mirzam"]
    mirzam = next((c for c in candidates
                   if c and (Path(c).exists() or shutil.which(c))), None)
    if mirzam is None:
        sys.exit("no mirzam binary found; build one (cargo build --release "
                 "--bin mirzam) or pass --mirzam")
    mirzam = str(Path(mirzam).resolve()) if Path(mirzam).exists() else mirzam

    env = dict(os.environ)  # MIRZAM_CHROMIUM passes through when set
    env.setdefault("CHROME_NO_SANDBOX", "1")

    workdir = Path(tempfile.mkdtemp(prefix="mirzam-agent-context-"))
    try:
        results = [bench_size(n, args.defects, mirzam, workdir, env)
                   for n in sizes]
    finally:
        if args.keep:
            print(f"decks kept in {workdir}", file=sys.stderr)
        else:
            shutil.rmtree(workdir, ignore_errors=True)

    outdir = Path(args.out)
    outdir.mkdir(parents=True, exist_ok=True)
    doc = {
        "note": "json token counts are estimates: ceil(bytes/4); "
                "the exact count is tokenizer-specific and no API was called. "
                "Image tokens follow ceil(w/28)*ceil(h/28) after downscaling "
                "to the tier's long-edge and token caps.",
        "mirzam": mirzam, "defects": args.defects,
        "vision_tiers": {k: {"max_long_edge_px": v[0], "max_tokens": v[1]}
                         for k, v in TIERS.items()},
        "results": results,
    }
    (outdir / "results.json").write_text(json.dumps(doc, indent=2) + "\n",
                                         encoding="utf-8")
    with (outdir / "results.md").open("w", encoding="utf-8") as f:
        print_report(results, f)
    print()
    print_report(results, sys.stdout)
    print(f"\nresults written to {outdir / 'results.md'} and "
          f"{outdir / 'results.json'}")


if __name__ == "__main__":
    main()
