# Quick start

Five ways in, depending on what you have. Pick the first row that describes you.

| You have | Use | Gets you |
|---|---|---|
| A browser | **[the browser editor](https://ayatough.github.io/Mirzam/try/)** | A finished `.html` deck, no install, works on a phone |
| A terminal | **the `mirzam` CLI** | Everything: live preview, PDF export, includes, local images |
| VS Code | **the preview extension** | The deck beside the Markdown, re-rendering as you type |
| Helix, Neovim, anything | **`mirzam lsp`** | Diagnostics as you type, and an outline of the slides |
| Obsidian | **your vault** | Write there, build with the CLI or the browser editor |
| A coding agent | **`mirzam skill install`** | Claude Code writes the deck and checks its own layout |

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
| `theme: house.css` from disk | Name a built-in in `theme:`, or use the CLI |
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
mirzam export pdf notes.md --split h2 --theme mirzam -o notes.pdf
mirzam check deck.md                 # clipped panes, unresolved connectors, and the rest
```

`export pdf` takes the same `--split`, `--theme`, `--fit` and `--mode`
as `build`, so a deck assembled with one of those flags exports to PDF with
the same slides in one command — there is no need to `build` first. It always
reads the Markdown source, never a built `out/index.html`: pass it the `.md`
file.

`check` builds the deck and renders it with headless Chromium to catch what a
build's own warnings cannot: content clipped by its pane, an unresolved
connector, an animation left mid-entrance, the layout debug overlay baked in —
a slide that renders, just wrong. It takes the same deck-shaping flags `build`
does, so a `--split` deck is checked as it would actually publish, and exits
non-zero when it finds something, for a CI gate that doesn't need `cargo` or a
browser-automation library installed.

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

## 4. In any editor — diagnostics as you type

`mirzam lsp` is a language server: an editor starts it, sends it the buffer as
you type, and gets back the problems the build would have reported — an unknown
theme, a pane that is not in the grid, a `<!-- layout: -->` naming no master, a
citation key nothing defines — each underlined where the mistake is, plus an
outline of the deck's slides. It reads; it never writes to your files, and it
never opens a browser, so the layout checks (content clipped by its pane, panes
overlapping) are still `mirzam check`.

**See it work without configuring anything.** A language server prints nothing
on its own — started by hand it just sits there — so there is a probe that runs
one whole session and shows you the answer:

```bash
node scripts/lsp-probe.mjs --outline deck.md
```

```
deck.md
  deck.md:3:8    build.theme   unknown theme `nosuchtheme`; using `mirzam`. …
  deck.md:19:10  build.layout  slide 1: pane `figure` is not in the layout
  outline:
    7: Opening
    25: Second slide
```

If that prints your deck's problems, the server works and anything left is
editor configuration.

**Helix** — in `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "markdown"
language-servers = ["mirzam"]

[language-server.mirzam]
command = "mirzam"
args = ["lsp"]
```

**Neovim** (0.11 or newer), in your config:

```lua
vim.lsp.config.mirzam = { cmd = { "mirzam", "lsp" }, filetypes = { "markdown" } }
vim.lsp.enable("mirzam")
```

**Zed** — it takes a language server through an extension, so for now use the
probe or one of the editors above.

**VS Code** — the preview extension does not start it yet; that is the next
piece of this work. Until then the probe and `mirzam check` are the way.

Two things to know about what it reports. Diagnostics are **warnings, never
errors**: a deck with a problem still renders, and that is deliberate
everywhere in this tool. And the underline is placed by looking for the word
the message quotes, so it is exact when the message names something (`` `fig`
``, `` `nord2` ``) and falls back to the slide's first line when it does not.

## 5. In Obsidian

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

## 6. On a phone

- **Writing:** the browser editor, above. It is the whole toolchain. **New**
  starts an empty deck, which is how you begin one on a phone.
- **Reviewing:** open a built `.html` from your files or a share. Swipe to turn
  the page, swipe up for notes, two-finger tap for the shortcut sheet.
- **Presenting from it:** the deck is one file, so AirDrop or a cloud folder is
  the entire deployment step.

## 7. With a coding agent

If Claude Code writes your decks, give it the markup and the checker in one
command, from the repository the decks live in:

```bash
mirzam skill install
```

That writes `.claude/skills/mirzam/` — the loop (write the deck, run
`mirzam check --format json`, fix what it names) and the whole syntax card,
both embedded in the binary, so what the model reads matches the binary it is
calling. `--user` installs it into `~/.claude/skills/` for every directory
instead. Where no binary can run — claude.ai, the phone app —
`mirzam skill install --zip` writes the archive those upload. Details, and the
JSON the checker emits: [agents.md](agents.md).

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
- **[Troubleshooting](troubleshooting.md)** — a slide that doesn't fit, markup
  showing as literal text, the current PDF steps, and every build warning
- **[The examples](../examples/)** — six numbered decks that teach the markup in
  order, starting at [`01-start.md`](../examples/01-start.md), plus
  [`research.md`](../examples/research.md), a report with math, a chart and a
  cited bibliography, and [`seminar.md`](../examples/seminar.md), the same shape
  in Japanese with a quoted figure
- **[All of them running](https://ayatough.github.io/Mirzam/)**

Press `/` in any deck to see what the viewer responds to.
