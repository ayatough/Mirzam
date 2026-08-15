---
title: Mirzam - Slides that live in your repo
author: Mirzam
aspect: "16:9"
theme: mirzam
mode: dark
transition: slide-left 320ms
vars:
  decks_built: 1240
  time_saved_min: 47
  seats: 8
  price_per_seat: 12
---

::: pane hero {.bleed bg-light=../docs/brand/mirzam-hero-light.webp bg-dark=../docs/brand/mirzam-hero-dark.webp text=dark}
# Slides that live in<br>your repository {.title-slide}

Plain Markdown in. Presentation-grade decks out.
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  problem         |  cost           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[The problem]{.eyebrow}
## Decks are where engineering time goes to die
:::

::: pane problem {.card valign=middle}
### What teams actually do

- Rebuild the same diagram in three tools
- Fight text boxes instead of writing
- Email `deck_final_v7_REAL.pptx`
- Watch a 90-slide file crawl
:::

::: pane cost {.card valign=middle}
### What it costs

<div class="metric">{{ time_saved_min }} min</div>
<div class="metric-label">wasted per deck on layout alone</div>

Multiply by every review cycle, every quarter.
:::

<!-- note: Open with the pptx-in-email story; everyone recognizes it. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+---------+---------+----------------+
|         |         |                |
|         |         |                |
|  a      |  b      |  c             |
|         |         |                |
|         |         |                |
+---------+---------+----------------+
```

::: pane head
[The idea]{.eyebrow}
## Text is the source of truth
:::

::: pane a {.card}
### Write

Ordinary Markdown, versioned in git. Reviewable in a pull request, diffable line by line.
:::

::: pane b {.card}
### Arrange

Draw the layout as ASCII. What you sketch is what you get - no dragging, no snapping.
:::

::: pane c {.card}
### Ship

One HTML file, or a PDF. Video and animation survive; nothing is flattened to a screenshot.
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  chart           |  points         |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Traction]{.eyebrow}
## Adoption is compounding
:::

::: pane chart
```chart
type: area
id: adoption
title: Decks built per quarter
y_label: decks
data: data/adoption.csv
```
:::

::: pane points {valign=middle}
- **{{ decks_built }}** decks built last quarter
- Self-serve growth is [outpacing enterprise]{#growth .u} 3:1
- Median deck: 24 slides, 0 design tickets

*Data: `data/adoption.csv`, rendered at build time.*
:::

```connect
#growth -> #adoption-0-5 : color=@accent2
```

<!-- note: The arrow points at the final Self-serve data point - it follows the chart, not a screenshot. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  left            |  right          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[How it works]{.eyebrow}
## One engine, everywhere you write
:::

::: pane left {valign=middle}
The [parser]{#t-parse .u} and [renderer]{#t-render .u} are one Rust core, compiled twice: a native binary for the CLI and WebAssembly for editors and browsers.

Same input, same pixels - terminal, VS Code, or a phone.
:::

::: pane right
:::

```shape
rect #core   at(74%, 32%) size(34%, 14%) label="Rust core" stroke=@accent1
rect #native at(63%, 62%) size(20%, 13%) label="CLI"
rect #wasm   at(85%, 62%) size(20%, 13%) label="WASM" stroke=@accent2
arrow from(#core.s) to(#native.n)
arrow from(#core.s) to(#wasm.n)
text at(74%, 82%) "identical output" .small
```

```connect
#t-parse -> #core.w : color=@accent2
#t-render -> #core.w : color=@accent2 style=dashed
```

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  speed           |  bars           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
|  note                              |
+------------------------------------+
```

::: pane head
[Performance]{.eyebrow}
## Deck size stops mattering
:::

::: pane speed {valign=middle}
<div class="metric metric-up">2.3 ms</div>
<div class="metric-label">to reflect an edit in a 500-slide deck</div>

Only the slide you touched is re-rendered - the rest is cached.
:::

::: pane bars
```chart
type: bar
id: perf
title: Full build vs. single-slide edit (ms)
highlight: edit
data: |
  deck size, full build, edit
  20 slides, 4.6, 0.3
  120 slides, 19.5, 0.7
  500 slides, 78.2, 2.3
```
:::

::: pane note {align=center}
*Measured on the standing benchmark in CI, release build.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  table           |  split          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Where it fits]{.eyebrow}
## Against the tools you already use
:::

::: pane table {valign=middle}
| | PowerPoint | Marp | **Mirzam** |
|---|:---:|:---:|:---:|
| Diffable in git | — | ✓ | **✓** |
| Visual layout control | ✓ | — | **✓** |
| Video & animation | ✓ | — | **✓** |
| Charts from data | ✓ | — | **✓** |
| Opens in any editor | — | ✓ | **✓** |
:::

::: pane split
```chart
type: pie
id: split
title: Where deck time goes today
data: |
  activity, share
  Layout fiddling, 42
  Writing, 31
  Charts, 18
  Review, 9
```
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  price           |  what           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
|  foot                              |
+------------------------------------+
```

::: pane head
[Pricing]{.eyebrow}
## Priced per seat, not per deck
:::

::: pane price {.card valign=middle align=center}
<div class="metric">${{ price_per_seat }}</div>
<div class="metric-label">per seat / month</div>

A {{ seats }}-person team: **${{ price_per_seat * seats }}/mo**
:::

::: pane what {.card valign=middle}
### Included

- Unlimited decks and exports
- Editor extensions and CLI
- Self-hosted rendering, no upload
- Your files stay yours: plain `.md`
:::

::: pane foot {align=center}
*Open core: the renderer is MIT-licensed. You are never locked in.*
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
## Write the deck. Ship the deck.

**mirzam build deck.md** — that is the whole workflow.

*github.com/ayatough/Mirzam*
:::

::: pane foot {align=right}
[Made with Mirzam]{.small}
:::

<!-- note: Close by rebuilding this very deck live from the terminal. -->
