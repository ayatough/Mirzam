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
   A new fenced block goes on `mirzam_syntax::BLOCK_KINDS` in the same change;
   that test walks the list, so a form missing from it is a form nobody checked.
2. **The core must not touch the filesystem.** File and asset access go through
   `mirzam-syntax::FileProvider` and `mirzam-render::AssetSource`. A stray
   `std::fs` call in a core crate breaks the WebAssembly build, which is how the
   editor extension and browser run.
3. **English everywhere in the repository**: comments, identifiers, CLI output,
   commit messages, docs. `docs/ja/` holds Japanese translations and is never the
   source of truth. `examples/seminar.md` is deliberately Japanese; it is the CJK
   typography sample, and `examples/research.md` is its English counterpart.
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
node --test editors/vscode/test/*.test.js   # only if you touched editors/vscode
for d in 01-start 02-writing 03-layout 04-components 05-motion 06-theming pitch research seminar slideshow; do
  cargo run -q --bin mirzam -- check "examples/$d.md"   # needs a browser, nothing else
done
```

**Run these on the same toolchain CI does.** CI uses `dtolnay/rust-toolchain@stable`,
which is often newer than what is installed here, and each release adds lints. A
clean local run on an older compiler is not evidence: a `question_mark` lint that
did not exist in 1.94 broke the first build that ever reached CI. If `rustc
--version` is behind, `rustup toolchain install stable` and run the gate with
`cargo +stable`.

plus, when the change is user-visible:

- a line in `CHANGELOG.md` under `[Unreleased]`
- syntax changes documented in `docs/syntax.md` and shown in `examples/04-components.md`
  (or `examples/05-motion.md` if it moves, `examples/06-theming.md` if it is a setting)
- layout behaviour documented in `docs/layout.md` and shown in `examples/03-layout.md`
- golden snapshots updated **deliberately**, with the diff reviewed:
  `MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden`

**Then push it to `main`.** There is no second reader waiting on a branch, and
the preview site publishes from `main` and nowhere else, so a change parked on a
branch cannot be looked at where it matters. This makes the gate above the whole
of the review: run it before you push, not after. If something does land broken,
`git revert` it — reverting a small commit costs less than the branch dance
would have cost every commit that was fine.

### Where a change shows up

The site is two builds in one deployment, because the person checking a change
and the person arriving from a link want different things.

| URL | Built from | Carries |
|---|---|---|
| [`/`](https://ayatough.github.io/Mirzam/) | the latest tag | the released version, indexed |
| [`/next/`](https://ayatough.github.io/Mirzam/next/) | the tip of `main` | a `DEV` banner, `v<tag> +<n> · <sha>`, the changelog's `[Unreleased]` section, and `noindex` |

So **a push to `main` moves `/next/`, and only a release moves `/`.** That is
what lets work land directly on `main` without a half-finished afternoon
becoming the front page. It also means the author reviews a change at
`/next/` — say where to look when a change is visual.

`.github/workflows/pages.yml` runs on CI *finishing*, not on the push, so a
commit whose tests are red is never published. Do not change that back: with no
pull request in the way, it is the only gate there is. It also runs on `release:
published`, because a release is tagged *after* its commit's CI has already
rebuilt the site — without that second trigger the root lands on the previous
release every time.

**Write the changelog entry for a reader on a phone.** `/next/` renders
`[Unreleased]` on the page, so that section is not paperwork filed for a future
release — it is the only summary of your change the author will see before
looking at the slides. Say what changed and why it changed, not which files you
touched; the diff already has those.

**The author reviews from a phone, away from a machine.** So when a change is
visual, name the deck and the slide number to open at `/next/`, and do not
assume a terminal is available to reproduce anything.

## Verify by rendering, not by reading

Rendering bugs are invisible in HTML diffs. Several real bugs in this repository
(a clipped heading, arrows striking through their own sentence, a viewer update
aborting mid-way) were found only by looking at pixels.

```bash
cargo run --bin mirzam -- check examples/pitch.md      # automated checks
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
| `mirzam-figure` | Captioned figures on a laid-out page | Which line is a caption, which ink is the picture |
| `mirzam-render` | HTML assembly, theme, viewer runtime | Output structure, CSS, viewer behaviour |
| `mirzam-cli` | `build`/`serve`/`export`/`import`, build cache, benchmark | Commands, caching, watching, reading a PDF |
| `mirzam-wasm` | Browser/editor bindings | Anything the editor extension needs |

The viewer runtime (navigation, connector drawing) is the JavaScript string in
`crates/mirzam-render/src/theme/viewer.js`. It ships inside every deck, so keep
it small and dependency-free.

### The sample decks

`examples/` is three groups, and a new slide belongs to exactly one of them.

`01-start.md` is on its own: the path from reading about Mirzam to having a
deck. It is the only one with an order to it.

**02 to 06 are a reference, not a course.** They were labelled a tutorial to be
read in order, which was a claim nothing supported — the numbers are subject
areas, and nobody reads 04 because they finished 03. They are written in the
markup they document, so the source beside the slides is the example. Ordering
a change by "which deck owns this":

| Deck | Owns |
|---|---|
| `02-writing.md` | Everything inside a pane: headings, emphasis, lists, tables, maths, footnotes, emoji. Held to `markup_coverage.rs` — a mark that renders and is not shown here fails CI |
| `03-layout.md` | Layout rules, one per slide; the companion to `docs/layout.md` |
| `04-components.md` | Charts, shapes, connectors, media, annotations |
| `05-motion.md` | `anim`, transitions, `effects` |
| `06-theming.md` | Themes, frontmatter, attributes, custom CSS |

`pitch.md`, `research.md`, `seminar.md` and `slideshow.md` are the third group:
complete decks, written the way somebody would write one for an audience —
`slideshow.md`'s audience being whoever walks past the kiosk it loops on. They
are not feature catalogues, so do not add a slide to them to demonstrate
something.

A feature gets **one** home. Before adding a slide, check the feature is not
already shown in a sibling deck — the previous layout had video in two files and
animation in two more, which is how they drifted apart.

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
| Brand | `docs/brand/`, the landing page in `scripts/build-site.sh` | Mark, palette, type; nothing here ships inside a user's deck |

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
slide to `examples/04-components.md` → update snapshots.

**Change how something looks.** Edit `crates/mirzam-render/src/theme/base.css` for
layout shared by every theme, or `theme/themes/*.css` for one theme's tokens →
rebuild the sample decks → run the layout checker → look at the screenshots →
update snapshots.

**Touch the brand.** `docs/brand/` is presentation, not product: the README header,
the site, link previews. Deck themes are separate on purpose, so a change here
never rewrites a snapshot. The wordmark carries its type as outlines because
GitHub loads no webfonts - regenerate it rather than retyping the paths, and see
[docs/brand/README.md](docs/brand/README.md) for the recipe and the weights.

**Fix a rendering bug.** Reproduce it in a minimal deck first and keep that deck as
a test fixture if it is small. Check whether `scripts/check-layout.mjs` could have
caught it; if not, consider teaching it to.

**Cut a release.** `./scripts/release.sh <version>` writes the version into
every file that carries it, closes the changelog and runs the gate; then commit,
push to `main`, and dispatch the Release workflow with `publish`. The checklist
at the end of [docs/development.md](docs/development.md) says what the script
does not.

## Things that will waste your time

- **Use `mirzam check`, not `check-layout.mjs`, unless you need a live tab.**
  They run the same in-page checks from the same source, but `check` drives a
  one-shot headless Chromium from the binary you already built, and the script
  imports `playwright-core`, which is not a repository dependency and leaves
  `package.json`, `package-lock.json` and `node_modules` to clean up before
  committing. Either way, point it at a browser if none is on `PATH`:
  ```bash
  MIRZAM_CHROMIUM=/opt/pw-browsers/chromium-*/chrome-linux/chrome \
    cargo run -q --bin mirzam -- check examples/pitch.md
  ```
  The script is still the right tool for a screenshot or a recording, where the
  tab has to stay open — that is what `scripts/record-demo.mjs` uses.
- **The gallery warns about its diagram unless you have mermaid-cli.**
  `examples/04-components.md` holds a `mermaid` fence, so `build` and `check`
  say `no diagram renderer found` on a machine without `mmdc` and show the
  fence as a code block. That is the documented degradation and it fails
  nothing: the golden snapshot normalizes the diagram away, so `cargo test`
  passes either way. CI installs mermaid-cli in the layout job and holds that
  one deck to `--strict`, which is what stops the site publishing the code
  block. Install it (`npm install -g @mermaid-js/mermaid-cli`) only if you are
  changing what the diagram looks like.
- **An attribute span has to stay in one paragraph.** `[text]{.small}` may
  wrap onto the next source line, but a blank line between the brackets is a
  paragraph break and leaves them as literal `[text]{.small}` on the slide.
  The layout checker measures boxes, so it passes this happily — the build's
  "still on the slide as text" warning is what catches it.
- **`git push` of a tag is refused for an agent** (403; the credentials are
  scoped to branches) — but cutting the release is still yours to do, and the
  403 is not a reason to hand it back. Two facts make it work: the version-bump
  commit has to be **on `main`** (a release is cut from `main`, not from the
  branch you were given), and the **Release** workflow with `publish` checked
  makes the tag from the manifest itself. Dispatch it however you can reach the
  API — `gh workflow run release.yml --ref main -f publish=true`, the GitHub
  MCP server's workflow-dispatch tool, or a `POST` to
  `/actions/workflows/release.yml/dispatches`. Never push a tag by hand. The
  [release checklist](docs/development.md#release-checklist) assumes all of
  this.
- The `wasm-bindgen` CLI must match the version resolved in `Cargo.lock` exactly.
  `scripts/build-wasm.sh` handles this; do not install it by hand.
- `comrak` is built with `default-features = false` because its default features
  pull in a C library that does not build for `wasm32`.
- Chromium builds without proprietary codecs cannot play H.264. Sample decks use
  `webm` for that reason.
- `mirzam serve` diffs *rendered output*, not source, so an image swap reaches the
  browser. Keep it that way.
