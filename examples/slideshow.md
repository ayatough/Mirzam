---
title: Mirzam — Slides on a loop
author: Mirzam
aspect: "16:9"
theme: mirzam
mode: dark
transition: fade 1500ms
autoplay: 6s loop
---

```pane
+------------------------------------+
|                                    |
|  hero                              |
|                                    |
+------------------------------------+
```

::: pane hero {.bleed bg=media/bg/mesh.jpg dim=0.45 scrim=bottom valign=bottom}
[Mirzam]{.eyebrow}
## Slides on a loop {.sl-title}

[This deck is turning its own pages — `autoplay: 6s loop` in the frontmatter. `A` pauses it, `H` shows the controls it hides.]{.sl-note .small}
:::

```anim
[enter] .sl-title : words blur-in 500ms stagger=60ms
[enter] .sl-note  : fade-in 400ms delay=1100ms
```

<!-- note: Everything on a slide plays on arrival; the loop only turns pages. -->

---

```pane
+------------------------------------+
|                                    |
|  hero                              |
|                                    |
+------------------------------------+
```

::: pane hero {.bleed bg=media/bg/city-night.jpg dim=0.4 scrim=bottom valign=bottom}
## One photograph, one line {.sl-city}

[A full-bleed image and a caption that arrives on its own beat: an exhibition loop, a screensaver, a slideshow — the same file either way.]{.sl-c1}
:::

```anim
[enter] .sl-city : chars fade-in 400ms stagger=20ms
[enter] .sl-c1   : slide-in 400ms dir=up delay=800ms
```

---

```pane
+------------------------------------+
|                                    |
|  hero                              |
|                                    |
+------------------------------------+
```

::: pane hero {.bleed bg=media/bg/mountains.jpg dim=0.35 scrim=bottom valign=bottom}
## Captions on their own clock {.sl-mtn}

[Each line lands a beat after the one before — a `delay=` on its entrance.]{.sl-m1}

[Nothing waits for the page to turn; the loop keeps one clock.]{.sl-m2}
:::

```anim
[enter] .sl-mtn : words fade-in 400ms stagger=40ms
[enter] .sl-m1  : slide-in 400ms dir=up delay=900ms
[enter] .sl-m2  : slide-in 400ms dir=up delay=1700ms
```

<!-- note: Click steps would hold a slide one interval per step; a loop of photos wants everything on arrival instead. -->

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
[The recipe]{.eyebrow}
## The whole file is Markdown
:::

::: pane body {.card valign=middle}
````markdown
---
autoplay: 6s loop
---

::: pane hero {.bleed bg=photo.jpg dim=0.4 scrim=bottom}
## A caption that arrives on its own beat {.telop}

[And a line that follows it, unprompted.]{.sub}
:::

```anim
[enter] .telop : words blur-in 500ms stagger=60ms
[enter] .sub   : fade-in 400ms delay=900ms
```
````
:::

<!-- note: One photo per slide, one anim block per caption. That is the entire pattern. -->

---

```pane
+------------------------------------+
|                                    |
|  hero                              |
|                                    |
+------------------------------------+
```

::: pane hero {.bleed bg=media/bg/city-night.jpg bg-pos=top dim=0.5 scrim=bottom valign=middle align=center}
## Write Markdown. Loop it anywhere. {.sl-end}

[`?autoplay=off` stills this deck · `H` shows the controls · github.com/ayatough/Mirzam]{.sl-e1 .small}
:::

```anim
[enter] .sl-end : words fade-in 450ms stagger=50ms
[enter] .sl-e1  : fade-in 400ms delay=1200ms
```

<!-- note: After this slide the deck wraps forwards to the first, entrances and all. -->
