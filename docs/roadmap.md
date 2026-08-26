# Roadmap

Where Mirzam is and what comes next. Design rationale lives in
[architecture.md](architecture.md); the older Japanese planning notes are under
[ja/roadmap.md](ja/roadmap.md).

## Status

`v0.10.0` is the current release: everything below marked Done is in it, built
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
| A working page on a slide (`![alt](page.html)`), sandboxed and inlined | Done |
| Figure captions and credits (`caption=`, `credit=`), a credit that cites | Done |
| A language server (`mirzam lsp`): diagnostics and an outline | Done |
| The same server's completion, hover and definitions | Done |
| Handout PDF: notes beside each slide (`export pdf --handout`) | Done |
| Data-driven slides: one slide per CSV row (```` ```each ````) | Done |
| A slide's own autoplay dwell; autoplay that waits for a clip | Done |
| Code line highlighting and line numbers (```` ```js {2,4-5 lines} ````) | Done |
| PPTX export, stage one: slide pictures and real speaker notes | Done |
| Quoting a figure out of a paper (`import pdf`) | Done |
| That figure rendered to SVG with nothing installed | Done · unreleased |
| Plugins, PPTX with native text boxes | Later |

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

The one worth naming here is **carrying an element from one slide to the
next** — and it joins structural math editing as a thing that was **built and
then withdrawn**. A `.carry` runtime landed on a branch after `v0.9.0`: the
same `#id` on two consecutive slides, a lifted clone flown between its two
boxes, backwards as good as forwards, with the PDF and a scriptless reader
seeing two complete ordinary slides. It worked exactly as specified — and
watching it in a real deck, the author judged the motion unnatural, so it was
pulled before it merged. The mechanism is written up in
[W18's brief](workstreams.md#w18--carrying-an-element-from-one-slide-to-the-next)
along with what the withdrawal teaches: the invariants were never the hard
part, the *feel* is — a straight-line flight with linear scaling, over the top
of the page turn, does not read as the component travelling. It stays Next,
with that as the bar. The reasoning behind the rest of the current queue is in
the [post-v0.9 plan](reports/2026-08-v0.10-plan.md).

### Measured performance

Release build, from the standing benchmark, on the machine
[the August 2026 report](reports/2026-08-performance.md) describes:

| Deck | Full build | Single-slide edit |
|---|---:|---:|
| 20 slides | 7.4 ms | 0.7 ms |
| 120 slides | 20.3 ms | 1.3 ms |
| 500 slides | 79.9 ms | 3.5 ms |
| 100 slides, 800 formulas | 28.4 ms | 1.5 ms |

Exactly one slide re-renders per edit. The residual growth is the linear cost of
re-reading and hashing the source, not rendering.

Every release from `v0.1.0` was rebuilt and measured again on that one machine,
which is the only way to tell a slower release from a faster laptop. Eight of
them cost 1 to 3 ms on a full build and 0.3 ms on an edit — **the same amount at
every deck size**, so what grew is the fixed cost of a build and not the cost of
a slide. The per-slide figure is if anything lower than it was, and the design
goal holds: 25× the slides costs 5× the edit.

The number that did move is what a deck weighs: the smallest deck there is had
gone from 55 KB at `v0.1.0` to 144 KB, because the viewer and the base
stylesheet shipped whole — half of them prose explaining the code — inside every
deck. Those comments are now stripped at compile time and the same deck is
78 KB. What is left is the maths font, embedded entire the moment a deck holds
one formula; the report says why subsetting it is not the afternoon's work it
looks like.

Build time, by feature: a code fence costs 0.11 ms, a chart 0.04 ms, a formula
0.023 ms, a shape nothing worth measuring. A fence was 0.40 ms until the
highlighters stopped being rebuilt one per fence — a hundred-fence deck went
from 52 ms to 28 ms — and it is still the most expensive item in the renderer,
so it stays where to look first if a build ever feels slow.

## Still open

Two things below `1.0` that were not features so much as unfinished thinking.
One of them, the language server, now has a brief and a place in the table
above; it is kept here because the reasoning that got it there belongs beside
the other one.

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

This one stopped being unfinished thinking, and then stopped being unbuilt:
`mirzam lsp` publishes diagnostics and an outline as of the next release. The
brief is [W25](workstreams.md#w25--a-language-server-the-editor-understands-the-deck),
and writing it turned up how much was already there. Every diagnostic it would
publish is a warning `check --format json` emits today, under the kind
vocabulary the agent contract fixed; the source map turns a slide or a pane
back into a file and an offset; none of it needs the browser, since only the
layout pass does. So it is a channel and a range, not an analysis — it ships as
a `mirzam lsp` subcommand rather than a second binary to build for every
platform, and it adds no dependency, because JSON-RPC over stdio is a header
and a loop and `serde_json` is already here. What it is *not* is exact ranges:
a warning knows its slide, not its token, and until spans are threaded through
every warning the server finds the range by looking for the token the message
already quotes.

## Later

**Plugins.** Two extension points: WebAssembly passes that transform the document
before rendering, and JavaScript modules that register runtime effects. Themes stay
plain CSS plus a manifest, since that already works.

**Export beyond HTML and PDF.** The first stage landed: `mirzam export pptx`
writes PowerPoint via hand-written OOXML — slides as pictures plus real
speaker notes, which is where Marp, Slidev and Touying all stop. What remains
is the stage none of them ships and the market survey found to be the loudest
unmet ask across every Markdown slide tool: native text boxes, with elements
that have no OOXML equivalent rasterized rather than dropped. Google Slides
comes through the same path. Direct PDF generation without Chromium is a
separate, larger question that depends on adopting a text layout engine.

**A figure into SVG without a tool installed.** Landed. `mirzam import pdf`
converts a drawn figure with [hayro](https://github.com/LaurenzV/hayro) — a PDF
interpreter in pure Rust, Apache-2.0 OR MIT, from the typst ecosystem — so the
one step in the feature that asked the author to install something is gone. The
transpiler this section used to plan, three stages of font work and several
thousand lines, was never written: hayro does that daily, shadings and Type 3
fonts included, against a corpus this project could not assemble.

What the adoption cost, measured: `hayro-svg` resolves 61 crates, takes the
release binary from 7.5 MB to 12.6 MB, and moves the floor to Rust 1.92. "Nothing else is
bundled" became a short attribution list, since hayro carries PDFium's Foxit
substitute fonts (BSD-3) and two ICC profiles. `--format png` still wants
`mutool` or `pdftocairo`: rasterising is hayro's other half and another couple
of megabytes, which is a separate decision.

The obvious way to buy those megabytes back was measured and rejected. Built at
`opt-level = "z"` with fat LTO the binary is 9.8 MB — 2.8 MB off — and everything
takes about twice as long: a 500-slide deck builds in 189 ms instead of 88, and
the single-slide edit the design goal is stated in goes 4.1 ms to 8.6 ms.
`opt-level = "s"` is 10.3 MB at 1.4×. Even fat LTO alone, which looked free,
buys 654 KB for +22 % on the maths deck and +17 % on `import pdf` itself. So the
release profile stays where it is: the size dial is a worse trade than carrying
the megabytes, and the only remaining lever on them is a renderer this project
writes itself.

The guard-rails both survive, and one of them earned its place immediately.
hayro converts a *page* and keeps what falls outside it — a crop being a page
with its box narrowed, that meant 1.49 MB per figure against `mutool`'s 4 to
17 KB — so the conversion is followed by a culling pass, timid by construction
and unit-tested for it, which brings the same figures to 4–26 KB rendering
identically. The refusal path is the second: a font hayro cannot read or an
image that will not decode leaves the crop on disk and says which it was,
rather than shipping a picture with a hole in it. `MIRZAM_PDFTOOL` stays the
escape hatch, and the staged transpiler plan stays in this file's history.

**Still open here:** not emitting off-page geometry belongs upstream, not in a
culling pass this project maintains for good; and the conversion is pure Rust,
so the editor extension and the browser build could import a figure too — the
WASM dividend the old plan promised, now a matter of wiring rather than of
writing a renderer.

**Editing anywhere.** The WASM core already runs in a browser; a progressive web
app on top of it is what makes phone editing real. An Obsidian plugin reuses the
same core.

**The unattended screen, beyond autoplay.** `autoplay: 8s loop` landed as
pacing: one deck-wide interval driving the same advance the arrow key does,
which already covers the exhibition loop and the captioned photo slideshow.
The first two follow-ups landed next — `<!-- autoplay: 20s -->` holds the
slide that needs reading time, and a slide whose clip is still playing turns
when the clip ends rather than mid-sentence. What is deliberately left, in
rough order of pull:

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

**Richer data.** Column aggregation in tables and more chart types.
Data-driven slides left this list: ```` ```each ```` renders one slide per
CSV row as of the next release — the one structural advantage Typst's
scripting had over every Markdown tool, answered as a table where the table
would be rather than a loop.

**D2, through Mermaid's door.** Mermaid landed in `v0.7.0` exactly in the
shape planned here: the CLI shells out to a local `mmdc` at build time,
inlines the SVG with its palette rewritten to theme tokens, and a machine
without a renderer gets a code block *and* a `build.mermaid` warning — while
GitHub draws the same fence as a diagram natively. D2 arrives through the
same `DiagramRenderer` trait once wanted; nothing else has to move.

The gallery shows one as of the next release — "A diagram you did not draw" in
`examples/04-components.md` — which took installing mermaid-cli in the CI and
site builds, since a fence nothing draws publishes as a code block. The deck is
held to `--strict` there, so that downgrade fails the build rather than the
gallery quietly shipping source where a picture belongs.

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

**Handouts** landed: `mirzam export pdf --handout` prints one page per slide
with the speaker notes beside it, and ruled lines where a slide has none —
the most-reacted request on Marp's CLI, still unmet there. The other half of
this entry, the overview grid that shows the deck at a glance and jumps on
click, landed in `v0.8.0` as `O`, in the presenter window and on the phone
too.

**A `[span]{...}` may cross a source line now** — the fix this entry called
for (run the inline-attribute transform over paragraphs rather than lines,
fences untouched) landed. What remains true is the paragraph rule: a blank
line between the brackets is a wall, so a `[` left open in one paragraph
never swallows the next.

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
