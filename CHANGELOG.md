# Changelog

All notable changes to Mirzam are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
`0.MINOR.PATCH` while the project is pre-1.0: **a minor bump may change the
markup**. See [docs/development.md](docs/development.md#versioning) for the policy.

## [Unreleased]

### Added
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

### Changed
- Documentation is English-first; Japanese translations live under `docs/ja/`.
- All source comments, CLI output and UI strings are English.
- Math conversion moved from `latex2mathml` to `math-core`, fixing sub/superscript
  placement; decks containing math now bundle STIX Two Math.
- Upgraded comrak to 0.54 and enabled CJK-friendly emphasis.

### Fixed
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
- An image alone in a pane sat on a text baseline, so its descender space pushed
  it a few pixels past the band. Such an image is now laid out as a block.
- Fenced blocks were matched without regard to fence length, so a `pane` or
  `chart` block quoted inside a longer fence was executed instead of shown. This
  is how documentation about Mirzam is written, and it also fixed the README's own
  example block.

## [0.0.1] - unreleased

Initial spike: CLI (`build`, `serve`, `export pdf`), ASCII pane layout, file
transclusion, variables, math, shapes and live-routed connectors.
