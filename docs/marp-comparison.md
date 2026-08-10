# Mirzam and Marp, compared

> A snapshot from 2026-08. Both projects are under active development, so
> individual facts will age; the difference in design bets should not.
> Japanese translation: [docs/ja/marp-comparison.md](ja/marp-comparison.md).

Marp is the most mature Markdown slide tool: a Node.js ecosystem layered as
[Marpit](https://marpit.marp.app/) (core framework) → Marp Core (practical
features) → Marp CLI / VS Code extension, developed since 2016, with a large
community of themes, templates, and CI integrations. On the surface it looks a
lot like Mirzam — both take plain Markdown, split slides on `---`, style them
with CSS themes, and emit HTML and PDF. The resemblance ends there, because the
two projects place opposite bets on what belongs to the document.

## In one sentence each

**Marp's bet**: layout and design belong to the *theme CSS*. Markdown carries
content only; if you want a look, you write CSS (or use someone else's theme).
That is what keeps the framework minimal — Marpit explicitly states it has no
predefined classes and focuses on styling plain HTML elements with pure CSS.

**Mirzam's bet**: layout, figures, data, and the order things appear in belong
to the *document itself*. Hence the ASCII pane grid, charts generated from CSV,
connectors from a phrase in prose to a single bar, and the animation timeline
DSL — all living inside the Markdown file, degrading to code blocks under a
plain parser.

Nearly every difference below follows from this split.

## What they share

- Plain Markdown as the source: diffable in git, opens in any editor.
- `---` as the slide separator; frontmatter for deck-wide settings.
- Themes are CSS; custom CSS can be injected.
- HTML and PDF output; speaker notes, pagination, math.
- Care about not poisoning the Markdown. Marp hides directives in HTML
  comments; Mirzam enforces by test that every extension degrades to code
  blocks or literal text under vanilla CommonMark.

## Architecture

| | Marp | Mirzam |
|---|---|---|
| Implementation | Node.js + markdown-it | Rust (one core, native + WASM) |
| HTML conversion | requires Node runtime | single binary, no dependencies |
| PDF / images | drives Chrome/Edge/Firefox over CDP | prints the same HTML via Chromium (PDF only) |
| PPTX | yes (slides as raster images; editable output experimental) | not yet (planned) |
| Editor story | VS Code extension (preview reconverts the whole deck) | VS Code extension + browser playground, both running the CLI's exact code via WASM |
| Incremental builds | none (per-file reconversion) | per-slide cache; an edit re-renders exactly one slide |

## Ease of writing

**For simple decks, Marp asks you to learn less.** A title-and-bullets deck is
nearly vanilla Markdown plus `marp: true`; image directives like
`![bg right](img.png)` are terse and widely known.

**The moment you want layout, this inverts.** Two columns or a card row in Marp
means writing theme CSS or embedding raw HTML (`<div class="columns">`) in the
Markdown — "ease of writing" is outsourced to whoever wrote your theme. In
Mirzam the same thing stays inside the document:

````markdown
```pane
+--------+--------+
| left   | right  |
+--------+--------+
```

::: pane left
...
:::
````

Column widths come from character counts, row heights from line counts; the
picture you drew is the spec, and neither CSS nor HTML appears. Charts (from
CSV), shapes, connectors, annotations, and animations stay in fenced blocks the
same way.

The other difference is the *degradation guarantee*. Marp's HTML-comment
directives simply vanish in other tools, but `![bg](...)` shows up as a broken
image. Mirzam's CI enforces that every extension degrades cleanly under plain
CommonMark, and with `--split h2` the whole design points one way: an existing
README becomes a deck without editing.

## Rendering speed

Mirzam is structurally ahead. Measured (`docs/roadmap.md`):

| Deck | Full build | One-slide edit |
|---|---:|---:|
| 20 slides | 4.5 ms | 0.4 ms |
| 500 slides | 75.6 ms | 3.2 ms |

A single Rust binary; no browser or Node needed for HTML. Watch mode patches
only the changed `<section>` elements in the client. Marp pays Node startup and
per-file reconversion, and needs a real browser for PDF/PPTX/images; it
publishes no benchmarks.

Honestly, though: **for an ordinary 30-slide deck both are "fast enough."** The
gap matters for large decks, for in-editor preview latency (WASM can re-render
per keystroke), and for CI workflows that verify decks on every commit.

## Animation

Marp's animation support is real — but it covers a different scope.

**Marp**: *between-slide* transitions via the View Transitions API — 33
built-ins plus CSS-defined custom ones, browser-native and smooth. Within a
slide, fragmented lists (`*` bullets revealed stepwise). Per-element
choreography is left to hand-written CSS.

**Mirzam**: slide transitions plus an *in-slide* timeline DSL —
`[click 1] .callout : slide-in 400ms ease=spring(1,180,20)`, per-character
staggers, chaining with `after #id +100ms`; spring curves are sampled at build
time. It also separates `anim` (belongs to the document: ordered,
deterministic, present in PDF) from `effects` (belongs to the performance:
confetti, focus lines, danmaku — never reaching any export), and guarantees
that without JS, and in PDF, every slide is fully exposed.

## Where Marp is ahead

These are facts worth conceding:

- **Maturity and community**: a decade of use, theme collections, answered
  questions, CI actions, migration guides.
- **PPTX export**: decisive wherever "the boss only accepts PowerPoint."
  Mirzam does not have it yet.
- **Ecosystem extensibility**: markdown-it plugins bring Mermaid and friends.
- **VS Code reach**: install counts orders of magnitude higher; near-zero
  adoption friction.
- **Fewer concepts**: minimal bespoke syntax to learn for simple use.

## Where Mirzam is ahead

- **Layout stays in the document** (the ASCII pane grid) — exactly the space
  Marp leaves empty.
- **Charts from data with stable ids**: a connector or annotation can point
  from a phrase in prose to one bar of one chart. Impossible with pasted
  screenshots by construction.
- **Preview/PDF parity by construction**: the same HTML is printed, animations
  lay out in their final state, annotations reach the PDF.
- **Single self-contained HTML**: images, video, and fonts inlined; mailable.
- **Presenter view is the same file**, reopened with `?presenter=1` — no
  server, no second export.
- **Speed and incrementality**, as above.
- **Verifiability**: `check-layout` catches clipping, overlap, and dead
  references in CI. A testable deck is a property no other tool offers.

## Differentiation

To the worry "won't this end up nearly identical?": the shared part
(Markdown + `---` + theme CSS) is the thinnest layer of both tools, and
Mirzam's core — layout, data, annotation, verification — is territory Marp has
*chosen* not to enter; Marpit's minimalism is an explicit policy, which makes
the gap durable. Beyond that:

1. **Lead with "document → deck."** With `--split h2` and the degradation
   guarantee, a README or design memo becomes a deck unedited — only Mirzam
   does this. Marp is a tool for writing slides; Mirzam turns what you already
   wrote into slides.
2. **Data-driven decks.** Update a CSV and the charts, variables, and
   annotations follow — the weekly-report workflow Marp cannot express.
3. **Sell the testable deck.** A coding agent writes the deck and
   `check-layout` fails CI on broken layout — a slide substrate for the agent
   era, with no Marp counterpart.
4. **Close the PPTX gap** — the highest-leverage adoption item, as the roadmap
   already says.
5. **Don't compete on theme count.** Marp's theme corpus is unbeatable; talk
   instead about the qualitative difference — token-only themes with enforced
   light/dark symmetry.

## Summary

| Axis | Advantage |
|---|---|
| Learning cost for simple decks | Marp |
| Layout control (without CSS/HTML) | Mirzam |
| Build/preview speed, incrementality | Mirzam |
| Slide transitions | roughly even (browser-native vs. DSL) |
| In-slide animation | Mirzam |
| Output breadth (PPTX etc.) | Marp |
| Ecosystem and track record | Marp |
| Data integration (charts, variables, annotations) | Mirzam |
| PDF parity, degradation guarantee, CI verification | Mirzam |

Mirzam is not "an alternative Marp." It is what you get when the things Marp
placed outside the document — layout, data, motion, verification — are brought
inside it. The two tools resemble each other at the door and diverge with every
step past it.
