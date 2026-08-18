---
title: Mirzam — Slides on a loop
author: Mirzam
aspect: "16:9"
theme: mirzam
mode: dark
transition: fade 500ms
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

[This deck is turning its own pages — `autoplay: 6s loop` in the frontmatter. `A` pauses it; anything else just restarts the countdown.]{.sl-note .small}
:::

```anim
[enter]   .sl-title : words blur-in 500ms stagger=60ms
[click 1] .sl-note  : fade-in 400ms
```

<!-- note: The loop advances by click steps, so the caption above gets its own six seconds. -->

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
[enter]   .sl-city : chars fade-in 400ms stagger=20ms
[click 1] .sl-c1   : slide-in 400ms dir=up
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
## Captions keep the pace {.sl-mtn}

[Every advance is one click step — the same step `→` would take.]{.sl-m1}

[So a slide with two lines holds the screen two beats longer.]{.sl-m2}
:::

```anim
[enter]   .sl-mtn : words fade-in 400ms stagger=40ms
[click 1] .sl-m1  : slide-in 400ms dir=up
[click 2] .sl-m2  : slide-in 400ms dir=up
```

<!-- note: Two steps here, so this slide takes three intervals in the loop. -->

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
transition: fade 500ms
---

::: pane hero {.bleed bg=photo.jpg dim=0.4 scrim=bottom}
## A caption that arrives on its own beat {.telop}
:::

```anim
[enter] .telop : words blur-in 500ms stagger=60ms
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

[`?autoplay=off` stills this deck · `?controls=none` bares the screen · github.com/ayatough/Mirzam]{.sl-e1 .small}
:::

```anim
[enter]   .sl-end : words fade-in 450ms stagger=50ms
[click 1] .sl-e1  : fade-in 400ms
```

<!-- note: After this step the deck wraps forwards to the first slide, entrances and all. -->
