# Mirzam syntax

Mirzam decks are CommonMark files. Everything below is an addition that a plain
Markdown parser still renders as readable text — that rule is enforced by
`crates/mirzam-cli/tests/commonmark_compat.rs`.

| Extension | What a plain Markdown parser shows |
|---|---|
| Fenced blocks (`pane`, `shape`, `connect`, `chart`) | A code block |
| Those same blocks inside a longer fence (````` ```` `````) | Quoted, not executed — this is how a document shows Mirzam syntax |
| Fenced divs (`::: pane main`) | A paragraph of text |
| Inline attributes `{#id .class k=v}` | Literal text (Pandoc reads them as attributes) |
| Variables `{{ price * 12 }}` | Literal text |
| Transclusion `![[file.md]]` | An image-like link (Obsidian embeds it) |
| Speaker notes `<!-- note: ... -->` | Nothing; it is an HTML comment |

## Deck and slides

### Frontmatter

```yaml
---
title: Quarterly review
author: Your Name
aspect: "16:9"        # or "4:3"
css: themes/dark.css  # custom stylesheet, relative to this file
transition: fade      # how pages turn; see Animations below
vars:
  product: Mirzam
  price: 1200
---
```

### Slide breaks

Slides are separated by a horizontal rule (`---`) outside code fences.

A document written without slide breaks — a README, a set of notes — becomes a
deck by starting a new slide at every heading:

```bash
mirzam build README.md --split h2      # or h1, h3
```

or, for a file you own, in frontmatter:

```yaml
split: h2
```

`---` still breaks slides either way. Content before the first heading becomes the
opening slide. A section longer than a slide will overflow; the layout checker
reports it and [the layout guide](layout.md) says what to do about it.

### Splitting a deck across files

```markdown
![[sections/method.md]]
```

The file is expanded in place, slide breaks included. Frontmatter in the included
file is ignored, and circular includes are reported rather than followed.

### Speaker notes

```markdown
<!-- note: Skip the derivation if time is short. -->
```

Press `N` in the viewer to show them.

## Layout

For how space is allocated and what to do when content does not fit, see the
[layout guide](layout.md).

A slide's layout is one `pane` block. Without it the slide is a single pane.

````markdown
```pane
+--------------------+-------------+
|  head                            |
+--------------------+-------------+
|                    |             |
|  main              |  fig        |
|                    |             |
+--------------------+-------------+
```
````

- `+ - |` draw the borders; the identifier inside a cell names the pane.
- Column widths come from the character widths between borders; row heights from
  the number of lines. Draw a taller band to give it more of the slide.
- Repeat a name in adjacent cells to merge them. Merged regions must be
  rectangular, the same constraint CSS Grid areas have.
- Use `.` or leave a cell blank to leave it empty.

Assign content with a fenced div:

```markdown
::: pane main
Ordinary Markdown goes here.
:::

::: pane fig {align=center valign=middle}
![Result](img/result.svg){fit=contain}
:::
```

Pane attributes: `align=left|center|right`, `valign=middle|bottom`, and any extra
`.class` names your stylesheet defines. Content that is not assigned to a pane
flows into `main`, or the first pane if there is none.

### Background images

A pane can carry a photograph behind its text, with the treatments that make the
text readable over it.

```markdown
::: pane hero {.bleed bg=media/bg/city.jpg dim=0.4 blur=3 scrim=bottom}
# Ship the story
Plain Markdown in. Presentation-grade decks out.
:::
```

| Attribute | Values | Effect |
|---|---|---|
| `bg=` | a path | The image. Local files are inlined like any other asset. |
| `bg-fit=` | `cover` (default), `contain` | How the image fills the pane. |
| `bg-pos=` | a CSS position, e.g. `top`, `20% 40%` | Which part survives the crop. |
| `dim=` | `0`–`1` | Darkens the whole image. `0.4` is a good starting point. |
| `blur=` | pixels | Pushes the photo out of focus so text reads first. |
| `scrim=` | `bottom` (default), `top`, `left`, `right` | Fades that edge to black, leaving the rest of the photo visible. |
| `text=` | `light`, `dark` | Overrides the text colour. Light is chosen automatically whenever `dim` or `scrim` is set. |
| `.bleed` | class | Takes the background to the slide edge. Meant for a slide whose background covers everything, such as a title; it removes the grid's padding. |

`dim` and `scrim` combine: `dim` sets the floor, `scrim` adds the gradient on top
of it. If you set only `scrim`, the gradient runs from 0.75 to transparent.

Photographs are the one asset that can dominate a deck's file size. A
1600px-wide JPEG at quality 70 is around 100 KB; a 4000px original is several
megabytes, and it is inlined into every build. Downscale before you commit.

To pull photos from Unsplash, with the attribution the API requires:

```bash
export UNSPLASH_ACCESS_KEY=...
./scripts/fetch-backgrounds.sh mountains "city at night"
```

The images in `examples/media/bg/` are drawn by
`scripts/make-sample-backgrounds.py`, not downloaded, so the repository builds
with no network access.

## Inline syntax

### Attributes

```markdown
## Heading {#intro .center}
[a phrase]{#anchor .u}
![Figure](img/a.png){#fig1 fit=contain w=80%}
```

`#id` names an element so `connect` and (later) `anim` can target it. `.u`
underlines, `.center` / `.right` align, `.small` de-emphasizes; themes add more.

### Variables and arithmetic

```markdown
{{ product }} costs {{ price * 12 }} per year, or {{ round(price / 30) }} per day.
```

Values come from frontmatter `vars`. Arithmetic, parentheses, and `round`, `ceil`,
`floor` are supported. Anything that fails to evaluate is left as written, so a
typo never silently deletes text.

### Math

```markdown
Inline $E = mc^2$, and display style:

$$
\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
$$
```

LaTeX is converted to MathML when the deck is built, so nothing runs in the
browser. Decks containing math bundle the STIX Two Math font (~540 KB) so they
render identically on machines without one installed. If a formula fails to
convert, its source is shown in red with the error in the tooltip.

### Media

```markdown
![Demo](media/demo.webm){.autoplay .loop .controls poster=media/first.png fit=contain}
![Animation](media/loop.gif){w=60%}
```

`mp4`, `webm`, `ogv`, `mov` become `<video>`; everything else stays an image.
`autoplay` implies `muted`, since browsers block audible autoplay. In PDF output a
video is replaced by its poster, or a placeholder if none was given.

Prefer `webm` for distribution: Chromium builds without proprietary codecs cannot
play H.264.

## Charts

````markdown
```chart
type: bar          # bar | line | area | pie
id: latency        # optional; ids of individual marks derive from it
title: p95 latency by region (ms)
y_label: ms
highlight: after   # dim every other series
data: |
  region, before, after
  us-east, 210, 120
  ap-ne, 380, 180
```
````

The first column holds categories; every other column is a series. `data` may
instead name a `.csv` file, which is resolved like any other asset (and watched by
`mirzam serve`). Values may contain `%` or thousands separators.

Each mark gets an id of the form `<chart-id>-<series>-<row>`, so the second bar of
the first series above is `#latency-0-1`. That is what makes it possible to point
an arrow at one bar. For a bar chart the id names a group holding the bar *and*
its value label, so animating a mark moves the number with the bar.

## Shapes

Shapes are drawn in page coordinates (percentages), on a layer above the panes.

````markdown
```shape
rect    #cache at(72%, 30%) size(30%, 14%) label="Cache" fill=@shape-fill stroke=@accent2
ellipse #db    at(72%, 70%) size(26%, 16%) label="Database"
arrow   from(#cache.s) to(#db.n) style=dashed
line    from(10%, 90%) to(40%, 90%)
text    #cap   at(72%, 88%) "95% hit rate" .small
```
````

- Shapes: `rect`, `ellipse`, `text`, `arrow`, `line`.
- Edges for endpoints: `.n`, `.s`, `.e`, `.w`, `.c`.
- Colors: `@accent1`, `@accent2`, `@shape-fill`, … resolve to theme variables, so
  shapes follow a theme change. Literal CSS colors also work.

## Connectors

```markdown
The [edge cache]{#t-edge .u} answers first.

```connect
#t-edge -> #cache.w : color=@accent2 style=dashed
#a <-> #b
#a -- #c : curve=0
```
```

- Operators: `->` (arrow), `<->` (both ends), `--` (plain line).
- Either endpoint may be a text anchor, a shape, or a chart mark.
- Omit the edge and Mirzam picks the natural one from relative position.
- Attributes: `color=`, `style=dashed`, `curve=` (0 for a straight line).

Connector endpoints are resolved in the browser *after* layout, on every show,
resize and hot reload. That is why arrows keep pointing at the right thing when
the window changes size or the theme changes metrics.

## Presentation effects

Flourishes the speaker fires with a key, bound per slide:

````markdown
```effects
1 : flash
2 : shake
3 : lines
4 : boom
e : burst 🎉
c : confetti
m : danmaku "そこ、大事です"
```
````

| Effect | |
|---|---|
| `flash` | one bright pulse over the slide |
| `shake` | the slide shakes |
| `lines` | 集中線 — lines converging on the middle |
| `boom` | an explosion out of the centre |
| `burst <emoji>` | emoji thrown upward |
| `confetti` | paper instead of emoji |
| `danmaku "<text>"` | a comment sweeps across, Nico-Nico style |

**This is not animation, and the difference is the point.** An `anim` block
belongs to the document: ordered, deterministic, and present in the PDF. An
effect belongs to the *performance* — it happens because someone pressed a key
in front of an audience, it never reaches the exported file, and a talk where
none of them fire is the same talk. Nothing here can change what the deck says.

- One key per line, one character. `Esc` clears anything still on screen, and
  turning the page cancels it.
- `← → Space PageUp PageDown Home End N F L D Esc` belong to the viewer;
  binding one is a build warning, not a silent shadowing of navigation.
- No effect may reflow the slide — they animate transforms and opacity only,
  in a throwaway layer above the page.
- Under `prefers-reduced-motion` the movement is dropped and the flash is brief.

[`examples/motion.md`](../examples/motion.md) has a slide bound to all seven.

## Annotations

Circle the part you are talking about, point at it, label it. An `annotate`
block sits beside the pane it decorates, the way `connect` does:

````markdown
::: pane shot
![p95 by region](img/latency.png)
:::

```annotate
target: shot
circle 62,38 34x34 : label="the hot corner"
rect   10,10 20x14 : color=@accent2 style=dashed
arrow  18,86 -> 55,48
text   6,90 "coordinates are percentages of the picture"
```
````

- **`target:`** is a pane name, or a `#id`. **A pane holding one picture means
  that picture** — a photo, a video or a chart. That matters: a picture is
  centred in its pane and rarely fills it, so measuring the pane would put
  every mark somewhere you did not point.
- **Coordinates are percentages of the target**, and `x,y` is the *centre* of
  a `rect` or `circle`, the way `shape` reads. `WxH` is its size. So the
  annotation stays put when the pane is resized, the deck is projected at a
  different aspect, or the picture is replaced with a bigger one.
- **An anchored item needs no coordinates at all.** Write `circle #latency-1-2`
  and the mark is taken from that element's live box — a chart mark, a shape,
  anything with an id. `pad=` in pixels gives it room to breathe. This survives
  a data change, which coordinates do not.
- **Attributes:** `label=`, `color=` (a `@token` or a literal), `style=dashed`,
  and `pad=` for anchored items.
- Either end of an `arrow` may be an anchor: `arrow 12,70 -> #latency-1-2`
  stops at the edge of the mark rather than in the middle of it.

A block whose `target:` or anchor matches nothing on the slide is a warning,
not a build failure: the slide renders unannotated and the warning names it.

Annotations are resolved in the browser after layout, like connectors — and,
unlike everything else that runs there, the overlay is inlined into the PDF
export too, so the marks survive the export. See
[architecture.md](architecture.md#annotations-and-the-pdf) for why that is the
one script the print page carries.

## Animations

````markdown
```anim
[enter]   .title       : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[after #latency-0-2 +200ms] .caption : fade-in 300ms
[exit]    slide        : iris-out 500ms
```
````

One line is one track: `[trigger] target : effect duration attributes...`.

- **Triggers:** `enter`, `click N` (the Nth click-to-advance within the
  slide), `exit`, and `after #id [+Nms]` (relative to another track's target,
  the offset optional and possibly negative).
- **Targets:** a `#id` or `.class`, or the literal `slide` for the whole
  section. A `shape` with an id is one group — a box and its label, an arrow
  and its head — so animating it moves the whole thing. An optional `chars`, `words` or `lines` keyword before the effect
  name splits the target's text at build time — the wrapping spans are already
  in the HTML, so the runtime only ever selects them, never mutates the DOM to
  make them. Splitting never breaks inline markup (`<strong>` and friends stay
  intact), a multi-byte character, or an HTML entity.
- **Effects:** `fade-in`, `fade-out`, `slide-in` / `slide-out` and `wipe-in` /
  `wipe-out` (all four require `dir=left|right|up|down`), `zoom-in`,
  `zoom-out`, `blur-in`, `grow-x`, `grow-y`, `pop`, `draw`, `iris-out`.
  A `slide` travels; a `wipe` stays put while an edge uncovers it. `draw`
  runs the strokes tip-first over the full duration and inks the fills —
  an arrow's head, a label's glyphs — in over the last stretch, once the
  line has arrived at them.
- **Attributes:** a bare `400ms` sets the duration; `delay=`, `stagger=` (for a
  split target) and `ease=` are otherwise `key=value`. `ease` is a named curve
  (`out-cubic`, `in-out-back`, …) or `spring(mass,stiffness,damping)`, resolved
  to a sampled curve at build time so nothing simulates physics in the browser.

A line that points at nothing — a target that matches no element, or an
`after` reference to a missing id — is a warning, not a build failure: the
slide renders unanimated and the warning names the offending line.

### Presenting an animated slide

`→` advances to the slide's next `click` step; once the steps run out it turns
the page. `←` steps back, then goes to the previous slide. The page counter
shows the step alongside the slide (`3 / 12 · 1/2`) when there is one. Arriving
at a slide from a later one shows it with every step already played, since it is
a slide the room has already seen.

Stepping back within a slide snaps rather than playing in reverse: going back is
a correction, and a correction should be immediate.

### Slide transitions

How pages turn is a deck-wide setting, because it is the same pair of
whole-slide tracks repeated on every slide:

```yaml
---
transition: slide-left 400ms ease=out-cubic
---
```

`none`, `fade`, `slide-left`, `slide-right`, `slide-up`, `slide-down`,
`wipe-left`, `wipe-right`, `wipe-up`, `wipe-down`, `zoom` and `iris`, each
optionally with a duration and an `ease=`. Going backwards plays the
directional ones the other way.

[`examples/motion.md`](../examples/motion.md) demonstrates all of this: text
entrances, a chart whose bars grow one click at a time, a diagram that assembles
itself box by box, and a slide that overrides the deck's page turn.

A slide that declares its own whole-slide track overrides the matching half —
`[enter] slide : …` replaces the incoming transition for that slide, `[exit]
slide : …` the outgoing one. There is no separate per-slide `transition:`,
because that is what those two tracks already are.

### What animation never changes

Elements are laid out in their **final** state, and the runtime is the only
thing that ever puts one in its starting state. So a deck read without
JavaScript, and the PDF export — which ships no scripts at all — both show every
slide fully revealed. Animation is something a deck gains in a browser, never
something it depends on.

Under `prefers-reduced-motion` the reveals still happen, and stepping still
works, but nothing travels to get there: an element appears instead of arriving.

## Theming

### Named themes

```yaml
---
theme: nord
---
```

| Name | Source |
|---|---|
| `default` | ours |
| `nord` | [Nord](https://www.nordtheme.com/), MIT |
| `solarized` | [Solarized](https://ethanschoonover.com/solarized/), MIT |
| `vscode` | VS Code Light+/Dark+, MIT |

An unknown `theme:` name is a warning, not a build failure, and falls back to
`default`. See
[`themes/CREDITS.md`](../crates/mirzam-render/src/theme/themes/CREDITS.md) for
where each palette comes from and how it maps to Mirzam's tokens.

### Dark mode

Every built-in theme defines both a light and a dark palette. Which one shows:

1. `mode: dark` (or `mode: light`) in frontmatter, if set - baked into the
   deck, so there is no flash of the wrong palette on load.
2. `?mode=dark` in the URL, read by the viewer before it draws anything.
3. `D` in the viewer, which toggles for that reading session only.
4. Otherwise, the reader's `prefers-color-scheme` - a deck with no explicit
   `mode:` just follows the system, live, with no reload.

### Custom CSS

Set `css:` in frontmatter and override the theme tokens - this layers on top
of whichever `theme:` is selected, built-in or default:

```css
:root {
  --mz-slide-bg: #0d1117;
  --mz-fg: #e9edf5;
  --mz-accent1: #5b8cff;
  --mz-accent2: #2dd4bf;
  --mz-chart3: #f6c177;   /* chart series 3-6 */
}
```

See [`examples/themes/pitch.css`](../examples/themes/pitch.css) for a complete
theme, including utility classes such as `.card`, `.metric` and `.eyebrow` that
the sample decks use.
