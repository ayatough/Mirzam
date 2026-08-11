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
| Marks `==highlight==`, `++underline++`, `:tada:` | Literal text (many renderers, GitHub included, read them) |
| Term lists (`Term`, then `: meaning`) | The two lines, as written |
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
| `bg-light=`, `bg-dark=` | a path | A different image for that colour mode, overriding `bg=` there. Naming one leaves `bg=` as the other mode's image. |
| `bg-fit=` | `cover` (default), `contain` | How the image fills the pane. |
| `bg-pos=` | a CSS position, e.g. `top`, `20% 40%` | Which part survives the crop. |
| `dim=` | `0`–`1` | Darkens the whole image. `0.4` is a good starting point. |
| `blur=` | pixels | Pushes the photo out of focus so text reads first. |
| `scrim=` | `bottom` (default), `top`, `left`, `right` | Fades that edge to black, leaving the rest of the photo visible. |
| `text=` | `light`, `dark` | Overrides the text colour. Light is chosen automatically whenever `dim` or `scrim` is set. |
| `.bleed` | class | Takes the background to the slide edge. Meant for a slide whose background covers everything, such as a title; it removes the grid's padding. |

`dim` and `scrim` combine: `dim` sets the floor, `scrim` adds the gradient on top
of it. If you set only `scrim`, the gradient runs from 0.75 to transparent.

A deck that is read in both colour modes can name a photograph for each:

```markdown
::: pane hero {.bleed bg-light=media/bg/dawn.jpg bg-dark=media/bg/night.jpg dim=0.35}
```

Both images are inlined, and the deck shows whichever matches the mode it is in
— including after the reader presses `D`, which a `<picture>` element could not
follow: its `media` query can only ask the operating system. The treatments
(`dim`, `blur`, `scrim`, `text`) apply to both, so pick a pair that wants the
same handling. `text=dark` is often the right one here: it takes the theme's own
foreground colour, which flips with the mode the way the photo does.

A PDF has no reader to ask, so the export follows the deck's `mode:` and prints
the light image when there is none. A deck whose stylesheet is dark by default
should say `mode: dark`, or its PDF will pair the light photo with light text.

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

### The Markdown you already write

All of CommonMark works on a slide, plus GitHub's tables, strikethrough and task
lists. Listing it may look redundant; it is not. This reference described only
what Mirzam *adds* for two releases, so "does it do tables?" had no answer here,
and both strikethrough and task lists shipped working and undocumented.

| | |
|---|---|
| `**bold**`, `*italic*`, `inline code` | as anywhere |
| `~~text~~` | strikethrough (GFM) |
| `# ` to `###` | headings; `#` is the deck's title |
| `- ` and nested `  - ` | bullets |
| `1.` numbered lists | the renderer counts, so inserting an item renumbers the rest |
| `- [ ]` / `- [x]` | task lists (GFM) |
| `> ` | a quotation |
| `***` or `___` | a horizontal rule — **not** `---`, which breaks the slide |
| `[text](url)`, bare URLs | links, kept clickable, printed beside the words in the PDF |
| `| a | b |` | tables; `---` left, `---:` right, `:---:` centred |
| ` ``` ` fences and indents | code blocks. The language is recorded but not yet coloured — syntax highlighting is not implemented |
| `[^key]` | footnotes, landing on the slide that cites them |
| `<!-- -->` | comments; `<!-- note: -->` is a speaker note |
| raw HTML | passed through, so `<div class="box">` works |

`crates/mirzam-cli/tests/markup_coverage.rs` holds this table, the renderer and
`examples/02-writing.md` to each other: a mark that renders but is missing from
either fails the build.

### Attributes

```markdown
## Heading {#intro .center}
[a phrase]{#anchor .u}
![Figure](img/a.png){#fig1 fit=contain w=80%}
```

`#id` names an element so `connect` and (later) `anim` can target it.

The classes the renderer brings, before any theme adds its own:

| Class | |
|---|---|
| `.u` | an accent-coloured rule under the words |
| `.center` `.right` | alignment |
| `.small` `.big` `.huge` | size |
| `.muted` `.accent` `.accent2` `.danger` | colour |
| `.box` | a bordered aside |

Every colour here is a theme token, so it moves with the palette and survives
`D`. That is also why there is no syntax for writing a colour directly: a hex
value picked against a white slide is the one thing that cannot follow the
deck into dark mode.

### Marks beyond CommonMark

```markdown
==marked== and ++underlined++ and :tada:

Term
: What the term means.
```

| Written | Becomes | In a plain Markdown reader |
|---|---|---|
| `~~text~~` | struck through | struck through — this one is GFM, not ours |
| `==text==` | a marker-pen wash in the accent colour | literal `==text==` |
| `++text++` | an underline | literal `++text++` |
| `:tada:` | 🎉 | literal `:tada:` |
| a line, then `: definition` | a term list | the two lines, as written |

A term list sets the definition **beside** its term rather than under it, the
way Typst sets one:

```markdown
Apple
: A red fruit.

Orange
: A mandarin.
```

```
Apple: A red fruit.
Orange: A mandarin.
```

The definition follows immediately rather than lining up in a column with its
neighbours — a column has to be as wide as the longest term, so one long entry
maroons every short one from its own definition. A definition that wraps is
given a hanging indent instead, so its second line clears the terms above it.

**Underline is `++`, not `__`.** Some editors take double underscores for an
underline; CommonMark and GFM both read them as **bold**, and Mirzam's whole
premise is that the same file renders on GitHub. Taking `__` would mean a
document written anywhere else silently changes meaning when it becomes a
deck — with no way to warn, since `__bold__` is perfectly valid markup.

Typing the emoji character directly always worked and still does; the
shortcode is for the keyboards that make that hard.

Task lists work too — `- [ ]` and `- [x]` — and always have. Mirzam draws the
box itself rather than leaving the browser's: the native one is about 13px
whatever the type around it does, and takes its colour from the operating
system, which is the one mark on a slide that would not follow the theme or
`D`.

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

#### A `<picture>` that picks art by colour scheme

The markup a README uses so its logo survives GitHub's dark theme is rewritten
into one image per mode:

```html
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo-dark.svg">
  <img src="logo-light.svg" alt="Mirzam" width="340">
</picture>
```

Written as-is it would follow the **machine**, while the deck's mode follows
`mode:`, `?mode=` or the reader pressing `D` — so a light deck on a dark phone
showed the pale logo on a white slide. Both images still ship; which one is
displayed now follows the deck. Every other attribute you wrote, `alt` and
`width` included, is carried into both copies.

A `<picture>` whose sources select on anything else — a width breakpoint, a
`webp` fallback — is left alone, because there the element is doing a job this
would break.

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

### When to reach for one, and when not to

**A connector is at its best between two boxes in a diagram**: both ends are
shapes, the route is short, and the line is part of the picture rather than
something laid over it.

**From a sentence to a figure, prefer a [paired
annotation](#tying-a-phrase-to-a-figure).** An arrow from prose has to leave the
text without striking through it, cross the slide without colliding with
anything, and arrive somewhere meaningful — three problems, none of which the
audience asked for. Marking the phrase and the target *at the same moment, in
the same colour* says the same thing with nothing travelling between them, and
it survives an edit to the sentence.

The connector syntax is not going anywhere; it is simply the wrong tool for
that particular job.

## Audio, and video that lives somewhere else

```markdown
![Interview with the author](media/talk.mp3)
![The paper's own talk](https://www.youtube.com/watch?v=…)
```

- An audio file becomes a player with the alt text as its label, inlined like
  any other asset — a deck with a recording in it is still one file.
- A YouTube or Vimeo page URL becomes an embed, served from
  `youtube-nocookie.com`. **This is the one thing in a deck that is not
  self-contained:** the frame is fetched when the slide is shown, so it needs
  the network and it cannot be printed. The PDF gets a placeholder carrying
  the link instead, and audio gets its label without the transport.
- What a reference *is* follows from what it points at, so the attribute block
  is optional: `![clip](talk.mp4)` is a video whether or not you wrote `{}`.

## When a slide has too much on it

By default a pane **clips** what does not fit. That keeps the layout you drew
and `scripts/check-layout.mjs` reports the overflow before anyone presents it —
but nothing warns you while you are writing, and text that silently disappears
is a bad way to find out.

```yaml
---
fit: shrink        # every pane on every slide
---
```

```markdown
::: pane body {fit=shrink}
```

```bash
mirzam build README.md --split h2 --fit shrink   # for a document with no frontmatter
```

`fit=shrink` gives up the type size to keep the words: the pane's contents are
scaled down in small steps until they fit, to a floor of 55%, and re-measured
on every page turn and window resize. It runs in the PDF too — it only ever
makes content smaller than a box it is already overflowing, so a page that runs
it shows strictly more than one that does not. Without JavaScript you get the
clipping default, which is the documented fallback rather than a broken state.

If a pane is shrinking a lot, that is the deck telling you the slide has two
slides' worth on it.

### Carrying one pane on to the next slide

When shrinking is the wrong answer — a prose pane you would rather break at a
sentence you chose — put `<!-- next -->` where the break belongs:

```markdown
::: pane body
The estimator is unbiased under the stated conditions.

<!-- next -->

The variance, though, is where the argument actually happens.
:::
```

That slide becomes **two slides**, identical except for `body`. Every other
pane — the figure, the heading, the citations — is the same markup rendered
into the same place, and the viewer *cuts* between the parts instead of turning
the page, so the audience sees only the text change. `<!-- more -->` is accepted
as the same marker, and both are HTML comments, so a plain Markdown parser
shows nothing.

The expansion happens before a slide is parsed, so the parts are ordinary
slides: they animate, annotate and export like any other, and the PDF gets one
page per part.

Two rules follow from what this is:

- **One pane per slide may break.** Two panes breaking at once is a cross
  product nobody can predict. Mirzam reports it and renders the slide whole.
- **`<!-- next -->` outside every pane** breaks the slide body itself, which is
  what you want on a slide with no `pane` layout at all.

## Table of contents

````markdown
```toc
from: 2        # skip the deck's `#` title
depth: 2       # deepest heading listed
current: true  # mark the section being presented
```
````

Collects every heading in the deck, links each entry to the slide it is on, and
draws a leader out to the page number. Clicking an entry goes there; the address
is the slide number the viewer already keeps in the URL, so an entry works with
JavaScript switched off.

- **`from`** (default `1`) is the shallowest level listed, **`depth`** (default
  `2`) the deepest. `from: 2` is the usual setting: the title of the talk is not
  an item on its own agenda.
- **`current: true`** marks the last entry at or before the slide on screen —
  the section you are *inside*, not the heading you last passed. That is what
  turns an agenda slide into a progress indicator you can return to.
- A heading appears once, at the first slide that carries it, so a slide broken
  by `<!-- next -->` contributes one entry rather than three.
- Headings written inside speaker notes stay out: a note is what you say, not
  part of the structure.
- The slide carrying the list is not in it.
- **In the PDF** each entry shows its page number instead of a link, since a
  link to slide 7 means nothing on paper.

This is the first block that needs to know about slides other than its own. It
resolves in a second pass once the whole deck has rendered, which is why a
`toc` block previewed on a single slide renders as nothing rather than as a
guess.

## Citations

`[^key]` marks a claim and the note lands at the foot of **that slide** — a
reference belongs on the slide that made the claim, not in a bibliography at
the end that nobody will be looking at.

```markdown
Attention replaced recurrence[^vas], and the same block pretrains[^dev].

[^vas]: Vaswani et al., *Attention Is All You Need*, NeurIPS 2017.
[^dev]: Devlin et al., *BERT*, NAACL 2019. https://arxiv.org/abs/1810.04805
```

A bare DOI or arXiv URL becomes a link on its own. See
[`examples/seminar.md`](../examples/seminar.md) for the shape of a reading-group
talk: a figure quoted from the paper, annotated and pointed at from the prose,
with its citation at the foot of the same slide.

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
m : danmaku "this bit matters"
```
````

| Effect | |
|---|---|
| `flash` | one bright pulse over the slide |
| `shake` | the slide shakes |
| `lines` | speed lines converging on the middle |
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

[`examples/05-motion.md`](../examples/05-motion.md) has a slide bound to all seven.

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
  `pad=` for anchored items, `id=` to name the mark, and `step=N` to hold an
  item back until the Nth click. Attributes always come after ` : `, including
  on a `text` item: `text 6,90 "…" : step=2`.
- **`id=` makes the mark itself a target.** A `connect` arrow can run from a
  phrase in the prose to the circle drawn over the photograph — the only way
  to point at something that does not exist until the page is laid out. The
  connector appears with the mark and is re-routed whenever it moves.
- **`step=` counts as a click for the slide**, so `→` reveals the annotation
  before it turns the page — and a page with no viewer, the PDF included,
  shows every item regardless. An annotation waits for a click; it does not
  depend on one.
- Either end of an `arrow` may be an anchor: `arrow 12,70 -> #latency-1-2`
  stops at the edge of the mark rather than in the middle of it.

### Tying a phrase to a figure

An annotation may mark **words** as well as part of a picture, and that is what
replaces an arrow running from a sentence across the slide:

````markdown
::: pane note
Origin traffic keeps falling — [by Q3 it is the smaller half]{#c-q3}
:::

```annotate
highlight #c-q3     : color=@accent2 step=1
rect      #cook-1-2 : color=@accent2 step=1 pad=6
```
````

Both halves are ordinary annotation items with the **same `step`**, so they
arrive together and in one colour. A room reads that as a pairing instantly,
and nothing crosses the slide to say it.

| Mark | What it does |
|---|---|
| `highlight #id` | A wash behind the words, like a marker pen |
| `underline #id` | A rule under them |
| `box #id` | A rounded outline around them; `pad=` gives it room |

- These three take an **`#id` and nothing else**. Where the words are is the
  browser's business; a percentage would be a guess that goes stale the moment
  the sentence is edited.
- **They follow the lines the words are on.** A phrase that wraps is two line
  boxes, not one rectangle with the middle of the sentence inside it.
- A block whose items are *all* anchored needs no `target:` line — there are no
  percentages to measure against anything.

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

[`examples/05-motion.md`](../examples/05-motion.md) demonstrates all of this: text
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

## Driving the viewer

Press **`/`** and the deck tells you. The overlay lists every key, and — the
reason it exists — the `effects` keys *this slide* binds, which are the ones
nobody can guess. `Esc` or `/` closes it.

| | |
|---|---|
| `→` `Space` `PageDown` | Next click step, then the next slide |
| `←` `PageUp` | Back a step, then the previous slide |
| `Home` `End` | First / last slide |
| `N` | Speaker notes |
| `P` | Presenter window |
| `F` | Fullscreen |
| `D` | Dark / light |
| `L` | Outline the layout |
| `/` | The cheat sheet |
| `Esc` | Close the sheet; clear any effect in flight |

Clicking the left third of the slide goes back and anywhere else goes forward,
which is what a presenter with a clicker or a trackpad is using. A drag that
ends on the deck is a text selection, not a page turn.

A quiet control cluster sits below the bottom-right corner — previous, next and
the cheat sheet — and fades in when you move the pointer or touch the screen.
It is outside the deck, so it never covers slide content, and it is never
printed.

### The presenter window

`P` opens a second window showing the current slide, the next one, that slide's
speaker notes, the time and an elapsed timer — click the timer to restart it.
Put it on your laptop and the audience window on the projector.

It is **the same file**, opened again with `?presenter=1`. There is no second
document, no server and no export step: a deck is one file, and this is that
file rendered differently.

The two windows stay in step over a `BroadcastChannel`, falling back to the
window handles when the deck is opened from `file://`, where two windows have no
shared origin to meet on. Neither window is privileged — turn the page in either
and both move. What crosses the link is the *position*, not a keystroke, so a
window opened halfway through a talk adopts the slide already on screen instead
of starting from the beginning, and closing or reloading either one strands
nothing.

The audience window is unchanged: no extra chrome appears on it. `N` still
opens the notes panel there, for a talk given on one screen.

`D` and `L` travel across the link too. Dark mode and the layout outline are
properties of the deck rather than of one window — a presenter who switches to
light mode means the projector as well.

The next-slide preview is a still: it is built from the slide as authored, so
it never inherits the current window's animation state, and it does not run
animations, annotations or connectors of its own.

### On a phone

There is no keyboard, so every control has a gesture:

| | |
|---|---|
| Swipe left / right | Next / previous |
| Swipe up / down | Show / hide speaker notes |
| Two-finger tap | The cheat sheet |
| Tap left third / elsewhere | Back / forward |
| Long press | Select text, as anywhere else |

The deck claims horizontal swipes from the browser, so swiping right turns the
page instead of navigating away from the deck. On a touch device the cheat
sheet leads with these gestures rather than with the keys.

**The long press is not bound to anything**, because on a phone that gesture is
how you select text, and a deck a reader cannot quote from is a worse deck. For
the same reason, a drag that starts or ends with a selection on screen is
treated as adjusting the selection, never as a page turn.

## Theming

### Named themes

```yaml
---
theme: nord
---
```

| Name | Source |
|---|---|
| `default` | ours — Mirzam's own palette, so a deck that chooses nothing is already in the project's colours |
| `nord` | [Nord](https://www.nordtheme.com/), MIT |
| `solarized` | [Solarized](https://ethanschoonover.com/solarized/), MIT |
| `vscode` | VS Code Light+/Dark+, MIT |
| `mirzam` | Mirzam's own palette, from [the brand sheet](brand/palette.md) |

An unknown `theme:` name is a warning, not a build failure, and falls back to
`default`. See
[`themes/CREDITS.md`](../crates/mirzam-render/src/theme/themes/CREDITS.md) for
where each palette comes from and how it maps to Mirzam's tokens.

A named theme is a **palette**, not a design. `theme: mirzam` gives a deck
Mirzam's colours in both modes; the identity's typography — Space Grotesk, the
weight ladder, the violet rule under a section heading — lives in
`examples/themes/mirzam.css`, because a built-in theme is loaded before the
layout stylesheet and can only set tokens. Write `css: themes/mirzam.css` for
the whole thing.

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

#### A custom theme needs both modes, or it has none

The built-in tokens are wrapped in `:where()` and carry no specificity, which
is what lets a plain `:root` in your stylesheet override them. The same thing
makes a one-palette custom theme **pin the deck to one mode**: your `:root`
beats the built-in light *and* dark tokens, so `D` changes `data-mode` and
nothing on screen moves. Give the second mode a selector that outranks your own
`:root`:

```css
:root                     { --mz-slide-bg: #0d1117; --mz-fg: #e9edf5; }
:root[data-mode="light"]  { --mz-slide-bg: #ffffff; --mz-fg: #10151f; }
```

Two rules follow from that, and both are checked for the sample themes by
`cargo test -p mirzam-cli --test sample_themes`:

- **Every token set in one mode must be set in the other.** A token you set
  once keeps its other-mode value — which is how a dark panel ends up on a
  white slide.
- **Name a colour once.** A literal buried in a rule (`p { color: #c7cede }`)
  cannot have a second mode. Put it in a token of your own — `--aurora-body`
  in the sample — and set that token twice.

A theme that deliberately only ever appears one way is fine; write `mode:` in
the deck's frontmatter and say so.
