# Mirzam

**Presentation decks that live in your repository.** Write plain Markdown, draw the
layout as ASCII, and get a deck with real charts, diagrams, video and math — as a
single HTML file or a PDF.

> Mirzam (β Canis Majoris) — "the announcer", the star that rises before Sirius.

[Syntax](docs/syntax.md) · [Layout guide](docs/layout.md) · [Architecture](docs/architecture.md) · [Roadmap](docs/roadmap.md) · [Contributing](docs/development.md) · [Agents](AGENTS.md) · [日本語](docs/ja/README.md)

---

## Why

Slide tools make you choose. WYSIWYG editors give you control but produce opaque
binaries that no one can review. Markdown slide tools are diffable but leave you
with one column and a title. Mirzam is an attempt at both: the source is ordinary
Markdown that renders fine on GitHub, and the output is a deck you would actually
present.

````markdown
```pane
+------------------+-----------------+
|  head                              |
+------------------+-----------------+
|                  |                 |
|  main            |  chart          |
|                  |                 |
+------------------+-----------------+
```

::: pane head
## Latency after the cache rollout
:::

::: pane main
p95 dropped in [every region]{#win .u}, with the largest win in `ap-ne`.
:::

::: pane chart
```chart
type: bar
id: latency
data: |
  region, before, after
  us-east, 210, 120
  ap-ne, 380, 180
```
:::

```connect
#win -> #latency-1-2 : color=@accent2
```
````

The layout is the box drawing. The chart comes from the data next to it. The arrow
points from a phrase in the sentence to an individual bar, and re-routes itself
whenever the layout changes.

## Features

| | |
|---|---|
| **Layout you can see** | Draw panes as ASCII; column widths and row heights follow what you drew |
| **Charts from data** | `chart` blocks read inline CSV or a `.csv` file and render SVG at build time |
| **Diagrams that stay linked** | `shape` blocks draw boxes; `connect` blocks point from text to any element, resolved live |
| **Math** | LaTeX converted to MathML at build time — no client-side JavaScript |
| **Video and GIF** | Embedded in the HTML, replaced by a poster frame in PDF |
| **Backgrounds that stay readable** | A photo behind a pane, with `dim`, `blur` and gradient `scrim` so the text still wins |
| **Files that scale** | Split a deck across files with `![[section.md]]`; only edited slides re-render |
| **Runs anywhere** | One Rust core, compiled to a native CLI and to WebAssembly for editors and browsers |
| **Still Markdown** | Every extension degrades to harmless code blocks in a plain CommonMark parser — enforced by a test |

## Install

Requires a Rust toolchain (1.75+).

```bash
git clone https://github.com/ayatough/Mirzam
cd Mirzam
cargo build --release          # target/release/mirzam
```

## Use

```bash
mirzam build deck.md -o out          # single self-contained HTML
mirzam build README.md --split h2    # any document becomes a deck, unedited
mirzam serve deck.md                 # live preview at localhost:4321
mirzam export pdf deck.md -o deck.pdf
```

In the viewer: `←` `→` to navigate, `N` for speaker notes, `F` for fullscreen.

### In your editor

```bash
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-0.0.1.vsix
```

Open a `.md` file and press `Ctrl+K V` (`Cmd+K V` on macOS). Editing re-renders only
the slide you touched, and moving the cursor scrolls the preview to match.

### In a browser

```bash
./scripts/serve-wasm-demo.sh    # http://localhost:8080
```

## Examples

| Deck | What it shows |
|---|---|
| [`examples/pitch.md`](examples/pitch.md) | A sales pitch: metric tiles, charts from CSV, a custom dark theme |
| [`examples/showcase.md`](examples/showcase.md) | Every component, side by side with its source |
| [`examples/cookbook.md`](examples/cookbook.md) | Layout rules, one per slide — the companion to [docs/layout.md](docs/layout.md) |
| [`examples/seminar.md`](examples/seminar.md) | A research talk in Japanese: math, tables, CJK typography |
| [`examples/media.md`](examples/media.md) | Video and GIF embedding |

```bash
cargo run --bin mirzam -- build examples/pitch.md -o out && open out/index.html
```

All of them are published as live decks alongside the docs; see
`./scripts/build-site.sh` to build the site locally.

## Status

The MVP is feature-complete and covered by regression tests in CI. It is `0.x`:
the markup will keep changing, so pin a commit if you depend on it.

- **Working:** build, live-reload server, PDF export, ASCII pane layout, file
  splitting, variables and arithmetic, math, charts, shapes, live connectors,
  video, background images, custom themes, speaker notes, VS Code extension,
  WebAssembly core
- **Next:** animation (`anim` blocks), presenter mode, PowerPoint export
- **Performance:** a 500-slide deck builds in 78 ms; a single-slide edit
  re-renders in 2.3 ms

See the [roadmap](docs/roadmap.md) for the full plan and [development
guide](docs/development.md) to work on it.

## License

MIT. The bundled STIX Two Math font is licensed under the SIL Open Font License.
