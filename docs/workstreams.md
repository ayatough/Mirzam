# Workstreams

The plan for the next batch of features, split so that several agents can work at
the same time without colliding. Read [AGENTS.md](../AGENTS.md) first: the
non-negotiables and the definition of done apply to every stream here.

Each stream below is a **vertical slice** — parser, renderer, runtime, docs, sample
slide and tests — that can be merged on its own. Half a feature cannot.

## Ground rules for this batch

1. **Branch from `main`, not from another stream.** W0 lands first; everything
   else branches from `main` after it.
2. **One stream owns a file.** The contention hotspots are listed per stream. If
   you need to touch a file another stream owns, change the contract in this
   document instead and say so.
3. **Golden snapshots are regenerated at merge time, not during development.**
   Two streams that both change rendered output will conflict in
   `crates/mirzam-cli/tests/snapshots/*.html`. Land one, regenerate, then the
   next. Keep your own diff to the snapshot minimal by adding sample slides at
   the *end* of a deck.
4. **New markup goes in `examples/showcase.md` and `docs/syntax.md`**, per the
   definition of done. New layout behaviour goes in `examples/cookbook.md` and
   `docs/layout.md`.
5. **The final state is the resting state.** Anything animated, annotated or
   themed must look correct with JavaScript disabled and in PDF export. The
   runtime opts *into* motion; it never opts out of correctness.

## Sequencing

| Phase | Streams | Why |
|---|---|---|
| 0 | W0 | Splits the one file every other stream needs to edit |
| 1 | W1, W3, W4, W5, W7 | Independent; different crates and different theme files |
| 2 | W2, W6 | Need a contract from phase 1, not its code |
| 3 | W8 | Needs W6 (annotations) and W7 (source map) |
| 4 | W9 | Integration, then `main` |

## Assignment

Difficulty is about how expensive a wrong decision is, not about how much typing
there is. The model column follows from that:

- **Opus** — cross-cutting invariants, geometry or timing that unit tests cannot
  fully pin down, and anything that writes to a user's files.
- **Sonnet** — the specification exists and correctness is checkable by tests:
  parsers, palettes, source maps.
- **Fable** — narrow, additive, fully specified, low blast radius.

| # | Stream | Difficulty | Model | Depends on | State |
|---|---|---|---|---|---|
| W0 | Theme file split + layout debug overlay | B | Sonnet | — | ✅ |
| W1 | `anim` DSL → timeline IR | B | Sonnet | — | ✅ |
| W2 | Animation runtime and slide transitions | S | Opus | W0, W1 contract | ✅ |
| W3 | Named themes and dark mode | B | Sonnet | W0 | ✅ |
| W4 | Presentation effects | C | Fable | W0 | |
| W5 | Typst-flavoured math | A | Sonnet | — | |
| W6 | Annotations on images and charts | S | Opus | W0 | ✅ |
| W7 | Source map through transclusion | A | Sonnet | — | |
| W8 | Annotation editing, written back to Markdown | S | Opus | W6, W7 | |
| W9 | Release hardening and merge to `main` | A | Opus | all | |

---

## Shared contracts

Defined here so phase 1 and phase 2 can start on the same day. Changing one of
these is a change to this document first.

### C1. Animation timeline

`mirzam-render` emits, inside each `<section class="slide">` that has animation:

```html
<script type="application/json" class="mz-anim">{ ... }</script>
```

```json
{
  "steps": 2,
  "tracks": [
    {
      "trigger": { "kind": "enter" },
      "target":  { "sel": ".title", "split": "chars" },
      "effect":  "fade-in",
      "dur": 400, "delay": 0, "stagger": 30, "ease": "cubic-bezier(.33,1,.68,1)"
    }
  ]
}
```

- `trigger.kind` is `enter` | `click` (with `n`) | `exit` | `after` (with `id`,
  optional `offset` ms).
- `target.sel` is a CSS selector. `target.split` is absent, `chars`, `words` or
  `lines`; **splitting happens at build time**, so the wrapping spans are in the
  HTML and the runtime only selects them.
- `steps` is the number of `click` triggers on the slide; the viewer needs it to
  know when `→` advances a step and when it turns the page.
- `ease` is a CSS easing function, ready to hand to the Web Animations API.
  Named curves and `spring(mass,stiffness,damping)` are both resolved at build
  time — the deck carries the curve it uses, not a table of curves it might.

The **transition** is a property of the deck, not of a slide, and rides on
`#deck` rather than in this blob:

```html
<div id="deck" data-transition='{"in":"slide-in","out":"slide-out","dir":"left","dur":320,"ease":"…"}'>
```

A slide overrides its half of the page turn by declaring an ordinary
whole-slide track (`[enter] slide : …` / `[exit] slide : …`). That is what a
transition is, so there is no second way to write one.

**The resting state rule.** Elements are laid out in their *final* state, and
the runtime is the only thing that ever puts one in its initial state — by
writing inline styles it saves and restores, never by a stylesheet rule. So the
rule holds by construction: no stylesheet can hide content, and a page with no
JavaScript (print and PDF ship none) shows every slide fully revealed.

Under `prefers-reduced-motion` the reveals still happen and stepping still
works; only the movement is dropped.

### C2. Annotation model

Emitted per `annotate` block, at the end of the slide:

```html
<script type="application/json" class="mz-annot" data-target="[data-pane=&quot;fig&quot;]">{ ... }</script>
```

```json
{
  "items": [
    { "kind": "rect",   "x": 40, "y": 22, "w": 18, "h": 12,
      "label": "cache miss", "color": "var(--mz-accent2)" },
    { "kind": "arrow",  "x": 12, "y": 70, "x2": 38, "y2": 30, "dashed": true },
    { "kind": "circle", "anchor": "latency-1-2", "pad": 6 }
  ]
}
```

`data-target` is a CSS selector: a `#id` written as such, or `[data-pane="…"]`
for a bare pane name. It rides on the tag rather than in the blob because the
runtime needs it before parsing anything.

Coordinates are percentages, and `x,y` is the **centre** of a `rect` or
`circle` — the way `shape` reads, so one convention covers both.

**There is no `space` field.** The origin is derived rather than declared,
because every value a author could pick is one they could pick wrongly:

| The target resolves to | Origin |
|---|---|
| A pane holding exactly one picture (`img`, `video`, `svg`, `canvas`) | that picture's **painted** box |
| Anything else | its border box |
| An item with `anchor` | the live box of the element it names — no coordinates at all |

The painted box is not the element box whenever the picture is fitted with
`object-fit: contain`, or an SVG with `preserveAspectRatio="… meet"` — a chart,
for instance. Both are letterboxed inside a larger element, and measuring the
element instead would offset every mark by exactly the letterboxing.

The overlay is one absolutely-positioned SVG per block, sized to the slide and
re-measured with a `ResizeObserver`. Labels are HTML rather than SVG text, so
they stay real text: selectable, themable and never stretched by the overlay's
own scaling.

**The overlay ships in the PDF.** It is the one script the print page carries;
see [architecture.md](architecture.md#annotations-and-the-pdf) for why that
does not weaken the no-JavaScript guarantee.

### C3. Theme tokens

A theme is a set of CSS custom properties, defined for both modes:

```css
:root[data-theme="nord"]                    { --mz-slide-bg: …; --mz-fg: …; … }
:root[data-theme="nord"][data-mode="dark"]  { --mz-slide-bg: …; --mz-fg: …; … }
```

The token list is whatever `crates/mirzam-render/src/theme/themes/default.css`
defines; extending it means extending every built-in theme in the same commit.

### C4. Effect registry

`effects.js` is inlined only into decks that declare effects:

```js
MZ.effects.register("shake", (ctx) => { /* ctx: {slide, layer, palette, dur} */ });
```

Effects never run in print and never mutate the document; they draw into
`ctx.layer`, a pointer-events-none overlay that is cleared when they finish.

---

## W0 — Theme file split and layout debug overlay

**Difficulty B · Sonnet · blocks everything else**

`crates/mirzam-render/src/theme.rs` is one 430-line file holding all CSS, the
print CSS and the viewer JavaScript. Five of the streams below need to add to it.
Split it first:

```
crates/mirzam-render/src/theme/
  mod.rs          assembly, the public API, the include_str! list
  base.css        layout, typography, panes
  print.css
  viewer.js
  themes/default.css
```

`mod.rs` keeps the existing public functions; the CSS and JS move to files
included with `include_str!` so they keep shipping inside the binary. No output
change: the golden snapshots must be byte-identical after this commit. Verify
that explicitly — it is the point of the commit.

Then add the debug overlay, as the first user of the new structure:

- The renderer puts `data-pane="<name>"` on every pane div.
- `L` in the viewer toggles `mz-debug` on `<html>`: panes get a coloured outline,
  their name in a corner label, and the grid gaps are tinted. Off by default,
  never in print.
- `mirzam build --debug-layout` bakes it on, for screenshotting a broken deck.

**Owns:** `crates/mirzam-render/src/theme*`. **Done when:** snapshots unchanged by
the split, the overlay documented in `docs/layout.md`, `check-layout.mjs` still
green.

## W1 — `anim` DSL → timeline IR

**Difficulty B · Sonnet**

`BlockKind::Anim` is already recognised by `mirzam-syntax`; nothing consumes it.
Add `crates/mirzam-anim`, a pure crate: text in, [C1](#c1-animation-timeline) out.

````markdown
```anim
[enter]   .title       : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[after #latency-0-2 +200ms] .caption : fade-in 300ms
[exit]    slide        : iris-out 500ms
```
````

Effect set for v1: `fade-in`, `fade-out`, `slide-in`, `slide-out` (with a
direction), `grow-x`, `grow-y`, `pop`, `draw` (SVG stroke), `iris-out`. Easings:
the CSS named curves plus `spring(m,k,c)`, resolved to a sampled `linear()` curve
at build time so the runtime needs no physics.

The renderer's job in this stream is only to emit the JSON and to perform
build-time text splitting for `chars`/`words`/`lines` (which must not break
inline markup or CJK). Driving it is W2.

Errors are warnings, not failures: an `anim` line that points at nothing renders
the slide unanimated and reports through the existing warning channel.

**Owns:** `crates/mirzam-anim`, the anim extraction pass in `mirzam-render`.

## W2 — Animation runtime and slide transitions ✅

**Difficulty S · Opus · landed**

The viewer is a step machine. `→` advances to the next click step if the slide
has one, otherwise turns the page; `←` steps back, then goes to the previous
slide; arriving at a slide from a later one shows it with every step already
played. Timelines run through the Web Animations API, out of `theme/anim.js`,
which is inlined only into decks that animate something.

Transitions are deck-wide (`transition:` in frontmatter, [C1](#c1-animation-timeline)),
because a per-slide transition is already expressible as a whole-slide
`enter`/`exit` track — and a slide that declares one overrides that half of the
page turn.

Three things were not obvious going in:

- **A repaint is not a page turn.** Resize, font-load and the live-reload patch
  all go through the same code path as a navigation. The font-load repaint fires
  a few frames after load, which is exactly when slide one's entrance is
  running: staging the slide again cancelled it every time. The runtime now
  refuses to re-stage a slide that has animations in flight.
- **A whole-slide track must not reach into the slide.** Finalizing a
  `[exit] slide` track scanned every stroked descendant to clean up after a
  possible `draw`, and in doing so un-armed an unrelated `draw` track on a
  shape. Only a `draw` track touches stroked descendants now.
- **Sliding a whole slide is a page turn, not a reveal**: opaque, travelling its
  own width, with the arriving slide covering the departing one. A paragraph
  doing `slide-in` still fades as it arrives, because that is a reveal.

**Owns:** `theme/viewer.js`, `theme/anim.js`. **Coordinate with:** W4 (both add
key bindings — the key table lives in `viewer.js` and W2 owns it).

## W3 — Named themes and dark mode ✅

**Difficulty B · Sonnet · landed**

`meta.theme` is already parsed from frontmatter and currently ignored. Make
`theme: nord` work, with built-ins compiled in:

| Name | Source | Licence |
|---|---|---|
| `default` | ours | — |
| `nord` | Nord palette | MIT |
| `solarized` | Solarized | MIT |
| `vscode` | VS Code Light+/Dark+ | MIT |

Record each palette's origin and licence in `themes/CREDITS.md`, the way
`scripts/fetch-backgrounds.sh` records photograph attribution. Ship palettes, not
copied stylesheets.

Every theme defines **both** modes explicitly ([C3](#c3-theme-tokens)). Do not
derive dark from light by inversion — inverted accents lose contrast against a
dark background, which is exactly the failure this stream exists to prevent.
Instead, prove it: a unit test computes the WCAG contrast ratio for every
(token, background) pair in every theme and mode, and fails below 4.5 for body
text and 3.0 for chart marks and UI lines. That test is the deliverable as much
as the palettes are.

Mode selection: `mode: dark` in frontmatter, `?mode=dark` in the viewer, `D` to
toggle, `prefers-color-scheme` when unset.

**What the contrast test could not see.** It checked every *token* and passed,
while dark mode still shipped with inline code, `pre` blocks and table headers
light-on-light — because those surfaces were hard-coded hex in `base.css`, and a
literal is invisible to a test that only reads the palette. Two things follow,
and both are now in place: the surfaces are tokens (`--mz-surface`,
`--mz-danger-*`) checked as text-on-surface pairs, and `base.css` may not carry
a color literal at all unless it says in a comment why it does not belong to a
theme. A palette test is only as good as the guarantee that the palette is the
only place colors come from.

**Owns:** `theme/themes/*.css`, `themes/CREDITS.md`.

## W4 — Presentation effects

**Difficulty C · Fable**

Ephemeral, presenter-triggered flourishes: a flash over the whole page, a shake,
an explosion, speed lines, a burst of emoji, a Nico-Nico-style comment sweep.

````markdown
```effects
1 : flash
2 : shake
e : burst 🎉
d : danmaku "そこ、大事です"
```
````

**Is this the same feature as animation?** It shares the runtime and nothing else.
Animations are part of the document: deterministic, ordered, and present in the
PDF. Effects are part of the *performance*: fired by a key, never in the exported
file, and it must not matter if one never fires. So: same primitives
([C4](#c4-effect-registry)), separate authoring surface, separate JS file that is
only inlined when a deck declares effects.

Constraints: dependency-free, no layout thrash (compositor properties only), an
effect that overruns is cancelled at the slide change, and `Esc` clears
everything.

**Owns:** `theme/effects.js`, `crates/mirzam-render/src/effects.rs`.

## W5 — Typst-flavoured math

**Difficulty A · Sonnet**

LaTeX is hard to write from memory; Typst's math syntax is not. Support it as an
alternative front end.

**On the licence question:** Typst is Apache-2.0, which is compatible with this
MIT project — depending on it would be legal, with the usual notice requirement.
It is still the wrong dependency: Typst's math goes through its own layout engine
to SVG/PDF, not to MathML, and it would pull a very large tree into a crate that
must also compile to `wasm32`. Write a subset parser instead. The syntax we want
is small and stable, and the semantics are already ours.

Add `crates/mirzam-tmath`: Typst math source → AST → **LaTeX**, then through the
existing `math-core` path to MathML. Lowering to LaTeX rather than straight to
MathML reuses the spacing, stretchy delimiters and font handling that already
work; the AST stays the seam if we ever want to change that.

v1 surface: `a/b`, `^`, `_`, `sqrt()`, `root()`, `sum`, `product`, `integral`,
Greek by name, `->` `=>` `!=` `<=` `>=` `in` `subset`, `mat(1,2;3,4)`, `cases()`,
`abs()`, `norm()`, `"literal text"`, `&` alignment, `#` escapes.

Selected per deck: `math: typst` in frontmatter, default `latex`. Existing decks
must be untouched — `examples/seminar.md` is the regression test for that. Build
a golden corpus of expression pairs (Typst source, expected MathML) and make it
the crate's test suite.

**Owns:** `crates/mirzam-tmath`, the math dispatch in `mirzam-render/src/inline.rs`.

## W6 — Annotations on images and charts ✅

**Difficulty S · Opus · landed**

Circle the interesting part of a screenshot, point an arrow at it, label it —
what everyone opens PowerPoint to do.

````markdown
::: pane fig
![p95 by region](img/latency.png)
:::

```annotate
target: fig
rect   40,22 18x12 : label="cache miss"
arrow  12,70 -> 38,30
text   10,80 "throughput doubles here"
circle #latency-1-2 : pad=6
```
````

The block sits **beside** the pane rather than inside it, the way `connect`
does. Both are overlays that name what they point at, and one shape of thing
should be written one way. Model in [C2](#c2-annotation-model).

The three things that made or broke this stream, and how they came out:

1. **The painted box, not the element box.** This is real, and it bit
   immediately: `target: fig` names a pane, the photo inside it is centred and
   354px tall in a 421px pane, and the first working build put every mark 8px
   off. Two rules fix it — a pane holding one picture *means* that picture, and
   a picture's box is measured from its natural size and fit (`object-fit` for
   `img`/`video`, `preserveAspectRatio` for an `svg`, which is the same rule
   under another name). A browser probe asserts the mark lands within 2px of
   the declared percentage, and stays there through a resize.
2. **Charts need no coordinates**, and this is the better way to write it:
   `circle #load-0-2 : pad=10` is exact and survives a data change. It resolves
   to 0.0px off the mark's centre.
3. **PDF.** The print page runs the overlay script. An annotation is drawn over
   the slide and hides nothing, so the guarantee a scriptless page protects is
   untouched — and a PDF that silently dropped the marks would be worse than
   one that carries them. `annot.js` therefore never reaches for the viewer,
   and a test enforces it. Reasoning in
   [architecture.md](architecture.md#annotations-and-the-pdf).

**Owns:** `crates/mirzam-annot`, `theme/annot.js`.

## W7 — Source map through transclusion

**Difficulty A · Sonnet**

`expand_includes_tracked` knows *which* files a deck came from. W8 needs to know
*where*: given a byte range in the expanded document, which file and which byte
range in it. Pure, self-contained, and unit-testable — nested includes, an
include inside a fence, CRLF, and a file included twice are the interesting
cases.

Extend the tracker to return a sorted map of `(expanded_range → (file, range))`,
add `SlideSource` byte ranges for each fenced block so a block can be located
without re-parsing, and expose a lookup that is a binary search, not a scan.

Useful on its own beyond W8: error messages can finally say which file a warning
came from.

**Owns:** `crates/mirzam-syntax`.

## W8 — Annotation editing, written back to Markdown

**Difficulty S · Opus · depends on W6 and W7**

The one the user asked about directly: drag the circle in the preview, and the
Markdown updates.

**It is possible, and this is the shape of it.** `mirzam serve` already owns the
source files and already watches them. Add an edit channel:

1. `mirzam serve --edit` inlines the annotation editor and enables `POST /edit`.
2. Dragging a handle updates the overlay locally and posts
   `{ file, start, end, sha, text }` — `start`/`end` from W7's source map, `sha`
   over the current bytes in that range.
3. The server verifies the file still hashes to `sha` (someone may have edited it
   in the editor meanwhile), rewrites exactly that byte range, and returns the new
   range. On mismatch it refuses and the client re-syncs from the rebuilt deck.
4. The file change triggers the normal watch-and-rebuild path, so the preview
   converges through the same code as a manual edit. No second source of truth.

Constraints that are not optional: only under `--edit`, never in an exported
deck; only files inside the deck's root, resolved against symlinks; the rewrite
touches only the annotation block's byte range, preserving surrounding formatting
and the file's line endings; and numbers are written back rounded to one decimal
so a drag does not produce a wall of digits in the diff.

Out of scope for v1: editing anything other than annotation blocks; multi-client
editing; undo (the editor's own undo, on the Markdown file, is the story).

**Owns:** `crates/mirzam-cli/src/serve.rs`, `theme/annot-edit.js`.

## W9 — Release hardening and merge to `main`

**Difficulty A · Opus · last**

The batch is only useful once it is on `main` and usable.

- Every new block form goes through `commonmark_compat.rs`. That test is the
  promise the project makes; five new fenced blocks is exactly when it gets
  checked properly.
- `check-layout.mjs` learns the new failure modes it can see: an annotation
  drawn outside its target, a debug overlay left on, an animated element left in
  its initial state.
- One sample deck per feature, all built in CI, all published to the docs site.
- Benchmark: confirm the per-slide edit cost has not regressed past the numbers
  in [roadmap.md](roadmap.md), and update them.
- `docs/syntax.md`, `docs/layout.md`, `docs/ja/README.md`, `CHANGELOG.md`.
- Then tag `v0.1.0` — deliberately deferred until now.
