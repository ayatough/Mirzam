# Mirzam next to Marp and Touying

> A snapshot from 2026-08. All three projects are moving targets, so individual
> facts will age. This is not a scoreboard — each tool is good at what it set
> out to do. It is a map of where the bets differ, kept for positioning
> decisions. Japanese translation: [docs/ja/comparison.md](ja/comparison.md).

Three text-first slide tools, three different answers to the same question —
*where does the visual half of a deck live?*

- **[Marp](https://github.com/marp-team/marp-cli)** (Node.js, since 2016):
  in the *theme CSS*. Markdown carries content only; layout and design are the
  theme author's job. Marpit, its core, states minimalism as policy — no
  predefined classes, pure CSS styling.
- **[Touying](https://github.com/touying-typ/touying)** (Typst package):
  in the *program*. A deck is Typst source — a real language with functions,
  closures, and typesetting primitives — so anything expressible in code is
  expressible on a slide.
- **Mirzam** (Rust): in the *document*. Layout, charts, connectors, and
  animation order are written in the Markdown file itself, as blocks that
  degrade to code blocks under any plain parser.

## Shared ground

All three: plain-text sources that diff in git, slide decks from lightweight
markup, CSS-or-code theming, incremental reveal, speaker notes, math, PDF
output, and a watch/preview loop.

## At a glance

| | Marp | Touying | Mirzam |
|---|---|---|---|
| Source language | Markdown | Typst | Markdown |
| Runs on | Node.js (+ browser for PDF) | Typst compiler (Rust) | single Rust binary / WASM |
| Primary output | HTML | PDF | single-file HTML |
| PDF | via browser | native | via Chromium print |
| PPTX | yes (raster; editable experimental) | via touying-exporter | planned |
| Layout control | theme CSS / raw HTML | typesetting code (`grid`, `place`) | ASCII pane grid |
| Between-slide transitions | View Transitions API (33 built-in) | — (PDF pages) | transition DSL |
| In-slide animation | fragmented lists | `#pause` `#only` `#uncover` `#alternatives` — steps become pages | `anim` timeline DSL + performance-only `effects` |
| Charts from data | via plugins | programmatic (CeTZ ecosystem) | `chart` block from CSV, stable per-mark ids |
| Video / audio | HTML embeds | — (PDF; GIF via HTML exporter) | inlined `<video>`/`<audio>`, YouTube/Vimeo |
| Extensibility | markdown-it plugins | full language + Typst Universe | built-in DSLs only (deliberately) |
| Learning curve | lowest | highest | in between |

## Next to Marp

The overlap is real — Markdown, `---` separators, CSS themes, HTML and PDF —
but it is the thinnest layer of both tools. The divergence points:

- **Layout.** Marp leaves it to the theme; multi-column content means theme
  CSS or raw HTML in the Markdown. Mirzam's pane grid keeps it in the
  document. This is the single largest difference, and Marpit's stated
  minimalism makes it a durable one.
- **Motion.** Marp's slide transitions (View Transitions API, browser-native)
  are polished and its fragmented lists cover stepwise reveal. Mirzam adds an
  in-slide timeline (per-element, per-character, spring easing) and separates
  document animation from performance-only effects.
- **Data.** Marp has no built-in charts; Mirzam builds them from CSV at build
  time and gives every mark an addressable id.
- **Reach.** Marp has a decade of themes, answers, CI actions, and a PPTX
  exporter; its VS Code extension is ubiquitous. This is ecosystem gravity
  Mirzam does not have.
- **Speed.** Both are fast enough for ordinary decks. Mirzam's per-slide
  incremental cache (an edit re-renders exactly one slide; 4.5 ms full build
  for 20 slides) matters at scale, in editor previews, and in CI.

## Next to Touying

Touying is the strongest of the three on raw capability, because it inherits
all of Typst: a real programming language, a real typesetting engine, and the
Typst Universe package ecosystem (CeTZ for diagrams, fletcher for arrows,
plotting libraries). Its animation model is elegant — `#pause`, `#only`,
`#uncover`, `#alternatives`, even inside equations — and compiles each step to
a PDF page, so it presents anywhere a PDF opens and collapses cleanly into a
handout. Divergence points:

- **Language.** Typst is Touying's power and its price. Everything is
  programmable — themes are code, diagrams are code — which suits programmers
  and rewards investment. But it is a syntax learned for this purpose, and the
  source renders nowhere outside Typst tooling. Mirzam's source is Markdown
  that GitHub, an editor, or Obsidian already displays; the ceiling is lower,
  the floor is much higher.
- **Output orientation.** Touying is PDF-first; HTML and PPTX come from a
  separate exporter that wraps SVG pages in impress.js. Mirzam is HTML-first —
  video, audio, live-routed connectors, hover, the presenter window — with PDF
  as a print of the same HTML. Which is right depends on the venue: a PDF
  presents from any machine; an HTML deck carries media and motion.
- **Animation as pages vs. animation as runtime.** Touying's steps are extra
  pages: robust, portable, but every reveal multiplies page count, and motion
  between states is not part of the model. Mirzam's steps are runtime state
  over a single laid-out slide: springs and staggers exist, the chart never
  redraws between fragments, and the PDF shows each slide fully exposed.
- **Typography.** Typst justifies, hyphenates, and sets math with a real
  typesetting engine — print-quality output is its home turf. Mirzam
  deliberately delegates typography to the browser and keeps the core to
  geometry.
- **Performance at scale.** Typst compiles in milliseconds and Touying avoids
  the counter/locate cost that slowed Polylux. In practice, decks heavy with
  `#pause` multiply into many physical pages, and whole-document features
  (outlines, references, global state) recompute across them, which is where
  large decks can get slow or uneven — worth re-testing on current versions,
  as both Typst and Touying optimize actively. Mirzam's unit of work is the
  slide: the cache guarantees an edit re-renders one slide (75.6 ms full
  build / 3.2 ms edit at 500 slides), and click steps are runtime state, not
  extra pages.
- **Verification.** Both can be built in CI. Mirzam's `check-layout`
  additionally renders the deck and fails on clipped panes, overlaps, and
  dead connector/annotation references — checks that only exist because
  layout and references are structured data rather than code.

## Differentiation points for Mirzam

Where Mirzam occupies space the other two have left open — kept short, since
these are directions rather than verdicts:

1. **Document → deck.** The degradation guarantee plus `--split h2` means a
   README or design memo becomes a deck without editing. Marp is for writing
   slides; Touying is for programming them; Mirzam turns what you already
   wrote into them.
2. **Layout is drawn, not styled or programmed.** The ASCII grid is a third
   way between Marp's "write CSS" and Touying's "write code."
3. **Data with addresses.** Charts from CSV where a connector or annotation
   can point at one bar — prose and evidence linked without a screenshot.
4. **HTML-native presentation.** Self-contained single file, inlined media,
   presenter view as the same file with `?presenter=1`, effects that exist
   only in performance.
5. **Decks that CI can reject.** Layout checks, degradation tests, and
   light/dark token symmetry are enforced, not encouraged — which also makes
   Mirzam a safe target for coding agents writing decks.
6. **No language to learn — and none to lean on.** Mirzam is deliberately not
   programmable. Users who want closures in their slides are Touying's
   audience, and chasing them would cost the simplicity that is the point.

## Summary

Marp keeps the document minimal and puts the visuals in CSS. Touying makes the
document a program and puts the visuals in code. Mirzam keeps the document
plain but moves layout, data, and motion into it as structured, verifiable
text. All three render Markdown-or-markup into slides; past that door they
walk in different directions.
