---
title: Mirzam - Theming and settings
author: Mirzam
aspect: "16:9"
theme: mirzam
mode: dark
css: themes/mirzam.css
math: typst
footer: Theming and settings
slide-number: "{n} / {total}"
---

<!-- chrome: none -->

# Theming and settings {.title-slide}

Every knob a deck has, and the order they are read in. This deck sets four of
them on itself: `theme: mirzam` for the palette, `css: themes/mirzam.css` for
the typography on top of it, `math: typst` for how its one formula is written,
and `footer:` for the line along the bottom of every slide after this one.

<!-- note: Press D. The whole deck flips, because the stylesheet defines both modes. This slide says `chrome: none`, which is why the footer starts on slide 2. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  list            |  how            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Palette]{.eyebrow}
## Named themes
:::

::: pane list {valign=middle}
| `theme:` | Palette |
|---|---|
| `default` | ours |
| `nord` | Nord |
| `solarized` | Solarized |
| `vscode` | VS Code Light+/Dark+ |
| `mirzam` | Mirzam's own |
| `wuwei` | warm greyscale |
:::

::: pane how {valign=middle}
```yaml
---
theme: nord
---
```

- A theme is a **palette**, not a design: it sets colour tokens and nothing
  else
- Each one defines a light and a dark variant
- An unknown name is a warning, not a failed build, and falls back to
  `default`

*On the command line: `mirzam build deck.md --theme nord`.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  order           |  why            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Light and dark]{.eyebrow}
## Four ways to choose a mode
:::

::: pane order {.card valign=middle}
1. `mode: dark` in frontmatter
2. `?mode=dark` in the URL
3. `D` in the viewer
4. the reader's `prefers-color-scheme`

*First one that is set wins.*
:::

::: pane why {valign=middle}
`mode:` is baked into the file, so a deck that must be dark never flashes
white while it loads.

Leave it unset and the deck follows the machine it is opened on, live, with no
reload — which is usually what you want for something you are emailing to
somebody.

`D` only lasts for that reading session; it changes nothing on disk.
:::

---

```pane
+--------------------------------------------+
|                                            |
|  head                                      |
+--------------------+-----------+-----------+
|                    |           |           |
|                    |           |           |
|  src               |  day      |  night    |
|                    |           |           |
|                    |           |           |
+--------------------+-----------+-----------+
```

::: pane head
[Per pane]{.eyebrow}
## Two palettes on one slide
:::

::: pane src {.card valign=middle}
```markdown
::: pane day {theme=wuwei mode=light}
Read at a desk.
:::
```

- A whole slide asks in a comment: `<!-- theme: nord -->`
- `theme=` alone follows the deck's mode; either may be given alone
- A pane's theme beats its slide's, which beats the deck's
:::

::: pane day {theme=wuwei mode=light valign=middle}
### Day
Two palettes on one slide — this half is **wuwei** in light.
:::

::: pane night {theme=wuwei mode=dark valign=middle}
### Night
The same theme drawn again for dark, rather than inverted.
:::

---

```pane
+--------------------------------------------------+
|  head                                            |
+----------------+----------------+----------------+
|                |                |                |
|  one           |  two           |  three         |
|                |                |                |
+----------------+----------------+----------------+
|                |                |                |
|  four          |  five          |  how           |
|                |                |                |
+----------------+----------------+----------------+
```

::: pane head
[Gallery]{.eyebrow} **Every palette, in the deck's own mode** — no pane below
names a mode, so `D` flips all five along with the slide around them.
:::

::: pane one {theme=default valign=middle}
[default]{.eyebrow}

Ink on paper, [a link](https://example.com), **bold**.

[`default` and `mirzam`: one palette.]{.small}
:::

::: pane two {theme=nord valign=middle}
[nord]{.eyebrow}

Cool blues, [a link](https://example.com), **bold**.

[Arctic and even, from Nord.]{.small}
:::

::: pane three {theme=solarized valign=middle}
[solarized]{.eyebrow}

Low glare, [a link](https://example.com), **bold**.

[Tuned for reading for a long time.]{.small}
:::

::: pane four {theme=vscode valign=middle}
[vscode]{.eyebrow}

Editor colours, [a link](https://example.com), **bold**.

[Light+ and Dark+, for a deck about code.]{.small}
:::

::: pane five {theme=wuwei valign=middle}
[wuwei]{.eyebrow}

Warm greyscale, [a link](https://example.com), **bold**.

[No accent colour, and quiet on purpose.]{.small}
:::

::: pane how {.card valign=middle}
- Each pane is just `{theme=…}`, no `mode=`
- So they follow the deck; `mode=` pins one
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  what           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Reference]{.eyebrow}
## Every frontmatter field
:::

::: pane src {.card valign=middle}
```yaml
---
title: Quarterly review
author: Your Name
theme: mirzam
mode: dark
aspect: "16:9"
css: themes/mirzam.css
split: h2
transition: fade 240ms
fit: shrink
math: typst
vars:
  seats: 8
---
```
:::

::: pane what {valign=middle}
- All optional; a deck with no frontmatter builds
- `css:` is resolved **relative to the deck file**
- `split:` starts a slide at every heading of that level
- `transition:` is the deck-wide page turn; a slide can override its half
- `math:` picks the formula syntax, `latex` (the default) or `typst`
- `vars:` are substituted with `{{ }}`, and arithmetic works: `{{ seats * 12 }}`
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  what           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Reference]{.eyebrow}
## The four that repeat on every slide
:::

::: pane src {.card valign=middle}
```yaml
masters: masters/cookbook.md
layout: body
footer: Internal
slide-number: "{n} / {total}"
```

```markdown
## body

+----------+
|  head    |
+----------+
|  main    |
+----------+
```

```markdown
<!-- layout: none -->
<!-- chrome: none -->
```
:::

::: pane what {valign=middle}
- `masters:` is a Markdown file: a heading names a shape, the `pane` block
  under it is the drawing. A mapping written here works for a short set
- `layout:` picks the deck's default, `<!-- layout: -->` one slide's
- A slide's own `pane` block always wins; `none` opts out
- `footer:` and `slide-number:` draw on every slide **and in the PDF**
- `<!-- chrome: none -->` drops both, as this deck's title slide does
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  what           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Per element]{.eyebrow}
## Settings smaller than a deck
:::

::: pane src {.card valign=middle}
```markdown
## Heading {#intro .center}

[a phrase]{#win .u}

::: pane hero {.card valign=middle}
Content of the pane.
:::
```

An attribute block takes an `#id`, any number of `.class`es, and `key=value`
pairs.
:::

::: pane what {valign=middle}
- On a **heading or a span**: the id a connector or an annotation points at,
  and the classes your stylesheet styles
- On a **pane**: `align`, `valign`, `bg`, `dim`, `blur`, `scrim`, `fit`
- `.center`, `.right`, `.small` and `.u` come with the renderer; anything else
  is yours to define

*In a plain Markdown reader the braces are literal text — that is the price,
and the reason the syntax is this quiet.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  warn           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Custom CSS]{.eyebrow}
## Override the tokens, not the layout
:::

::: pane src {.card valign=middle}
```css
:root {
  --mz-slide-bg: #0d1117;
  --mz-fg: #e9edf5;
  --mz-accent1: #5b8cff;
  --mz-accent2: #2dd4bf;
}
:root[data-mode="light"] {
  --mz-slide-bg: #ffffff;
  --mz-fg: #17202a;
  --mz-accent1: #2f5fe0;
  --mz-accent2: #0f766e;
}
```
:::

::: pane warn {valign=middle}
The built-in tokens carry no specificity, which is what lets a plain `:root`
beat them — and is why a stylesheet that sets its colours **once** pins the
deck to one mode. `D` then changes `data-mode` and nothing moves.

Set every token in both blocks, or set none and let the theme do it.

[A test holds `examples/themes/*.css` to that rule, in both modes.]{.small}
:::

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
[Custom CSS]{.eyebrow}
## Margins are tokens too
:::

::: pane src {.card valign=middle}
```css
:root {
  --mz-grid-pad-y: 56px;
  --mz-grid-pad-x: 72px;
  --mz-grid-gap: 28px;
}
.framed {
  --mz-pane-border: 1px solid var(--mz-border);
  --mz-pane-radius: 8px;
  --mz-pane-pad: 14px 18px;
}
```

```markdown
::: pane fig {.framed}
```
:::

::: pane out {valign=middle}
Six tokens set a deck's margins, the gap between panes, and the padding,
border and radius of a pane. Every one carries its built-in value as a
fallback, so a deck that sets none renders exactly as it always did.

These are **not** palette tokens — no built-in theme defines one, and `theme:`
stays a choice of colour.

Because custom properties inherit, the same names work at any scale: on
`:root` they move the deck, on a class you put on one pane they move that pane.
:::

<!-- note: The stylesheet this deck loads sets the first three. Compare the margins here with 04-components.md, which does not. -->


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
[Custom CSS]{.eyebrow}
## Bullets are a choice, not a default
:::

::: pane src {.card valign=middle}
```css
.markers {
  --mz-bullet: "→  ";
  --mz-bullet-2: "·  ";
  --mz-number: upper-roman;
  --mz-marker: var(--mz-accent2);
}
```

A quoted string is a marker too — and **carries its own trailing space**, since
the browser adds none after one.
:::

::: pane out {.markers valign=middle}
- A point, and under it
  - a qualification

1. Write the deck
2. Build it

[Each depth reads its own dial, so the level under this one keeps its own mark.]{.small}
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  what           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Overflow]{.eyebrow}
## When a slide has too much on it
:::

::: pane src {.card valign=middle}
```yaml
---
fit: shrink
---
```

```markdown
::: pane notes {fit=shrink}
```

```bash
mirzam build long.md --fit shrink
```
:::

::: pane what {valign=middle}
Without it, a pane that overflows is **clipped** — the last line simply is not
there, and nothing on screen says so.

`fit: shrink` scales the pane's text down until it fits instead. It is the
right default for a document you converted rather than wrote, and the wrong
one for a deck you are still editing, where you want to see the overflow and
cut something.

*`node scripts/check-layout.mjs` finds them either way.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  src             |  what           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Math]{.eyebrow}
## A dialect for formulas
:::

::: pane src {.card valign=middle}
```yaml
---
math: typst
---
```

```markdown
$x = (-b pm sqrt(b^2 - 4a c))/(2a)$
```

Which renders as:

$x = (-b pm sqrt(b^2 - 4a c))/(2a)$
:::

::: pane what {valign=middle}
- Typst's math syntax, without the backslashes: `a/b` is a fraction, `sqrt()`
  is a root, Greek goes by name — `alpha`, `Omega`
- Per **deck**, not per formula; the default is `latex`, so every existing
  deck reads as it always did
- Both dialects render to MathML through the same path, so the result is
  identical down to the font
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
## That is every setting

frontmatter · `{#id .class key=value}` · a stylesheet of your own

*Read in that order, last one wins.*
:::

::: pane foot {align=right}
<span class="foot">examples/06-theming.md</span>
:::
