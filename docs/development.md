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
- Chromium or Chrome (only for `mirzam export pdf`)

## Common tasks

```bash
cargo run --bin mirzam -- build examples/pitch.md -o out
cargo run --bin mirzam -- serve examples/showcase.md

cargo run --release -p mirzam-cli --bin mirzam-bench   # performance benchmark
./scripts/build-wasm.sh                                # WASM package into pkg/
./scripts/serve-wasm-demo.sh                           # browser playground
./scripts/build-vsix.sh                                # VS Code extension

node scripts/check-layout.mjs --build examples/pitch.md   # layout validation
./scripts/build-site.sh                                  # docs site + live decks
node scripts/make-brand-raster.mjs                       # docs/brand/ social card + icon PNG
```

The layout checker and the brand rasteriser both need a browser: `npm i
playwright-core && npx playwright install chromium`, or point
`MIRZAM_CHROMIUM` at an existing Chromium.

## Recording a demo

A screen recording of a slide tool is the one piece of documentation that cannot
be written, and it is the piece most likely to come out badly — a hesitation
before a keypress, a cursor crossing the slide, a pause of the wrong length.
None of that is a recording problem; it is a *performing* problem, and a script
does not hesitate.

```bash
node scripts/record-demo.mjs --build examples/pitch.md -o media/pitch --gif
```

It builds the deck, drives it in a browser — every slide held, every click step
taken, the layout overlay and dark mode shown once each — and writes a `.webm`.
Keypresses appear as a chip at the bottom of the frame, because a viewer cannot
see a keyboard and a deck that advances by itself demonstrates nothing.

The run is reproducible, which is the part worth having: change a theme, re-run
it, and the demo is the deck as it is today rather than as it was the afternoon
someone had time to record it.

| Flag | |
|---|---|
| `--gif` | also write a GIF (see below) |
| `--dwell 2.2` `--step 1.1` | seconds a slide, and a click step, is held |
| `--width` `--height` | frame size, default 1280×720 |
| `--fps` `--gif-width` | GIF size levers, default 12 fps at 800px |
| `--no-keys` | drop the keypress chips |

**The GIF needs a real ffmpeg.** Playwright ships one beside its browsers and it
records the video, but it is a stripped build with two encoders and a dozen
filters — no `palettegen`, no GIF encoder at all. The script checks what a
candidate ffmpeg can *do* rather than whether it exists, so a missing one is
reported before the recording rather than as a filter-graph error after it, and
it prints the two-pass command if you would rather convert by hand. GitHub does
not render a committed `.webm` inline, so a GIF (or a still linking to a hosted
video) is what a README can actually show.

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
scripts/           build helpers
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

## Conventions

- **English everywhere in the repository**: code comments, identifiers, commit
  messages, CLI output, documentation. Japanese translations live under
  `docs/ja/` and are a convenience, never the source of truth.
- Comments explain *why*, not *what*. The surrounding code already says what.
- Every new syntax feature needs three things: a parser test, a line in
  `docs/syntax.md`, and an appearance in `examples/showcase.md`.
- Anything that affects how content is placed also needs a rule in
  `docs/layout.md` and a slide in `examples/cookbook.md`, so the guidance is
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
4. Add it to `examples/showcase.md` and refresh the snapshots.
5. If it is a fenced block, add its name to `mirzam_syntax::BLOCK_KINDS` and a
   sample body to `sample_block` in `tests/commonmark_compat.rs`. The promise
   that plain Markdown still reads is only kept for forms that list is aware
   of.

## Versioning

Mirzam is pre-1.0 and versioned as `0.MINOR.PATCH`, with all crates sharing the
workspace version.

- **`0.x` means the markup can change.** Breaking syntax changes are allowed in a
  minor bump, and they must be listed in [`CHANGELOG.md`](../CHANGELOG.md).
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

1. `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean
2. `cargo run --release -p mirzam-cli --bin mirzam-bench` shows no regression
3. Sample decks build and pass the layout check: `pitch`, `showcase`, `cookbook`,
   `seminar`, `media`, `motion`
4. `./scripts/build-wasm.sh` and `./scripts/build-vsix.sh` succeed
5. `CHANGELOG.md` updated, version bumped in the root `Cargo.toml` and in
   `editors/vscode/package.json`
6. Cut the release, either way round:
   - `git tag vX.Y.Z && git push origin vX.Y.Z`, or
   - run the **Release** workflow with `publish` checked, which makes the tag
     from the manifest version and needs no local git at all. Use this when the
     credentials to hand are scoped to branches — a CI job, an agent, a machine
     that is not yours.

   Either way the workflow publishes the binaries, and until it is green there
   is nothing for `install.sh` to fetch. Running the workflow with `publish`
   unchecked builds the whole matrix and cuts nothing, which is how to find out
   that a runner image has been retired *before* a tag is waiting on it.
