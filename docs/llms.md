# Mirzam syntax card

Everything needed to write a correct Mirzam deck, on one page. A Mirzam deck is
a CommonMark file: every addition below degrades to readable text in a plain
Markdown parser. Full reference: [syntax.md](syntax.md) and
[layout.md](layout.md). Machine-readable checking: [agents.md](agents.md).

```bash
mirzam build deck.md -o out        # one self-contained HTML file
mirzam check deck.md --format json # every problem, as records — run this after every edit
```

## Traps — read these first

Each of these fails **silently or as literal text**, and each has cost a real
author a slide.

1. **An attribute span must fit on one source line.** `[a phrase]{.small}`
   wrapped across a line break is left alone and the brackets reach the slide.
   Rewrap the sentence.
2. **`shape`, `pane`, `connect`, `annotate`, `effects`, `anim` only parse at
   slide top level** — never inside `::: pane`. Inside a pane the fence is
   ordinary Markdown and renders as a code block. Only `chart`, `toc` and
   `bibliography` belong inside a pane.
3. **A footnote definition must be on the slide that cites it.** `[^k]` with
   its `[^k]:` on another slide stays literal text. For a source cited from
   several slides use `[@key]` + `bibliography:` instead.
4. **A `connect` endpoint naming an id that does not exist draws nothing.** No
   arrow, no gap, no error on the slide. Same for an `annotate` anchor.
5. **`.metric`, `.card`, `.eyebrow` are renderer classes**, along with `.box`
   and the rest listed below — a slide copied without its `css:` keeps them.
   What a theme changes about them is their `--mz-card-*`, `--mz-eyebrow-*`
   and `--mz-metric-*` tokens, not the classes.
6. **`---` breaks a slide.** A horizontal rule inside a slide is `***`.
7. **Row heights come from the number of lines in the drawing**, borders
   excluded. A one-line `head` band gets one share against a five-line `body`.
8. **A pane clips what does not fit**, and nothing warns while you write. Run
   `mirzam check`.

## Frontmatter

Optional, but the first thing in the file when present.

```yaml
---
title: Quarterly review     # deck title, used for the browser tab
author: Your Name
aspect: "16:9"              # or "4:3"
theme: nord                 # mirzam (default) | nord | solarized | vscode | wuwei
                            #   a token set: colours, and for mirzam the type too
mode: dark                  # light | dark; unset follows the reader's machine
css: themes/dark.css        # custom stylesheet, relative to this file
split: h2                   # also start a new slide at every h1/h2/h3
fit: shrink                 # scale an overfull pane's text down instead of clipping
math: typst                 # latex (default) | typst
transition: slide-left 400ms ease=out-cubic
masters: masters.md         # named layouts; a path, or a mapping of name to drawing
layout: body                # the master a slide takes when it draws no grid
footer: Internal            # drawn on every slide and in the PDF
slide-number: "{n} / {total}"
bibliography: refs.bib      # a .bib path, or entries written inline
citation-style: numeric     # numeric -> [1]; author -> [Vaswani+17]
vars:
  product: Mirzam
  price: 1200
---
```

`transition:` takes `none`, `fade`, `slide-left|right|up|down`,
`wipe-left|right|up|down`, `zoom`, `iris`, each optionally with a duration and
`ease=`.

## Slides

Separated by `---` outside code fences. `--split h2` (or `split: h2`) also
starts a slide at every heading of that level. Comments a slide can carry, all
invisible in a plain Markdown reader:

| | |
|---|---|
| `<!-- note: … -->` | Speaker note; `N` shows it |
| `<!-- layout: two-up -->` | Draw this slide on that master; `none` opts out of the deck default |
| `<!-- theme: nord -->` `<!-- mode: dark -->` | A theme for this slide only — tokens inherit, so its type comes too |
| `<!-- chrome: none -->` | Drop the footer and slide number here (title slides, `.bleed` slides) |
| `<!-- next -->` | Split this slide in two, changing one pane. `<!-- more -->` is the same marker |

Transclusion pastes a file in where the line was, so put a `---` on each side
unless the section should continue the slide before it. The included file's
frontmatter is ignored — `masters:` and everything else live in the root deck.

```markdown
---
![[sections/method.md]]
---
```

## Layout: the `pane` block

One `pane` block per slide, at slide top level. Without one the slide is a
single pane.

````markdown
```pane
+----------------+-----------------+
|  head                            |
+----------------+-----------------+
|                |                 |
|  main          |  fig            |
|                |                 |
+----------------+-----------------+
```
````

`+ - |` draw borders and the word inside a cell names the pane. Column widths
come from character widths, row heights from **line counts**. Repeat a name in
adjacent cells to merge them, rectangularly. `.` or a blank cell leaves the
space empty.

Assign content with a fenced div. Content that names no pane flows into `main`,
or the first pane if there is none.

```markdown
::: pane main
Ordinary Markdown goes here.
:::

::: pane fig {align=center valign=middle}
![Result](img/result.svg){fit=contain}
:::
```

Pane attributes: `align=left|center|right`, `valign=middle|bottom`,
`fit=shrink`, `theme=`, `mode=`, the background attributes below, and any
`.class` your stylesheet defines.

Background image on a pane:

```markdown
::: pane hero {.bleed bg=media/city.jpg dim=0.4 blur=3 scrim=bottom}
# Ship the story
:::
```

`bg=`, `bg-light=`, `bg-dark=`, `bg-fit=cover|contain`, `bg-pos=`, `dim=0…1`,
`blur=<px>`, `scrim=bottom|top|left|right`, `text=light|dark`, and `.bleed` to
run the image out to the slide edges that pane reaches — all four for a pane
that is the whole slide (pair that with `<!-- chrome: none -->`), three for a
pane drawn down one half, which leaves the pane beside it as it was.

## Inline

All of CommonMark, plus GFM tables, strikethrough and task lists. Beyond that:

| Written | Is |
|---|---|
| `==text==` | a marker-pen wash in the accent colour |
| `++text++` | an underline (**not** `__`, which is bold) |
| `:tada:` | 🎉 |
| a line, then `: definition` | a term list, definition beside the term |
| `{{ price * 12 }}` | a variable from `vars`, with `+ - * /`, parentheses, `round`, `ceil`, `floor` |
| `$E = mc^2$`, `$$…$$` | math, converted to MathML at build time |
| `[^k]` … `[^k]: …` | a footnote landing on **that slide** |
| `[@key]` | a reference from `bibliography:`, listed by a `bibliography` block |
| `![[file.md]]` | transclusion |
| `***` | a horizontal rule |
| ` ```rust ` | a code block, syntax highlighted at build time |

Fenced code is coloured for 36 languages (`rust`, `python`, `js`, `ts`, `go`,
`c`, `cpp`, `java`, `sh`, `sql`, `html`, `css`, `json`, `yaml`, `toml`, `md`,
`diff`, … and the usual aliases). Name the language or the block stays plain —
which is also what `chart`, `shape` and the rest stay when they land somewhere
that leaves them as code. Colours are the theme's `--mz-code-*` tokens, so do
not write a palette into the deck.

### Attributes

```markdown
## Heading {#intro .center}
[a phrase]{#anchor .u}
![Figure](img/a.png){#fig1 fit=contain w=80%}
![Demo](media/demo.webm){.autoplay .loop .controls poster=media/first.png}
```

`#id` names an element so `connect`, `anim` and `annotate` can target it. One
source line only.

Classes the **renderer** provides — everything else comes from your `css:`:
`.u` (accent rule under the words) · `.center` `.right` · `.small` `.big`
`.huge` · `.muted` `.accent` `.accent2` `.danger` · `.box` (an aside inside a
pane) · `.card` (a pane raised off the slide) · `.eyebrow` (the label over a
heading) · `.metric` with `.metric-up` `.metric-label` · on a pane, `.bleed`
`.terms-aligned` `.terms-stacked`. There is no syntax for a literal colour:
colours are theme tokens, so they survive dark mode.

`mp4`, `webm`, `ogv`, `mov` become `<video>`; everything else stays an image.
An audio file becomes a player; a YouTube or Vimeo page URL becomes an embed
(the one thing in a deck that is not self-contained). Prefer `webm`.

## `chart` — inside a pane

````markdown
```chart
type: bar          # bar | line | area | pie
id: latency        # optional; mark ids derive from it
title: p95 latency by region (ms)
y_label: ms
highlight: after   # dim every series but this one
data: |
  region, before, after
  us-east, 210, 120
  ap-ne, 380, 180
```
````

First column is categories, every other column a series. `data:` may instead
name a `.csv` file. Each mark gets the id `<chart-id>-<series>-<row>`, so
`#latency-0-1` is the second bar of the first series — that is what an arrow or
an annotation points at.

## `shape` — page or pane coordinates

At slide top level, percentages of the whole slide, on a layer above the
panes. Inside a `::: pane`, percentages of that pane's rectangle — resize the
pane and the drawing follows. Neither form clips: coordinates past 100%
deliberately reach outside the frame.

````markdown
```shape
rect    #cache at(72%, 30%) size(30%, 14%) label="Cache" fill=@shape-fill stroke=@accent2
ellipse #db    at(72%, 70%) size(26%, 16%) label="Database"
arrow   from(#cache.s) to(#db.n) style=dashed
line    from(10%, 90%) to(40%, 90%)
text    #cap   at(72%, 88%) "95% hit rate" .small
```
````

Kinds `rect`, `ellipse`, `text`, `arrow`, `line`; edges `.n` `.s` `.e` `.w`
`.c`; colours `@accent1`, `@accent2`, `@shape-fill`, … resolve to theme tokens.
Both forms are one layer: ids resolve across it. Page-level shapes ignore the
grid — reserve their area with an empty pane, or write the block in the pane.
Pane rectangles come from the grid's margin and gutter, so change those with
frontmatter `grid-pad-x`/`grid-pad-y`/`grid-gap` (not CSS) in a deck that
anchors shapes to panes.

## `connect` — slide top level only

````markdown
```connect
#t-edge -> #cache.w : color=@accent2 style=dashed
#a <-> #b
#a -- #c : curve=0
```
````

Operators `->`, `<->`, `--`. Either endpoint may be a text anchor
(`[edge cache]{#t-edge}`), a shape or a chart mark. Omit the edge and the
natural one is chosen. Attributes after ` : `: `color=`, `style=dashed`,
`curve=`. From a sentence to a figure prefer a paired annotation — an arrow
crossing prose is usually worse than marking both ends in one colour.

## `annotate` — slide top level only

````markdown
```annotate
target: shot
circle 62,38 34x34 : label="the hot corner"
rect   10,10 20x14 : color=@accent2 style=dashed
arrow  18,86 -> 55,48
text   6,90 "coordinates are percentages of the picture"
circle #latency-1-2 : pad=6
highlight #c-q3     : color=@accent2 step=1
```
````

- `target:` is a pane name or a `#id`. A pane holding one picture *means* that
  picture.
- `x,y` is the **centre** of a `rect`/`circle`, `WxH` its size, both as
  percentages of the target.
- An anchored item (`circle #id`) needs no coordinates and survives a data
  change. `highlight`, `underline` and `box` take an `#id` and nothing else.
- A block whose items are all anchored needs no `target:`.
- Attributes come after ` : ` — `label=`, `color=`, `style=dashed`, `pad=`,
  `id=`, `step=N` (hold until the Nth click).

## `anim` — slide top level only

````markdown
```anim
[enter]   .title       : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[after #latency-0-2 +200ms] .caption : fade-in 300ms
[exit]    slide        : iris-out 500ms
```
````

One line is one track: `[trigger] target : effect duration attributes…`

- **Triggers:** `enter`, `click N`, `exit`, `after #id [+Nms]`.
- **Targets:** `#id`, `.class`, or `slide`. An optional `chars` / `words` /
  `lines` before the effect splits the text.
- **Effects:** `fade-in` `fade-out` `slide-in` `slide-out` `wipe-in` `wipe-out`
  (those four need `dir=left|right|up|down`), `zoom-in` `zoom-out` `blur-in`
  `grow-x` `grow-y` `pop` `draw` `iris-out`.
- **Attributes:** a bare `400ms` is the duration; `delay=`, `stagger=`, `ease=`
  (a named curve or `spring(mass,stiffness,damping)`).

Elements are laid out in their **final** state, so a deck read without
JavaScript, and the PDF, show every slide fully revealed.

## `effects` — slide top level only

````markdown
```effects
1 : flash
2 : shake
e : burst 🎉
m : danmaku "this bit matters"
```
````

One single-character key per line: `flash`, `shake`, `lines`, `boom`,
`burst <emoji>`, `confetti`, `danmaku "<text>"`. These belong to the
performance — they never reach the exported file and never change what the deck
says. `← → Space PageUp PageDown Home End N F L D Esc` belong to the viewer and
cannot be bound.

## `toc` and `bibliography` — inside a pane

````markdown
```toc
from: 2        # shallowest heading level listed (default 1)
depth: 2       # deepest (default 2)
current: true  # mark the section being presented
```

```bibliography
show: cited    # cited (default) | all
back: true     # print the slides each entry was cited on
```
````

`toc` collects every heading in the deck and links each entry to its slide;
both resolve after the whole deck has rendered, so either shows nothing when
previewed alone. `bibliography` needs `bibliography:` in frontmatter — without
it `[@key]` is ordinary text.

## Slide masters

Draw the deck's three or four shapes once. `masters: masters.md` names a
Markdown file whose headings name the shapes and whose `pane` block under each
is the drawing. A slide picks one with `<!-- layout: two-up -->`; `layout:` in
frontmatter is the deck default. Resolution order, innermost first: the slide's
own `pane` block, `<!-- layout: … -->`, frontmatter `layout:`, otherwise a
single pane. A master fixes the **pane names**, so every `::: pane` in the deck
must agree with the drawing.

## When a slide has too much on it

In order of preference: give the band more lines · shorten the text · move it
to another pane · break the pane with `<!-- next -->` · `fit: shrink`.

```markdown
::: pane body
The estimator is unbiased under the stated conditions.

<!-- next -->

The variance, though, is where the argument actually happens.
:::
```

That renders as two slides identical but for `body`, and the viewer cuts
between them. **Only one pane per slide may break.**

## Check it

```bash
mirzam check deck.md --format json
```

Reports, per slide and pane: `clipped`, `overlap`, `nesting`, `connector`,
`annotation`, `animation`, `slack`, `debug`. Exits non-zero on any of them.
The JSON schema — kinds, severities, `file`/`line` through transclusion — is in
[agents.md](agents.md).
