# Working on Mirzam

Instructions for coding agents (and humans who like checklists). Read this before
changing anything; it is short on purpose.

## What this project is

A Markdown-based slide renderer. Source decks are plain CommonMark; the renderer
turns them into a self-contained HTML deck or a PDF. See [README](README.md) for
the user view, [docs/architecture.md](docs/architecture.md) for the design, and
[docs/development.md](docs/development.md) for the full contributor guide.

## Non-negotiables

1. **Extensions must never break plain Markdown.** Anything new has to degrade to
   a code block, a paragraph, or literal text in a plain CommonMark parser.
   `crates/mirzam-cli/tests/commonmark_compat.rs` enforces this — do not weaken it.
2. **The core must not touch the filesystem.** File and asset access go through
   `mirzam-syntax::FileProvider` and `mirzam-render::AssetSource`. A stray
   `std::fs` call in a core crate breaks the WebAssembly build, which is how the
   editor extension and browser run.
3. **English everywhere in the repository**: comments, identifiers, CLI output,
   commit messages, docs. `docs/ja/` holds Japanese translations and is never the
   source of truth. `examples/seminar.md` is deliberately Japanese; it is the CJK
   typography sample.
4. **Do not weaken a test to make it pass.** If a quality gate fails, either the
   change is wrong or the expectation genuinely moved — and if it moved, say so in
   the commit message.

## Definition of done

A change is finished when all of these hold:

```bash
export RUSTFLAGS="-D warnings"               # CI sets this; without it, clippy only advises
cargo test --workspace                       # 23 suites
cargo clippy --workspace --all-targets       # zero warnings
cargo fmt --all -- --check
node scripts/check-layout.mjs --build examples/pitch.md examples/showcase.md examples/cookbook.md
```

**Run these on the same toolchain CI does.** CI uses `dtolnay/rust-toolchain@stable`,
which is often newer than what is installed here, and each release adds lints. A
clean local run on an older compiler is not evidence: a `question_mark` lint that
did not exist in 1.94 broke the first build that ever reached CI. If `rustc
--version` is behind, `rustup toolchain install stable` and run the gate with
`cargo +stable`.

plus, when the change is user-visible:

- a line in `CHANGELOG.md` under `[Unreleased]`
- syntax changes documented in `docs/syntax.md` and shown in `examples/showcase.md`
- layout behaviour documented in `docs/layout.md` and shown in `examples/cookbook.md`
- golden snapshots updated **deliberately**, with the diff reviewed:
  `MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden`

## Verify by rendering, not by reading

Rendering bugs are invisible in HTML diffs. Several real bugs in this repository
(a clipped heading, arrows striking through their own sentence, a viewer update
aborting mid-way) were found only by looking at pixels.

```bash
cargo run --bin mirzam -- build examples/pitch.md -o /tmp/out
node scripts/check-layout.mjs /tmp/out/index.html      # automated checks
```

If you have a browser driver available, screenshot the slides you touched and
look at them. Claiming a visual change works without having seen it is not
acceptable here.

## Where things live

| Crate | Owns | Touch it when |
|---|---|---|
| `mirzam-syntax` | Frontmatter, transclusion, slide splitting, `::: pane`, fenced blocks | Adding a new block or inline form |
| `mirzam-core` | Deck metadata, `{{ }}` evaluator | Frontmatter fields, expression functions |
| `mirzam-layout` | ASCII grid → proportional grid | Pane sizing and merging rules |
| `mirzam-shape` | Shape DSL → SVG | Shape kinds, shape attributes |
| `mirzam-chart` | Chart DSL + CSV → SVG | Chart types, data parsing |
| `mirzam-connect` | Connector DSL → JSON | Connector syntax (routing is in the viewer) |
| `mirzam-render` | HTML assembly, theme, viewer runtime | Output structure, CSS, viewer behaviour |
| `mirzam-cli` | `build`/`serve`/`export`, build cache, benchmark | Commands, caching, watching |
| `mirzam-wasm` | Browser/editor bindings | Anything the editor extension needs |

The viewer runtime (navigation, connector drawing) is the JavaScript string in
`crates/mirzam-render/src/theme/viewer.js`. It ships inside every deck, so keep
it small and dependency-free.

## Running several agents at once

The current batch of work is already split into streams, with the shared
contracts written down: see [docs/workstreams.md](docs/workstreams.md). If you
were handed a stream number, that is your brief.

In general, work splits cleanly along crate boundaries. These streams rarely
collide:

| Stream | Files | Notes |
|---|---|---|
| Syntax / parsing | `crates/mirzam-syntax`, `mirzam-core` | Pure functions, heavily unit-tested |
| Charts | `crates/mirzam-chart` | Self-contained; only `render/charts.rs` connects it |
| Shapes | `crates/mirzam-shape` | Self-contained |
| CLI / server | `crates/mirzam-cli` | Owns caching and watching |
| Editor / browser | `editors/vscode`, `web/`, `mirzam-wasm` | JavaScript plus bindings |
| Docs / samples | `docs/`, `examples/` | Prose and decks |

**Contention hotspots.** Coordinate before two agents edit these at once:

- `crates/mirzam-render/src/theme/base.css` — the layout/typography rules every theme shares
- `crates/mirzam-render/src/theme/viewer.js` — the runtime shipped inside every deck
- `crates/mirzam-cli/tests/snapshots/*.html` — every rendering change rewrites them,
  so two agents changing output will conflict; land one, regenerate, then the next
- `Cargo.toml` (workspace members), `CHANGELOG.md` — append-only, but still merge
  conflicts if edited simultaneously

**Splitting work.** Give each agent a whole vertical slice — parser plus tests plus
docs plus sample slide — rather than one layer of several features. A slice that
ends with green quality gates can be merged independently; half a feature cannot.

## Playbooks

**Add a syntax feature.** Recognize it in `mirzam-syntax` (or, for content inside a
pane, in the renderer's extraction pass — see `crates/mirzam-render/src/charts.rs`)
→ put the semantics in its own crate → emit HTML/SVG from `mirzam-render` using
theme variables, never hard-coded colors → document in `docs/syntax.md` → add a
slide to `examples/showcase.md` → update snapshots.

**Change how something looks.** Edit `crates/mirzam-render/src/theme/base.css` for
layout shared by every theme, or `theme/themes/*.css` for one theme's tokens →
rebuild the sample decks → run the layout checker → look at the screenshots →
update snapshots.

**Fix a rendering bug.** Reproduce it in a minimal deck first and keep that deck as
a test fixture if it is small. Check whether `scripts/check-layout.mjs` could have
caught it; if not, consider teaching it to.

**Cut a release.** Follow the checklist at the end of
[docs/development.md](docs/development.md).

## Things that will waste your time

- The `wasm-bindgen` CLI must match the version resolved in `Cargo.lock` exactly.
  `scripts/build-wasm.sh` handles this; do not install it by hand.
- `comrak` is built with `default-features = false` because its default features
  pull in a C library that does not build for `wasm32`.
- Chromium builds without proprietary codecs cannot play H.264. Sample decks use
  `webm` for that reason.
- `mirzam serve` diffs *rendered output*, not source, so an image swap reaches the
  browser. Keep it that way.
