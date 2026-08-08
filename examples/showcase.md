---
title: Mirzam Component Gallery
author: Mirzam
aspect: "16:9"
css: themes/pitch.css
vars:
  uptime: 99.95
  regions: 6
---

# Component gallery {.title-slide}

Every block on the following slides is plain Markdown. View the source alongside.

<!-- note: This deck doubles as the visual regression sample and the syntax cheat sheet. -->

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
[Layout]{.eyebrow}
## The layout *is* the ASCII you draw
:::

::: pane src {.card valign=middle}
```markdown
+----------+----------+
|  head               |
+----------+----------+
|          |          |
|  left    |  right   |
|          |          |
+----------+----------+
```
:::

::: pane out {valign=middle}
- Column widths follow the character widths
- Row heights follow the line counts
- Repeat a name to merge cells
- Nothing to drag; the diff is readable

*Same semantics as CSS Grid areas, so it is predictable.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+---------+---------+----------------+
|         |         |                |
|         |         |                |
|  m1     |  m2     |  m3            |
|         |         |                |
|         |         |                |
+---------+---------+----------------+
```

::: pane head
[Components]{.eyebrow}
## Metric tiles
:::

::: pane m1 {.card align=center valign=middle}
<div class="metric metric-up">{{ uptime }}%</div>
<div class="metric-label">uptime, trailing 90 days</div>
:::

::: pane m2 {.card align=center valign=middle}
<div class="metric">{{ regions }}</div>
<div class="metric-label">regions served</div>
:::

::: pane m3 {.card align=center valign=middle}
<div class="metric">{{ regions * 3 }}</div>
<div class="metric-label">availability zones</div>
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  bars            |  line           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Charts]{.eyebrow}
## Data in, chart out
:::

::: pane bars
```chart
type: bar
id: latency
title: p95 latency by region (ms)
data: |
  region, before, after
  us-east, 210, 120
  eu-west, 260, 140
  ap-ne, 380, 180
```
:::

::: pane line
```chart
type: line
id: errors
title: Error rate (%)
y_label: "%"
data: |
  week, checkout, search
  W1, 1.8, 0.9
  W2, 1.2, 0.8
  W3, 0.7, 0.6
  W4, 0.4, 0.5
```
:::

<!-- note: Both charts are written as CSV in the slide; no image files involved. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  text            |  fig            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Connectors]{.eyebrow}
## Sentences that point at things
:::

::: pane text {valign=middle}
Traffic reaches the [edge cache]{#t-edge .u} first. On a miss it falls through to the [origin]{#t-origin .u}, and only then to the [database]{#t-db .u}.

Move the boxes, resize the window, change the theme - the arrows re-route themselves, because endpoints are resolved from the live layout.
:::

::: pane fig
:::

```shape
rect #edge   at(76%, 26%) size(30%, 13%) label="Edge cache" stroke=@accent2
rect #origin at(76%, 52%) size(30%, 13%) label="Origin"
rect #db     at(76%, 78%) size(30%, 13%) label="Database"
arrow from(#edge.s) to(#origin.n) style=dashed
arrow from(#origin.s) to(#db.n) style=dashed
```

```connect
#t-edge -> #edge.w : color=@accent2
#t-origin -> #origin.w : color=@accent2
#t-db -> #db.w : color=@accent2
```

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  type            |  math           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Typography]{.eyebrow}
## Text, tables and math
:::

::: pane type {valign=middle}
**Bold**, *italic*, ~~struck~~, `inline code`, and [links](https://example.com).

> Pull quotes get an accent rule and quieter text.

| Plan | Seats | Price |
|---|---:|---:|
| Team | 8 | $96 |
| Business | 40 | $420 |
| Enterprise | — | Contact |
:::

::: pane math {valign=middle}
Inline math such as $O(1)$ sits in running text.

$$
p_{95} = \mu + 1.645\,\sigma\sqrt{\frac{n}{n-1}}
$$

*Converted to MathML at build time - no client-side JavaScript.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  desc            |  clip           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Media]{.eyebrow}
## Video is a first-class citizen
:::

::: pane desc {valign=middle}
```markdown
![Demo](media/demo.webm){.autoplay .loop .controls}
```

- Plays inline in the HTML export
- Falls back to the poster frame in PDF
- The file is embedded, so one HTML is the whole deck
:::

::: pane clip
![Demo clip](media/demo.webm){.autoplay .loop .controls poster=media/demo-poster.png fit=contain}
:::

---

```pane
+------------------------------------+
|                                    |
|  main                              |
|                                    |
+------------------------------------+
|  foot                              |
+------------------------------------+
```

::: pane main {align=center valign=middle}
## That is the whole vocabulary

`pane` · `::: pane` · `shape` · `connect` · `chart` · attributes · variables

*Nothing here needs a mouse.*
:::

::: pane foot {align=right}
<span class="foot">examples/showcase.md</span>
:::
