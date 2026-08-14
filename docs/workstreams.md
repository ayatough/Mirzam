# Workstreams

The plan for the next batch of features, split so that several agents can work at
the same time without colliding. Read [AGENTS.md](../AGENTS.md) first: the
non-negotiables and the definition of done apply to every stream here.

Each stream below is a **vertical slice** — parser, renderer, runtime, docs, sample
slide and tests — that can be merged on its own. Half a feature cannot.

## Ground rules for this batch

1. **Branch from `main`, not from another stream.** Every stream in the current
   batch branches from `main`; none of them blocks another.
2. **One stream owns a file.** The contention hotspots are listed per stream. If
   you need to touch a file another stream owns, change the contract in this
   document instead and say so.
3. **Golden snapshots are regenerated at merge time, not during development.**
   Two streams that both change rendered output will conflict in
   `crates/mirzam-cli/tests/snapshots/*.html`. Land one, regenerate, then the
   next. Keep your own diff to the snapshot minimal by adding sample slides at
   the *end* of a deck.
4. **New markup goes in `examples/04-components.md` and `docs/syntax.md`**, per
   the definition of done. New layout behaviour goes in `examples/03-layout.md`
   and `docs/layout.md`.
5. **The final state is the resting state.** Anything animated, annotated or
   themed must look correct with JavaScript disabled and in PDF export. The
   runtime opts *into* motion; it never opts out of correctness.

## Where this stands

The first batch (W0–W7) is on `main` and in daily use. What follows is the
second batch, written after presenting with the thing: every item below came
from a deck someone was actually trying to give.

| Phase | Streams | Why |
|---|---|---|
| done | W0–W4, W6, W7 | Animation, effects, themes, annotations, source map |
| 1 | W10, W11 | Independent; the viewer's own chrome and the authoring layer |
| 2 | W12, W13 | W12 needs the chrome from W10; W13 needs nothing |
| 3 | W14 | Retires an authoring pattern the other streams now cover |
| 4 | W9 | Integration, benchmark, `v0.1.0` — done; the release is tagged |
| 5 | W20, W21 | The [market survey](reports/2026-08-market-survey.md)'s P0: visible code, and a loop an agent can close |
| 6 | W22, W23, W16 | The survey's P1. W22 first: it is what makes a theme an identity rather than a palette, which W16 then has something to exhibit. W23 develops alongside but lands after, because both move rendered output |
| 6 | W22 | The last frontmatter key that asks the author to know how the CSS is assembled |

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
| W4 | Presentation effects | C | Fable | W0 | ✅ |
| W6 | Annotations on images and charts | S | Opus | W0 | ✅ |
| W7 | Source map through transclusion | A | Sonnet | — | ✅ |
| W10 | Continuation: one pane carries on, the rest hold | B | Opus | — | ✅ |
| W11 | Viewer chrome: cheat sheet, touch controls, gestures | C | Fable | — | ✅ |
| W12 | Presenter window | B | Sonnet | W11 | ✅ |
| W13 | Table of contents from headings | B | Sonnet | — | ✅ |
| W14 | Linking by annotation, not by arrow | C | Sonnet | W6 | ✅ |
| W15 | Brand and visual identity | B | — | — | ✅ |
| W9 | Release hardening and `v0.1.0` | A | Opus | all | ✅ |
| W16 | Showing the thing working: demo recording, themes gallery | C | — | — | |
| W17 | A theme per slide | B | — | — | ✅ |
| W18 | Carrying an element from one slide to the next | S | — | W2 | |
| W5 | Typst-flavoured math | A | Sonnet | — | ✅ |
| W19 | Structural math editing: tap and place, not type | S | Fable | W5 | withdrawn |
| W8 | Annotation editing, written back to Markdown | S | Opus | W6, W7 | deferred |
| W20 | Syntax highlighting at build time | B | Opus | — | ✅ |
| W21 | An authoring contract for agents | B | Opus | — | ✅ |
| W22 | One door to a deck's look: `theme:` absorbs `css:` | B | Opus | — | |
| W23 | Mermaid diagrams, rendered at build time | B | Opus | — | |

### What is deferred, and why

Not cancelled; off the critical path to a tool people present with every week.

- **W8 (drag an annotation back into the Markdown).** The most expensive stream
  in the plan — an edit channel, byte-range rewriting, conflict handling — and
  the one whose absence hurts least, because the editing surface people
  actually use is their editor, with `mirzam serve` beside it. W7 landed, so
  the hard half is already done whenever this comes back.

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

The token list is whatever `crates/mirzam-render/src/theme/themes/mirzam.css`
defines — the fallback theme, and so the one every other has to match;
extending it means extending every built-in theme in the same commit.

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

## W4 — Presentation effects ✅

**Difficulty C · Fable · landed**

Seven effects: `flash`, `shake`, `lines` (speed lines), `boom`, `burst <emoji>`,
`confetti`, `danmaku "<text>"`. The key table is validated against the
viewer's own bindings, so shadowing navigation is a build warning rather than
a surprise on stage.

The one thing that was not obvious: **the slide-change detector must ignore
this file's own writes.** Cancelling on any class mutation under `#deck` made
`shake` — the one effect that sets a class on the slide — cancel itself, and
left a stray timer that killed whatever effect was running half a second
later. Cancellation now triggers on the active section actually changing.

Writing the sample also surfaced a real viewer bug, unrelated to effects:
arriving at a slide whose `exit` transition was still in flight hit the
`busy()` repaint guard and skipped staging it, stranding the slide off-screen
under the transform its exit had left behind. Editor cursor sync and live
reload both land there. Arriving is never a repaint, and no longer takes that
path.

Ephemeral, presenter-triggered flourishes: a flash over the whole page, a shake,
an explosion, speed lines, a burst of emoji, a Nico-Nico-style comment sweep.

````markdown
```effects
1 : flash
2 : shake
e : burst 🎉
d : danmaku "this bit matters"
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

## W10 — Continuation: one pane carries on, the rest hold

**Difficulty B · Opus**

A pane of prose is often one slide's worth of *layout* and two slides' worth of
*text*. `fit: shrink` answers that by making the type smaller, which is the
right answer up to a point and the wrong one past it. Typst's Touying splits
the page automatically and chooses the break itself; the break is exactly the
decision an author wants to make.

```markdown
::: pane body
The first half of the argument.

<!-- next -->

The second half, on the following slide.
:::
```

**This is a build-time expansion, not a runtime feature.** A slide with *n*
breaks in one pane becomes *n + 1* slides, identical except for that pane. Say
that plainly and everything downstream keeps working unchanged: `anim`,
`annotate`, `connect`, notes and the cache all see ordinary slides.

- The marker is an HTML comment, so a plain Markdown parser shows nothing at
  all — the same reason speaker notes are written that way.
- Only one pane may break per slide. Two panes breaking at once is a cross
  product nobody can predict; report it and render the slide unsplit.
- **No page turn between continuation slides.** The other panes are identical,
  and animating them is the flicker this feature exists to avoid. The generated
  slides carry a marker the viewer reads as "cut, do not animate".
- A slide's `anim` steps repeat on each continuation, which is what an author
  writing "click 1 reveals the chart" means on every one of them.
- Decide and document: does the counter say `4 / 20` or `4 / 12 (2 of 3)`? Start
  with real slide numbers; a sub-count is a later refinement, not a blocker.

**Owns:** `mirzam-syntax` (the marker), the slide expansion in
`mirzam-cli/src/pipeline.rs`, `theme/viewer.js` (the cut).

**Landed as specified.** `expand_continuations` runs on the slide text between
splitting and parsing, so nothing downstream learns the feature exists; the
generated sections carry `data-cont="<group>"`, and `show()` in the viewer turns
the page only when the group changes. The counter shows real slide numbers.

Two things worth knowing:

- `BuildOutput::slides` stays the **authored** list — one entry per slide as
  written. The source map is about source, and splitting a slide does not
  create a second one to write back to.
- A marker inside a fence is a marker being quoted, not one being used, so the
  documentation for this feature can be written in Mirzam.

## W11 — Viewer chrome: cheat sheet, touch controls, gestures

**Difficulty C · Fable**

The viewer's keys are invisible until someone tells you about them, and on a
phone there is no keyboard to press.

- **`/` opens a cheat sheet**, as an overlay listing every key the deck
  responds to — including the `effects` this deck binds, which are the ones
  nobody can guess. `Esc` or `/` closes it.
- **A control cluster** in the bottom-right: previous, next, and a button that
  opens the same cheat sheet. Quiet by default, fading in on pointer movement
  or touch, and never in print.
- **Gestures**: swipe left/right to turn the page, swipe up for notes, and a
  two-finger tap (or long press) for the cheat sheet. The existing click-zone
  behaviour stays, since it is what a presenter with a clicker uses.
- The hint line at the bottom-left becomes redundant; fold it into this.

Constraints: no library, no layout thrash, and the cluster must not sit over
slide content — it is chrome outside the deck, like the page counter.

**Owns:** `theme/viewer.js`, `theme/base.css` (the `#hud`/`#hint` region).

**Landed as specified.** `#hint` is gone; `#chrome` holds the counter and the
cluster in one row below the deck's bottom-right corner, and `mz-awake` on
`<html>` drives nothing but an opacity. The sheet reads the same per-slide
`script.mz-fx` tag `effects.js` does, so it works in a deck that binds none and
therefore never inlines that file.

One thing the brief did not anticipate, found by driving a phone rather than
reasoning about one: **a swipe right left the deck entirely.** Chrome reads a
horizontal overscroll as *back*, and a presenter who swipes the wrong way loses
the talk. `touch-action: pan-y` on `html, body` claims the gesture; vertical
scrolling and pinch zoom still work, and a test guards the rule.

Two smaller things worth writing down:

- A long press opens the sheet, and the click the browser synthesises after it
  lands on the sheet that just opened — which closed it again. Gestures now set
  a flag the following click consumes.
- On `pointer: coarse` the sheet leads with the gestures. A cheat sheet full of
  keys is no use on a device with no keyboard.

**Corrected after use on a phone.** Binding the long press to the cheat sheet
took away the gesture a reader uses to *select text*, so a slide could no longer
be quoted from. The long press is now bound to nothing, the two-finger tap and
the `?` button carry the sheet, and a drag that starts or ends with a selection
on screen is never read as a page turn.

**The chrome no longer uses fixed colours.** It sits on `--mz-bg`, which a theme
may make light or dark, and CSS cannot ask which — so a fixed palette was
readable on one and not the other. It now borrows the deck's own paper
(`--mz-slide-bg`, `--mz-surface`, `--mz-fg`, `--mz-muted`), which the WCAG test
holds to a ratio in every theme and both modes. That guarantee is something a
literal cannot have. `--mz-muted` on `--mz-surface` joined the checked pairs,
and two light palettes needed a shade darker to pass it.

## W12 — Presenter window

**Difficulty B · Sonnet · after W11**

`P` opens a second window: the current slide, the next one, the slide's notes,
a clock and an elapsed timer. Both windows are the same self-contained file
opened with a flag, so nothing needs a server and the deck stays one file.

- The two windows stay in step over `BroadcastChannel`, falling back to
  `localStorage` events. Either window may drive; the presenter window is not
  privileged.
- Closing or reloading either one must not strand the other.
- The audience window loses nothing: no extra chrome appears on it.
- The notes panel (`N`) stays, for a talk given on one screen.

**Owns:** `theme/presenter.js`, the `P` binding in `theme/viewer.js`.

**Landed as specified,** with one deliberate change of mechanism. The link
carries **absolute state** (`{slide, step}`), not commands. Forwarding
keystrokes would drift the moment a window missed one; forwarding position is
self-healing, and a window opened halfway through a talk adopts the slide
already on screen. Verified: a third viewer tab opened late landed on slide 8
of 8 without being told how it got there.

`BroadcastChannel` covers two tabs of a served deck, as planned. It does *not*
cover two `file://` windows — they have opaque origins and never meet on a
channel — so the opener/child window handles carry the same message alongside
it. Sending on both and ignoring our own id makes the duplicate free. This
replaces the planned `localStorage` fallback, which fails on `file://` for the
same reason the channel does.

`MZDeck` on the viewer is the seam between the two files, and a test holds both
sides to it. `D` and `L` travel over the link alongside the position: dark mode
and the layout outline are properties of the deck, not of one window, and a
presenter switching to light mode means the projector too. The next-slide preview is built from each slide's markup captured
before anything ran: the animation runtime arms elements by writing inline
styles, and a preview cloned from the live DOM would show a slide with holes in
it.

## W13 — Table of contents from headings

**Difficulty B · Sonnet**

````markdown
```toc
depth: 2
```
````

Collects every heading in the deck, renders a list, and links each entry to the
slide it is on. Clicking an entry goes there.

The wrinkle is that this is the first thing in the deck that **needs to know
about slides other than its own**. Slides render independently and are cached
by content hash, so the block emits a placeholder and the pipeline substitutes
the resolved list once every slide has rendered — the same shape as the chart
placeholder, one level up.

- `depth:` limits the heading level; default 2.
- A heading only appears once, at the first slide that carries it.
- Worth having and cheap once the plumbing exists: `current:` marks the section
  being presented, which is how a section-divider deck shows progress.
- The PDF gets the same list, with the page numbers rather than links.

**Owns:** `mirzam-render/src/toc.rs`, the substitution pass in `pipeline.rs`.

**Landed as specified,** with two additions the brief did not have and one
correction it implied.

- `from:` as well as `depth:`. Almost every deck's title is an `h1` and its
  sections are `h2`, so without a floor the agenda leads with the name of the
  talk. `from: 2` is what an author actually wants.
- **The slide carrying the list is not in it.** "Agenda" is not an item on the
  agenda; listing it was the first thing that looked wrong on a real deck.
- The pass runs in `mirzam-render` and is called by *both* assemblers — the CLI
  pipeline and the WASM renderer — because the browser build assembles a deck
  too and would otherwise silently drop a table of contents the CLI produces.

The marker carries its own options (`<!--mz-toc:from:depth:current-->`), so a
cached slide takes part in the second pass without being re-rendered, and the
per-slide hashes are taken *after* resolution so `serve` still patches the right
sections when a heading changes. The PDF gets page numbers from one line of
`print.css` rather than a second rendering path: the number ships in every
entry and the screen simply hides it.

`examples/seminar.md` traded its hand-maintained agenda for this, which is the
honest demonstration — that list had to be edited every time a section moved.

## W15 — Brand and visual identity ✅

**Difficulty B · runs beside everything else**

Icon, teaser images, and a palette of Mirzam's own. Deliberately written as a
separate stream because it touches almost nothing the feature streams touch, so
it can run in parallel on its own branch and merge whenever it is ready.

**Owns, exclusively:**

| Path | What |
|---|---|
| `docs/brand/` (new) | Icon, wordmark, teaser images. SVG where possible |
| `examples/themes/*.css` | Sample themes, including any new palette |
| `examples/brand.md` (new) | A deck that shows the identity off, if one is wanted |
| `scripts/build-site.sh` | The landing page: favicon, hero, wording |
| `README.md` — the header block above `## Why` | Logo, badges, tagline |

**Must not touch** (the feature streams are in them): `crates/**`,
`docs/syntax.md`, `docs/layout.md`, `docs/quickstart.md`, `docs/workstreams.md`,
`examples/*.md` other than a new one, `crates/mirzam-cli/tests/snapshots/`.

A palette that should become a *built-in* theme rather than a sample lands as
`examples/themes/<name>.css` first and is promoted in one line later — that
keeps `theme/mod.rs`, which the feature streams edit, out of two hands at once.

**Two rules a palette must satisfy**, both enforced by
`cargo test -p mirzam-cli --test sample_themes`, and both learned the hard way:

1. **Define every token in both modes.** A one-palette theme pins the deck to
   one mode, and `D` then appears broken rather than absent. The bare `:root`
   block is light; `:root[data-mode="dark"]` is dark.
2. **Meet the contrast floors.** Body text 4.5:1 on `--mz-slide-bg` and on
   `--mz-surface`, chart marks 3:1. Dark mode made by inverting light mode fails
   this, which is exactly what the test exists to catch.

Images are inlined into every deck that uses them, so a 4000px original becomes
megabytes in every build. Downscale before committing; SVG where the artwork
allows it.

**Definition of done** is the same as every other stream: `cargo test
--workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --all --
--check`, and `./scripts/build-site.sh` completing with its link check green.


### Answers to W15's requests against W9's paths

W15 asked for four things across the boundary. All four are granted; two are
already done.

**`theme: mirzam` — done, in this repository now.** `theme/themes/mirzam.css`,
registered in `mod.rs` and `THEME_NAMES`, credited in `themes/CREDITS.md`, and
listed in `docs/syntax.md`. It rides the existing contrast test and passes in
both modes.

It is a **token translation, not a promotion**, and the difference is worth
stating because "one line later" turned out not to be true in a second sense as
well. A built-in theme is concatenated *before* `base.css`, so a type rule
written in one is overridden by the stylesheet it is meant to sit on top of.
`theme: mirzam` therefore gives a deck the palette; Space Grotesk, the weight
ladder and the violet rule under a section heading stay in
`examples/themes/mirzam.css`, reached with `css:`. Both are documented that way.
The other translation is mode order: the sample file is dark-first, a built-in
cannot be, since the bare selector is what a light-preferring reader gets.

**`docs/syntax.md` — done**, as the row above and a paragraph saying what a
named theme does and does not carry.

**`examples/brand.md` on the quality gates — granted in advance.** The deck does
not exist yet, so there is nothing to wire up; when it lands, W15 may edit these
three, which are otherwise W9's:

| Path | Change |
|---|---|
| `crates/mirzam-cli/tests/common/mod.rs` | one line in `EXAMPLE_DECKS` |
| `crates/mirzam-cli/tests/snapshots/brand.html` | generated, never hand-written: `MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden` |
| `.github/workflows/ci.yml` | one deck name in the layout-check list |

The judgement behind the request is right and worth keeping: a sample deck that
CI does not render is a deck that breaks without anyone finding out, and the
other six are only trustworthy because they are checked.

**`assets.rs` `srcset` — ratified, not reverted.** It crossed into `crates/**`,
and it should have: a `<picture>` offering a light and a dark wordmark is how
the README stays legible on both GitHub themes, and without inlining the
`srcset` the source the reader's theme picks is the one still pointing at a
relative path. The comma split is correct for data URIs with commas in the
payload, and W9 depends on it.

**`docs/brand/` stays where it is.** The root README reaches it as
`docs/brand/...`, `scripts/build-site.sh` copies from there, and every deck and
document that references the assets is already written against that path.
Moving it back would be a rename with no reader-visible benefit.

**Landed.** The mark, wordmark, icon, hero images, palette and pipeline diagram
are in `docs/brand/`; `examples/themes/mirzam.css` carries the identity as a
deck theme and every published sample deck names it. (`pitch.css` was redrawn in
the same palette at the time; it has since been deleted, having become a copy of
`mirzam.css` differing only in numbers nobody had chosen.) `assets.rs` gained `srcset` inlining, needed
so a `<picture>` offering a light and a dark wordmark still makes a
self-contained deck.

One boundary was crossed afterwards, by W9 rather than by this stream: the
landing page's "Try it" section in `scripts/build-site.sh` became "Install it",
pointing at the prebuilt binaries. That is content, not design — the page's look
is untouched.

## W14 — Linking by annotation, not by arrow

**Difficulty C · Sonnet · after W6**

An arrow from a sentence to a picture is the hardest thing in the deck to make
look deliberate: it has to leave the text without striking through it, cross
the slide without colliding with anything, and arrive somewhere meaningful.
`connect` does it, and it is still the feature most likely to look wrong.

**The linkage is what the author wants; the line is one way to draw it.** Mark
the phrase and mark the target *at the same moment, in the same colour*, and a
room reads the pairing instantly with nothing crossing the slide:

```markdown
The largest win was in [ap-ne]{#t-ap}.

```annotate
target: chart
highlight #t-ap      : color=@accent2 step=1
rect      #lat-0-2   : color=@accent2 step=1 pad=8
```
```

So: an annotation may target **text** as well as a picture, and gains the
kinds text needs — `highlight`, `underline`, `box`. Both halves are annotation
items with the same `step`, which is what makes them arrive together.

`connect` stays for what it is good at: shape to shape inside a diagram, where
both ends are boxes and the route is short. The docs should recommend that and
stop recommending text-to-picture arrows.

**Owns:** `crates/mirzam-annot`, `theme/annot.js`, the connector section of
`docs/syntax.md`.

**Landed as specified.** `highlight`, `underline` and `box` take an `#id` and
nothing else — where the words are is the browser's business, and a percentage
would be a guess that goes stale as soon as the sentence is edited.

Two things fell out of building it:

- **A phrase that wraps is two line boxes, not one rectangle.**
  `getBoundingClientRect` returns their union, which also covers the end of one
  line and the start of the next — words the author did not mark. The marks are
  drawn from `getClientRects`, one per row, with runs on the same row merged
  (a phrase carrying `<strong>` reports a rectangle per run). Measured on the
  cookbook: the union would have been 519px wide, the widest line mark is 224px.
- **`target:` is now optional** when every item is anchored. Requiring one would
  mean naming a box the author never refers to, which is exactly the shape of a
  block pairing a phrase with a chart mark. Such a block hangs on the slide
  itself (`:scope`), since an anchored mark finds its own element.

The docs no longer recommend an arrow from prose to a figure: `connect` is
presented as the tool for two boxes in a diagram, and the syntax reference sends
text-to-figure to the paired annotation. Cookbook rule 5 was rewritten to the new
way, and the connector rule beside it now points at it.

## W16 — Showing the thing working

**Difficulty C · not started**

Everything below `v0.1.0` is documentation the project cannot write in prose.

- **The demo recording.** `scripts/record-demo.mjs` exists and works; what is
  missing is deciding what gets recorded and where it lives. A README GIF has to
  be small (GitHub will not render a large one, and will not render a committed
  `.webm` at all), so it wants one deck, twenty seconds, no toggles. A full
  walkthrough is a different recording and belongs on a hosted video, linked
  rather than embedded. CI runners have a full ffmpeg, so the GIF can be
  regenerated on a schedule instead of going stale in the repository.
- **A themes gallery.** There are five built-in palettes and two sample
  stylesheets, and no page shows them. One slide, rendered in each theme and
  both modes, screenshotted by the same browser machinery the layout checker
  uses — generated rather than curated, so it cannot drift from the CSS.
- **A per-slide screenshot pass**, which the two above both want anyway:
  `mirzam` renders, a script shoots, the docs embed. Everything needed is
  already in `check-layout.mjs`.

## W17 — A theme per slide

**Difficulty B · not started**

`theme:` is deck-wide today: the name is baked onto `<html data-theme=…>` and
every slide gets it. A deck that wants one section in another palette — a dark
interlude, a slide quoting someone else's brand — cannot say so.

The tokens are already scoped in a way that almost allows it. Each built-in
theme is `:where(:root[data-theme="x"])`; dropping `:root` would make the same
block apply to any element carrying the attribute, so a
`<section class="slide" data-theme="nord">` would re-tokenise its own subtree
with no change to `base.css`.

Two things are not free, and they are the whole difficulty:

- **Mode.** The dark blocks are `:root[data-theme="x"][data-mode="dark"]` and
  `…:not([data-mode="light"])`. A section carries no `data-mode`, so a deck
  forced to dark would leave a per-slide theme in light. The selectors have to
  read the mode from the document and the theme from the nearest ancestor,
  which is a different shape: `:where(:root[data-mode="dark"] [data-theme="x"])`
  and so on, times four themes times three blocks.
- **The contrast test.** `every_theme_and_mode_meets_wcag_contrast` parses the
  token blocks by selector. It has to keep parsing them.

Frontmatter stays the default; the per-slide form is a pane-style attribute on
the slide. The point is a section that reads differently, not a deck that
changes clothes every page — worth saying in the docs, because the feature
invites the second thing.

## W18 — Carrying an element from one slide to the next

**Difficulty S · not started**

The case: a slide presents three components, and the next three slides take one
each. Today that is four page turns, and the audience re-finds the component
each time. What it should be is the component *moving* — growing out of the
overview into the slide about it, while the other two leave.

This is a shared-element transition: the same thing named on two consecutive
slides, animated between the two boxes it occupies rather than crossfaded. It is
what Keynote calls Magic Move and reveal.js calls auto-animate, and it is the
one animation feature a deck tool is asked for that Mirzam has no answer to.

The mechanism is FLIP, and the parts are all present:

- Elements are already named — `{#id}` is the same attribute annotations and
  connectors resolve against. An id appearing on slide N and slide N+1 is the
  declaration; no new syntax is strictly needed, though an opt-in keyword is
  probably kinder than making every reused id move.
- The runtime already measures boxes against a scaled deck (`annot.js` does it
  for every mark) and already owns the page turn (`anim.js` `turn`).
- The resting-state rule survives: the transition is between two states that are
  each already final, so the PDF and a reader without JavaScript see two ordinary
  slides.

The hard parts are the ones that make it Difficulty S: the two slides are
separate DOM subtrees, so the moving element has to be lifted into a layer that
outlives both; the deck's own page-turn effect must be suppressed for exactly
the elements that are moving and kept for everything else; and going *backwards*
has to be as good as going forwards, which is where most implementations of this
give up.

## W20 — Syntax highlighting at build time

**Difficulty B · landed** — synoptic in the render pass, six `--mz-code-*`
tokens per theme and mode, contrast-tested at 4.5:1; unknown languages render
byte-identically to before. Line numbers and highlight ranges stay a later
stream, as briefed.

A fenced block records its language and renders it uncoloured. That was an
honest state, not a fixed one — and the market moved: Shiki is now the default
in both Marp and Slidev, presenterm and Typst highlight too, so uncoloured
code is the single most visible gap for the developer-talk audience this tool
courts. Out of the August 2026
[market survey](reports/2026-08-market-survey.md), this is one of the two P0
items.

The question that stalled it is cost, not desire: a highlighter means shipping
grammar definitions, and the browser build already pays 103 KB gzipped for the
emoji table. So the stream starts with a measurement, not a dependency:

1. **Measure.** Build `mirzam-wasm` with each candidate — `syntect` with a
   trimmed syntax set, a smaller pure-Rust highlighter, a hand-rolled lexer
   for the half-dozen languages decks actually show — and compare the gzipped
   delta against that 103 KB yardstick. Decide after the number exists.
2. **Build** the winner into the render pass: highlighting happens at build
   time, the output is spans with classes, and the deck stays self-contained
   with no client-side JavaScript.

**Measured, 2026-08-13** (gzipped `mirzam-wasm`, baseline 1,442 KB, spike
reachable from an exported function so nothing was dead-stripped):
`synoptic` costs **+33 KB** with its full 38-language table built in — a third
of the yardstick, no dump generation, and it emits token kinds (`keyword`,
`string`, `comment`, …) that map straight onto classes. `syntect` costs
+191 KB trimmed to seven grammars and +481 KB full; worse, a trimmed dump
cannot be derived from the shipped binary set (`into_builder()` keeps dangling
context indices and panics), so trimming means vendoring `.sublime-syntax`
sources. The tree-sitter crates do not build for `wasm32` at all (C headers).
**Decision: synoptic, in both the CLI and the browser build, full table.**
syntect stays the documented fallback if grammar fidelity disappoints.

The shape of the output is fixed regardless of the engine chosen:

- **Colors come from theme tokens**, never from the highlighter's own palette.
  A `--mz-code-*` token set in `base.css`, overridable per theme, keeps Nord
  code Nord and keeps the WCAG contrast test meaningful. Highlighters that
  emit their own hex colors have that output mapped to the tokens.
- **An unknown or absent language stays a plain block** — exactly today's
  rendering, which is also what the CommonMark-compat rule demands.
- **Native and WASM builds may differ in what they can afford**, and that is
  allowed: if the grammar set is too heavy for the browser bundle, the CLI
  highlights everything and the WASM build carries the trimmed set. The
  measurement decides where that line is.

Stops at: coloring tokens. Line numbers, line highlighting and diff ranges are
a later stream once the engine exists. Sample slides go in `02-writing.md`
(code inside a pane is its territory); `docs/syntax.md` drops the "renders it
uncoloured" sentence the day this lands, and the golden snapshots are
regenerated deliberately, since every code block in every deck changes.

## W21 — An authoring contract for agents

**Difficulty B · landed** — 1 and 2 in `v0.5.0`; 3, the installed skill,
followed: `mirzam skill install` with stamp, hash guard and drift diagnostic
(`build.skill`), plus the `mirzam-writing` zip for surfaces without a terminal.

The second P0 from the [market survey](reports/2026-08-market-survey.md). The
visible trend behind it: "the AI drafts, the human reviews the diff" is
becoming how decks get written, comparison articles now score slide tools on
LLM-friendliness — and Mirzam is accidentally well-placed, because the source
renders on GitHub (the diff is reviewable) and the layout is ASCII (the model
can *see* the grid it is emitting). This stream makes that accident a
contract.

Three deliverables, in order of leverage:

1. **A machine-readable `check`.** `mirzam check --format json` emits the
   diagnostics the human-readable form prints — overflow, orphan panes,
   unresolved connectors and anchors, the lot — as structured records: kind,
   severity, slide number, pane, source file and line via the source map, and
   the message. The schema is documented and versioned in `docs/`; a field
   can be added but never renamed. This is the loop-closer: the usability
   evaluation's headline was that every persona shipped a visual defect
   unnoticed, and an agent is the persona that *will* run the checker after
   every edit if the output is parseable. Exit codes stay as they are;
   `--format text` stays the default.
2. **A syntax card the model can be handed.** One file, `docs/llms.md`,
   generated-or-checked against `docs/syntax.md`, compact enough to sit in a
   model's context: every fence kind, every frontmatter field, every
   attribute, one example each, and the sharp edges called out (attribute
   spans cannot cross lines; shapes live at slide top level). Published on
   the site as `llms.txt` the way the emerging convention has it.
3. **A skill the binary installs.** Not a file in this repository for users
   to find: `mirzam skill install` writes `.claude/skills/mirzam/` into the
   *user's* deck repository (`--user` targets `~/.claude/skills/` instead),
   with the SKILL.md and the syntax card embedded in the binary — so the
   card a model reads always matches the binary it drives, which matters
   while the markup is `0.x`. The skill's shape: check `mirzam --version`
   first (and say how to install it if absent, which is what makes the same
   skill work in a cloud session); write the deck; run
   `mirzam check --format json`; fix what it names; repeat.

   What is not free about it, and how it is paid for:

   - **Nothing in Claude Code versions a local skill.** So Mirzam does:
     `skill install` stamps the generated SKILL.md with the version that
     wrote it, and `check`/`build`, on finding a stamped card near the deck,
     compare stamps and emit an ordinary diagnostic when they disagree —
     "card is 0.5.0, binary is 0.6.0, run `mirzam skill install`" (or, for a
     teammate whose binary is the stale side, "upgrade the binary"). The
     agent reads the diagnostic in the loop it already runs and repairs the
     drift itself. That needs the JSON document to say which *binary*
     produced it, so the top level gains a `mirzam` version field — additive,
     so the schema stays at `1`.
   - **A user may have edited the installed skill.** `skill install` records
     a content hash and refuses to overwrite a modified card without
     `--force`.
   - **claude.ai, the desktop app and the phone cannot run a binary** —
     their skills execute in a sandbox with no filesystem and no network to
     fetch a release from. So there is a second, deliberately smaller skill:
     the syntax card alone, packaged as the `.zip` those surfaces upload
     (Settings → Capabilities/Customize → Skills; phones receive it by
     account sync). Claude writes correct Mirzam markdown and hands it to
     the person, whose renderer is the browser editor — which runs on the
     phone, so the degradation is honest: writing works everywhere, the
     *checking* half of the loop needs the CLI. `mirzam skill install --zip`
     emits it, and the release workflow attaches it to each release.

Stops at: the contract. No generation features in the product, no API keys,
no model calls — Mirzam stays the renderer and the checker; the agent is
somebody else's process consuming a stable interface. An MCP wrapper stays
out until a surface that needs one demands it; the two skills cover the
terminal and the sandbox.

## W22 — One door to a deck's look: `theme:` absorbs `css:`

**Difficulty B** — the token work is mechanical and testable; the frontmatter
change is a break, and breaks are where a wrong decision is expensive.

`theme:` is a choice of **colour only**, and the identity — the faces, the
weight ladder, the violet rule under a section heading — lives in
`examples/themes/mirzam.css` behind a second frontmatter key. That split is an
artefact of how the CSS is assembled, not a design anybody chose, and it reads
as a bug from outside: seven of the nine sample decks write `css:` and no
`theme:` at all, so the key that looks like the way to set a deck's look is the
one they do not use. The expectation to meet is the obvious one — **`theme: wuwei`
changes the colours and the type, and nothing else has to be written.**

Two deliverables. The first is what makes the second honest.

1. **Type becomes tokens, and the built-in `mirzam` theme carries its own
   identity.**

   The mechanism is already here and already documented: `base.css` reads 45
   distinct `--mz-*` values, and 26 of them are non-palette dials
   (`--mz-grid-pad-*`, `--mz-pane-border`, `--mz-bullet`, `--mz-slide-chrome-*`)
   that carry their current value as a fallback and that **no built-in theme
   sets**. What is missing is type: `base.css` hard-codes the body face, and
   `h1`/`h2`/`h3` hard-code size, weight and tracking. Add the vocabulary —
   `--mz-font`, `--mz-font-display`, `--mz-font-mono`; per level `-size`,
   `-weight`, `-tracking`; `--mz-body-size`/`-leading` — each with today's value
   as its fallback, so **a deck that sets none renders identically**. That is
   the acceptance test for *this step*: the golden snapshots move (the
   stylesheet is inlined in them) but the pixels do not, and the claim is
   checked by rendering, not by reading the diff. It is not the acceptance test
   for the migration below, which changes how nine decks look on purpose — the
   two are separate gates and conflating them hides a real break.

   The signature rule joins them: `--mz-h2-rule-w` defaults to `0`, so
   `base.css` can carry the `h2::after` block and no theme but `mirzam` draws
   anything. `--mz-h2-border` keeps today's full-width border as its own
   default.

   **The vocabulary is larger than the type ladder, and the whole of it has to
   be listed before the work starts** — deliverable 1 is a promise to stop if
   the identity does not fit in tokens, and that promise is worthless if the
   list is written as the work goes. Auditing every rule
   `examples/themes/mirzam.css` overrides against the `base.css` rule it
   overrides gives, beyond the type ladder above:

   | What the sample theme moves | `base.css` today | Token needed |
   |---|---|---|
   | `h1`/`h2` line-height | `h2` sets `1.25`, `h1` none | per-level `-leading`, not just `--mz-body-leading` |
   | `h3 { color: fg }` | `color: var(--mz-accent1)` | `--mz-h3-color` |
   | the violet rule's gradient | — | 2 colours + height, radius, `margin-top`; `--mz-h2-rule-w` alone draws a rule with no colour |
   | `strong { color: fg; weight: 600 }` | `color: var(--mz-accent1)` | `--mz-strong-color`, `--mz-strong-weight` |
   | `blockquote { border: accent1; color: fg }` | `border: 4px solid var(--mz-accent2)`, `color: var(--mz-muted)` | `--mz-quote-border`, `--mz-quote-fg` |
   | `pre`, `p code`, `li code` background | `var(--mz-surface)` for both | `--mz-code-bg`, `--mz-code-fg` |
   | `th { color: fg }` | background matches; no colour | `--mz-th-fg` |
   | `.card` radius, padding, fill | — (see below) | `--mz-card-*` |
   | `.title-slide` weight, tracking | sets `font-size` only | `--mz-title-weight`, `--mz-title-tracking` |

   One rule in that file is **not** expressible as a token and must land in
   `base.css` as a rule: `:is(.center, [style*="text-align:center"]) h2::after
   { margin-inline: auto }` centres a block box under a centred heading, which
   is selector logic, not a dial. That is the one carve-out; if a second one
   appears, the stop condition has been met.

   `.eyebrow`, `.metric` (with `.metric-up` and `.metric-label`) and `.card`
   move into `base.css` as token-driven vocabulary. Note that **`.card` is not
   already there** — `base.css` has `.box`, whose own comment reads "`.card` in
   the sample themes is this with a shadow and a fill; this one is in base so a
   deck that picked no stylesheet still has somewhere to put a caveat." Moving
   `.card` in means deciding what `.box` is for afterwards; two near-identical
   bordered blocks in one stylesheet is the `pitch.css` mistake again.
   `docs/syntax.md:1564` points at the *sample file* for all three rather than
   calling them base vocabulary, so that paragraph is rewritten, not cited.

   The case for moving them is stronger than "eight decks use them": all nine
   do, and `examples/seminar.md` — which loads no stylesheet at all — writes
   `[先行研究]{.eyebrow}` on line 288 and renders it unstyled today. That is a
   live bug this deliverable closes.

   Then `themes/mirzam.css` (the built-in, not the sample) sets the type
   tokens, `examples/themes/mirzam.css` mostly dissolves, and the sample decks
   write `theme: mirzam`. **That migration is the proof the design works**; if
   the identity cannot be expressed in tokens, this stream has the wrong shape
   and should stop rather than grow an escape hatch.

   **The migration flips the sample decks' default mode, and that decision
   comes first.** The two files disagree about which mode is bare, deliberately
   and with the reason written in both headers: `examples/themes/mirzam.css` is
   dark-first (`:root` is dark; light needs `mode: light`) because that is how
   the mark is drawn and how a deck is shown, while `themes/mirzam.css` is
   light-first (`:where([data-theme="mirzam"])` is light; dark arrives through
   `prefers-color-scheme` or `data-mode`) because a built-in cannot impose a
   preference on a reader whose system has one. So `css: themes/mirzam.css` →
   `theme: mirzam` turns eight dark decks into decks that follow the viewer's
   system — including in PDF export, where there is no viewer to follow.

   **Decided, and already done: the renderer keeps following the viewer, and
   the decks say what they want.** An unset `mode:` still means
   `prefers-color-scheme`. What changed is that a deck which is dark stopped
   being dark by accident of which stylesheet it loaded: `01-start`,
   `02-writing`, `03-layout`, `05-motion` and `research` now write `mode: dark`
   themselves, joining `04-components`, `06-theming` and `pitch`, which already
   did. Nothing moved a pixel — `examples/themes/mirzam.css` was pinning them
   dark through its own `:root` — which is the point: the same rendering now
   survives the loss of that file.

   `examples/seminar.md` is deliberately left naming no theme, no stylesheet
   and no mode, because "this deck follows the room it is opened in" is worth
   demonstrating in a sample rather than only describing in `docs/syntax.md`.
   So the split this migration has to preserve is eight-to-one, and it is now
   written in the decks rather than implied by a stylesheet.

   The other half is also in: the fallback theme name is `mirzam`, not
   `default`. That is a rename and not a repaint — `theme_tokens("default")`
   and `theme_tokens("mirzam")` were 66 identical token values. Both fallbacks
   (`theme_attrs`, `theme_css_for`), `theme_tokens` and the unknown-theme
   warning now name the same one.

   And then `default` went entirely: a name that only ever meant "the other
   one" is the duplication this stream exists to remove. `themes/default.css`
   is deleted, `mirzam` is the single name, and `theme: default` takes the
   unknown-name path with a message that says what to write instead rather than
   "unknown theme". Nothing repaints, because the sheet it stopped loading was
   the one it now loads under another name.

   Worth knowing while judging how much any of this matters: **on screen it
   mostly does not.** The viewer carries a mode toggle (`D`, and a button,
   deliberately — `viewer.js` notes a phone has no keyboard), it reads
   `prefers-color-scheme` when no `data-mode` is set so the first toggle goes
   the way the reader expects, and the choice persists in `localStorage` across
   decks. The toggle also keeps working on the eight decks precisely because
   `examples/themes/mirzam.css` defines both modes — a one-palette stylesheet is
   what makes `D` appear dead, which is the trap the diagnostics below catch.
   Where the frontmatter is the only lever is **PDF export and print**: there is
   no viewer, no toggle and no `localStorage`, and `mirzam export pdf` takes the
   baked mode or its own `--mode`. So the decision above is really a decision
   about what a deck's PDF looks like.

2. **`theme:` takes a built-in name or a path, and `css:` is retired.**

   ```yaml
   theme: mirzam                        # a built-in
   theme: themes/acme.css               # a file, relative to the deck
   theme: [mirzam, themes/tweaks.css]   # a built-in, then a file over it
   ```

   An entry ending in `.css` is a path; anything else is a built-in name. No
   built-in is named that way and no stylesheet path is not, so the rule needs
   no escape syntax — it costs a constraint on future theme names, which is
   cheap. A list is cascade order; a scalar is a list of one, so every existing
   `theme: nord` parses unchanged. Paths resolve relative to the deck, the same
   as `masters:` and `bibliography:`.

   The pipeline barely moves: built-in tokens, then `base.css`, then each `.css`
   entry in order — the slot `custom_css` occupies today. A file theme has to
   load *after* `base.css`; that is what lets `css:` override type now, and it
   is what will let a file theme do it after.

   `css:` becomes an alias for one release: accepted, mapped onto the list, and
   warned about with the line to write instead. Then removed. Pre-1.0 is why
   the window is one release rather than several — not a licence to skip it;
   the warning tells a user exactly what to type, and that is worth more than
   the lines it costs. What those lines actually cost is worth stating, because
   the alias is not one branch in the parser: `--css` on the CLI, the `css:`
   arm in `check.rs`'s diagnostic-code table, the wasm `FileProvider` path and
   `references.js` all have to accept both forms for that release and drop the
   old one together.

**The asymmetry to document rather than hide.** A built-in theme is tokens,
loaded before `base.css`; a file theme may write any rule, loaded after. That is
not arbitrary. Custom properties **inherit**, so a pane's `theme=` resolving
inwards is free; plain rules cascade by specificity and source order, so it is
not. Rendered proof: a `mirzam` pane inside a `wuwei` slide keeps the theme's
type when the theme is expressed as rules and takes its own when it is expressed
as tokens. So the rule to write down is a gradient, not a wall:

| Written as | Applies to | Works with a pane's `theme=` |
|---|---|---|
| tokens (`--mz-*`) | deck, slide, pane | yes |
| rules (`h1 { }`, `.foo { }`) | the deck | no |

Which is also why deliverable 1 comes first: it is what makes the upper row wide
enough to hold a real identity. A file theme written in tokens registers under
its filename stem (`themes/acme.css` → `acme`) and becomes usable in a pane's
`theme=`, which no custom theme can do today.

**That last sentence hides a requirement on the author, and it has to be
written down or it reads as a promise the renderer cannot keep.** Registering a
stem does not make a stylesheet scopable: a file that writes `:root { --mz-accent1:
… }` sets the tokens on the document, and a pane carrying `data-theme="acme"`
picks up nothing. For the stem to mean anything the file must write
`[data-theme="acme"] { … }` itself — the selector the built-ins use, minus the
`:where()`, which they only need because a deck's own stylesheet has to be able
to outrank them. So the rule is: **a file theme is usable in a pane's `theme=`
if, and only if, it scopes its tokens to its own stem.** Say so where the stem
rule is documented, and have `check` say it too — a `theme=acme` that silently
does nothing is exactly the class of failure the diagnostics below exist for.

Two more consequences of registering stems, neither costed above. A stem that
collides with a built-in (`themes/nord.css`) needs a rule; the cheap one is
that the built-in wins and the collision warns, because the alternative lets a
file in the deck's directory silently redefine what `theme: nord` means. And
`known_theme()` / `scope_attrs()` / `scope_warnings()` are pure functions over
a static list today — `scope_attrs` silently drops any name that is not
built-in, which is precisely what would drop every file theme. Making them
deck-aware is a signature change in `mirzam-render`, not a table edit, and it
is the one part of this stream that is not mechanical.

**A deck-specific class does not need a file.** A raw `<style>` block in the
deck reaches the page untouched — verified — which is the right home for the one
or two classes a single deck invents, and one fewer file than `css:` required.
`theme: [mirzam, tweaks.css]` is where that goes when it outgrows a block.

**Diagnostics.** An unknown built-in name already warns. An unreadable path
already warns, under `css:`, and keeps its wording. The addition is that the
gates `sample_themes.rs` holds the built-in themes to — every token set in one
mode set in the other, contrast floors — become `check` diagnostics that run
against a *user's* theme. A one-palette custom theme silently pinning a deck to
one mode is the trap `docs/syntax.md` spends a section warning about; the
checker should be the thing that catches it.

Stops at: the frontmatter and the token vocabulary. No theme registry, no
`@import`, no webfont fetching — the faces stay named-not-fetched, because a
deck is one self-contained file and a venue may have no network.

**Contention.** `crates/mirzam-render/src/theme/base.css` (every rule that
gains a token), `theme/themes/*.css`, and `crates/mirzam-cli/tests/snapshots/*.html`
— every deck's stylesheet changes, so this stream should not share the batch
with another that moves rendered output. Also touches `mirzam-core`
(`DeckMeta.css` → a theme list), `mirzam-render` (`themes_used`, `theme_attrs`,
`page_fingerprint`, `PageOptions.custom_css` → many), `mirzam-cli` (`--css`
retires with the key), `mirzam-wasm` (a file theme arrives through
`FileProvider`, the path the `css:` fix already built), and
`editors/vscode/src/references.js`, whose `frontmatterPath()` reads a scalar and
must learn the list form — with a case in
`editors/vscode/test/references.test.js`, since a theme file the host fails to
collect is a deck that previews unstyled. Also `known_theme`, `scope_attrs` and
`scope_warnings` in `theme/mod.rs`, per the stem rule above, and `check.rs`'s
`("css:", "build.css")` diagnostic-code mapping.

Every deck and every page that writes the key moves with it, and the list is
longer than the sample decks: `examples/*.md` (eight decks), `docs/syntax.md`
(the theming section and the utility-class paragraph), `docs/llms.md` (three
places), `docs/quickstart.md`, `docs/troubleshooting.md`, `docs/agents.md`,
`docs/ja/quickstart.md`, `docs/ja/README.md`, and
`crates/mirzam-cli/src/skill/writing-skill.md` — the last of those is W21's
authoring contract, so leaving it stale means agents keep writing a key that no
longer exists.

## W23 — Mermaid diagrams, rendered at build time

**Difficulty B · not started**

The [market survey](reports/2026-08-market-survey.md)'s P1. Diagrams-as-code
became table stakes the day GitHub rendered Mermaid natively; Marp made it a
built-in in 2026 after the request sat as its most-upvoted discussion for six
years. Waiting for the plugin system was the earlier plan, and the market moved
first.

The design follows from what is already here, and the pane-anchored shape work
settles the first question rather than opening it:

- **It follows the `chart` path, not the `shape` path.** A shape block inside a
  pane draws into a percentage coordinate space whose rectangle the build
  computes. Mermaid emits an SVG carrying its own `viewBox`, which wants to
  scale to fit the box it lands in — which is what a chart already does. Taking
  the chart path also keeps Mermaid out of the build-time pane arithmetic, so a
  margin moved only in CSS cannot desynchronise a diagram the way it can a
  shape.
- **The renderer arrives through a trait, the way a chart's CSV does.**
  `mirzam-render` must not touch the filesystem, and it must not spawn a
  process either — same reason, the WebAssembly build has neither.
  `AssetSource` is the precedent: the renderer asks, the host answers. The CLI
  implements the trait by running an external renderer; `mirzam-wasm`
  implements nothing and every `mermaid` fence stays a code block there.
- **No renderer is a warning, never a silent fallback.** Without one the fence
  renders as an ordinary code block *and* the build says so — `build.mermaid`,
  so it reaches `check --format json` and an agent repairs it in the loop it
  already runs. A deck that shipped its diagram as source code without saying
  so is the exact failure the usability evaluation found four times over.
- **Colours are rewritten to theme tokens.** Mermaid emits its own palette.
  Baked in, a diagram would ignore the deck's theme and stay light when the
  reader presses `D`. The output's fills and strokes are rewritten to
  `var(--mz-*)` references — the same move W20 made when it mapped token kinds
  onto classes rather than inlining a highlighter's colours.
- **`build` stays browser-free.** Chromium can render Mermaid, and Mirzam
  already drives one for `export pdf` and `check` — but making an ordinary
  build need a browser is a regression in what this tool is. `mmdc` if it is on
  `PATH`; a Chromium path is an opt-in second route at most.

What is not free: an external Node tool sits against "one Rust binary, no
Node", which is a real part of why people arrive. The precedent that makes it
acceptable is PDF export, which already requires a browser nobody ships with
the binary — an optional external renderer, warned about when absent, is a
shape this project has already accepted once.

Worth naming, because it points the other way from a tax: **GitHub renders a
```mermaid fence as a diagram.** This is the one extension that reads *better*
in a plain CommonMark viewer than the code block it degrades to, so it
strengthens the "source renders on GitHub" wedge instead of spending it.

`mermaid` joins `mirzam_syntax::BLOCK_KINDS` in the same change, per
non-negotiable 1.

Stops at: Mermaid. D2 arrives through the same trait once the shape is proven,
and is not part of this stream.

**Contention.** `crates/mirzam-cli/tests/snapshots/*.html` (a new fence in
`examples/04-components.md` rewrites them), `docs/syntax.md`, `docs/llms.md`.
Overlaps W22 in the snapshots and in `docs/llms.md`; W22 lands first and this
one regenerates.

## W5 — Typst-flavoured math ✅

**Difficulty A · Sonnet · landed**

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

**Landed as specified**, with two things the corpus found that the plan did not
predict: braces around a script base are not neutral in LaTeX (`{\sum}_a^b`
loses its movable limits and `{e}^x` shifts the spacing of the operator before
it, so the emitter braces only composite bases), and `sin(x)/x` requires
function names to glue to their argument list before `/` binds, or the fraction
takes the parens and leaves `sin` behind. The corpus compares MathML — Typst
source lowered and rendered, against the LaTeX a person would have written by
hand — which is what caught both. One deliberate deviation: the dialect is part
of the render cache key, because flipping `math:` changes every formula while
changing no slide's source text.

A second pass widened the surface past the v1 list — accents, letter styles,
dotted symbol variants, `vec`/`binom`/`floor`/`ceil`, delimited matrices,
`op()` — and changed the failure mode: an unknown dotted name (`subset.eq`
before it was known) or an unknown word called like a function (`hat(x)`)
used to render as the nearest letters, silently. Everything outside the
subset is now a parse error shown in red, which is the same honesty the
LaTeX path already had.

## W19 — Structural math editing: tap and place, not type

**Difficulty S · after W5**

`(-b pm sqrt(b^2 - 4a c))/(2a)` is kinder than the LaTeX it replaces, but it
is still a line of punctuation — and punctuation is what phone keyboards are
worst at. The observation behind this stream: the letters of a formula are
easy to type anywhere; it is the *structure* — what is a subscript, what
sits over what — that costs keystrokes. So let the author type `a b c` and
place the structure by touch: select a node, tap superscript, drop the next
piece into the slot that opens.

The order of work, which is also its dependency order:

1. **Spans.** Every `mirzam-tmath` AST node records the byte range of source
   it came from. Selection in the editor is a node; a node is a range; a
   range is something that can be replaced in text.
2. **A printer.** AST → Typst-math source, the inverse of the parser. The
   property that keeps it honest, over the whole corpus:
   `parse(print(tree)) == tree`. Without this, every edit operation would
   need its own way of writing text, and they would disagree.
3. **Edit operations.** Wrap in a fraction, attach a script, insert a
   symbol, delete a node — each one AST in, AST out, then the printer turns
   the result back into text. Pure functions in the crate, tested without
   any UI existing yet.
4. **The editor draws its own boxes.** Not the MathML: hit-testing
   `math-core`'s output would need a node identity that the LaTeX lowering
   erases. MathQuill proved the other route years ago — the editor renders
   its own nested HTML boxes, one per AST node, so selection and drop
   targets *are* the tree. The true rendering, from the printed source
   through the normal pipeline, sits beside it as the preview.
5. **The output is text.** The editor emits a Typst-math string into the
   Markdown and nothing else. A deck edited this way is indistinguishable
   from a deck typed by hand, which is what keeps the editor optional and
   the deck format plain.

Steps 1–3 are crate work with no UI decisions in them, checkable by tests,
and worth doing even if the UI stalls: spans sharpen every parse error, and
the printer is what any future tool that *writes* math needs.

**Steps 1–3 landed.** `parse` exposes the spanned tree, `print` writes it
back, `edit` holds the operations, and the corpus now carries the round-trip
property. Two things the property test decided that the plan had left open:
a parenthesised group of exactly one thing normalises to that thing at
parse time, or a reparse of printed output could never reproduce an
editor-built tree (`a_(b^c)` must equal the tree the editor assembled, not
that tree wrapped in a group); and scripting a fraction parenthesises it in
the *operation*, because `(a/b)^2` is writable and `Script{Frac}` is not —
the tree is kept inside what the syntax can say, which is what keeps
"indistinguishable from a deck typed by hand" true.

**Steps 4–5 landed as the Math panel in the browser editor.** The wasm side
exposes exactly two calls — `math_state` and `math_apply` — and the JS keeps
no model: every tap sends one operation, gets back new source, a tree for
the boxes and MathML for the preview. Selection taught the design two
things: placing something selects it (so "type x, tap x²" needs no
intermediate tap), and tapping a selected box must *not* toggle it off, or
the flow above breaks — clearing selection is what the empty space beside
the boxes is for. Insertion guards the frontmatter: a cursor that never
moved sits at offset 0, and a formula must not land inside the YAML. What
v1 does not do, knowingly: no drag-and-drop (tap-select then tap-place
covers the flows drag was imagined for), and no in-place editing of matrix
cells (the boxes show them; the entry field writes `mat(...)` whole).

One thing v1 got wrong and testing on a real deck caught: it offered to
set `math: typst` on any deck, which breaks every LaTeX formula already
there — a dialect is per deck, so flipping it is destructive, not a
convenience. The panel now adapts its *output* instead: Typst-math into a
Typst deck, the same tree lowered to LaTeX into a LaTeX deck (built once,
`to_latex` was already there). Re-editing by tap stays a Typst-deck
feature, since LaTeX cannot be parsed back; in a LaTeX deck the cursor
inside a formula inserts after it rather than mangling it.

First-user feedback then removed the remaining ceremony. Put and Move —
each a mode, each an extra tap — became a drop and a keystroke: dragging a
palette symbol or a node onto the formula chooses the slot by where it
lands (top is the shoulder, bottom the subscript, the sides its
neighbours, a hole takes anything), with the slot previewed beside the
finger; the entry places on Enter. The same feedback found the one real
data-loss bug: the panel anchored its insert to byte offsets captured at
open, and editing the deck underneath moved them — it anchors to the
formula's *text* now and re-finds it. Two structural rules came out of the
drag work: deleting a container's only child leaves a hole (an empty
`sqrt()` has nothing left to aim an edit at), and dropping onto a hole is
its own move slot.

The second round of feedback was about the drops that did not land:
before/after lived in a band a few pixels tall, a fraction could not take
anything into its halves, a full root could not take more. Three fixes,
one per cause. Geometry: the side strips are before/after at any height
and never thinner than 12px. Semantics: `Into` means something everywhere
— a hole is filled, a container's contents grow, a leaf becomes a run —
and "beside" a node in a fixed slot joins it there, since a numerator has
no sibling list. Reality: a fraction's face is entirely its halves, so
the drop rule reads the parent — landing on a numerator joins the
numerator, which *is* "dropping onto the top of the fraction". Palette
drops go through one `place` operation now, the same landing rules as
`move` minus the deletion.

The proxy boxes went away before that. Step 4 as planned drew its own
boxes because `math-core`'s output has no node identity — but nothing says
the *editor's* preview must come from `math-core`. A second emitter draws
the structure itself (`mfrac`, `msup`, `msqrt`) stamping `data-path` on
every element, and hands each leaf to the real converter so every glyph is
the one the deck will show — so the tap surface *is* the typeset formula.
On top of that landed **Move** (tap the node, tap the destination, pick
shoulder/subscript/before/after — one wasm-side operation, because the
deletion shifts the very paths a two-step caller would aim with) and the
placement rule a first user's fingers taught: a hole is filled, a node in
a sequence is continued after, a node in a fixed slot is replaced —
"after" does not exist inside a denominator.

**Where it stops.** It lives in this repository — the AST layer in
`mirzam-tmath`, bindings in `mirzam-wasm`, the component under `web/` —
because the editor and the grammar have to move together while the grammar
is still growing. If it ever earns users outside Mirzam, extraction is
mechanical, because the boundary is text in, text out. It edits the Typst
dialect only: a `math: latex` deck does not get it, by design. And it is an
editor, not an IME — no handwriting recognition, no guessing.

**Owns:** spans, printer and edit operations in `mirzam-tmath`, their
`mirzam-wasm` bindings, and the editor component under `web/`.

**Withdrawn after field testing, before any release carried it.** On the
phone it was built for, fingers covered the drop targets, and the verdict
from real use was blunt: typing the Typst source was faster than dragging
it into shape. The panel UI is gone from the browser editor; the wasm
bindings (`math_state` / `math_apply`, the path-stamped MathML emitter)
still compile behind the `math-editor` feature of `mirzam-wasm` — off by
default, costing the shipped binary nothing — and their tests still run.
Steps 1–3 were never the experiment and stay in the crate unconditionally:
spans sharpen every parse error, the printer is what any tool that writes
math needs, and the round-trip property now guards the grammar as it
grows. The effort that would have gone into step 4's second iteration went
into the grammar instead — brackets and mixed fences, spacing words, wide
accents, `|->` and the rest — because the real lesson of the testing was
that the *text* is the editor, so the text had better be able to say
everything.

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

## W7 — Source map through transclusion ✅

**Difficulty A · Sonnet · landed**

`expand_includes_tracked` knows *which* files a deck came from; `SourceMap`
knows *where*. `expand_includes_mapped` returns it alongside the expanded
text, `SlideSpan` carries each slide's offset in that text, and `BlockSpan`
carries each fenced block's range within its slide. Composing the three turns
"this block, on this slide" into "these bytes, in this file" — which is the
whole point, and the round trip is tested through a real build.

Three things the design turns on:

- **A run, not a table.** Spans are coalesced while the invariant
  `out.len() == src.len()` holds, so an ordinary deck is a handful of runs and
  lookup is a binary search. A CRLF line breaks the run rather than being
  folded in — the expansion emits `\n` where the file has `\r\n`, and merging
  would drag every later offset out of place by one byte per line.
- **Refusing beats guessing.** `resolve` returns `None` for a range that
  covers generated text (the note left in place of a circular include) or
  crosses a file boundary. A caller about to rewrite those bytes must be
  stopped, not handed a range that means something else.
- **Substitution is a derivation.** Variable substitution rewrites lines and
  changes their length, so the map is carried through it: a line left alone
  still points at its file, and a line that had a `{{ }}` in it points at
  nothing. The value on screen is not text anyone typed there.

Useful already, ahead of W8: a warning on a slide that came from an included
file now names that file.

**Owns:** `crates/mirzam-syntax`.

## W8 — Annotation editing, written back to Markdown (deferred)

**Difficulty S · Opus · deferred; W7 landed, so the source map is ready**

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

## W9 — Release hardening and `v0.1.0` ✅

**Difficulty A · Opus · last**

`main` is already where the work lands; what is left is calling a version of it
finished.

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

**Landed, with one thing added and one reinterpreted.**

- **Prebuilt binaries were pulled into this stream.** They were not in the brief,
  and they were the largest thing standing between the project and anyone who
  wanted to use it: the honest answer to "how do I make a deck?" began with
  "install Rust". `.github/workflows/release.yml` builds five targets on a `v*`
  tag, smoke-tests each native one by building a deck with it, and publishes
  archives with checksums and the tag's changelog section as the notes.
  `scripts/install.sh` is the one-liner on the other end. The tag must equal `v`
  plus the workspace version, and the build fails if it does not — the install
  script derives its URL from the tag, so a mismatch would publish archives
  nobody could fetch.
- **`commonmark_compat.rs` now walks a list rather than repeating one.**
  `mirzam_syntax::BLOCK_KINDS` is the canonical set of fenced forms, and a test
  in `mirzam-syntax` proves every name on it is live. Adding a block form
  without a compatibility story now fails a test instead of passing silently,
  which is the difference between a promise and a habit.
- **"An annotation drawn outside its target" was the wrong check to write.**
  W14 made anchored marks legitimately land anywhere on the slide — that is what
  pairing a phrase with a bar *is* — so containment within a target would have
  failed correct decks. What the checker gates on instead is a mark that could
  not be drawn at all: the runtime counts them (`MZAnnot.missing`), because a
  dropped mark is silent by design and a sentence that says "the circled bar"
  outlives the circle. The other two landed as written, via `MZAnim.armed` and a
  look at `<html class="mz-debug">` before anything else is measured.
- **Two things the release found.** The repository had no `LICENSE` file despite
  the README claiming MIT, and the declared Rust floor (1.75) had been wrong
  since `math-core` landed. Both are fixed, and a CI job now builds on exactly
  the declared toolchain so the second cannot drift again.
- Benchmark re-measured: edit latency at 500 slides went 2.3 ms → 3.2 ms, from
  the whole-document passes W10 and W13 added, not from rendering. The shape is
  unchanged and the numbers in [roadmap.md](roadmap.md) now match.
