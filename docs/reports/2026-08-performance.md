# Performance: eight releases, and the field — August 2026

Two questions, one machine, one afternoon. **Has anything got slower since
`v0.1.0`?** — eight releases added syntax highlighting, bibliographies, Mermaid,
an overview grid, embedded widgets and figure credits, and any of them could
have cost something. And **where does Mirzam stand against the tools it is
compared with?** — the market survey claims "500 slides in 76 ms" as a position
nobody can follow quickly, and nobody had measured the other two on the same
hardware.

Both are reproducible: `scripts/bench-history.sh` and `scripts/bench-compare.py`.

## The bench

| | |
|---|---|
| Machine | Intel Xeon @ 2.10 GHz, 4 cores, 16 GB, Linux 6.18 container |
| Mirzam | working tree at `0.8.0`, `--release` (`lto = "thin"`, `codegen-units = 1`) |
| Marp | `@marp-team/marp-cli` 4.5.0 with `marp-core` 4.4.0, Node 22.22 |
| Typst | `typst` 0.15.1 with Touying 0.7.1, `themes.simple` |
| Browser | Chromium 141, headless — Mirzam and Marp both drive it for PDF |

Wall time from a cold process, one warm-up discarded, then the median
(cross-tool) or the fastest (history) of the runs that follow. Peak RSS counts
the command and everything it spawned, so a browser counts against the tool
that started it.

## Every output is counted before it is believed

A build that quietly dropped half a deck would otherwise be the fastest row in
the table. The harness counts slides in the HTML and pages in the PDF, and says
so when the number is not the number of slides it asked for.

This caught a real distortion. **Touying spills a slide that does not fit onto a
second page**, so the first math run had Typst rendering 1000 pages against the
other two tools' 500 — and losing on time for a reason that had nothing to do
with speed. The math slide was shortened to five formulas, which fits
everywhere (`mirzam check` leaves the tightest pane 98 px clear), and every
number below is one page per slide in all three tools.

## Eight releases: full build

`crates/mirzam-cli/src/bin/mirzam-bench.rs` has not changed a byte since
`v0.1.0`, so each tag's own copy measures the same four decks. Fastest of five
runs each, all on this machine, in milliseconds:

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

## Eight releases: single-slide edit

The number the design goal is about — what an editor waits for after a
keystroke, in milliseconds:

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

## What the two tables say

Read the columns, not the rows. Eight releases cost **1 to 3 ms on a full build
and 0.3 ms on an edit, and the amount does not depend on how big the deck is**.
A 20-slide build got 64% slower and a 500-slide build got 4% slower, which is
one fact stated twice: the cost is per *build*, not per *slide*. Fit the 20- and
120-slide columns and `v0.1.0` renders a slide in 0.146 ms against the working
tree's 0.129 — the part that scales with the deck is if anything faster.

The design goal was that **edit latency does not grow with deck size**, and it
holds: 25× the slides costs 5× the edit. Two steps are visible, and each lands
where a release added a whole-build pass — **v0.5.0**, which added build-time
syntax highlighting and bibliographies, and **v0.7.0**, which added Mermaid and
column balancing. Neither is a defect; both are a fixed cost a feature bought,
recorded here so the next step is noticed while it is still small.

## The thing that did grow

The smallest deck there is — a title, one heading, a sentence, two bullets —
built by each version's own binary. `mirzam build`, fastest of five, process
start included:

| Version | One-slide build | `index.html` |
|---|---:|---:|
| v0.1.0 | 6.1 ms | 55 KB |
| v0.3.0 | 6.2 ms | 69 KB |
| v0.5.0 | 9.3 ms | 76 KB |
| v0.6.0 | 9.7 ms | 92 KB |
| v0.7.0 | 10.0 ms | 106 KB |
| v0.8.0 | 9.7 ms | 134 KB |
| working tree | 10.0 ms | 144 KB |

The time column is the fixed cost above seen from outside. The other column was
the finding: **a deck that said almost nothing had gone from 55 KB to 144 KB**,
and every release had added to it. That was the viewer and the shared stylesheet
— `theme/viewer.js` at 56 KB and `theme/base.css` at 68 KB, shipped whole and
commented, inside every deck built.

Half of that is prose explaining the code to whoever maintains it, and every
reader of every deck was downloading it. It is now stripped at compile time, so
**the same deck is 78 KB**: the sources keep their comments, the shipped copies
keep their line numbering, and nothing else about a deck changes. Every table
below that mentions a size is measured after that change.

## Where the bytes are

The sample decks, by what is inside the HTML:

| Deck | Total | CSS | Viewer JS | `data:` URIs |
|---|---:|---:|---:|---|
| 01-start | 82 KB | 37 KB | 36 KB | — |
| 03-layout | 320 KB | 37 KB | 47 KB | 206 KB JPEG |
| research | 617 KB | 563 KB | 36 KB | **525 KB woff2** |
| seminar | 711 KB | 563 KB | 48 KB | 525 KB woff2, 76 KB PNG |
| 04-components | 1,246 KB | 563 KB | 61 KB | 525 KB woff2, 175 KB JPEG, 113 KB PNG |

## The three levers, and what happened to them

**The comments went, and that was the big one.** 52% of `base.css` and 43% of
`viewer.js` was prose. Stripping it at compile time takes **66 KB off every
deck** — the smallest goes 144 KB → 78 KB, `01-start` 151 → 85, `pitch` 233 →
156 — with no runtime cost and no change to the sources.

**The maths font stays whole, and the reason is worth writing down.** It is
525 KB of base64 and 77% of `research.md`, and the sample decks use 40
codepoints of its 4,605. Subsetting to those 40 breaks the maths, silently:

- the radical vanishes, because `<msqrt>` draws U+221A and no source text
  contains it;
- letters change face, because `<mi>a</mi>` is drawn from the mathematical
  italic block (U+1D44E), not from the `a` in the markup;
- delimiters stop stretching unless the subsetter closes over the MATH table's
  variant glyphs — `fontcull`/klippa does not, and Chromium rejected its output
  outright, falling back for everything.

The layout checker passes all three; only looking at the pixels catches them.
Doing this properly means computing the implied codepoints, closing over the
MATH variants, and gating it on a rendering comparison — and, because no pure
Rust path re-compresses WOFF2, carrying the font decompressed: 403 KB → 1.06 MB
in the binary and the WebAssembly bundle. Worth doing deliberately; not worth
doing quickly.

**Images are still embedded at source resolution.** A 206 KB photograph on a
1920-px slide is untouched.

## Where the time goes

One deck per feature, 100 slides each, fastest of seven. The empty deck is the
floor everything else is measured against:

| 100 slides of | Build | Over empty | HTML |
|---|---:|---:|---:|
| nothing | 20 ms | — | 100 KB |
| prose and a list | 27 ms | +7 ms | 115 KB |
| a table | 25 ms | +5 ms | 124 KB |
| 300 formulas | 32 ms | +12 ms | 678 KB |
| one formula, in the whole deck | 23 ms | +3 ms | **628 KB** |
| 100 code fences | **66 ms** | **+46 ms** | 170 KB |
| one code fence, in the whole deck | 26 ms | +6 ms | 103 KB |
| 100 charts | 29 ms | +9 ms | 308 KB |
| 300 shapes | 23 ms | +3 ms | 180 KB |

## What costs what

**Syntax highlighting is the expensive thing, and maths is not.** A code fence
costs about **0.40 ms**; a formula costs **0.03 ms**, a thirteenth of it. A
chart is 0.09 ms and a shape is free. Highlighting also charges about 3 ms the
first time a deck uses it — which is exactly the step the release history shows
between `v0.4.0` and `v0.5.0`, the release that added it. A correlation named
earlier in this report, measured here.

**Maths is the expensive thing by weight, and one formula costs as much as
three hundred.** The deck with a single formula in it builds in the time prose
does and weighs 628 KB, because the font goes in whole the moment anything on
any slide is maths. That is the same 525 KB the levers section declines to
subset today, seen from the other side.

So: if a build ever feels slow, the place to look is the highlighter. If a deck
is ever too heavy to send, the place to look is the font — and after that, the
photographs.

## Against Marp and Touying

The three tools do not produce the same artefact, and the comparison has to
respect that. Mirzam and Marp both build a self-contained HTML deck; Typst
cannot, and Touying closed its HTML issue as *not planned*. All three produce a
PDF, but Mirzam and Marp get there by driving Chromium while Typst writes it
directly — which is most of what the PDF tables measure.

## HTML deck, Mirzam against Marp

Plain content on the left of each pair, five formulas per slide on the right:

| Slides | Mirzam | Marp | Mirzam maths | Marp maths |
|---:|---:|---:|---:|---:|
| 20 | 9.5 ms | 526 ms | 11.8 ms | 544 ms |
| 120 | 23.2 ms | 520 ms | 31.8 ms | 674 ms |
| 500 | 82.3 ms | 702 ms | 108 ms | 947 ms |

Peak memory, same runs:

| Slides | Mirzam | Marp | Mirzam maths | Marp maths |
|---:|---:|---:|---:|---:|
| 20 | 10 MiB | 125 MiB | 10 MiB | 136 MiB |
| 120 | 10 MiB | 136 MiB | 11 MiB | 147 MiB |
| 500 | 10 MiB | 155 MiB | 13 MiB | 187 MiB |

## What the HTML numbers say

Marp's floor is Node: half a second before any Markdown is read, which is why
20 slides and 120 slides cost it the same. Above that floor the two are closer
than the ratio suggests — Marp adds ~176 ms going from 20 to 500 plain slides,
Mirzam ~73 ms.

The gap that does not close is memory. **Mirzam builds a 500-slide deck in
10 MiB; Marp needs 155 MiB**, and 187 MiB once there is maths on the slides.
That is the number behind the survey's "Slidev OOMs on large decks" note, and
it is a factor of 15.

## PDF, plain content

| Slides | Mirzam | Marp | Typst | Mirzam peak | Marp peak | Typst peak |
|---:|---:|---:|---:|---:|---:|---:|
| 20 | 510 ms | 1,640 ms | **232 ms** | 198 MiB | 182 MiB | 66 MiB |
| 120 | **812 ms** | 2,051 ms | 1,293 ms | 215 MiB | 201 MiB | 267 MiB |
| 500 | **2,510 ms** | 3,910 ms | 8,542 ms | 272 MiB | 253 MiB | 2,191 MiB |

## PDF, five formulas per slide

| Slides | Mirzam | Marp | Typst | Mirzam peak | Marp peak | Typst peak |
|---:|---:|---:|---:|---:|---:|---:|
| 20 | 579 ms | 2,903 ms | **216 ms** | 200 MiB | 188 MiB | 60 MiB |
| 120 | **1,251 ms** | 5,781 ms | 1,123 ms | 233 MiB | 232 MiB | 243 MiB |
| 500 | **4,108 ms** | 23,625 ms | 7,585 ms | 353 MiB | 389 MiB | 2,123 MiB |

## What the PDF numbers say

**Typst wins the short deck and loses the long one.** No browser to start, so
20 slides is a fifth of a second and nothing else is close. But the curve bends
the wrong way: 25× the slides costs it 37× the time and **2.1 GB of resident
memory**, against Mirzam's 272 MiB. Touying holds the whole document and its
incremental cache in memory, and a 500-slide course is where that stops being
free.

**Mirzam's browser costs it the short deck and pays for itself by 120** — half
of that first 510 ms is Chromium starting, and the per-slide cost that remains
is the lowest of the three. **Maths is where Marp's browser work shows**: KaTeX
renders every formula in the page, and 500 slides take 23.6 seconds against
Mirzam's 4.1.

## What each deck weighs

| Deck | Mirzam | Marp | Typst |
|---|---:|---:|---:|
| 500 slides, plain, HTML | 441 KB | 390 KB | — |
| 500 slides, maths, HTML | 1.2 MB | 6.3 MB | — |
| 500 slides, plain, PDF | 3.6 MB | 1.5 MB | 2.2 MB |
| 500 slides, maths, PDF | 6.1 MB | 537 KB | 1.6 MB |

Plain HTML is a wash. With maths, Marp's page is **five times heavier** — KaTeX
writes a tree of spans per formula where Mirzam writes MathML. The PDF column
runs the other way and is not a quality measure: how much font gets embedded
differs per renderer, and a smaller PDF here is not a better one.

## The sample decks, for scale

The working-tree binary, `mirzam build`, fastest of four runs each:

| Deck | Slides | Build | Source | HTML |
|---|---:|---:|---:|---:|
| 01-start | 6 | 13 ms | 5.4 KB | 85 KB |
| 02-writing | 11 | 17 ms | 10.0 KB | 633 KB |
| 03-layout | 11 | 16 ms | 11.1 KB | 328 KB |
| 04-components | 24 | 22 ms | 25.8 KB | 1.3 MB |
| 05-motion | 11 | 14 ms | 12.2 KB | 396 KB |
| 06-theming | 16 | 24 ms | 19.3 KB | 674 KB |
| pitch | 9 | 10 ms | 8.2 KB | 156 KB |
| research | 9 | 11 ms | 7.8 KB | 632 KB |
| seminar | 12 | 12 ms | 10.7 KB | 731 KB |
| slideshow | 5 | 10 ms | 3.8 KB | 348 KB |

A real deck is 10 to 24 ms end to end, process start included — against Marp's
half-second floor.

## What these numbers do not say

- **They are not a verdict on the tools.** Typst's PDF is tagged and
  accessible; Mirzam's is a browser print. Marp's themes are a CSS contract
  people already know. Speed is one axis.
- **The generated decks are simple on purpose** — headings, prose, a list, a
  table, some maths, the parts all three express natively. Nothing here
  measures charts, connectors, transclusion or embedded media, because two of
  the three have no equivalent to measure against.
- **Only one-shot builds were measured.** All three have a watch mode, and both
  Typst and Mirzam rebuild incrementally inside it. Mirzam's incremental path is
  measured above; Typst's is not measured here at all.
- **Fonts were missing.** This container has none of the families the sample
  decks name, so line breaks — and therefore layout work — differ from a
  machine that has them.
- **Four cores, shared.** The absolute milliseconds belong to this container.
  The ratios are what travel.

## Reproducing

```bash
cargo build --release --bin mirzam
export MIRZAM_CHROMIUM=/path/to/chrome        # also CHROME_PATH, for Marp

./scripts/bench-history.sh                    # the eight-release tables
python3 scripts/bench-compare.py \
  --sizes 20,120,500 --profiles plain,math --targets html,pdf --reps 3
```

Marp comes from `npm install -g @marp-team/marp-cli`; Typst from `cargo install
typst-cli`, with Touying unpacked into
`~/.cache/typst/packages/preview/touying/0.7.1` if `packages.typst.org` is
unreachable. `bench-compare.py` skips a tool it cannot find rather than guessing
at its numbers.
