# Performance: eight releases, and the field — August 2026

Two questions, one machine, one afternoon.

1. **Has anything got slower since `v0.1.0`?** Eight releases added syntax
   highlighting, bibliographies, Mermaid, an overview grid, embedded widgets and
   figure credits. Any of them could have cost something.
2. **Where does Mirzam stand against the tools it is compared with?** Marp is
   the Markdown competitor; Typst with Touying is the PDF one. `docs/reports/2026-08-market-survey.md`
   claims "500 slides in 76 ms" as a Mirzam advantage. Nobody had measured the
   other two on the same hardware.

Both are reproducible: `scripts/bench-history.sh` and `scripts/bench-compare.py`.

## The bench

| | |
|---|---|
| Machine | Intel Xeon @ 2.10 GHz, 4 cores, 16 GB, Linux 6.18 container |
| Mirzam | working tree at `0.8.0`, `--release` (`lto = "thin"`, `codegen-units = 1`) |
| Marp | `@marp-team/marp-cli` 4.5.0 with `marp-core` 4.4.0, Node 22.22 |
| Typst | `typst` 0.15.1 with Touying 0.7.1, `themes.simple` |
| Browser | Chromium 141.0.7390.37, headless — Mirzam and Marp both drive it for PDF |

Every figure below is wall time from a cold process, one warm-up run discarded,
and the median (cross-tool) or the fastest (history) of the runs that follow.
Peak RSS is the maximum resident set of the command and everything it spawned,
so the browser counts against the tool that started it.

**Every output is counted before it is believed.** A build that quietly dropped
half a deck would otherwise be the fastest row in the table. The harness counts
slides in the HTML and pages in the PDF, and says so when the number is not the
number of slides it asked for. This caught a real distortion: Touying spills a
slide that does not fit onto a second page, so the first math run had Typst
rendering 1000 pages against the other two tools' 500 — and losing on time for
a reason that had nothing to do with speed. The math slide was shortened to
five formulas, which fits everywhere (`mirzam check` leaves the tightest pane
98 px clear), and the numbers below are one page per slide in all three tools.

## 1. Nothing that scales with the deck got slower

`crates/mirzam-cli/src/bin/mirzam-bench.rs` has not changed a byte since
`v0.1.0`, so each tag's own copy measures the same four decks. Fastest of five
runs each, all on this machine:

### Full build (ms)

| Version | 20 slides | 120 slides | 500 slides | 100 slides, 800 formulas |
|---|---:|---:|---:|---:|
| v0.1.0 | 4.5 | 19.1 | 76.6 | 25.0 |
| v0.2.0 | 4.5 | 18.2 | 73.6 | 24.0 |
| v0.3.0 | 5.0 | 18.9 | 76.4 | 25.4 |
| v0.4.0 | 5.1 | 19.1 | 76.4 | 25.2 |
| v0.5.0 | 7.0 | 20.2 | 81.9 | 27.2 |
| v0.6.0 | 7.0 | 19.2 | 80.0 | 26.9 |
| v0.7.0 | 7.3 | 20.6 | 82.5 | 28.7 |
| v0.8.0 | 7.4 | 20.3 | 80.3 | 27.8 |
| working tree | 7.4 | 20.3 | 79.9 | 28.4 |

### Single-slide edit (ms)

| Version | 20 slides | 120 slides | 500 slides | 100 slides, 800 formulas |
|---|---:|---:|---:|---:|
| v0.1.0 | 0.3 | 0.9 | 3.1 | 1.2 |
| v0.2.0 | 0.4 | 0.9 | 3.0 | 1.2 |
| v0.3.0 | 0.4 | 1.0 | 3.1 | 1.2 |
| v0.4.0 | 0.4 | 0.9 | 3.1 | 1.2 |
| v0.5.0 | 0.4 | 1.0 | 3.2 | 1.2 |
| v0.6.0 | 0.4 | 1.0 | 3.2 | 1.2 |
| v0.7.0 | 0.7 | 1.3 | 3.5 | 1.5 |
| v0.8.0 | 0.7 | 1.2 | 3.6 | 1.5 |
| working tree | 0.7 | 1.3 | 3.5 | 1.5 |

Read the columns, not the rows. Eight releases cost **1 to 3 ms on a full build
and 0.3 ms on an edit, and the amount does not depend on how big the deck is**.
A 20-slide build got 64% slower and a 500-slide build got 4% slower, which is
one fact stated twice: the cost is per *build*, not per *slide*. The slope
confirms it — fit the 20- and 120-slide columns and `v0.1.0` renders a slide in
0.146 ms against the working tree's 0.129, so the part that scales with the
deck is if anything slightly faster than it was.

The design goal was that **edit latency does not grow with deck size**, and it
still holds: 25× the slides costs 5× the edit (0.7 ms against 3.5 ms).

Two steps are visible, and each lands where a release added a whole-build pass:

- **v0.4.0 → v0.5.0, +2 ms on a full build.** That release added build-time
  syntax highlighting and bibliographies.
- **v0.6.0 → v0.7.0, +0.3 ms on an edit.** That release added Mermaid rendering
  and column balancing.

Neither is a defect to fix; both are a fixed cost that a feature bought. They
are recorded here so that the next step is noticed while it is still small.

## 2. Against Marp and Touying

The three tools do not produce the same artefact, and the comparison has to
respect that. Mirzam and Marp both build a self-contained HTML deck; Typst
cannot, and Touying closed its HTML issue as *not planned*. All three produce a
PDF, but Mirzam and Marp get there by driving Chromium while Typst writes it
directly — which is most of what the PDF table below is measuring.

### HTML deck, plain content

| Slides | Mirzam | Marp | Mirzam peak | Marp peak |
|---:|---:|---:|---:|---:|
| 20 | 9.5 ms | 526 ms | 10 MiB | 125 MiB |
| 120 | 23.2 ms | 520 ms | 10 MiB | 136 MiB |
| 500 | 82.3 ms | 702 ms | 10 MiB | 155 MiB |

### HTML deck, five formulas per slide

| Slides | Mirzam | Marp | Mirzam peak | Marp peak |
|---:|---:|---:|---:|---:|
| 20 | 11.8 ms | 544 ms | 10 MiB | 136 MiB |
| 120 | 31.8 ms | 674 ms | 11 MiB | 147 MiB |
| 500 | 108 ms | 947 ms | 13 MiB | 187 MiB |

Marp's floor is Node: half a second before any Markdown is read, which is why
20 slides and 120 slides cost it the same. Above that floor the two are closer
than the ratio suggests — Marp adds ~180 ms going from 20 to 500 plain slides,
Mirzam ~73 ms. The gap that does not close is memory: **Mirzam builds a
500-slide deck in 10 MiB, Marp needs 155 MiB**, and 187 MiB once there is maths
on the slides. That is the number behind the survey's "Slidev OOMs on large
decks" note, and it is a factor of 15.

### PDF, plain content

| Slides | Mirzam | Marp | Typst | Mirzam peak | Marp peak | Typst peak |
|---:|---:|---:|---:|---:|---:|---:|
| 20 | 510 ms | 1,640 ms | **232 ms** | 198 MiB | 182 MiB | 66 MiB |
| 120 | **812 ms** | 2,051 ms | 1,293 ms | 215 MiB | 201 MiB | 267 MiB |
| 500 | **2,510 ms** | 3,910 ms | 8,542 ms | 272 MiB | 253 MiB | 2,191 MiB |

### PDF, five formulas per slide

| Slides | Mirzam | Marp | Typst | Mirzam peak | Marp peak | Typst peak |
|---:|---:|---:|---:|---:|---:|---:|
| 20 | 579 ms | 2,903 ms | **216 ms** | 200 MiB | 188 MiB | 60 MiB |
| 120 | **1,251 ms** | 5,781 ms | 1,123 ms | 233 MiB | 232 MiB | 243 MiB |
| 500 | **4,108 ms** | 23,625 ms | 7,585 ms | 353 MiB | 389 MiB | 2,123 MiB |

Three readings:

- **Typst wins the short deck and loses the long one.** No browser to start, so
  20 slides is a fifth of a second — nothing else is close. But the curve bends
  the wrong way: 25× the slides costs it 37× the time and **2.1 GB of resident
  memory**, against Mirzam's 272 MiB. Touying holds the whole document and its
  incremental cache in memory; a 500-slide course is where that stops being
  free.
- **Mirzam's browser costs it the short deck and pays for itself by 120.** Half
  a second of that 510 ms is Chromium starting; the remaining per-slide cost is
  the lowest of the three.
- **Maths is where Marp's Chromium work shows.** KaTeX renders every formula in
  the page, and a 500-slide deck takes **23.6 seconds** against Mirzam's 4.1.

### What each deck weighs

| Deck | Mirzam | Marp | Typst |
|---|---:|---:|---:|
| 500 slides, plain, HTML | 441 KB | 390 KB | — |
| 500 slides, maths, HTML | 1.2 MB | 6.3 MB | — |
| 500 slides, plain, PDF | 3.6 MB | 1.5 MB | 2.2 MB |
| 500 slides, maths, PDF | 6.1 MB | 537 KB | 1.6 MB |

Plain HTML is a wash. With maths, Marp's page is **five times heavier** —
KaTeX writes a tree of spans per formula where Mirzam writes MathML. The PDF
column runs the other way and is not a quality measure: how much font gets
embedded differs per renderer, and a smaller PDF here is not a better one.

## 3. The sample decks, for scale

The working-tree binary, `mirzam build`, fastest of four runs each:

| Deck | Slides | Build | Source | HTML |
|---|---:|---:|---:|---:|
| 01-start | 6 | 13 ms | 5.4 KB | 151 KB |
| 02-writing | 11 | 17 ms | 10.0 KB | 701 KB |
| 03-layout | 11 | 16 ms | 11.1 KB | 399 KB |
| 04-components | 24 | 22 ms | 25.8 KB | 1.4 MB |
| 05-motion | 11 | 14 ms | 12.2 KB | 475 KB |
| 06-theming | 16 | 24 ms | 19.3 KB | 747 KB |
| pitch | 9 | 10 ms | 8.2 KB | 233 KB |
| research | 9 | 11 ms | 7.8 KB | 698 KB |
| seminar | 12 | 12 ms | 10.7 KB | 805 KB |
| slideshow | 5 | 10 ms | 3.8 KB | 424 KB |

A real deck — charts, shapes, connectors, video, a Mermaid fence, a widget — is
10 to 24 ms end to end, process start included. Marp's floor alone is twenty
times that.

## What these numbers do not say

- **They are not a verdict on the tools.** Typst's PDF is tagged and
  accessible; Mirzam's is a browser print. Marp's themes are a CSS contract
  people already know. Speed is one axis.
- **The generated decks are simple on purpose.** Headings, prose, a list, a
  table, some maths — the parts all three express natively. Nothing here
  measures charts, connectors, transclusion or embedded media, because two of
  the three tools have no equivalent to measure against.
- **Only one-shot builds were measured.** All three have a watch mode, and both
  Typst and Mirzam rebuild incrementally inside it; Mirzam's incremental path
  is measured in section 1, and Typst's is not measured here at all. A
  comparison of the two edit loops is the obvious next report.
- **Fonts were missing.** This container has none of the families the sample
  decks name, so line breaks — and therefore layout work — differ from a
  machine that has them.
- **Four cores, shared.** The absolute milliseconds belong to this container.
  The ratios are what travel.

## Reproducing

```bash
cargo build --release --bin mirzam
export MIRZAM_CHROMIUM=/path/to/chrome        # also CHROME_PATH, for Marp

./scripts/bench-history.sh                    # section 1
python3 scripts/bench-compare.py \
  --sizes 20,120,500 --profiles plain,math --targets html,pdf --reps 3
```

Marp comes from `npm install -g @marp-team/marp-cli`; Typst from `cargo install
typst-cli`, with Touying unpacked into `~/.cache/typst/packages/preview/touying/0.7.1`
if `packages.typst.org` is unreachable. `bench-compare.py` skips a tool it
cannot find rather than guessing at its numbers.
