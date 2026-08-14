---
name: mirzam
description: Write, check and build Mirzam slide decks - Markdown whose layout is an ASCII box drawing, rendered to one self-contained HTML file or a PDF. Use when asked to make or edit slides, a deck or a presentation in a repository, or when a Markdown file carries Mirzam markup (a pane drawing, ::: pane, or a shape, chart, connect, annotate, anim or effects block).
---

# Mirzam decks

Mirzam renders a CommonMark file into a slide deck. The layout is a box drawing
in the source, so a deck is reviewable in a diff and you can see the grid you
are writing.

**The syntax card is [`references/llms.md`](references/llms.md), beside this
file.** Read it before writing any markup: it is the whole language on one page,
and the binary that installed it wrote it, so the card and the renderer agree.

## First, find the CLI

```bash
mirzam --version
```

If that prints a version, run the loop below.

If the command is missing, install it — no Rust toolchain is needed:

```bash
curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh
```

That puts `mirzam` in `~/.local/bin`; add it to `PATH` for this session with
`export PATH="$HOME/.local/bin:$PATH"`. In a repository holding Mirzam's own
source, `cargo run --bin mirzam --` works instead.

If neither is possible — no network, or a sandbox with no shell — then you
cannot check or build anything. Write the deck from the syntax card anyway,
hand the `.md` to the person, and point them at the browser editor,
<https://ayatough.github.io/Mirzam/try/>, which renders and downloads the deck
with nothing installed and works on a phone.

## The loop

1. **Read `references/llms.md`**, starting with its "Traps" section. Most of
   what goes wrong in a Mirzam deck fails *silently* — markup that reaches the
   slide as literal text, an arrow that draws nothing — and those traps are the
   list of ways.
2. **Write or edit the deck.** One `.md` file unless it grows past a dozen
   slides, then split it with `![[section.md]]`.
3. **Check it:**
   ```bash
   mirzam check deck.md --format json
   ```
4. **Fix what it names.** Every diagnostic carries a `kind`, a `severity`, and
   the `file` and `line` it came from — through transclusion, so it names the
   file the slide was actually written in.
5. **Repeat 3 and 4** until `"ok": true` and the remaining warnings are ones you
   have read and judged harmless.
6. **Build:**
   ```bash
   mirzam build deck.md -o out          # one self-contained HTML file
   mirzam export pdf deck.md -o deck.pdf
   ```

**Do not skip step 3.** A heading clipped by its band, a `shape` block written
inside a pane so it renders as source code, a connector pointing at an id that
was renamed: none of these show up in a diff, and all of them are what the
checker is for. It needs a Chromium — set `MIRZAM_CHROMIUM` if one is not on
`PATH`.

## Reading the check output

```json
{ "schema": "mirzam-check", "version": 1, "mirzam": "0.5.0",
  "ok": false, "slides": 9, "diagnostics": [ … ], "notes": [ … ] }
```

| Kind | Means |
|---|---|
| `layout.*` | The deck was rendered and measured, and something does not fit or did not draw. `severity: error`; these decide the exit code |
| `build.*` | The build's own warnings: markup that parsed but will not do what it looks like. `severity: warning` |
| `build.skill` | *This* card and the binary are different versions — see below |

An unknown `kind` may appear at any time; fall back to `severity` and `message`
rather than dropping the record. The full schema is in the repository's
`docs/agents.md`.

## When `check` reports `build.skill`

The card you are reading is stamped with the version that wrote it, and the
binary compares stamps.

- **The card is older than the binary** — run `mirzam skill install` to rewrite
  it, then re-read it. You are allowed to do this yourself; it is the repair the
  diagnostic is asking for.
- **The binary is older than the card** — say so to the person and let them
  upgrade it. Do not downgrade the card.

## House rules for a deck that reads well

- **One idea per slide, one thought per pane.** A pane clips what does not fit,
  and `check` is the only thing that will tell you.
- **Draw the grid first.** Row heights come from how many lines you drew,
  column widths from the character widths — so the drawing is the design.
- **Prefer the deck's own theme** over a custom stylesheet. `.metric`, `.card`
  and `.eyebrow` come with the renderer, so a slide copied out of a sample deck
  keeps its shape without `examples/themes/mirzam.css` behind it.
- **Put the spoken sentence in a speaker note** (`<!-- note: … -->`) rather than
  crowding the slide with it.
