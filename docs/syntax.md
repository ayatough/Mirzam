# Mirzam syntax

Mirzam decks are CommonMark files. Everything below is an addition that a plain
Markdown parser still renders as readable text — that rule is enforced by
`crates/mirzam-cli/tests/commonmark_compat.rs`.

| Extension | What a plain Markdown parser shows |
|---|---|
| Fenced blocks (`pane`, `shape`, `connect`, `chart`) | A code block |
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
vars:
  product: Mirzam
  price: 1200
---
```

### Slide breaks

Slides are separated by a horizontal rule (`---`) outside code fences.

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
an arrow at one bar.

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

## Theming

Set `css:` in frontmatter and override the theme tokens:

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

## Reserved

` ```anim ` is parsed but not yet implemented; it renders as a note in the corner
so decks written today keep working when animation lands. See the
[roadmap](roadmap.md).
