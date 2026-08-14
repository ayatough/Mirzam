---
title: Mirzam - Motion
author: Mirzam
aspect: "16:9"
css: themes/mirzam.css
mode: dark
transition: slide-left 320ms
---

# Motion, written down {.title-slide}

Press `→` to walk through. Every animation on the following slides is three
lines of Markdown, and none of it is needed to read the deck.

<!-- note: The deck-wide transition is slide-left; individual slides override it below. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------------------------+
|                                    |
|  body                              |
+------------------------------------+
```

::: pane head
[The idea]{.eyebrow}
## Animation is a timeline, not a plugin {.m-tl}
:::

::: pane body {valign=middle}
An `anim` block compiles to a timeline embedded in the slide — triggers,
targets, effects and easing all resolved at build time, so the runtime plays a
curve instead of computing one. Press `→` twice.

[This line waited for a click.]{.m-callout .small}

[And this one followed it, unprompted.]{.m-echo .small}
:::

```anim
[enter]   .m-tl      : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] .m-callout : slide-in 400ms dir=up ease=spring(1,180,20)
[click 2] .m-echo    : fade-in 300ms
```

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
[Entrances]{.eyebrow}
## One line, one movement {.m-title}
:::

::: pane src {.card valign=middle}
````markdown
```anim
[enter]   .m-title : chars fade-in 500ms stagger=25ms
[click 1] .m-a     : slide-in 400ms dir=up
[click 2] .m-b     : wipe-in 500ms dir=right
[click 3] .m-c     : zoom-in 400ms ease=out-back
```
````

`→` plays the next one.
:::

::: pane out {valign=middle}
[**slide-in** — arrives travelling]{.m-a}

[**wipe-in** — uncovered by an edge]{.m-b}

[**zoom-in** — grows into place]{.m-c}
:::

```anim
[enter]   .m-title : chars fade-in 500ms stagger=25ms
[click 1] .m-a     : slide-in 400ms dir=up
[click 2] .m-b     : wipe-in 500ms dir=right
[click 3] .m-c     : zoom-in 400ms ease=out-back
```

<!-- note: chars splitting happens at build time; the runtime only selects the spans. -->

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
[Charts]{.eyebrow}
## Bars that grow as you talk
:::

::: pane chart
```chart
type: bar
id: rev
title: Revenue by region ($M)
data: |
  region, 2024, 2025
  Americas, 18, 27
  EMEA, 12, 21
  APAC, 7, 19
```
:::

::: pane note {valign=middle}
Every mark already has an id — and the id names the bar *and* its value
label, as one group — so a bar rises with its number on top:

```markdown
[click 1] #rev-0-0 : wipe-in dir=up
[click 1] #rev-1-0 : wipe-in dir=up delay=80ms
```

[Growth is broad, not one region carrying the quarter.]{.m-punch}
:::

```anim
[click 1] #rev-0-0 : wipe-in 450ms dir=up ease=out-cubic
[click 1] #rev-1-0 : wipe-in 450ms dir=up delay=80ms ease=out-cubic
[click 2] #rev-0-1 : wipe-in 450ms dir=up ease=out-cubic
[click 2] #rev-1-1 : wipe-in 450ms dir=up delay=80ms ease=out-cubic
[click 3] #rev-0-2 : wipe-in 450ms dir=up ease=out-cubic
[click 3] #rev-1-2 : wipe-in 450ms dir=up delay=80ms ease=out-cubic
[after #rev-1-2 +150ms] .m-punch : fade-in 400ms
```

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
[Diagrams]{.eyebrow}
## A diagram that draws itself
:::

::: pane text {valign=middle}
Shapes carry ids, and `draw` animates a stroke from nothing to its full length.

```markdown
[click 1] #ingest : pop 350ms
[click 2] #a1     : draw 500ms
```

[Each box lands, then the arrow reaches for the next one.]{.m-cap .small}
:::

::: pane fig
:::

```shape
rect #ingest  at(74%, 24%) size(34%, 13%) label="Ingest" stroke=@accent2
rect #store   at(74%, 50%) size(34%, 13%) label="Store"
rect #serve   at(74%, 76%) size(34%, 13%) label="Serve" stroke=@accent2
arrow #a1 from(#ingest.s) to(#store.n)
arrow #a2 from(#store.s) to(#serve.n)
```

```anim
[click 1] #ingest : pop 350ms ease=out-back
[click 2] #a1     : draw 450ms
[click 3] #store  : pop 350ms ease=out-back
[click 4] #a2     : draw 450ms
[click 5] #serve  : pop 350ms ease=out-back
[after #serve +200ms] .m-cap : fade-in 350ms
```

<!-- note: Five clicks: box, arrow, box, arrow, box. The caption follows on its own. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+-----------+-----------+------------+
|           |           |            |
|  a        |  b        |  c         |
|           |           |            |
+-----------+-----------+------------+
|  foot                              |
+------------------------------------+
```

::: pane head
[Images]{.eyebrow}
## Photos come and go
:::

::: pane a {.ph-a valign=middle}
![A grid of glowing nodes](media/bg/mesh.jpg)
:::

::: pane b {.ph-b valign=middle}
![A city at night](media/bg/city-night.jpg)
:::

::: pane c {.ph-c valign=middle}
![Mountains at dusk](media/bg/mountains.jpg)
:::

::: pane foot
[A pane holding a photo is a target like any other. Each one arrives on its own click, and none of them leaves — stepping `←` takes them back off in the order they came.]{.small}
:::

```anim
[click 1] .ph-a : fade-in 500ms
[click 2] .ph-b : zoom-in 500ms
[click 3] .ph-c : wipe-in 500ms dir=up
```

<!-- note: Three photos, three clicks, three different entrances. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------------------------+
|                                    |
|  body                              |
|                                    |
+------------------------------------+
```

::: pane head
[Page turns]{.eyebrow}
## This slide arrives by wiping {.m-h}
:::

::: pane body {valign=middle}
The deck turns pages with `transition: slide-left`. A slide overrides its half
of that with an ordinary whole-slide track — this one wipes in and irises out:

```markdown
[enter] slide : wipe-in 450ms dir=right
[exit]  slide : iris-out 450ms
```

`none`, `fade`, `slide-*`, `wipe-*`, `zoom` and `iris` are the deck-wide names.
:::

```anim
[enter] slide : wipe-in 450ms dir=right
[exit]  slide : iris-out 450ms
```

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------------------------+
|                                    |
|  body                              |
|                                    |
+------------------------------------+
```

::: pane head
[The rule]{.eyebrow}
## Motion is something a deck gains {.m-last}
:::

::: pane body {valign=middle}
[Elements are laid out in their **final** state. The runtime is the only thing that ever puts one in its starting state, so a deck read without JavaScript — and the PDF export, which ships none — shows every slide fully revealed.]{.m-p1}

[Under `prefers-reduced-motion` the reveals still happen and stepping still works. Only the movement is dropped.]{.m-p2}
:::

```anim
[enter]   .m-last : words blur-in 500ms stagger=60ms
[click 1] .m-p1   : fade-in 400ms
[click 2] .m-p2   : fade-in 400ms
[enter]   slide   : zoom-in 400ms
```

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  keys            |  stage          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Effects]{.eyebrow}
## Things you do, not things the deck does
:::

::: pane keys {valign=middle}
An `effects` block binds a key to a flourish. Press one; `Esc` clears it.

| Key | |
|---|---|
| `1` | flash |
| `2` | shake |
| `3` | speed lines |
| `4` | explosion |
| `e` | emoji |
| `c` | confetti |
| `m` | a comment sweeps past |
:::

::: pane stage {valign=middle align=center}
[These never reach the PDF.]{.m-fx}

An animation is part of the document — ordered, exported, the same every
time. An effect is part of the *performance*. Nothing is lost if you never
press the key.
:::

```effects
1 : flash
2 : shake
3 : lines
4 : boom
e : burst 🎉
c : confetti
m : danmaku "this bit matters"
```

```anim
[enter] .m-fx : fade-in 400ms
```
