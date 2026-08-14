---
name: mirzam-writing
description: Write Mirzam slide decks as Markdown where the mirzam CLI cannot be run - a chat, the desktop app, a phone. Use when asked for slides, a deck or a presentation in Mirzam format, or to edit Markdown carrying Mirzam markup (a pane drawing, ::: pane, or a shape, chart, connect, annotate or anim block). Produces a .md file the person renders in the browser editor or with the CLI.
---

# Writing Mirzam markdown, without the CLI

Mirzam renders a CommonMark file into a slide deck: the layout is a box drawing
in the source, and the output is one self-contained HTML file.

**You cannot run the `mirzam` CLI here.** This environment has no filesystem to
install a binary into and no browser to render with, so the half of the normal
workflow that *checks* a deck — `mirzam check`, which measures the rendered
slides and reports anything clipped — is not available to you. Write carefully
instead, and hand the rendering to the person.

## What to do

1. **Read [`references/llms.md`](references/llms.md)**, beside this file. It is
   the whole markup on one page: every fence, every frontmatter field, every
   attribute, and the traps that fail silently. Start with the "Traps" section.
2. **Write the deck as one `.md` file.** Keep it to one file: transclusion
   (`![[section.md]]`), `data:` pointing at a `.csv`, and `css:` pointing at a
   stylesheet all need a filesystem, and the browser editor has none either.
   Inline the CSV inside the `chart` block and use a built-in `theme:`.
3. **Be conservative about fit.** Nothing here measures a slide, so leave room:
   one idea per pane, short lines, no ten-paragraph pane under a one-line band.
   A pane clips silently what does not fit in it.
4. **Give the person the finished `.md`** and tell them how to see it:

   - **In a browser, nothing installed:** <https://ayatough.github.io/Mirzam/try/>
     — paste the Markdown, watch the deck, download the finished `.html`. It
     runs the real renderer compiled to WebAssembly, uploads nothing, and works
     on a phone.
   - **With the CLI**, if they have a terminal:
     ```bash
     mirzam check deck.md      # clipped panes, connectors to nowhere, and the rest
     mirzam build deck.md -o out
     ```
     Installing it: `curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh`.
     The CLI is the only way to check layout, export a PDF, or split a deck
     across files.

Say which of the two you are recommending and why: if the deck is one file with
no local images, the browser editor is the whole toolchain.
