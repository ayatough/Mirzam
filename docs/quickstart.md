# Quick start

Four ways in, depending on what you have. Pick the first row that describes you.

| You have | Use | Gets you |
|---|---|---|
| A browser | **[the browser editor](https://ayatough.github.io/Mirzam/try/)** | A finished `.html` deck, no install, works on a phone |
| A terminal | **the `mirzam` CLI** | Everything: live preview, PDF export, includes, local images |
| VS Code | **the preview extension** | The deck beside the Markdown, re-rendering as you type |
| Obsidian | **your vault** | Write there, build with the CLI or the browser editor |

---

## 1. In a browser — nothing to install

**→ [ayatough.github.io/Mirzam/try](https://ayatough.github.io/Mirzam/try/)**

The same Rust core that powers the CLI, compiled to WebAssembly. Type Markdown
on the left, watch the deck on the right, press **Download deck** and you have
one self-contained `.html` file — the identical output `mirzam build` writes.
Open it, present from it, email it, put it on a share.

- **Nothing is uploaded.** The renderer runs in your browser; your draft lives in
  that browser's local storage and nowhere else.
- **Pictures work.** Attach, drop or paste an image; it is inserted as
  `![](shot.png)` and its bytes end up inside the downloaded file. Screenshots
  from a paper are exactly the case this is for.
- **On a phone** the editor and the preview take turns, with a tab each.
- **Start from nothing.** **New** empties the editor for a deck of your own;
  **Sample** puts the example back. Your draft is only in that browser, so both
  ask before replacing it — **Save .md** first if you want to keep a copy.

What the browser build cannot do, because it has no filesystem:

| | Instead |
|---|---|
| `![[section.md]]` across files | Keep the deck in one file, or use the CLI |
| `data: chart.csv` from disk | Paste the CSV into the `chart` block's `data:` |
| `css: theme.css` from disk | Use a built-in `theme:`, or the CLI |
| PDF export | Build the deck with the CLI and run `mirzam export pdf` |

## 2. On the command line — the whole thing

**No Rust required.** Every release ships a binary for macOS, Linux and
Windows:

```bash
curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh
```

That downloads the right archive for your machine, checks it against the
published checksum, and puts `mirzam` in `~/.local/bin` (set `MIRZAM_BIN_DIR`
to change that). On Windows, take the `.zip` from the
[releases page](https://github.com/ayatough/Mirzam/releases) and put the
`mirzam.exe` somewhere on your `PATH`.

To build it yourself instead, you need Rust 1.91 or newer from
[rustup.rs](https://rustup.rs):

```bash
git clone https://github.com/ayatough/Mirzam
cd Mirzam
cargo install --path crates/mirzam-cli --bin mirzam   # into ~/.cargo/bin
```

(Or `cargo build --release` and run `./target/release/mirzam`, which is not on
your `PATH`.)

```bash
mirzam new deck.md                   # a deck to start from
mirzam serve deck.md                 # live preview at localhost:4321
mirzam build deck.md -o out          # one self-contained HTML file
mirzam export pdf deck.md -o deck.pdf
mirzam build notes.md --split h2     # any document becomes a deck, unedited
```

`new` writes a starter deck — frontmatter, a title slide and a slide break, the
shape shown under [Your first deck](#your-first-deck) — and never overwrites a
file that is already there. `mirzam new deck.md --empty`
writes a blank file instead, for starting from nothing rather than from a
template; `serve` is happy to watch it while you type the first slide.

`serve` re-renders only the slides you touched, so a large deck stays instant
while you write.

## 3. In VS Code

```bash
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-*.vsix
```

Open a `.md` file and press `Ctrl+K V` (`Cmd+K V` on macOS). Editing re-renders
only the slide you touched, and moving the cursor scrolls the preview to match.

The extension bundles the WebAssembly core, so it does not shell out to the CLI
— but the CLI is still what exports PDFs.

## 4. In Obsidian

Mirzam has no Obsidian plugin. It does not need one to *write* in: every
extension degrades to something harmless in a plain Markdown editor, and the
transclusion syntax is Obsidian's own.

| In Obsidian you see | Because |
|---|---|
| `![[sections/method.md]]` embedded inline | Mirzam uses Obsidian's own syntax |
| `pane`, `chart`, `shape` as code blocks | They are fenced code blocks |
| `::: pane main` as a line of text | It is a line of text |
| Nothing where the speaker notes are | They are HTML comments |

So: keep the deck in your vault, write it there, and build it with the CLI
pointed at the vault path — or paste it into the browser editor when you are
away from your machine.

## 5. On a phone

- **Writing:** the browser editor, above. It is the whole toolchain. **New**
  starts an empty deck, which is how you begin one on a phone.
- **Reviewing:** open a built `.html` from your files or a share. Swipe to turn
  the page, swipe up for notes, two-finger tap for the shortcut sheet.
- **Presenting from it:** the deck is one file, so AirDrop or a cloud folder is
  the entire deployment step.

---

## Your first deck

Six lines is a deck:

```markdown
---
title: Weekly
---

# What changed this week {.title-slide}

---

## Latency

p95 dropped in **every region** after the cache rollout.
```

Then give it a layout, which is the part Mirzam exists for:

````markdown
```pane
+------------------+-----------------+
|  head                              |
+------------------+-----------------+
|  main            |  chart          |
+------------------+-----------------+
```

::: pane head
## Latency after the cache rollout
:::

::: pane main
p95 dropped in every region, with the largest win in `ap-ne`.
:::

::: pane chart
```chart
type: bar
data: |
  region, before, after
  us-east, 210, 120
  ap-ne, 380, 180
```
:::
````

The box drawing *is* the layout: column widths come from the character widths
you drew, row heights from the number of lines.

## Where to go next

- **[Syntax reference](syntax.md)** — every block and inline form, and what a
  plain Markdown parser shows instead
- **[Layout guide](layout.md)** — sizing panes, what to do when content does not
  fit, keeping arrows out of the text
- **[The examples](../examples/)** — six numbered decks that teach the markup in
  order, starting at [`01-start.md`](../examples/01-start.md), plus
  [`seminar.md`](../examples/seminar.md), a research talk with math, a quoted
  figure and citations
- **[All of them running](https://ayatough.github.io/Mirzam/)**

Press `/` in any deck to see what the viewer responds to.
