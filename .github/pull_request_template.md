Thanks for this. Two questions before the diff, from
[CONTRIBUTING.md](../CONTRIBUTING.md):

- **Is there an issue for it?** Small fixes need none. Anything with a design
  decision in it — new syntax, a new block form, a change to how something is
  laid out — should have been discussed first, because the syntax is the part
  that is expensive to get wrong.
- **Does the gate pass?** `cargo test --workspace`, `cargo clippy --workspace
  --all-targets`, `cargo fmt --all -- --check`. For anything visual, also
  `node scripts/check-layout.mjs --build examples/pitch.md` — and please look at
  what it rendered.

**What this changes, and why.** The why is the part that is hard to recover
later; the diff already says the what.

**How you know it works.** Not which tests exist — what you did to convince
yourself. If it is visual, say what you looked at.
