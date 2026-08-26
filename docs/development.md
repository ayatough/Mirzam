# Development guide

Everything a contributor needs: how to build, what the quality gates check, how the
crates fit together, and how versions are handled.

## Setup

Rust 1.91+ is the only hard requirement. The floor is declared as
`rust-version` in the root manifest and comes from `math-core`, not from our own
code; CI builds the workspace on exactly that toolchain so the number stays
true.

```bash
cargo build              # native CLI
cargo test --workspace   # all tests, including the quality gates
```

Optional, installed automatically by the scripts that need them:

- `wasm32-unknown-unknown` target and `wasm-bindgen-cli` (for the WASM build)
- Node.js (only to package the VS Code extension)
- Chromium or Chrome (only for `mirzam export` and `mirzam check`)
- ffmpeg (only for `mirzam export video`; the trimmed build Playwright
  installs beside its browsers qualifies and is found automatically)

## Common tasks

```bash
cargo run --bin mirzam -- build examples/pitch.md -o out
cargo run --bin mirzam -- serve examples/04-components.md

cargo run --release -p mirzam-cli --bin mirzam-bench   # performance benchmark
./scripts/build-wasm.sh                                # WASM package into pkg/
./scripts/serve-wasm-demo.sh                           # browser playground
./scripts/build-vsix.sh                                # VS Code extension

cargo run --bin mirzam -- check examples/pitch.md         # layout validation
./scripts/build-site.sh                                  # docs site + live decks
node scripts/make-brand-raster.mjs                       # docs/brand/ social card + icon PNG
node scripts/shoot-slides.mjs --build examples/pitch.md -o shots   # one PNG per slide
node scripts/make-theme-gallery.mjs -o site/themes       # every theme, both modes
node scripts/record-demo.mjs --editor -o media/edit-loop --gif     # the README GIF

./scripts/check-versions.sh                              # every version agrees
./scripts/release.sh 0.5.0 --dry-run                     # what a release would change
```

`mirzam check` and the brand rasteriser both need a browser. `check` finds
Chromium on `PATH` by itself; otherwise point `MIRZAM_CHROMIUM` at one. The
brand rasteriser and `scripts/check-layout.mjs` — the same checks driven through
a tab that stays open, which is what a screenshot or a recording needs — want
`npm i playwright-core && npx playwright install chromium` on top of that.

## Screenshots, the themes gallery and the demo

Three scripts, one piece of browser plumbing. `scripts/lib/deck-browser.mjs`
builds a deck, opens it, waits until the images and fonts have actually settled,
and either measures it or photographs it; `check-layout.mjs`, `shoot-slides.mjs`
and `make-theme-gallery.mjs` are the three things worth doing with that. All of
them want `npm i playwright-core && npx playwright install chromium`, and
`MIRZAM_CHROMIUM` if the browser is not on `PATH`.

### One PNG per slide

```bash
node scripts/shoot-slides.mjs --build examples/pitch.md -o shots
node scripts/shoot-slides.mjs out/index.html -o shots --slide 3 --mode dark
```

Each slide is shown in its **resting** state — entrance animations finished,
click steps taken, connectors redrawn — which is the state a reader without
JavaScript ends on and the only one a still can honestly be about. `--theme`,
`--mode`, `--split` and `--fit` are passed through to the build, `--slide` takes
slide numbers from 1, and `--no-chrome` drops the viewer's page counter for a
picture that is standing in for something else.

### The themes gallery

```bash
node scripts/make-theme-gallery.mjs -o site/themes
```

`scripts/gallery/specimen.md` is one slide carrying a heading, body text, a list,
a code block, a metric and a chart — one of everything a theme decides. The
script builds it once per theme and per mode, runs `mirzam check` over each
rendering, photographs it, and writes the page. Nothing on the gallery is typed
by hand, so it cannot drift from the stylesheets, and a theme whose type no
longer fits fails the site build instead of shipping a clipped heading.
`build-site.sh` calls it and publishes the result at `/themes/`; a machine
without playwright-core skips it and the landing page then omits the card.

### Recording the demo

A screen recording of a slide tool is the one piece of documentation that cannot
be written, and it is the piece most likely to come out badly — a hesitation
before a keypress, a cursor crossing the slide, a pause of the wrong length.
None of that is a recording problem; it is a *performing* problem, and a script
does not hesitate.

```bash
node scripts/record-demo.mjs --editor -o media/edit-loop --gif   # the edit loop
node scripts/record-demo.mjs --build examples/pitch.md -o media/pitch --gif
```

**`--editor` is the one the README carries.** It builds the WASM package, serves
`web/wasm-demo` over HTTP — `.wasm` cannot be loaded over `file://` — and types
a small deck into the editor while the preview rebuilds beside it: a title, an
ASCII pane grid becoming a layout, a chart forming out of three lines of CSV,
and a `theme:` line changing the deck's whole face. The typing *is* the content.
A viewing of a finished deck is something every slide tool can show; source
becoming slides as it is typed is not.

Without `--editor` it plays a *built* deck instead — every slide held, every
click step taken, the layout overlay and dark mode shown once each. Keypresses
appear as a chip at the bottom of the frame, because a viewer cannot see a
keyboard and a deck that advances by itself demonstrates nothing.

Either run is reproducible, which is the part worth having: change a theme,
re-run it, and the demo is the tool as it is today rather than as it was the
afternoon someone had time to record it.

| Flag | |
|---|---|
| `--editor` | record the edit loop instead of a built deck |
| `--gif` | also write a GIF (see below) |
| `--cadence 1` `--line-pause 150` | `--editor`: typing speed, and the beat at the end of a line. The pause has a floor — the editor rebuilds 120 ms after the last keystroke, and a shorter one is a pause the preview never notices |
| `--dwell 2.2` `--step 1.1` | seconds a slide, and a click step, is held |
| `--width` `--height` | frame size, default 1280×720 (1500×760 with `--editor`) |
| `--fps` `--gif-width` | GIF size levers, default 12 fps at 800px (10 at 1000 with `--editor`) |
| `--gif-colors` `--gif-dither` | palette size and dithering, default 256 and `bayer:bayer_scale=3` (128 and `none` with `--editor`, whose panels are flat) |
| `--no-keys` | drop the keypress chips |

**The GIF needs a real ffmpeg.** Playwright ships one beside its browsers and it
records the video, but it is a stripped build with two encoders and a dozen
filters — no `palettegen`, no GIF encoder at all. The script checks what a
candidate ffmpeg can *do* rather than whether it exists, so a missing one is
reported before the recording rather than as a filter-graph error after it, and
it prints the two-pass command if you would rather convert by hand. GitHub does
not render a committed `.webm` inline, so a GIF is what a README can actually
show — and it renders one inline only up to 10 MB, which is what the size levers
above are for.

`media/edit-loop.gif` is the only recording in the repository, and
`.github/workflows/demo.yml` re-records it whenever something that changes what
it shows changes: the editor page, the recording script, the WASM bindings or
the shared theme CSS. It fails the run if the result goes over 8 MB. The `.webm`
is kept out of git — it is an artifact of that workflow.

## Repository layout

```
crates/
  mirzam-syntax    source decomposition: frontmatter, transclusion, slide splitting,
                   pane divs, fenced blocks
  mirzam-core      deck metadata and the {{ }} expression evaluator
  mirzam-layout    ASCII pane grid -> proportional CSS grid
  mirzam-shape     shape DSL -> SVG layer
  mirzam-chart     chart DSL + CSV -> SVG
  mirzam-connect   connector DSL -> JSON for the runtime
  mirzam-render    assembles slides into HTML; owns the theme and viewer runtime
  mirzam-cli       build/serve/export commands, the build pipeline, benchmark
  mirzam-wasm      wasm-bindgen bindings over the same pipeline
editors/vscode     live preview extension (webview runs the WASM core)
web/wasm-demo      browser playground for the core
examples/          sample decks, themes and data, also used as test fixtures
docs/brand/        mark, palette and type for the README and the site
media/             the README's demo recording; regenerated by CI, not by hand
scripts/           build helpers
scripts/lib/       browser plumbing the checker, the screenshot pass and the
                   gallery share
scripts/gallery/   the one-slide theme specimen the gallery is generated from
```

Data flows one way: `syntax` decomposes text, `core` resolves metadata and
variables, `layout`/`shape`/`chart`/`connect` turn blocks into geometry, and
`render` assembles HTML. Nothing below `render` knows about HTML except as an
output format.

### I/O is injected, never assumed

`mirzam-syntax::FileProvider` and `mirzam-render::AssetSource` abstract file and
asset reads. The CLI supplies filesystem implementations; the WASM build supplies
host-provided tables. Anything that reaches for `std::fs` inside the core crates
will break the browser build, so route it through these traits.

`mirzam-render::DiagramRenderer` is the same arrangement for the other thing a
core crate may not do: **run a program.** A `mermaid` fence is drawn by
`mmdc`, which the CLI finds and spawns (`mirzam-cli/src/mermaid.rs`) and the
browser build cannot, so there the trait has no implementation at all and the
fence stays a code block. A host with no renderer passes `None`, which is an
ordinary state and not an error.

## Quality gates

`cargo test --workspace` runs all of these; CI runs them on every push.

| Gate | Location | What it protects |
|---|---|---|
| CommonMark compatibility | `tests/commonmark_compat.rs` | Extensions must degrade to harmless code blocks and readable text in a plain parser. It walks `mirzam_syntax::BLOCK_KINDS`, so a new block form is covered the moment it is added to that list |
| Golden snapshots | `tests/golden.rs`, `tests/snapshots/` | Rendered output of every sample deck; data URIs are normalized to their length |
| Incremental equivalence | `tests/incremental.rs` | An incremental build equals a full rebuild, and only affected slides re-render |
| Benchmark | `bin/mirzam-bench` | Edit latency stays flat as decks grow; values are logged, not asserted |
| Layout check | `scripts/check-layout.mjs` | Renders every sample deck in a browser and fails on what HTML snapshots cannot detect: clipped or overlapping content, an undrawn connector or annotation, an element left in its entrance state after its animation has played, and a debug overlay left baked in |
| Lint | CI | `cargo fmt --check` and `cargo clippy` with warnings denied |

When you intentionally change rendered output:

```bash
cargo test -p mirzam-cli --test golden        # look at the reported diff first
MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden
```

Review the snapshot diff in your commit like any other change — that is the point
of checking them in.

## Working with coding agents

[AGENTS.md](../AGENTS.md) is the entry point for agents: non-negotiables,
definition of done, crate ownership, and which work streams can run in parallel
without colliding. `CLAUDE.md` points there too. Keep it in sync when the build
commands or quality gates change — an agent that follows a stale checklist will
confidently produce work that fails CI.

That is an agent changing *Mirzam*. For an agent writing a *deck* with it,
[agents.md](agents.md) is the contract: the versioned JSON schema of
`mirzam check --format json`, and [llms.md](llms.md), the syntax card the site
publishes as `/llms.txt`. Both are documentation of a promise — a field in that
schema may be added but never renamed — so change them the way you would change
an API.

## Conventions

- **English everywhere in the repository**: code comments, identifiers, commit
  messages, CLI output, documentation. Japanese translations live under
  `docs/ja/` and are a convenience, never the source of truth.
- Comments explain *why*, not *what*. The surrounding code already says what.
- Every new syntax feature needs three things: a parser test, a line in
  `docs/syntax.md`, and an appearance in the deck that owns it —
  `examples/04-components.md` for most forms, `examples/07-charts.md` for
  anything a `chart` block reads.
- Anything that affects how content is placed also needs a rule in
  `docs/layout.md` and a slide in `examples/03-layout.md`, so the guidance is
  verified by the layout checker rather than merely asserted.
- New rendering behavior means updating the golden snapshots deliberately.

## Adding a syntax feature

1. Recognize the block or inline form in `mirzam-syntax` (or, for content that
   lives inside a pane, in the renderer's extraction pass — see how `chart` blocks
   are handled in `crates/mirzam-render/src/charts.rs`).
2. Put the semantics in a dedicated crate if it has any: parsing, validation and
   geometry belong away from `render`.
3. Emit HTML/SVG from `mirzam-render`, styling it through theme variables rather
   than hard-coded colors.
4. Add it to `examples/04-components.md` — or to `examples/07-charts.md` if it
   is part of the `chart` block — and refresh the snapshots.
5. If it is a fenced block, add its name to `mirzam_syntax::BLOCK_KINDS` and a
   sample body to `sample_block` in `tests/commonmark_compat.rs`. The promise
   that plain Markdown still reads is only kept for forms that list is aware
   of.

## Versioning

Mirzam is pre-1.0 and versioned as `0.MINOR.PATCH`, with all crates sharing the
workspace version.

- **`0.x` means the markup can change.** Breaking syntax changes are allowed in a
  minor bump, and they must be listed in [`CHANGELOG.md`](../CHANGELOG.md).
- **Which digit moves is read off the changelog**, so it is not a judgement call
  made twice: `[Unreleased]` carrying an `### Added`, `### Changed` or
  `### Removed` is a minor bump, because any of the three can move the markup
  under someone; a section that is only `### Fixed` is a patch.
  `scripts/release.sh` applies this rule and says so when the version it is
  given disagrees — it does not refuse, because the author may know something
  the section headings do not.
- A release is cut by bumping `[workspace.package] version` in the root
  `Cargo.toml`, updating the changelog, and tagging `vX.Y.Z`. Pushing the tag
  runs `.github/workflows/release.yml`, which builds `mirzam` for five targets,
  smoke-tests each native one by building a deck with it, and publishes the
  archives with that version's changelog section as the notes. The tag must
  equal `v` plus the workspace version — the release build checks, because
  `scripts/install.sh` derives the download URL from the tag.
- The VS Code extension carries its own version but is released alongside the
  core; its `package.json` version matches the tag it was built from.
- Nothing is published to crates.io yet. Depend on a git tag or commit.
- `1.0` is reserved for the point where the markup is stable enough that decks
  written today will keep rendering. See the roadmap for what still has to land
  first.

### When to release, and why not more often

**A release is not what happens when a feature lands.** Pushing to `main` builds
nothing for anyone to download; only a tag, or the Release workflow run with
`publish`, does that. So the two are already decoupled, and the question is only
when it is worth asking people to upgrade.

Release when there is **a reason to upgrade**, not when there is a change:

- a fix for something that stops someone getting work done — immediately, as a
  patch;
- enough accumulated features that the changelog reads like news — batched, on
  the order of a month;
- never merely because a stream finished.

Batching is not about the ten files a release attaches; GitHub keeps those
happily and nobody is counting. It is about the two things that *are* expensive
per release and are done by hand: writing a changelog section someone would
actually read, and checking that the documentation still describes the software.
Doing that well four times a year beats doing it carelessly forty.

The counterweight is that **there is already a continuous release**: every push
to `main` republishes the docs site and the browser editor, so anyone who wants
the newest behaviour has it without a version number. Tagged binaries are for
people who want a fixed thing, and those people are not helped by frequency.

### `main` is the development branch

There is no `develop`. A long-lived integration branch earns its cost when
`main` must stay releasable at every commit — because releases are cut
continuously, or because hotfixes have to bypass in-flight work. Neither is true
here: nothing is released without someone asking for it, and every push to
`main` already runs the full gate before it lands anywhere a reader can see.

Adding one would buy a merge on every change and a second CI surface, in
exchange for a property `main` already has.

The worry a `develop` branch is usually reaching for — *did the documentation
keep up?* — is not a branching problem, and a branch would not have caught any
of the times it went wrong here. What catches it is machinery: the dead-link
check in `scripts/build-site.sh`, `commonmark_compat.rs` walking
`BLOCK_KINDS` so a new block form cannot ship undocumented and unchecked, the
contrast test reading the shipped CSS rather than a copy of it. When
documentation drifts from code, the fix is another check of that kind, not
another branch.

Use a branch when work is genuinely speculative, or when two agents are editing
the same crate — which is what the ownership tables in
[workstreams.md](workstreams.md) exist to prevent.

## Release checklist

```bash
./scripts/release.sh 0.5.0            # or --dry-run first, to see the edits
```

That is steps 1 to 5 below, in one command: it writes the version into the five
files that carry it, closes `[Unreleased]` into a dated section with a fresh
empty one above it, and runs the gate. It stops there — it does not commit,
push or tag, because those are the steps worth looking at.

1. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets` and
   `cargo test --workspace` are clean
2. `cargo run --release -p mirzam-cli --bin mirzam-bench` shows no regression.
   **Read the second run**: the first pays for a cold cache and can report a
   full build several times slower than the table in [the
   roadmap](roadmap.md#measured-performance) without anything being wrong. What
   has to stay flat is single-slide edit latency, not full-build time
3. Sample decks build and pass `mirzam check`: `01-start`, `02-writing`,
   `03-layout`, `04-components`, `05-motion`, `06-theming`, `07-charts`,
   `pitch`, `seminar`
4. `./scripts/build-vsix.sh` succeeds — it builds the WASM package on its way to
   the extension, so it covers `build-wasm.sh` too
5. `CHANGELOG.md` closed, and the version written in the root `Cargo.toml`,
   `editors/vscode/package.json`, `Cargo.lock`, and the status sentence in both
   `README.md` and `docs/roadmap.md`. `./scripts/check-versions.sh` is the same
   check, and CI runs it on every push
6. **Read what you are about to release.** The script writes the version
   number; nothing writes the prose. Read the dated changelog section as a
   stranger would, and look at `/next/` — it is the release candidate, rendered
7. **Land the commit on `main`.** A release is cut from `main`, so a bump
   sitting on a branch cannot be tagged. `git commit -am "Release vX.Y.Z"`,
   saying in the body why this bump rather than what changed, then `git push
   origin HEAD:main`
8. Cut the release, either way round:
   - `git tag vX.Y.Z && git push origin vX.Y.Z`, or
   - run the **Release** workflow with `publish` checked, which makes the tag
     from the manifest version and needs no local git at all:
     `gh workflow run release.yml --ref main -f publish=true`, the Actions tab,
     or a `POST` to `/actions/workflows/release.yml/dispatches`.

   **An agent takes the second route**, always: pushing a tag is refused for
   credentials scoped to branches (403), and there is nothing to be gained by
   finding that out again. Either way the workflow publishes the binaries, and
   until it is green there is nothing for `install.sh` to fetch. Running it with
   `publish` unchecked builds the whole matrix and cuts nothing, which is how to
   find out that a runner image has been retired *before* a tag is waiting on it
9. Check the site rebuilt. Publishing the release triggers **Pages**, which is
   what moves the root onto the new tag — but the CI run for the version-bump
   commit *also* triggers it, a minute earlier, when the tag does not exist
   yet. That earlier run rebuilds the root from the previous release, so until
   the release-triggered one lands the front page is a version behind. If it
   did not fire, run Pages by hand.

### The tag is what the public sees

`ayatough.github.io/Mirzam/` is built from the latest tag;
`ayatough.github.io/Mirzam/next/` is built from `main`. So a release is not only
a version number — it is the moment months of work on `main` become the page a
stranger lands on. Two consequences worth holding on to:

- **Look at `/next/` before tagging.** It is the release candidate, rendered.
- **Do not let the gap grow too wide.** The longer the root lags `main`, the
  more the documentation people actually read describes a version that no
  longer exists. Somewhere around three or four unreleased features, or any
  markup change, is the point to cut one.
