---
title: Mirzam - Start here
author: Mirzam
aspect: "16:9"
css: themes/mirzam.css
---

# Start here {.title-slide}

The smallest deck that works, how a page break is written, and the three
commands. Six slides, and nothing on the way needs a mouse.

<!-- note: This is the first deck of the tutorial series: 01 start, 02 writing, 03 layout, 04 components, 05 motion, 06 theming. -->

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
[Step 1]{.eyebrow}
## Five lines are a deck
:::

::: pane src {.card valign=middle}
````markdown
# What changed this week

---

## Latency

p95 dropped in **every region**.
````
:::

::: pane out {valign=middle}
- Two slides, because of the one `---`
- No frontmatter, no configuration, no theme chosen
- The same file still reads as a document on GitHub

*Save it as `deck.md`, or have `mirzam new deck.md` write it. That is the whole
setup step.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  rule            |  auto           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Step 2]{.eyebrow}
## Where the page breaks
:::

::: pane rule {valign=middle}
A line of three or more hyphens, on its own, outside any code fence:

```markdown
Last line of this slide.

---

First line of the next one.
```

Inside a fenced block it is just text, so a code sample can contain one.
:::

::: pane auto {valign=middle}
A document nobody wrote for slides — a README, a set of notes — breaks at its
own headings instead:

```bash
mirzam build notes.md --split h2
```

or, for a file you own, in frontmatter:

```yaml
split: h2
```

`---` still breaks a page either way.
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
[Step 3]{.eyebrow}
## Frontmatter, if you want it
:::

::: pane src {.card valign=middle}
```yaml
---
title: Weekly review
author: Your Name
aspect: "16:9"      # or "4:3"
theme: mirzam
---
```
:::

::: pane out {valign=middle}
- Every field is optional; the deck above had none
- `title` names the browser tab and the PDF
- YAML at the top of a Markdown file is a convention GitHub already knows

*The full list, and how to override any of it, is `06-theming.md`.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  cmds            |  notes          |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Step 4]{.eyebrow}
## Three commands
:::

::: pane cmds {.card valign=middle}
```bash
mirzam serve deck.md

mirzam build deck.md -o out

mirzam export pdf deck.md -o deck.pdf
```
:::

::: pane notes {valign=middle}
- **serve** — live preview on `localhost:4321`, re-rendering only the slide
  you touched
- **build** — one self-contained HTML file. Images, video and fonts are
  inlined, so there is no assets folder to carry
- **export pdf** — the same deck, flattened, every animated slide fully
  revealed

*No install yet? The browser editor does `build` with no toolchain at all.*
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  keys            |  next           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Step 5]{.eyebrow}
## Presenting it
:::

::: pane keys {valign=middle}
| Key | |
|---|---|
| `→` `←` | next / previous, and step through a build |
| `N` | speaker notes |
| `S` | the Markdown behind this slide |
| `P` | presenter window: next slide, notes, timer |
| `F` | fullscreen |
| `D` | dark mode |
| `/` | every key this deck responds to |
:::

::: pane next {valign=middle}
Speaker notes are HTML comments, so they stay invisible everywhere else:

```markdown
<!-- note: Skip the derivation if time is short. -->
```

On a phone: swipe to turn the page, swipe up for the notes.

[**Next:** `02-writing.md` — what plain Markdown looks like on a slide.]{.small}
:::

<!-- note: Press N now; this is what the audience never sees. -->
