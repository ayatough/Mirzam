See [AGENTS.md](AGENTS.md) for how to work in this repository: the
non-negotiables, the definition of done, where each crate's responsibility ends,
and how to split work across several agents without colliding.

Two things worth repeating here because they are easy to skip:

- **Verify visual changes by rendering them.** `node scripts/check-layout.mjs
  --build examples/pitch.md` catches clipped and overlapping content; screenshots
  catch the rest. Do not report a visual fix you have not looked at.
- **Run the full gate before saying you are done**: `cargo test --workspace`,
  `cargo clippy --workspace --all-targets`, `cargo fmt --all -- --check`.
