---
title: Mirzam - Theming and settings
author: Mirzam
aspect: "16:9"
theme: mirzam
css: themes/mirzam.css
---

# Theming and settings {.title-slide}

Every knob a deck has, and the order they are read in. This deck sets two of
them on itself: `theme: mirzam` for the palette, `css: themes/mirzam.css` for
the typography on top of it.

<!-- note: Press D. The whole deck flips, because the stylesheet defines both modes. -->

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
