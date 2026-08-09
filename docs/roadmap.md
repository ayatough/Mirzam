# Roadmap

Where Mirzam is and what comes next. Design rationale lives in
[architecture.md](architecture.md); the older Japanese planning notes are under
[ja/roadmap.md](ja/roadmap.md).

## Status

`v0.1.0` is the first tagged release: everything below marked Done is in it,
built by CI and published as a prebuilt binary. It is still `0.x` — the markup
will keep changing.

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
| Background images (`bg`, `dim`, `blur`, `scrim`) | Done |
| `build`, `serve` (hot reload), `export pdf` | Done |
| Custom themes, speaker notes | Done |
| WebAssembly core, VS Code extension | Done |
| Quality gates and benchmark in CI | Done |
| Layout debug overlay (`L`, `--debug-layout`) | Done |
| `anim` DSL compiled to a timeline (`mirzam-anim`) | Done |
| Animation runtime, click-through steps, slide transitions | Done |
| Named themes (Nord, Solarized, VS Code) and dark mode | Done |
| Annotations on images and charts | Done |
| Marking a phrase and the thing it refers to together | Done |
| Presentation effects | Done |
| Presenter window, touch and gesture controls | Done |
| Per-pane continuation (`<!-- next -->`) | Done |
| Contents page generated from headings | Done |
| Browser editor (WebAssembly), prebuilt binaries | Done |
| A theme per slide | Next |
| Carrying an element from one slide to the next | Next |
| Demo recording and a generated themes gallery | Next |
| Dragging an annotation back into the Markdown | Next |
| Typst-flavoured math syntax | Later |
| Plugins, PPTX export | Later |

Each of those has a brief — what it is for, what is not free about it, and where
it stops — in [workstreams.md](workstreams.md). "Next" means the reasoning is
written down, not that a date exists.

The one worth naming here is **carrying an element from one slide to the next**:
a slide presents three components and the next three take one each, and the
component *moves* into its own slide rather than the deck turning the page under
it. It is the one animation a deck tool gets asked for that Mirzam has no answer
to today.

### Measured performance

Release build, from the standing benchmark:

| Deck | Full build | Single-slide edit |
|---|---:|---:|
| 20 slides | 4.5 ms | 0.4 ms |
| 120 slides | 18.8 ms | 0.9 ms |
| 500 slides | 75.6 ms | 3.2 ms |
| 100 slides, 800 formulas | 24.5 ms | 1.3 ms |

Exactly one slide re-renders per edit. The residual growth is the linear cost of
re-reading and hashing the source, not rendering.

Measured again at `v0.1.0`, after the deck gained per-pane continuation, a
contents page and annotations. Edit latency at 500 slides went from 2.3 ms to
3.2 ms: a build now expands `<!-- next -->`, resolves the contents page against
the finished deck, and only then hashes — so the whole-document pass got a
little longer while the per-slide render did not. The shape is what matters and
it is unchanged: 25× the slides costs about 8× the edit.

## Still open

Two things below `1.0` that are not features so much as unfinished thinking.

**Connector routing.** Today a connector is a single curve between two points: it
leaves and arrives along the edge normals, but it does not know what is in the
way. It will still cross a paragraph when the anchor sits in the middle of one,
and leaving vertically from an underline reads as unnatural when the target is
almost level with the text.

Doing better means routing with obstacles: treat panes, text blocks, shapes and
chart marks as rectangles to avoid, then find a short path with few bends that
does not cross them — the problem diagram editors solve with an orthogonal or
spline router over a visibility graph. Worth doing properly, not incrementally.

Two constraints shape where it lives:

- It needs the *rendered* geometry, which only exists in the browser after
  layout. It cannot be precomputed at build time.
- An exported deck should stay a small self-contained file, so pulling the WASM
  core into every deck to route arrows is the wrong trade — the core is
  megabytes, the router is kilobytes.

So the router should be a **standalone dependency-free JavaScript module** under
`web/router/`, unit-tested in isolation and inlined into a deck only when it has
connectors — not a Rust crate. If the algorithm grows past what is comfortable to
test in JS, the fallback is a routing-only WASM module, kept separate from the
core so decks without connectors pay nothing.

Until then, the layout guide documents how to place anchors so arrows stay clear
of the text. It is also less pressing than it was: pairing a phrase with the
thing it refers to, in one colour on one click, turned out to be the better
answer for text-to-figure — and it crosses nothing, so it needs no route.

**Language server.** Completion for pane names and anchor ids, diagnostics for
references that point at nothing, hover for chart data — surfaced in the VS Code
extension, which today previews but does not understand.

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
