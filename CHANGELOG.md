# Changelog

All notable changes to Mirzam are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
`0.MINOR.PATCH` while the project is pre-1.0: **a minor bump may change the
markup**. See [docs/development.md](docs/development.md#versioning) for the policy.

## [Unreleased]

### Added
- `brand/`: the mark, palette and type used to present Mirzam — wordmark and
  icon in light and dark, hero backgrounds, the pipeline diagram, a 1200×630
  social card, and `mirzam-theme.css` carrying the Mirzam Light / Mirzam Dark
  tokens. Documented in [brand/README.md](brand/README.md) and
  [brand/palette.md](brand/palette.md); the rasters rebuild with
  `node scripts/make-brand-raster.mjs`.
- `srcset` is now inlined alongside `src` and `poster`, so a `<picture>` that
  offers one image for a light background and another for a dark one still
  makes a self-contained deck. Previously the source the reader's theme
  selected was the one left pointing at a relative path.
- `chart` blocks: `bar`, `line`, `area` and `pie` charts rendered to SVG at build
  time from inline CSV or a `.csv` file. Individual marks get stable ids
  (`<chart-id>-<series>-<row>`) so `connect` can point at a single bar or point.
- `examples/pitch.md` and `examples/showcase.md`, with the `themes/pitch.css`
  theme, demonstrating metric tiles, charts, diagrams and connectors.
- Pane attributes `align=` and `valign=`, plus extra classes on `::: pane`.
- VS Code extension with live preview, cursor-to-slide sync and HTML export.
- WebAssembly bindings (`mirzam-wasm`) and a browser playground under
  `web/wasm-demo`.
- Video and GIF embedding, with poster-frame substitution in PDF export.
- Quality gates in CI: CommonMark compatibility, golden snapshots, incremental
  build equivalence, and a standing performance benchmark.
- `docs/layout.md` and `examples/cookbook.md`: a layout guide whose every rule is
  demonstrated by a deck that CI renders and checks.
- `scripts/check-layout.mjs`: renders decks in a browser and fails on clipped or
  overlapping content and undrawn connectors — problems HTML snapshots cannot see.
- Documentation site published to GitHub Pages: the guides plus every sample deck
  rendered as a live page (`scripts/build-site.sh`, `.github/workflows/pages.yml`).
- `AGENTS.md` and `CLAUDE.md`: working agreement for coding agents, including how
  to split work across several agents without colliding.
- Pane background images: `bg=` with `dim=`, `blur=`, `scrim=`, `bg-fit=`,
  `bg-pos=` and `text=`, plus a `.bleed` class for a full-slide background. The
  photo is inlined like any other asset, so a deck is still one file.
- `scripts/fetch-backgrounds.sh` downloads photographs from Unsplash and records
  the attribution the API requires; `scripts/make-sample-backgrounds.py` draws
  the sample backgrounds in `examples/media/bg/` so the repository builds offline.
- Heading-based slide splitting: `mirzam build doc.md --split h2`, or `split: h2`
  in frontmatter, turns an ordinary document into a deck without editing it. The
  project README is published as a deck on the docs site to demonstrate it.
- Layout debug overlay: `L` in the viewer (or `mirzam build --debug-layout`)
  outlines every pane, labels it with its band name, and tints the grid gaps.
  Off by default and never in print.
- `anim` blocks: `mirzam-anim` compiles triggers (`enter`, `click N`, `exit`,
  `after #id`), targets (ids, classes, the whole slide, and `chars`/`words`/
  `lines` splitting), a standard effect set and easing (including `spring(...)`
  resolved to a sampled curve at build time) into the timeline JSON embedded
  per slide. Text splitting happens at build time so the wrapping spans are
  already in the HTML. A target that matches nothing is a warning, not a
  build failure.
- The animation runtime plays those timelines. `→` steps through a slide's
  `click` triggers before turning the page and the counter shows the step;
  arriving from a later slide shows every step already played. The runtime is
  inlined only into decks that animate something, and it is the only thing that
  ever puts an element in its starting state — so a deck read without
  JavaScript, and the PDF export, still show every slide fully revealed. Under
  `prefers-reduced-motion` the reveals happen without the movement.
- Slide transitions: `transition: fade | slide-left | slide-right | slide-up |
  slide-down | iris | none` in frontmatter, with an optional duration and
  `ease=`. A slide overrides its half of the page turn with an ordinary
  whole-slide `[enter] slide` / `[exit] slide` track.
- Named themes: `theme: nord | solarized | vscode` in frontmatter, alongside
  the existing `default`. Every theme defines both light and dark tokens
  explicitly (dark is never derived from light by inversion), verified by a
  unit test that computes the WCAG contrast ratio for every token against
  `--mz-slide-bg` in every theme and mode. See
  `crates/mirzam-render/src/theme/themes/CREDITS.md` for each palette's origin
  and licence.
- Dark mode: `mode: dark`/`mode: light` in frontmatter, `?mode=` in the
  viewer's URL, `D` to toggle for the session, and `prefers-color-scheme`
  when nothing is set - in that priority order, with no reload needed for the
  OS-preference case.
- `examples/motion.md`: the animation sample. Text entrances, a chart whose bars
  grow one click at a time, a diagram that assembles itself box by box and
  arrow by arrow, photos that fade in and out (and come back when you step
  back), and a slide that overrides the deck's page turn.
- More effects: `wipe-in` / `wipe-out` (an edge uncovers the content instead of
  moving it), `zoom-in` / `zoom-out` and `blur-in`. More transitions:
  `wipe-left|right|up|down` and `zoom`.
- Audio: `![Interview](talk.mp3)` becomes a player, inlined like any other
  asset. `.mp3`, `.m4a`, `.wav`, `.ogg`, `.flac`, `.opus` and friends, each
  served with the media type a browser will actually play.
- YouTube and Vimeo page URLs become embeds, from `youtube-nocookie.com`. This
  is the one thing in a deck that is not self-contained: the frame is fetched
  when the slide is shown, and the PDF gets a placeholder carrying the link.
- Media is recognised by what it points at rather than by whether attributes
  were written, so a bare `![clip](talk.mp4)` is a video instead of a broken
  image.
- `fit: shrink` in frontmatter, or `{fit=shrink}` on a pane: content that would
  overflow is scaled down until it fits rather than clipped, to a floor of 55%,
  re-measured on every page turn and resize. Runs in the PDF too, for the same
  reason the annotation overlay does — it only ever reveals what a clipped pane
  would have swallowed.
- Citations: `[^key]` footnotes render at the foot of the slide that cites them,
  and a bare DOI or arXiv URL becomes a link. `examples/seminar.md` gains a
  slide quoting a figure from the paper under discussion, annotated, pointed at
  from the prose, with its references beneath it.
- `effects` blocks: presenter-triggered flourishes bound to a key — `flash`,
  `shake`, `lines` (集中線), `boom`, `burst 🎉`, `confetti` and a Nico-Nico-style
  `danmaku`. These are part of the performance rather than the document: they
  never reach the PDF, `Esc` clears them, a page turn cancels them, and binding
  a key the viewer already uses is a build warning. Nothing they draw can
  reflow the slide.
- `annotate` blocks: circle, box, arrow and label anything on a slide. An item
  is placed either in percentages of what the target *paints* — a pane holding
  one picture means that picture, letterboxing excluded — or by naming another
  element's id, which needs no coordinates and survives a data change. The
  overlay is re-measured on every resize, and it is the one script the print
  page carries, so the marks reach the PDF. `step=N` holds an item back until
  the Nth click, counting towards the slide's steps like any other build —
  and a page with no viewer still shows every mark. `id=` names a mark so a
  `connect` arrow can run from a sentence to the circle drawn over a
  photograph; the connector is routed once the mark exists and re-routed
  whenever it moves.
- `mirzam build --base-url <url>` says where the input file's directory lives
  once published, so a deck served from somewhere other than beside its source
  still resolves its links to other documents.

### Changed
- A warning raised on a slide that came from an included file now names that
  file: `mirzam-syntax` keeps a source map from the expanded document back to
  the files it was assembled from, through nested includes, a file included
  twice, CRLF line endings and variable substitution.
- A `shape` with an id is emitted as one group: a box and its label, an arrow
  and its head. Animating `#box` now moves the whole shape rather than leaving
  its label behind, and connectors resolve against the group's box.
- A bar chart mark's id likewise names a group holding the bar *and* its value
  label, so a bar animated with `wipe-in dir=up` rises with its number on top
  instead of leaving it hanging in the air.
- `draw` no longer fades the whole shape in alongside the stroke, which showed
  an arrow's head at half strength before the line had reached it. Strokes draw
  tip-first over the full duration; fills — the head, a label's glyphs, a box's
  wash — ink in over the last stretch.
- Documentation is English-first; Japanese translations live under `docs/ja/`.
- All source comments, CLI output and UI strings are English.
- Math conversion moved from `latex2mathml` to `math-core`, fixing sub/superscript
  placement; decks containing math now bundle STIX Two Math.
- Upgraded comrak to 0.54 and enabled CJK-friendly emphasis.

### Fixed
- `D` appeared to do nothing on the sample decks. It was working — `data-mode`
  flipped — but all four decks share `examples/themes/pitch.css`, which set its
  palette once, on a plain `:root` that outranks the built-in tokens for both
  modes. The theme now defines light and dark, and names its own shades as
  tokens instead of burying literals in rules (a literal cannot have a second
  mode). Two tests hold every theme under `examples/themes/` to the rule: each
  token set for one mode must be set for the other, and both modes must meet
  the same WCAG ratios the built-in themes do.
- Slides were transparent, so a page turn showed the departing slide through
  the arriving one and the previous layout appeared to linger. A stray `*/` had
  closed a comment early in `base.css`; the prose after it became CSS, and the
  parser's error recovery swallowed the rule that paints a slide opaque. Two
  tests now stand where the comment was: one rejects a `*/` outside a comment
  in any shipped stylesheet, the other asserts the rule itself is present.
- Turning back to a slide that declares its own `[enter] slide` track played no
  arrival at all — the custom track replaced the page turn, and a backwards
  entrance is deliberately not replayed — so the departing slide slid away with
  nothing covering it. Going backwards now always plays the deck's page turn,
  reversed.
- Arriving at a slide whose exit transition was still running left it stranded
  off-screen: the guard that stops a repaint from cancelling an animation in
  flight also skipped staging the slide being arrived at, so it kept the
  transform its exit had left behind. Reachable from the editor's cursor sync
  and from live reload, both of which repaint during a page turn.
- Advancing past the last slide (or retreating before the first, or pressing
  `End` while already there) replayed the current slide's entrance: the viewer
  clamped the index and treated the result as an arrival. Navigating to the
  slide already showing is now a no-op.
- An element faded out with a click could not be brought back: stepping back
  only re-armed the track, while the finished animation kept holding the hidden
  end state. Stepping back now cancels it and restores the element — and
  arriving from a *later* slide correctly keeps it hidden, since that exit has
  already played.
- Links inside a deck published away from its source 404'd: the README rendered
  as a deck at `/decks/readme/` still pointed at `docs/layout.md`, which does
  not exist there. The site now builds every deck with `--base-url`.
- A deck's own `css:` stopped overriding the palette: named themes moved the
  tokens behind `:root[data-theme="…"]`, which outranks the plain `:root` a
  custom stylesheet uses, so every deck with a custom theme silently reverted
  to the built-in one — dark styling on a light background in light mode, and
  text lost against the background in dark mode. Built-in theme selectors are
  wrapped in `:where()` now, so they carry no specificity and an author's
  `:root` always wins.
- The sample background used to demonstrate `blur=` was already out of focus,
  so blurring it showed nothing. It carries a crisp grid now, and the mountain
  photo a sharp treeline.
- The documentation site linked its guides as `docs/*.html`, which the static
  Pages deployment never produced — no Jekyll runs on an uploaded artifact, so
  every one of those links 404'd. The prose is now linked to GitHub, and
  `scripts/build-site.sh` fails the build if the landing page points at a file
  the artifact does not contain.
- Inline code, `pre` blocks, table headers and the parse-error box kept a
  hard-coded light background in dark mode, so their text was light on light
  and unreadable. Those surfaces are now theme tokens (`--mz-surface`,
  `--mz-danger-*`), defined per theme and mode, and a test rejects any
  hard-coded color in the shared stylesheet unless it carries a comment saying
  why it does not belong to a theme.
- `End` in the viewer went to the first slide instead of the last: it read the
  arity of the slide-list function rather than calling it.
- Clicking to select text in the viewer turned the page. A click is only a page
  turn when it is not a drag, no text is selected, and it did not land on a
  control.
- Connectors from a text anchor left sideways and struck through their own
  sentence. They now leave from the centre of the underline, through the edge
  facing the target, and follow direction-aware curves.
- A heading band drawn too short silently hid its heading behind the pane below;
  heading panes now stay legible and the layout checker reports the overflow.
- `history.replaceState` threw inside srcdoc iframes, aborting preview updates in
  embedded viewers such as the VS Code webview.
- Multi-line `$$...$$` blocks were not converted.
- TeX like `\sqrt[3]{x}` was mangled by the span attribute rule.
- Asset-only changes (replacing an image file) now reach connected clients.
- `scripts/build-wasm.sh` read the wasm-bindgen version from `Cargo.toml` instead
  of the resolved version in `Cargo.lock`, producing a confusing schema mismatch.
- A mistyped subcommand (`mirzam server`) printed the usage text with no
  explanation, which read as if the input file were at fault. It now names the
  mistake and suggests the nearest command. `--help` prints to stdout and exits
  0, and the usage text no longer loses its indentation.
- An image alone in a pane sat on a text baseline, so its descender space pushed
  it a few pixels past the band. Such an image is now laid out as a block.
- Fenced blocks were matched without regard to fence length, so a `pane` or
  `chart` block quoted inside a longer fence was executed instead of shown. This
  is how documentation about Mirzam is written, and it also fixed the README's own
  example block.
- The pipeline diagram's ASCII layout panel drew as `| | |`: XML collapses runs
  of whitespace in `<text>` unless told otherwise, which flattened the one part
  of the illustration whose point was its alignment.

### Changed
- The published landing page and the browser editor now use the Mirzam palette
  and type — Space Grotesk for headings, Inter for text, IBM Plex Mono for code
  — and follow the reader's `prefers-color-scheme` instead of being dark only.
  The page carries a favicon and a social card, so a link to it unfurls.

## [0.0.1] - unreleased

Initial spike: CLI (`build`, `serve`, `export pdf`), ASCII pane layout, file
transclusion, variables, math, shapes and live-routed connectors.
