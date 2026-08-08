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
| Background images (`bg`, `dim`, `blur`, `scrim`) | Done |
| `build`, `serve` (hot reload), `export pdf` | Done |
| Custom themes, speaker notes | Done |
| WebAssembly core, VS Code extension | Done |
| Quality gates and benchmark in CI | Done |
| Layout debug overlay (`L`, `--debug-layout`) | Done |
| `anim` DSL compiled to a timeline (`mirzam-anim`) | Done |
| Animation runtime, slide transitions | Next |
| Named themes (Nord, Solarized, VS Code) and dark mode | Done |
| Annotations on images and charts, adjusted in the preview | Next |
| Typst-flavoured math syntax | Next |
| Presentation effects | Next |
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

The next batch is broken into parallel streams, with the interfaces between them
fixed in advance, in [workstreams.md](workstreams.md). What follows is the
rationale behind those streams.

**`anim` blocks.** `mirzam-anim` compiles the DSL to the timeline JSON described
in [workstreams.md](workstreams.md#c1-animation-timeline):

````markdown
```anim
[enter]   .title     : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[exit]    slide      : iris-out 500ms
```
````

Triggers (`enter`, `click N`, `exit`, `after #id`), targets (ids, classes, whole
slide, or text split into characters/words/lines), a standard effect set, and
programmable easing including springs, resolved to a sampled curve at build
time. See [syntax.md](syntax.md#animations) for the full syntax.

What is not built yet is driving the timeline: the Web Animations API playback,
click-through stepping, and slide transitions specified the same way. A deck
that declares `anim` blocks today compiles them into the page but nothing
plays them back.

**Presenter mode.** A second window with speaker notes, next slide, a timer, a
pointer, and step-through control for click-triggered animations.

**Language server.** Completion for pane names and anchor ids, diagnostics for
references that point at nothing, hover for chart data — surfaced in the VS Code
extension.

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
of the text.

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
