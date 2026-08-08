# Development guide

Everything a contributor needs: how to build, what the quality gates check, how the
crates fit together, and how versions are handled.

## Setup

Rust 1.75+ is the only hard requirement.

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
```

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
| CommonMark compatibility | `tests/commonmark_compat.rs` | Extensions must degrade to harmless code blocks and readable text in a plain parser |
| Golden snapshots | `tests/golden.rs`, `tests/snapshots/` | Rendered output of every sample deck; data URIs are normalized to their length |
| Incremental equivalence | `tests/incremental.rs` | An incremental build equals a full rebuild, and only affected slides re-render |
| Benchmark | `bin/mirzam-bench` | Edit latency stays flat as decks grow; values are logged, not asserted |
| Lint | CI | `cargo fmt --check` and `cargo clippy` with warnings denied |

When you intentionally change rendered output:

```bash
cargo test -p mirzam-cli --test golden        # look at the reported diff first
MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden
```

Review the snapshot diff in your commit like any other change — that is the point
of checking them in.

## Conventions

- **English everywhere in the repository**: code comments, identifiers, commit
  messages, CLI output, documentation. Japanese translations live under
  `docs/ja/` and are a convenience, never the source of truth.
- Comments explain *why*, not *what*. The surrounding code already says what.
- Every new syntax feature needs three things: a parser test, a line in
  `docs/syntax.md`, and an appearance in `examples/showcase.md`.
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

## Versioning

Mirzam is pre-1.0 and versioned as `0.MINOR.PATCH`, with all crates sharing the
workspace version.

- **`0.x` means the markup can change.** Breaking syntax changes are allowed in a
  minor bump, and they must be listed in [`CHANGELOG.md`](../CHANGELOG.md).
- A release is cut by bumping `[workspace.package] version` in the root
  `Cargo.toml`, updating the changelog, and tagging `vX.Y.Z`.
- The VS Code extension carries its own version but is released alongside the
  core; its `package.json` version matches the tag it was built from.
- Nothing is published to crates.io yet. Depend on a git tag or commit.
- `1.0` is reserved for the point where the markup is stable enough that decks
  written today will keep rendering. See the roadmap for what still has to land
  first — animation and presenter mode are the main gaps.

## Release checklist

1. `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean
2. `cargo run --release -p mirzam-cli --bin mirzam-bench` shows no regression
3. Sample decks build and look right: `pitch`, `showcase`, `seminar`, `media`
4. `./scripts/build-wasm.sh` and `./scripts/build-vsix.sh` succeed
5. `CHANGELOG.md` updated, version bumped, tag pushed
