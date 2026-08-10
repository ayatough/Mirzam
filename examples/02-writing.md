---
title: Mirzam - Writing in plain Markdown
author: Mirzam
aspect: "16:9"
---

# Writing in plain Markdown

Not one line of this deck uses a Mirzam extension. No `pane`, no `:::`, no
attributes — only CommonMark, split into slides by `---`.

It also picks no theme, so this is the deck you get before you choose anything.

<!-- note: This deck is the control group. If something here renders badly, it is a bug in the default theme, not in a feature. -->

---

## Headings set the rhythm

### A third level, for the detail underneath

Paragraphs are just paragraphs. A blank line separates them; a single newline
does not, so you can wrap the source at whatever width your editor likes and
the sentence stays one sentence.

The `#` on the opening slide is the first level — the same one you would put
at the top of a document. On a slide it reads as a title, so it is usually the
only `#` in the deck.

Two levels are usually enough after that. Three is a sign the slide wants to
be two slides.

---

## Emphasis, and what it costs

**Bold** for the one phrase the audience should leave with. *Italic* for a
term being introduced. ~~Struck~~ for something you are explicitly retracting.

`Inline code` keeps identifiers out of the prose — `--split h2`, `PATH`,
`deck.md` — and it never wraps mid-token.

Every one of these is cheap to write and expensive to overuse. A slide with
four bold phrases has none.

---

## Links

A [link to the syntax reference](https://github.com/ayatough/Mirzam/blob/main/docs/syntax.md)
reads as text and stays clickable in the HTML deck.

In the PDF the address is printed beside the words, because a reader holding
a page cannot click it.

A bare URL such as https://ayatough.github.io/Mirzam/ becomes a link on its
own.

---

## Lists, and how deep to nest them

- A top-level point, one line long
- Another one
  - A qualification, indented two spaces
  - Another qualification
    - A third level, which is already too many
- Back to the top level

The nesting is real, so it survives the PDF and the outline. It is still worth
asking whether the third level is a list item or a sentence.

---

## Numbered lists

1. Write the deck as text
2. Build it into one HTML file
3. Present from that file

The numbers come from the renderer, not from what you typed, so an item
inserted in the middle renumbers the rest:

```markdown
1. First
1. Second
1. Third
```

---

## Quotes and rules

> A quotation gets an accent rule and quieter text, so it reads as somebody
> else's voice without needing quotation marks around it.

A horizontal rule needs three asterisks or three underscores:

***

Three hyphens would have ended the slide here instead — that is the one place
where a Mirzam deck reads Markdown differently from a document.

---

## Tables

| Command | What it writes | When |
|---|---|---|
| `serve` | nothing; a live preview | while writing |
| `build` | one self-contained `.html` | before presenting |
| `export pdf` | one `.pdf` | to hand out |

Alignment comes from the separator row: `---` is left, `---:` is right,
`:---:` is centred. Wide tables shrink rather than clip.

---

## Code blocks

```rust
fn main() {
    println!("A fenced block keeps its indentation and its blank lines.");
}
```

The language after the fence is recorded but not yet coloured — syntax
highlighting is not implemented, so a code block is monospaced and nothing
more.

Indented blocks work too, but a fence is clearer about where the code stops.

---

## Footnotes land on the slide that cites them

Dispersive readout is the standard measurement for a superconducting
qubit[^blais], and the fidelity ceiling is set by the amplifier chain[^jpa].

The marker is a link; the note appears at the foot of *this* slide, not
collected at the end of the deck, because a slide is where the audience is
looking.

[^blais]: Blais et al., *Circuit quantum electrodynamics*, Rev. Mod. Phys. 93, 025005 (2021).
[^jpa]: Aumentado, *Superconducting parametric amplifiers*, IEEE Microw. Mag. 21, 45 (2020).

---

## That was all of it

Headings, emphasis, links, lists, quotes, rules, tables, code, footnotes — the
Markdown you already write, on a slide, with no configuration.

What plain Markdown cannot do is put two things side by side. That is the next
deck: `03-layout.md` draws the layout as ASCII, and everything above keeps
working inside it.
