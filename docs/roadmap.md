# Roadmap

Where Mirzam is and what comes next. Design rationale lives in
[architecture.md](architecture.md); the older Japanese planning notes are under
[ja/roadmap.md](ja/roadmap.md).

## Status

`v0.8.0` is the current release: everything below marked Done is in it, built
by CI and published as a prebuilt binary. **Done · unreleased** means it is on
`main` and waiting for the next tag — see the Unreleased section of
[CHANGELOG.md](../CHANGELOG.md). It is still `0.x` — the markup will keep
changing.

| Area | State |
|---|---|
| Markdown, frontmatter, slide splitting | Done |
| File splitting (`![[file.md]]`) | Done |
| ASCII pane layout, pane attributes | Done |
| Variables and arithmetic | Done |
| Math (LaTeX → MathML) | Done |
| Typst-flavoured math syntax (`math: typst`) | Done |
| Charts from inline CSV or `.csv` files | Done |
| Shapes and live-routed connectors | Done |
| Video and GIF, hosted video, click-to-play | Done |
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
| References from a BibTeX file, cited and listed | Done |
| Browser editor (WebAssembly), prebuilt binaries | Done |
| Slide masters (`masters:`, `<!-- layout: -->`) | Done |
| Footers and slide numbers | Done |
| A theme per slide, and per pane | Done |
| A theme sets type, not only colour; `theme:` takes a path | Done |
| Carrying an element from one slide to the next | Next |
| Demo recording (the edit loop, typed live) and a generated themes gallery | Done |
| Dragging an annotation back into the Markdown | Next |
| Syntax highlighting in code blocks (36 languages, theme-token colours) | Done |
| An authoring contract for agents (`check --format json`, `llms.txt`) | Done |
| Mermaid rendered to SVG at build time, in the deck's own colours | Done |
| Autoplay (`autoplay: 8s loop`, `?autoplay=`) — kiosk loops, screensavers | Done |
| Overview grid (`O`), go-to-slide by number, bare chrome (`H`) | Done |
| A working page on a slide (`![alt](page.html)`), sandboxed and inlined | Done · unreleased |
| Figure captions and credits (`caption=`, `credit=`), a credit that cites | Done · unreleased |
| Plugins, PPTX export | Later |

Each of those has a brief — what it is for, what is not free about it, and where
it stops — in [workstreams.md](workstreams.md). "Next" means the reasoning is
written down, not that a date exists.

Four of those rows come out of the
[August 2026 market survey](reports/2026-08-market-survey.md) — syntax
highlighting and the agent contract (its two P0s), then Mermaid and the theme
gallery (two of its four P1s). **PPTX export is the one item on that list still
unbuilt**, which is why it heads the Later section; the survey also carries the
priorities behind the rest of it and the reasoning for what is deliberately not
on this page.

One left the table rather than moving up it: **structural math editing** — the
Math panel in the browser editor — was built and then withdrawn before it was
ever released, because typing Typst source turned out to be faster than tapping
a formula into shape on the phone it was built for. The effort went into the
math grammar instead, which is why `v0.4.0` says more about what `$...$` can
hold than about how it is edited.

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

**Export beyond HTML and PDF.** PowerPoint via OOXML, staged: slides as images
plus real speaker notes first, which is where Marp, Slidev and Touying all
stop — then native text boxes, which none of them ships and the market survey
found to be the loudest unmet ask across every Markdown slide tool. Elements
with no OOXML equivalent are rasterized rather than dropped; Google Slides
comes through the same path. Direct PDF generation without Chromium is a
separate, larger question that depends on adopting a text layout engine.

**Editing anywhere.** The WASM core already runs in a browser; a progressive web
app on top of it is what makes phone editing real. An Obsidian plugin reuses the
same core.

**The unattended screen, beyond autoplay.** `autoplay: 8s loop` landed as
pacing: one deck-wide interval driving the same advance the arrow key does,
which already covers the exhibition loop and the captioned photo slideshow.
What it deliberately does not do yet, in rough order of pull:

- **A per-slide dwell** — `<!-- autoplay: 20s -->` on the slide that needs
  reading time — the natural next syntax, since slide-level overrides of
  deck-wide dials are already the house pattern (`<!-- theme: -->`,
  `<!-- layout: -->`, `<!-- chrome: -->`).
- **Waiting for media**: a slide holding a video should turn when the video
  ends, not cut it off mid-sentence. The viewer knows the video is there; it
  just does not listen for `ended` yet.
- **Pan and zoom on background images** (the Ken Burns effect), which is what
  separates "slides on a loop" from a screensaver made of photographs. It
  belongs to the `bg` attribute grammar, not to `anim`: the drift is a
  property of the image, repeated every time the slide comes round.
- **Video export.** A deck driving itself is a deck a recording harness can
  film with no script to write — `scripts/record-demo.mjs` already drives a
  live tab for the README demo — so `mirzam export video deck.md` producing a
  `.webm` is mostly plumbing that exists. That is the piece that turns the
  telop-over-image pattern into simple video authoring: write Markdown, get a
  clip.
- **A progress hint** for the kiosk visitor: some quiet mark that the screen
  is a loop and where it is in it, in the chrome that `?controls=none`
  removes — so a display can choose between perfectly bare and legible.

**Richer data.** Column aggregation in tables, more chart types, and data-driven
slides — a run of slides generated from the rows of a CSV, the one structural
advantage Typst's scripting has over every Markdown tool.

**D2, through Mermaid's door.** Mermaid landed in `v0.7.0` exactly in the
shape planned here: the CLI shells out to a local `mmdc` at build time,
inlines the SVG with its palette rewritten to theme tokens, and a machine
without a renderer gets a code block *and* a `build.mermaid` warning — while
GitHub draws the same fence as a diagram natively. D2 arrives through the
same `DiagramRenderer` trait once wanted; nothing else has to move.

What has not happened is a sample deck that *shows* one: no `mermaid` fence
appears in `examples/`, so the component gallery and the published site have
nothing to point at, and the syntax reference is the only place a reader sees
the feature at all. The reason is the renderer — no workflow installs `mmdc`,
so a fence in a gallery deck would publish as a code block with a
`build.mermaid` warning beside it. Showing it means installing mermaid-cli in
the site and CI builds first; the gap is in the gallery, not in the feature.

**More themes.** The contract for writing one landed: `theme:` takes a `.css`
path as readily as a built-in name, a theme of your own registers under its
filename stem and can be worn by a single pane, and `check` holds it to the
standard the built-ins are held to — so a custom theme is a supported artefact
rather than a stylesheet that happens to override the right tokens.
`examples/themes/blueprint.css` is the sample, and the gallery at `/themes/`
shows every built-in and that sample side by side, in both modes, regenerated
from the stylesheets on every site build. Slide masters — the layout half of the
same question — landed in `v0.4.0`: `masters:` names the shapes and a slide
picks one with `<!-- layout: -->`.

**A scope between the deck and the pane, for the rest of the dials.** A slide
can now say which theme, which master and whether it carries the deck's chrome,
each through an HTML comment. What still has no slide-level home is the
presentation dials — `--mz-terms-*`, `--mz-bullet`, `--mz-number` — which can be
set on a theme or a deck and overridden on a single pane, with nothing in
between, so a run of slides that wants one treatment repeats the attribute on
every pane in it. What is missing is a general way to attach attributes to a
*slide*, and above that to a section, since `## ` headings already give a deck
its outline: `## Text {…}` attaches to the heading, not to the slide it opens.

**Documentation that pairs source with its slide.** Every feature page today
shows markup or a rendered deck, rarely both at once. What is wanted is the
syntax reference as a two-column document — each example's Markdown beside a
screenshot of the slide it renders — generated by the per-slide screenshot
pass so the pictures cannot drift from the code that made them, the way the
themes gallery already cannot.

**Handouts.** A PDF export that carries the speaker notes beside each slide —
the most-reacted request on Marp's CLI, still unmet there. The other half of
this entry, the overview grid that shows the deck at a glance and jumps on
click, landed in `v0.8.0` as `O`, in the presenter window and on the phone
too.

**A `[span]{...}` cannot cross a source line.** The inline-attribute transform
runs line by line, so a span whose text is wrapped leaves its brackets and
braces on the slide as literal characters. Documented in the reference as a
constraint; the fix is to run the transform over paragraphs rather than lines,
which has to keep the existing rule that nothing inside a fence is touched.

## Non-goals

Named so the lists above stay lists. No real-time collaboration and no viewer
analytics — that ground belongs to server-backed products, and a deck that
needs an account is no longer a file in a repository. No WYSIWYG editing; the
source is the interface. No executable code cells — Quarto and Slidev own that,
and reproducing it means shipping their toolchain weight, which is the thing
people come here to escape.

## Open questions

- **Nested layouts.** Whether a pane can contain its own pane grid, or whether the
  shape layer covers those cases well enough.
- **Per-edit cost.** Making incremental builds genuinely O(1) requires keeping the
  parsed deck in memory across rebuilds. Worth it only if very large decks matter.
- **1.0.** Reserved for when the markup is stable enough that today's decks keep
  rendering. Animation and presenter mode are the last features expected to force
  syntax changes.
