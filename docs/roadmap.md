# Roadmap

Where Mirzam is and what comes next. Design rationale lives in
[architecture.md](architecture.md); the older Japanese planning notes are under
[ja/roadmap.md](ja/roadmap.md).

## Status

The MVP is feature-complete and covered by regression tests in CI.

| Area | State |
|---|---|
| Markdown, frontmatter, slide splitting | Done |
| File splitting (`![[file.md]]`) | Done |
| ASCII pane layout, pane attributes | Done |
| Variables and arithmetic | Done |
| Math (LaTeX → MathML) | Done |
| Charts from inline CSV or `.csv` files | Done |
| Shapes and live-routed connectors | Done |
| Video and GIF | Done |
| `build`, `serve` (hot reload), `export pdf` | Done |
| Custom themes, speaker notes | Done |
| WebAssembly core, VS Code extension | Done |
| Quality gates and benchmark in CI | Done |
| Animation (`anim`) | Next |
| Presenter mode | Next |
| Plugins, PPTX export, mobile editing | Later |

### Measured performance

Release build, from the standing benchmark:

| Deck | Full build | Single-slide edit |
|---|---:|---:|
| 20 slides | 4.6 ms | 0.3 ms |
| 120 slides | 19.5 ms | 0.7 ms |
| 500 slides | 78.2 ms | 2.3 ms |
| 100 slides, 800 formulas | 26.4 ms | 0.9 ms |

Exactly one slide re-renders per edit. The residual growth is the linear cost of
re-reading and hashing the source, not rendering.

## Next: animation and presenting

**`anim` blocks.** The syntax is reserved and parsed today, so decks written now
keep working:

````markdown
```anim
[enter]   .title     : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[exit]    slide      : iris-out 500ms
```
````

Triggers (`enter`, `click N`, `exit`, `after #id`), targets (ids, classes, whole
slide, or text split into characters/words/lines), a standard effect set, and
programmable easing including springs. Compiled to a timeline and driven by the Web
Animations API, with slide transitions specified the same way.

**Presenter mode.** A second window with speaker notes, next slide, a timer, a
pointer, and step-through control for click-triggered animations.

**Language server.** Completion for pane names and anchor ids, diagnostics for
references that point at nothing, hover for chart data — surfaced in the VS Code
extension.

## Later

**Plugins.** Two extension points: WebAssembly passes that transform the document
before rendering, and JavaScript modules that register runtime effects. Themes stay
plain CSS plus a manifest, since that already works.

**Export beyond HTML and PDF.** PowerPoint via OOXML, with elements that have no
native equivalent rasterized rather than dropped; Google Slides through the same
path. Direct PDF generation without Chromium is a separate, larger question that
depends on adopting a text layout engine.

**Editing anywhere.** The WASM core already runs in a browser; a progressive web
app on top of it is what makes phone editing real. An Obsidian plugin reuses the
same core.

**Richer data.** Column aggregation in tables, more chart types, and `mermaid` /
`d2` diagram blocks as plugins rather than built-ins.

## Open questions

- **Nested layouts.** Whether a pane can contain its own pane grid, or whether the
  shape layer covers those cases well enough.
- **Per-edit cost.** Making incremental builds genuinely O(1) requires keeping the
  parsed deck in memory across rebuilds. Worth it only if very large decks matter.
- **1.0.** Reserved for when the markup is stable enough that today's decks keep
  rendering. Animation and presenter mode are the last features expected to force
  syntax changes.
