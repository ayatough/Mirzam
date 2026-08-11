---
title: Mirzam Layout Cookbook
author: Mirzam
aspect: "16:9"
css: themes/mirzam.css
---

# Layout cookbook {.title-slide}

Each slide states one rule and demonstrates it. Read it beside `examples/03-layout.md`.

<!-- note: This deck is checked by scripts/check-layout.mjs in CI, so every rule here is verified, not asserted. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  out            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Rule 1]{.eyebrow}
## Bands get the share you draw
:::

::: pane src {.card valign=middle}
```markdown
+--------+------------+
|  side  |  main      |
+--------+------------+
```

`side : main = 8 : 12`

Row heights work the same way: a band
of five lines gets five shares.
:::

::: pane out {valign=middle}
The drawing is the specification, not a sketch.

Borders are **not** part of a band, so a one-line
heading band is genuinely one line tall — even
though the ASCII makes it look bigger.

Draw the band as tall as the content deserves.
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  bad             |  good           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Rule 2]{.eyebrow}
## A heading needs two lines, three if it wraps
:::

::: pane bad {.card valign=middle}
### Too tight

```markdown
+-------------+
|  head       |
+-------------+
```

An eyebrow plus a heading does not fit one
share. The heading is clipped, and a pane with
a background below it looks like it ate the title.
:::

::: pane good {.card valign=middle}
### Right

```markdown
+-------------+
|             |
|  head       |
+-------------+
```

One blank line is usually enough. The checker
tells you when it is not.
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+---------+---------+----------------+
|         |         |                |
|         |         |                |
|  top    |  middle |  air           |
|         |         |                |
|         |         |                |
+---------+---------+----------------+
```

::: pane head
[Rule 3]{.eyebrow}
## Alignment is per pane
:::

::: pane top {.card}
### Default

Content starts at the top. Best when a pane is
nearly full.
:::

::: pane middle {.card valign=middle}
### valign=middle

Centred in the band. Looks deliberate for short
content, airy for tall content.
:::

::: pane air {.card valign=middle align=center}
### Empty cells

A blank cell in the drawing reserves space. It is
the cleanest way to add breathing room.
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  note            |  fig            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Rule 4]{.eyebrow}
## Anchor near the edge that faces the figure
:::

::: pane note {valign=middle}
Between two boxes a connector is the right tool.
From a sentence it usually is not — see Rule 5.

When you do use one, give it a clear path: it
leaves through the top or bottom of the
underline, never sideways. Put the anchored
phrase at the end of a line, or on one of its own.

the request first hits the [cache]{#c-cache .u}

on a miss it reaches the [origin]{#c-origin .u}
:::

::: pane fig
:::

```shape
rect #k-cache  at(76%, 34%) size(30%, 15%) label="Cache" stroke=@accent2
rect #k-origin at(76%, 70%) size(30%, 15%) label="Origin"
```

```connect
#c-cache -> #k-cache.w : color=@accent2
#c-origin -> #k-origin.w : color=@accent2
```

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  chart           |  note           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Rule 5]{.eyebrow}
## Point at the data, not at a picture
:::

::: pane chart
```chart
type: bar
id: cook
title: Requests served (millions)
data: |
  quarter, cached, origin
  Q1, 3.1, 1.9
  Q2, 4.6, 1.7
  Q3, 6.2, 1.4
```
:::

::: pane note {valign=middle}
Every mark has an id: `#cook-<series>-<row>`.

Origin traffic keeps falling —
[by Q3 it is the smaller half]{#c-q3}

Press `→`: the phrase and the bar light up
together, in one colour.
:::

```annotate
highlight #c-q3     : color=@accent2 step=1
rect      #cook-1-2 : color=@accent2 step=1 pad=6
```

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  bad             |  good           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Rule 6]{.eyebrow}
## Over a photo, buy contrast before you spend it
:::

::: pane bad {bg=media/bg/mesh.jpg valign=middle align=center}
### Bare photo

Every word competes with the picture behind it.
:::

::: pane good {bg=media/bg/mesh.jpg dim=0.5 blur=3 valign=middle align=center}
### `dim=0.5 blur=3`

The photo becomes texture, and the words come forward.
:::

<!-- note: Same image, same text, both panes. Only the treatment differs. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  fig             |  body           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Rule 7]{.eyebrow}
## Break the prose where you want it broken
:::

::: pane fig {valign=middle}
```chart
type: bar
id: cont
title: Time to first byte (ms)
data: |
  release, p50, p95
  4.2, 180, 640
  4.3, 140, 520
  4.4, 95, 210
```
:::

::: pane body {valign=middle}
This pane carries `<!-- next -->`, so it arrives in three parts. Watch the
chart: it does not move, does not fade, does not redraw.

<!-- next -->

The audience is looking at one slide the whole time. Only the words change,
which is the point — a page turn under a figure you are still talking about
throws the eye away from it.

<!-- next -->

Shrinking the type would have kept it on one slide. Sometimes that is right.
When it is not, choose the break yourself instead of letting the box choose it.
:::

<!-- note: One slide in the source, three in the deck. The chart is byte-identical on all three, so the viewer cuts rather than turning the page. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------------------------+
|                                    |
|                                    |
|  body                              |
|                                    |
|                                    |
+------------------------------------+
```

::: pane head
[Rule 8]{.eyebrow}
## Let the checker find what your eye misses
:::

::: pane body {valign=middle}
```bash
node scripts/check-layout.mjs --build examples/03-layout.md
```

It renders every slide and reports what HTML snapshots cannot see:

- **clipped** — content taller or wider than its pane
- **overlap** — an overflowing pane running into its neighbour
- **connector** — declared but not drawn, usually a typo in an id

This deck passes it in CI, which is the only reason to trust the rules above.
:::
