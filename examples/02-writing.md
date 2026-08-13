---
title: Mirzam - Writing a slide
author: Mirzam
aspect: "16:9"
css: themes/mirzam.css
---

# Writing a slide {.title-slide}

Every mark Mirzam understands inside a pane: text, lists, tables, maths — and
the handful of additions CommonMark does not have.

Nothing here needs a layout. That is the next deck.

<!-- note: This deck is the reference the markup coverage test checks against. A mark that works and is not shown here fails CI. -->

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
[Contents]{.eyebrow}
## A contents page that writes itself
:::

::: pane src {.card valign=middle}
````markdown
```toc
from: 2
depth: 2
current: true
```
````

- `from` skips the deck's own title
- `current` marks the section you are inside
:::

::: pane out {valign=middle fit=shrink}
```toc
from: 2
depth: 2
current: true
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
|  src             |  out            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Structure]{.eyebrow}
## Headings
:::

::: pane src {.card valign=middle}
```markdown
# One, the title of the deck

## Two, a slide's own heading

### Three, for the detail under it
```

Two levels are usually enough on a slide. Three is a sign it wants to be two
slides.
:::

::: pane out {valign=middle}
## Two, a slide's own heading

### Three, for the detail under it

Paragraphs are paragraphs: a blank line separates them, a single newline does
not, so wrap the source where your editor likes.
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
[Emphasis]{.eyebrow}
## Marking a phrase
:::

::: pane src {.card valign=middle}
```markdown
**bold**  *italic*  `code`
~~struck~~
==highlighted==
++underlined++
[a link](https://example.com)
```

Underline is `++text++`, **not** `__text__` — CommonMark reads double
underscores as bold, and the same file has to render on GitHub.
:::

::: pane out {valign=middle}
**Bold** for the one phrase they should leave with. *Italic* for a term being
introduced. `inline code` for an identifier.

~~Struck~~ for something withdrawn. ==Highlighted== like a marker pen.
++Underlined++ where a rule reads better than a colour.

And [a link](https://example.com), which stays clickable.
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
[Emphasis]{.eyebrow}
## Colour and size
:::

::: pane src {.card valign=middle}
```markdown
[quiet]{.muted}   [loud]{.big}
[the point]{.accent}
[careful]{.danger}
```

There is no way to write a colour literal, on purpose: a hex value picked
against a white slide is the one thing that cannot follow the deck into dark
mode. Every class here is a theme token.
:::

::: pane out {valign=middle}
[small]{.small} · [normal]{.muted} · [big]{.big}

[accent]{.accent} · [accent2]{.accent2} · [danger]{.danger}

[Press `D`.]{.accent} Every one of them moves with the palette.
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
[Blocks]{.eyebrow}
## Quotes, asides and code
:::

::: pane src {.card valign=middle}
````markdown
> Somebody else's voice.

<div class="box">
An aside that is not a quotation.
</div>

```rust
fn main() { ... }
```
````

Name the language after the fence and the block is coloured. A horizontal rule
is `***` — three hyphens would end the slide.
:::

::: pane out {valign=middle}
> A quotation gets an accent rule and quieter text.

<div class="box">
<code>.box</code> is in the renderer, not a theme.
</div>

```rust
fn main() {
    println!("a fence keeps its shape");
}
```

```python
def greet(name):  # 36 languages
    return f"hello {name}"
```

[Colour comes from the theme, not the highlighter.]{.small}
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
[Lists]{.eyebrow}
## Four kinds of list
:::

::: pane src {.card valign=middle}
```markdown
- bullet
  - nested

1. numbered
2. and the renderer counts

- [x] done
- [ ] not

Term
: What it means.

Another
: And its meaning.
```
:::

::: pane out {valign=middle}
- A point, and under it
  - a qualification

1. Write the deck
2. Build it

- [x] Task lists work
- [ ] and always have

Term list
: Its meaning, beside it.

Another
: Never under it.
:::

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+----------+------------+------------+
|          |            |            |
|          |            |            |
|  one     |  two       |  three     |
|          |            |            |
|          |            |            |
+----------+------------+------------+
```

::: pane head
[Lists]{.eyebrow}
## One term list, three shapes
:::

::: pane one
### Default
Apple
: A red fruit.

A longer term
: A meaning long enough to wrap onto a second line.
:::

::: pane two {.terms-aligned}
### `.terms-aligned`
Apple
: A red fruit.

A longer term
: A meaning long enough to wrap onto a second line.
:::

::: pane three {.terms-stacked}
### `.terms-stacked`
Apple
: A red fruit.

A longer term
: A meaning long enough to wrap onto a second line.
:::

<!-- note: The classes go on the pane, not the list: `::: pane two
{.terms-aligned}`. Aligned is for definitions meant to be read against each
other; stacked is for definitions long enough that the term reads as a heading
over them. -->

---

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  table           |  math           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Data]{.eyebrow}
## Tables and mathematics
:::

::: pane table {valign=middle}
| Command | Writes | When |
|---|---|---:|
| `serve` | a live preview | writing |
| `build` | one `.html` | presenting |
| `export pdf` | one `.pdf` | handing out |

[`---` is left, `---:` right, `:---:` centred.]{.small}
:::

::: pane math {valign=middle}
Inline maths such as $O(1)$ sits in running text.

$$
p_{95} = \mu + 1.645\,\sigma\sqrt{\frac{n}{n-1}}
$$

[LaTeX becomes MathML at build time — no JavaScript, and it reaches the PDF.]{.small}
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
[Aside]{.eyebrow}
## Emoji, footnotes and what you do not show
:::

::: pane src {.card valign=middle}
```markdown
:tada: or 🎉 — both work

Text with a note[^a]

[^a]: The note itself.

<!-- an ordinary comment -->
<!-- note: what you say here -->
```
:::

::: pane out {valign=middle}
Shipping :rocket: — the shortcode is for keyboards that make the character
hard to type[^why].

An HTML comment is invisible everywhere. A comment beginning `note:` is your
script: press `N`.

[^why]: Typing 🎉 directly always worked and still does.
:::

<!-- note: This is a speaker note. The audience never sees it; the presenter window does. -->

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
## Still Markdown

Every mark above renders on GitHub. The four Mirzam adds — `==`, `++`,
`:emoji:`, term lists — show as the punctuation you typed, never as nothing.

**Next:** `03-layout.md` puts two of these side by side.
:::

::: pane foot {align=right}
<span class="foot">examples/02-writing.md</span>
:::
