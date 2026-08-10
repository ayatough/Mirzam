# Changelog

All notable changes to Mirzam are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
`0.MINOR.PATCH` while the project is pre-1.0: **a minor bump may change the
markup**. See [docs/development.md](docs/development.md#versioning) for the policy.

## [Unreleased]

### Added
- `bg-light=` and `bg-dark=` on a pane: one photograph per colour mode. Both
  are inlined and the deck shows the one that matches — including after the
  reader presses `D`, which a `<picture>` element cannot follow, since its
  `media` query can only ask the operating system. Naming one leaves `bg=` as
  the other mode's image; naming one *without* a partner warns, because the
  other mode would show a bare pane with photo-coloured text on it.
- `mirzam build --theme <name>`, `--css <file>` and `--fit shrink`: the
  frontmatter's theme, stylesheet and overflow behaviour, chosen from the
  command line. This is what lets a document that cannot carry frontmatter — a
  README, where it would surface as a stray table on GitHub — still be published
  as a deck with an identity, and without four of its sections cut off at the
  bottom of the slide.
- The landing page has a light/dark switch instead of only following the
  machine, and stores the choice where a deck's viewer reads it, so a deck
  opened from a light page opens light. The viewer's own `D` writes the same
  key, which also makes that toggle stick from one deck to the next.

### Changed
- `main` is documented as the working branch rather than a stable one, in the
  README, its Japanese translation, `CONTRIBUTING.md` and `AGENTS.md`. It is
  where development lands directly: the site publishes from `main` and nowhere
  else, so a change held on a branch cannot be reviewed where it counts. Depend
  on a release, not on `main`.
- The pitch deck's title slide carries Mirzam's own hero art, one image per
  mode, in place of the stock city photograph.
- The README deck on the site is built with Mirzam's theme rather than
  `default`, which is the one deck there that looked like someone else's.

### Fixed
- The landing page's "See a deck running" and "Source on GitHub" buttons were
  unclickable: the hero's scrim is a positioned sibling that came after the
  content, so it painted over both and swallowed every click aimed at them.

## [0.1.0] - 2026-08-09

First tagged release. Prebuilt binaries, a browser editor, and a deck you can
present from — animation, effects, annotations, a presenter window and a
contents page that writes itself.

### Added
- `scripts/record-demo.mjs`: records a deck being presented, by driving it in a
  browser rather than by anyone operating one. Writes a `.webm` with no extra
  tooling — and a GIF when a full ffmpeg is on the machine, which the one
  Playwright ships is not. Keypresses appear on screen, because a deck that
  advances by itself demonstrates nothing.
- `theme: mirzam` — Mirzam's own palette as a built-in theme, in both modes,
  so a deck gets the identity's colours from one word of frontmatter. It is the
  token half of `examples/themes/mirzam.css`; the typography stays in that file,
  because a built-in theme is loaded before the layout stylesheet and can only
  set tokens. `css: themes/mirzam.css` is still how you get the whole thing.
- **Prebuilt binaries.** `.github/workflows/release.yml` builds `mirzam` for
  x86-64 and arm64 Linux, Intel and Apple-silicon macOS, and x86-64 Windows on
  every `v*` tag, smoke-tests each native one by building a deck with it, and
  publishes the archives with checksums and that version's changelog section as
  the release notes. `scripts/install.sh` picks the right archive, verifies the
  checksum and drops the binary in `~/.local/bin` — using Mirzam no longer
  requires a Rust toolchain, which was the largest thing standing in front of
  anyone who just wanted to make a deck.
- `LICENSE`: the MIT text the README has always claimed, plus a note that the
  bundled STIX Two Math font travels under the SIL Open Font License wherever a
  deck goes.
- `docs/quickstart.md` and `docs/ja/quickstart.md`: four ways in — browser, CLI,
  VS Code, Obsidian — with an honest table of what the browser build cannot do
  and why.
- **Per-pane continuation.** `<!-- next -->` inside a pane carries that pane on
  to the next slide while every other pane holds still. The viewer recognises
  the two slides as one and cuts between them instead of turning the page, so a
  chart you are still talking about does not move. A build expands the marker
  into real slides, which means the PDF and a no-JavaScript reader get it too.
- **A contents page that writes itself.** A `toc` block collects the deck's
  headings, links each one to its slide, marks the section you are in, and
  prints page numbers instead of links in the PDF. `from:`, `depth:` and
  `current:` choose what it covers.
- **A presenter window.** `P` opens a second window with the next slide, your
  speaker notes, a clock and a talk timer. The two windows stay in step through
  `BroadcastChannel`, including dark mode and the layout overlay, so the screen
  the audience sees never disagrees with the one you are reading.
- **Viewer chrome.** A page counter and controls, and `/` for a cheat sheet that
  lists this particular deck's effect keys rather than a generic table. On a
  phone: swipe to turn the page, swipe up for notes, two-finger tap for the same
  sheet.
- **Marking a phrase and the thing it refers to, together.** `highlight`,
  `underline` and `box` take an `#id` and nothing else, and follow the line
  boxes that phrase actually occupies — a sentence that wraps gets one mark per
  line, not one box over both. Paired with a mark on a chart bar under the same
  `step`, they say "this phrase, that bar" in one colour without drawing
  anything across the slide. `target:` is now optional, because a block whose
  items are all anchored measures nothing against a box.
- **A browser editor**, published at [ayatough.github.io/Mirzam/try](https://ayatough.github.io/Mirzam/try/):
  open and save `.md`, attach, drop or paste images, and download the finished
  self-contained deck. The same Rust core as the CLI, compiled to WebAssembly;
  nothing is uploaded. It works on a phone.
- `mirzam_syntax::BLOCK_KINDS`, the canonical list of fenced forms the language
  claims. `commonmark_compat.rs` walks it, so a new block form is checked
  against a plain CommonMark parser the moment it is added — the promise that a
  deck still reads on GitHub is now kept by construction rather than by memory.
- `scripts/check-layout.mjs` learned three failure modes that HTML snapshots and
  the eye both miss: an annotation that could not be drawn (its anchor was
  renamed), an element still holding its entrance state after that entrance has
  played, and a `--debug-layout` overlay baked into a published build. The
  runtime answers for the first two through `MZAnnot.missing` and `MZAnim.armed`,
  and a test keeps those two names from drifting out from under the checker.
- `rust-version = "1.91"` in the root manifest, with a CI job that builds the
  workspace on exactly that toolchain. The README had claimed 1.75, which had
  not been true since `math-core` landed.

- `docs/brand/`: the mark, palette and type used to present Mirzam — wordmark and
  icon in light and dark, hero backgrounds, the pipeline diagram, a 1200×630
  social card, and `mirzam-theme.css` carrying the Mirzam Light / Mirzam Dark
  tokens. Documented in [docs/brand/README.md](docs/brand/README.md) and
  [docs/brand/palette.md](docs/brand/palette.md); the rasters rebuild with
  `node scripts/make-brand-raster.mjs`.
- `examples/themes/mirzam.css`: the identity as a deck theme — `css:
  themes/mirzam.css` in a deck's frontmatter. Both modes, chart series that can
  be told apart, and the brand type ladder. `examples/themes/pitch.css` keeps
  its name and its pitch-deck furniture but is redrawn in the same palette, so
  every published sample deck now looks like Mirzam.
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
  `shake`, `lines` (speed lines), `boom`, `burst 🎉`, `confetti` and a Nico-Nico-style
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
- The release profile enables thin LTO and strips symbols: the binary went from
  6.5 MB to 4.5 MB, at a link cost paid once per tag rather than once per edit.
- Documentation no longer recommends an arrow from a sentence to a figure. An
  arrow has to leave the prose, cross the slide and land somewhere meaningful,
  and none of that was ever what the audience asked for; `connect` is now
  presented as the tool for two boxes *inside* a diagram, and text-to-figure
  goes to the paired annotation above.
- The viewer chrome takes its colours from the deck's own paper tokens instead
  of a fixed dark palette, so it is legible whatever the theme does. This is
  what made the presenter window wrong in light mode, and it was never only the
  presenter window.
- Benchmark re-measured at this release: a 500-slide deck builds in 76 ms and a
  single-slide edit re-renders in 3.2 ms, up from 2.3 ms. A build now expands
  `<!-- next -->`, resolves the contents page against the finished deck and only
  then hashes, so the whole-document pass grew while the per-slide render did
  not.

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

- The published landing page and the browser editor now use the Mirzam palette
  and type — Space Grotesk for headings, Inter for text, IBM Plex Mono for code
  — and follow the reader's `prefers-color-scheme` instead of being dark only.
  The page carries a favicon and a social card, so a link to it unfurls.

### Fixed
- **Text selection on a phone.** The cheat sheet was bound to a long press, and
  a long press is how you select text — so a deck on a phone could not be quoted
  from. The binding is gone (two-finger tap and the `?` button open the sheet),
  and a selection drag is no longer read as a page swipe.
- Swiping right walked out of the deck: Chrome reads horizontal overscroll as
  browser-back. The deck now claims vertical panning only.
- Dark mode and the layout overlay were independent between the presenter and
  audience windows, so the two could disagree about what the audience was
  looking at. Both now travel over the same link as the slide and step.
- `--mz-muted` on `default/light` and `solarized/light` sat at 4.19:1 and 4.39:1
  against a surface — below the 4.5:1 the contrast guard requires. Caught by
  extending that guard to the muted-on-surface pair, which the chrome change
  above made load-bearing.
- A `draw` animation left a saved style snapshot on the element around the
  painted parts, which nothing ever restored; a later arming of that element
  would then quietly keep the stale one.

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

## [0.0.1] - never tagged

Initial spike: CLI (`build`, `serve`, `export pdf`), ASCII pane layout, file
transclusion, variables, math, shapes and live-routed connectors.
