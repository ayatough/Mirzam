---
title: Mirzam Charts
author: Mirzam
aspect: "16:9"
theme: mirzam
mode: dark
transition: fade 240ms
---

# Charts {.title-slide}

Six kinds, from CSV written in the slide. Every mark has a name, so an arrow can
point at one of them.

<!-- note: This deck owns the `chart` block. Shapes, connectors and the rest are in 04-components.md. -->

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
+---------------------+--------------+
|                     |              |
|                     |              |
|  chart              |  note        |
|                     |              |
|                     |              |
+---------------------+--------------+
```

::: pane head
[Charts]{.eyebrow}
## An axis that measures
:::

::: pane chart
```chart
type: line
id: recovery
title: Error rate after deploy (%), by minute
y_label: "%"
x: minute
data: |
  checkout, search, minute
  1.8, 0.9, 0
  1.2, 0.8, 1
  0.7, 0.6, 2
  0.5, 0.5, 5
  0.4, 0.4, 15
  0.4, 0.4, 60
```
:::

::: pane note {valign=middle}
- `minute` holds **numbers**, so the axis measures: the hour between 15 and 60
  is drawn as the hour it is, not as one more step
- A column of labels — `W1`, `2024 Q1` — keeps the even spacing it always had
- `x:` names the column, so a file whose value columns come first needs no
  editing to be plotted
:::

<!-- note: The tail is flat and sparse; on an evenly spaced axis it would read as a much faster recovery than it was. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  sum             |  share          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Charts]{.eyebrow}
## Stacked two ways
:::

::: pane sum
```chart
type: bar
stacked: true
title: "stacked: true — how much"
data: |
  quarter, self-serve, enterprise, partner
  Q1, 120, 40, 12
  Q2, 210, 65, 30
  Q3, 340, 110, 48
```
:::

::: pane share
```chart
type: bar
stacked: percent
title: "stacked: percent — what share"
data: |
  quarter, self-serve, enterprise, partner
  Q1, 120, 40, 12
  Q2, 210, 65, 30
  Q3, 340, 110, 48
```
:::

<!-- note: Same data, two questions. `true` takes the axis to the tallest column; `percent` fills every column, so the segments read as shares of it. -->


---

```pane
+------------------------------------+
|                                    |
|  head                              |
+---------------------+--------------+
|                     |              |
|                     |              |
|  chart              |  note        |
|                     |              |
|                     |              |
+---------------------+--------------+
```

::: pane head
[Charts]{.eyebrow}
## On its side
:::

::: pane chart
```chart
type: bar
horizontal: true
title: Agreed, by question (%)
data: |
  question, agree
  Deployment frequency, 82
  Lead time for changes, 64
  Mean time to restore, 41
  Change failure rate, 27
```
:::

::: pane note {valign=middle}
- A column chart is bad at exactly this: a name worth reading has nowhere to go
  under a column
- The side margin grows to hold the longest one
- The ranked list these usually are reads top to bottom anyway
:::

<!-- note: `horizontal: true`. Stacking works the same way along the row. -->


---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  dots            |  note           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Charts]{.eyebrow}
## Points, with nothing joining them
:::

::: pane dots
```chart
type: scatter
title: Settling time against load
y_label: ms
data: |
  load, before, after
  1.0, 21, 12
  2.4, 38, 19
  3.1, 32, 24
  4.8, 59, 28
  6.2, 61, 31
  8.0, 74, 39
```
:::

::: pane note {valign=middle}
`type: scatter` is a `line` chart without the line: the same value axis, the
same marks, the same ids.

Its x column is numbers, so the points sit where the numbers put them — which
is the whole reason to draw one.
:::

<!-- note: Everything the value axis learned applies here; a scatter is what needed it. -->


---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  pie             |  donut          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Charts]{.eyebrow}
## A pie, and a pie with a hole
:::

::: pane pie
```chart
type: pie
title: Where the time goes
data: |
  stage, ms
  Parse, 4
  Layout, 11
  Render, 9
  Write, 2
```
:::

::: pane donut
```chart
type: pie
inner: 0.6
title: The same, with the total
data: |
  stage, ms
  Parse, 4
  Layout, 11
  Render, 9
  Write, 2
```
:::

<!-- note: `inner:` is a fraction of the radius. From 0.45 up the hole carries the total, which a pie has nowhere else to put. -->


---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  cloud           |  note           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Charts]{.eyebrow}
## Three dimensions, still vector
:::

::: pane cloud
```chart
type: scatter3d
id: samples
azim: 45
elev: 26
title: Three runs, three settings
data: |
  width, depth, height, group
  1.8, 2.1, 1.4, warm
  2.4, 1.7, 1.9, warm
  1.5, 2.6, 1.1, warm
  2.1, 2.3, 1.7, warm
  4.9, 4.2, 3.4, hot
  5.4, 4.8, 3.9, hot
  5.1, 3.9, 3.1, hot
  4.6, 4.5, 3.7, hot
  3.2, 6.4, 2.0, cold
  3.8, 6.1, 2.4, cold
  2.9, 6.8, 1.8, cold
  3.5, 6.2, 2.2, cold
```
:::

::: pane note {valign=middle}
The points are projected **at build time** and emitted as ordinary marks, so
this prints to PDF at any resolution and ships no code to the browser.

`azim` and `elev` turn an orthographic camera. The three faces turned away are
the ones drawn, so the box is never in front of the data.
:::

<!-- note: Points only, and no rotating it in the viewer: both deliberate, and both written down in docs/syntax.md. -->


---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  note            |  cloud          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Charts]{.eyebrow}
## Every mark has a name
:::

::: pane note {valign=middle}
Every mark has a name: the chart's id, the series and the row. `#runs-1-0` is
[the first point of the second series]{#t-hot} — and an annotation can be
tied to it without knowing it came out of a projection.

It holds from every camera angle, because turning the camera changes which
disc is drawn over which and nothing else. A picture of a chart cannot offer
that.
:::

::: pane cloud
```chart
type: scatter3d
id: runs
azim: 30
elev: 24
data: |
  width, depth, height, group
  1.8, 2.1, 1.4, warm
  2.4, 1.7, 1.9, warm
  1.5, 2.6, 1.1, warm
  5.4, 4.8, 3.9, hot
  4.9, 4.2, 3.4, hot
  5.1, 3.9, 3.1, hot
```
:::

```annotate
highlight #t-hot    : color=@accent2 step=1
circle    #runs-1-0 : color=@accent2 step=1 pad=9
```

<!-- note: The pairing, not an arrow: both halves are one annotation with the same step, so they arrive together in one colour and nothing crosses the slide. The circle is placed from the mark's live box, which is how it reaches a point that only exists once the projection has run. -->


---

```pane
+------------------------------------+
|                                    |
|                                    |
|  main                              |
|                                    |
|                                    |
+------------------------------------+
|  foot                              |
+------------------------------------+
```

::: pane main {align=center valign=middle}
## That is the whole vocabulary

`bar` · `line` · `area` · `scatter` · `scatter3d` · `pie`

`x` · `stacked` · `horizontal` · `inner` · `azim` · `elev` · `zoom` ·
`y_label` · `highlight` · `colors` · `legend`

Everything else lives next door, in `04-components.md`.
:::

::: pane foot {align=right}
[examples/07-charts.md]{.small}
:::
