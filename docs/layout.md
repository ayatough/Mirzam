# Layout guide

How space is allocated, why content sometimes disappears, and how to control
connectors. Read this when a slide does not look the way you drew it.

For the full list of syntax see [syntax.md](syntax.md). Every rule here is
demonstrated in [`examples/cookbook.md`](../examples/cookbook.md), which you can
build and read side by side with its source.

## Sizing: the drawing is the specification

A `pane` block is not a sketch. Column widths come from the character widths
between borders, and row heights come from the number of lines in each band.

````markdown
```pane
+--------+------------------+
|  side  |  main            |     side : main = 8 : 18
+--------+------------------+
```
````

The most common surprise is vertical. A heading band drawn one line tall gets one
sixth of the slide when the body band is five lines tall — even though, in the
ASCII art, the border lines make it look larger:

````markdown
```pane
+----------------+       <- borders are not part of the band
|  head          |       <- one line: head gets 1 share
+----------------+
|                |
|                |
|  body          |       <- five lines: body gets 5 shares
|                |
|                |
+----------------+
```
````

**Draw the band as tall as the content deserves.** A heading with an eyebrow label
above it needs at least two lines; a heading that wraps needs three.

## When content does not fit

Panes clip their content. A heading that outgrows its band is cut off, and if the
pane below has a background it looks as if the heading vanished behind it.

There are three fixes, in order of preference:

1. **Give the band more lines.** The layout is the specification; if the content
   needs more room, say so in the drawing.
2. **Shorten the text.** A slide heading that wraps to three lines is usually two
   headings.
3. **Move it to another pane.** Long prose belongs in a body pane, not a heading
   band.

Heading panes (`head`, `title`) are allowed to overflow rather than be silently
cut, so the text stays legible while you fix the band. That is a diagnostic, not
a layout technique: run the checker below and the overflow is reported.

## Spacing

- Panes are separated by the grid gap; set it in your theme with
  `.grid { gap: … ; padding: … }`.
- Empty cells are legitimate. A `.` or blank cell reserves space, which is often
  the cleanest way to add breathing room around a figure.
- `valign=middle` centres content in its band. It looks deliberate for short
  content, and airy for tall content — prefer top alignment (the default) when a
  pane is nearly full.

```markdown
::: pane note {align=right}
::: pane fig {align=center valign=middle}
```

## Connectors

An arrow from a phrase to a figure is resolved in the browser after layout, so it
follows the text wherever it ends up. Two rules govern where it starts.

**Text anchors leave vertically.** A connector from an inline span starts at the
horizontal centre of its underline and leaves through the top or bottom edge,
whichever faces the target. It never leaves sideways, because that would run the
line through the rest of the sentence.

**Shapes and charts leave through the facing edge.** The natural edge is chosen
from relative position; override it when you want a specific route:

```markdown
#anchor -> #box.w     : color=@accent2      # arrive at the box's west edge
#anchor -> #box       : curve=0             # straight line
```

### Keeping arrows out of the text

Mirzam will not route around a paragraph — it draws the shortest sensible curve.
When an arrow crosses text you did not want it to cross, the fix is compositional:

- **Anchor near the edge of the text block** that faces the target. An anchor at
  the end of a line has a clear path to the right; one in the middle of a
  paragraph does not.
- **Break the line** so the anchored phrase sits on its own line, closest to the
  figure.
- **Put the anchor in a pane of its own** — a caption pane beside the figure — when
  several arrows would otherwise fan out across a paragraph.
- **Point at a different element.** Arrows into a chart can target one bar
  (`#chart-0-2`); arrows out of a long sentence rarely need to.

## Charts and shapes

- A chart fills its pane and keeps its aspect ratio. Give it a band at least three
  lines tall; below that the labels crowd.
- Shapes use page coordinates (percentages of the whole slide), not pane
  coordinates. They ignore the grid deliberately, which is what makes free
  placement possible — and what makes it your job to keep them clear of the text.
  Reserve the area with an empty pane so nothing else is laid out there.

## Checking a deck

HTML-level tests cannot see a clipped heading, so layout is checked by rendering:

```bash
node scripts/check-layout.mjs --build examples/pitch.md
```

It reports, per slide and pane:

- **clipped** — content is taller or wider than its pane
- **overlap** — an overflowing pane runs into its neighbour
- **connector** — a connector was declared but not drawn, usually a typo in an id

```
✗ pitch.md: 2 problem(s) across 9 slides
    slide 1 [clipped] pane "head": content is 100px taller than the pane
    slide 1 [overlap] pane "head": overflows 79px into pane below
```

The same check runs in CI over every sample deck, so the samples stay a reliable
reference.
