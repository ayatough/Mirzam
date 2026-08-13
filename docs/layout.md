# Layout guide

How space is allocated, why content sometimes disappears, and how to control
connectors. Read this when a slide does not look the way you drew it.

For the full list of syntax see [syntax.md](syntax.md). Every rule here is
demonstrated in [`examples/03-layout.md`](../examples/03-layout.md), which you can
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

There are four fixes, in order of preference:

1. **Give the band more lines.** The layout is the specification; if the content
   needs more room, say so in the drawing.
2. **Shorten the text.** A slide heading that wraps to three lines is usually two
   headings.
3. **Move it to another pane.** Long prose belongs in a body pane, not a heading
   band.
4. **Break the pane in two.** `<!-- next -->` inside a pane carries it on to the
   next slide while every other pane stays exactly where it is — the audience
   sees the words change and nothing else move. Rule 7 in
   [`examples/03-layout.md`](../examples/03-layout.md) demonstrates it; the
   [syntax reference](syntax.md#carrying-one-pane-on-to-the-next-slide) has the
   rules.

`fit: shrink` is the fifth answer and a different kind: it keeps the slide whole
and gives up type size instead. Use it when the overflow is small and the break
is not worth a page.

Heading panes (`head`, `title`) are allowed to overflow rather than be silently
cut, so the text stays legible while you fix the band. That is a diagnostic, not
a layout technique: run the checker below and the overflow is reported.

### Converting an existing document

`--split h2` turns a README or a set of notes into a deck without editing it, and
some sections will simply be longer than a slide. The checker reports each one.
In order of effort:

- build it with `--fit shrink`, which scales an overfull pane's text down
  instead of clipping it — the only one of these that needs no edit at all, and
  therefore the answer for a document you do not control or must not touch. It
  is `fit: shrink` in frontmatter, given on the command line for a document that
  has none; this is how the site publishes its own README as a deck
- split at a deeper level (`--split h3`) so sections are smaller
- add `---` where you want a break; it is still valid Markdown for the document
- move the long part into a `<details>` block or trim it

A document that reads well is not automatically a deck that presents well. The
conversion is a starting point, not a finished deck.

## Drawing the same shape once

A deck that draws the same three or four grids on every slide should name them
instead. `masters:` points at a Markdown file whose headings name the shapes,
`layout:` picks the deck's default, and `<!-- layout: two-up -->` picks another
on one slide; the [syntax reference](syntax.md#slide-masters) has the file
format and the resolution order. The drawing is the same drawing and goes
through the same parser, so every rule on this page applies to a master exactly
as it applies to a `pane` block.

Two consequences worth knowing before you convert a deck:

- **A master fixes the pane names**, so the sizing rule above becomes a decision
  you make once for the deck rather than per slide. Draw the heading band two
  lines tall in the master and every slide that uses it has room for an eyebrow.
- **A slide that needs a different shape just draws one.** Its own `pane` block
  always wins, so an exception costs nothing and needs no opt-out.

## Spacing

- Panes are separated by the grid gap. Set `--mz-grid-gap`, `--mz-grid-pad-y`
  and `--mz-grid-pad-x` — see [the token table](syntax.md#margins-padding-and-borders)
  — or write the rule yourself with `.grid { gap: … ; padding: … }`.
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

## Text over a photograph

A background image is the easiest way to make a slide look worse. Photographs
carry detail at every scale, and body text is exactly the scale they compete
with. Buy the contrast back before you spend it:

```markdown
::: pane hero {bg=media/bg/city.jpg dim=0.4 blur=3 scrim=bottom}
```

- **`dim`** is the blunt instrument and usually enough. Start at `0.4`; go to
  `0.55` for body text, and you can drop to `0.25` for a short heading.
- **`blur`** removes the detail rather than the light, so the photo keeps its
  colour. Two or three pixels is plenty; it reads as depth of field, not as a
  mistake.
- **`scrim`** darkens one edge only. Use it when the photograph is the point and
  you want the text to sit in a corner of it — pair it with `valign=bottom`.
- Text switches to light automatically as soon as any of these is set. Override
  with `text=dark` when the photo is genuinely pale.

The failure to look for is *partial* legibility: a heading that is readable over
the dark half of a photo and invisible over the bright half. Fix it with `dim`,
not by moving the text, because the crop changes with the pane's aspect ratio.

`.bleed` takes the background to the slide edge. It drops the grid's padding, so
put it on a slide whose background covers everything — a title or a section
divider — not on one pane among several.

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

It steps each slide through to its last click before measuring, so it sees the
slide the audience ends on rather than the one it starts as. It reports, per
slide and pane:

- **clipped** — content is taller or wider than its pane, or an element inside
  one hides part of itself by scrolling. A scroll box on a slide is the worse
  of the two: the pane still measures clean, and there is no reader who can
  scroll it
- **overlap** — an overflowing pane runs into its neighbour
- **connector** — a connector was declared but not drawn, usually a typo in an id
- **annotation** — a mark could not be drawn, usually because the `#id` it names
  was renamed or removed. The mark is dropped silently at runtime by design; the
  sentence that said "the circled bar" is not
- **animation** — an element is still in its entrance state after that entrance
  has played, so nobody ever sees it. That includes the PDF, which never steps
- **slack** — a pane fits, but by less than `--min-slack <px>` asked for
- **debug** — the pane overlay is baked into this build (see below). Fine for a
  screenshot, wrong for anything published

```
✗ pitch.md: 2 problem(s) across 9 slides
    slide 1 [clipped] pane "head": content is 100px taller than the pane
    slide 1 [overlap] pane "head": overflows 79px into pane below
```

### What a clean run does and does not promise

A pass says the deck fits **on the machine that ran the check**, and after the
verdict the check says which machine that was:

```
✓ 12 slides, no layout problems (1456 ms)
  · fonts: measured with Arial; not on this machine: Helvetica Neue, Hiragino
    Kaku Gothic ProN, Hiragino Sans, and 5 more. A reader who has them sees
    different line breaks
  · tightest pane: slide 2 "main", 3px of room left
```

A deck embeds no text font — only the maths face is inlined — so the type it is
measured in is whatever the checking machine resolved the deck's stack to. Swap
the font and the text changes extent; one extra wrapped line is about 28px
against a pane that had 3. That is why the tightest pane is worth reading even
when nothing failed, and why a deck that will be shown somewhere else can ask
for a margin rather than for a fit:

```bash
mirzam check deck.md --min-slack 24
```

Every pane with less than 24px of room left is then reported, and the check
exits non-zero — a CI gate that survives a font substitution instead of one
that passes right up until the room it presents in.

`--fit shrink` and the check agree by construction: the check measures the same
wrapper `fit.js` scales, so a pane the fit rescued passes and a pane still
overflowing at the 55% floor fails.

The same check runs in CI over every sample deck, so the samples stay a reliable
reference.

## Debugging layout visually

Press **`L`** in the viewer to toggle a debug overlay: every pane gets a dashed
outline and a name label in its corner (from the drawing's own band names), and
the grid gaps are tinted so you can see space you did not mean to leave. It is
off by default and never appears in PDF export.

```bash
mirzam build examples/pitch.md --debug-layout -o /tmp/debug
```

`--debug-layout` bakes the overlay on at load instead of requiring a keypress,
for screenshotting a broken deck headlessly (`check-layout.mjs` and CI use the
same rendered output, without the overlay, since the overlay is a human aid, not
part of what the checker measures — and the checker fails a deck that arrives
with it baked in, which is the one way it could reach an audience).
