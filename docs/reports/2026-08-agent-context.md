# The agent's feedback loop, in tokens — August 2026

[agents.md](../agents.md) makes a claim without a number: that an agent
fixing a deck through `mirzam check --format json` closes its loop without
looking at pictures, and that this is most of why Mirzam is usable by a model.
The number it was missing is what that loop *costs* — every diagnostic the
agent reads and every screenshot it would otherwise have read lands in an LLM
context window and is paid for, per round, in tokens. Nobody had measured
either side.

Two measurements, one afternoon. A **harness** that weighs both channels
deterministically across deck sizes: `scripts/bench-agent-context.py`. And a
**paired live run**: two identical agents, same model, same broken deck, one
allowed only the JSON checker and one allowed only screenshots, both run to a
deck they believed clean, both then verified against the checker as ground
truth.

## What a screenshot costs, exactly

Reading an image costs a vision model `⌈w/28⌉ × ⌈h/28⌉` tokens, after the
image is downscaled (aspect preserved) to its tier's caps — 1568 px long edge
and 1568 tokens on the standard tier, 2576 px and 4784 tokens on the
high-resolution tier of current models. The numbers are exact and published;
the harness implements the rule and reproduces the documentation's own table.

So one slide, photographed the way this repository photographs slides
(`scripts/shoot-slides.mjs`, 1440×810), costs **1,508 tokens** to look at, on
either tier. At 1920×1080 it is 1,560 (standard) or 2,691 (high-resolution).
That is the meter running on the visual loop before the model has said a word.

Text tokens in this report are **estimates** — `ceil(bytes / 4)`, since the
exact count is tokenizer-specific and nothing here calls an API. The image
side is exact; the text side is conservative to within tens of tokens either
way, against ratios measured in multiples.

## The harness: one round of review

Decks of 10, 30 and 100 slides, five seeded defect sites each (a clipped and
overlapping head band, a dangling connector, a dangling annotation anchor, an
unknown shape kind — the defect classes the [usability
evaluation](2026-08-usability-eval.md) watched real authors ship). The JSON
side is measured, not modelled: the deck is written at every "k defects
remaining" state and `mirzam check --format json` runs for real; its stdout is
what an agent reads. The screenshot side is one image per slide, priced by the
formula above.

| Slides | JSON tokens/round | Shots 1440×810 | Shots 1920×1080 high-res | Ratio (1440 : JSON) |
|---:|---:|---:|---:|---:|
| 10 | ~368 | 15,080 | 26,910 | **41×** |
| 30 | ~369 | 45,240 | 80,730 | **123×** |
| 100 | ~371 | 150,800 | 269,100 | **406×** |

The row to read twice is the JSON column: **it does not grow with the deck**.
368 tokens at 10 slides, 371 at 100 — the document scales with the number of
*findings*, not the number of slides, because a clean slide contributes
nothing. The screenshot sweep is linear in slides by construction. The claim
"500 slides in 76 ms" from the [performance report](2026-08-performance.md)
has a context-window sibling: 500 slides is still one screenful of JSON.

## The harness: a whole loop

Six rounds — one per fix, plus the clean pass that confirms it:

| Slides | JSON loop | Screenshots, full sweep (1440×810) | Screenshots, changed slide only |
|---:|---:|---:|---:|
| 10 | 2,205 | 90,480 | 22,620 |
| 30 | 2,214 | 271,440 | 52,780 |
| 100 | 2,226 | 904,800 | 158,340 |

The changed-only column is the strongest case the screenshot loop can make —
one full sweep, then a single re-shot per fix, an agent disciplined enough to
never re-check a slide it did not touch. It still pays **10× to 70× the whole
JSON loop**, and the full sweep at 100 slides is pushing a million tokens
where the JSON loop spends two thousand. On the sample decks the static
numbers are the same shape: a clean `check` of `pitch`, `research` or
`seminar` is ~470 bytes (~120 tokens) flat, where one visual pass is 13,572
tokens for `pitch` (9 slides) and 36,192 for `04-components` (24).

## The live run: two agents, one broken deck

The harness prices the channels; it does not prove an agent can *navigate* by
the cheap one. So: a 10-slide deck, five seeded defect sites, seven
diagnostics on first check (`layout.clipped`, `layout.overlap`,
`layout.connector`, `build.connect`, `build.annotate`, `build.shape` ×2). Two
agents on the same model class, identical copies, identical instructions
except for the feedback channel. Neither was told the deck was seeded or what
the defects were. Ground truth afterwards was `mirzam check` on both final
decks.

| | JSON loop | Screenshot loop |
|---|---:|---:|
| Rounds to done | 2 | 4 |
| Feedback read, per loop | 2,386 B ≈ **600 tokens** | 40 images × 1,508 = **60,320 tokens** |
| Tool calls | 16 | 71 |
| Wall time | 71 s | 463 s |
| Agent's total tokens, all-in | 50,247 | 158,105 |
| Final deck, per the checker | clean | clean |

Both agents fixed everything, and both preserved the deck's content — ids
corrected rather than blocks deleted, text shortened rather than removed. The
difference is the bill: **the feedback channel itself cost 100× more in the
visual loop**, and the whole task — instructions, syntax card, edits, all of
it — cost 3.1× more and took 6.5× longer. The all-in ratio is the honest
headline for "what does this save me"; the 100× is the honest headline for
"what does the *channel* cost", and it is the part that grows with deck size.

Round count is not noise. The JSON agent fixed all seven findings in one
pass because each record named the slide, the pane, the file and the line;
the screenshot agent spent rounds 2 and 3 on something it could only
discover by looking — its newly-drawn arrow crossed a word of prose — and
one of those rounds on an attribute (`curve=0`) that changed nothing
visible.

## What the screenshot loop saw that the checker cannot

That arrow is the caveat, and it cuts the other way. The connector the JSON
agent restored crosses the word "the" in the prose beneath it; `check` has no
diagnostic for an arrow that is drawn but ugly, so the JSON agent shipped it
and the screenshot agent did not. This is not news to the project — it is
[W14](../workstreams.md#w14--linking-by-annotation-not-by-arrow)'s founding
observation ("the feature most likely to look wrong"), and the syntax card
already tells an agent to prefer a paired annotation over an arrow from a
sentence to a figure, which is the mitigation that lives *inside* the JSON
loop: an agent that takes the card's advice draws no crossing arrow to begin
with. The seeded deck deliberately used the text-to-text `connect` that W14
warns about, and the run confirmed the warning empirically. The [performance
report](2026-08-performance.md) recorded the same asymmetry from the other
side: three font-subsetting breakages the checker passes and only pixels
catch. The honest division of labour stands as [AGENTS.md](../../AGENTS.md)
states it — the checker for the iteration loop, a rendering for what the
loop cannot see — and the numbers above say what that division is worth:
run the 100× channel once at the end, not once per fix.

## What these numbers do not say

- **Text tokens are estimated**, `ceil(bytes/4)`, no API called. The image
  formula is exact, but other assistants and other screenshot sizes price
  differently; the ratios, not the absolute counts, are what travel.
- **One live run per condition, one model.** The paired run is an
  existence proof with a price tag, not a distribution; rounds-to-done will
  vary run to run.
- **The seeded defects are the checker's own vocabulary.** A defect family
  `check` does not know stays invisible to the JSON loop by construction —
  that is the arrow above, generalised.
- **Build warnings reach both loops.** `mirzam build` prints them to stderr
  and paints banners on the slide, so the screenshot agent read the shape
  error in pixels. The gap measured here is the gap the *layout* pass makes.

## Reproducing

```bash
cargo build --release --bin mirzam
MIRZAM_CHROMIUM=/path/to/chromium \
  python3 scripts/bench-agent-context.py --sizes 10,30,100
```

The harness writes `results.md` and `results.json` (defaults to
`bench-agent-context-out/`), asserts that the seeded decks still produce
`layout.clipped` and `layout.connector` before trusting any number, and
skips nothing silently. The live run is a protocol rather than a script —
two agents, identical decks, one channel each, `mirzam check` as the
referee afterwards. Its deck was hand-written but seeds the same defect
recipes the generator carries, so an equivalent starting line is one
`--sizes 10` run away even if the agents' paths are not.
