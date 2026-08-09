# Contributing

Thank you for looking. Please read this before opening a pull request — the
answer for issues and the answer for code are deliberately different here.

## Issues: yes, please

**Bug reports, questions and feature requests are all welcome**, and they are
the most useful thing you can send. A deck that renders wrong, a phrase that
does not read on a phone, a piece of syntax that surprised you: those are hard
to find from the inside, and finding them is real work.

Include the Markdown that reproduces it if you can. A deck is a text file, so a
bug report can usually *be* the deck.

## Pull requests: open an issue first

Not because contributions are unwelcome — because of how this repository is
written.

Mirzam is built by a single author working with an AI assistant, and that shows
in the code: comments explain *why* a thing is the way it is rather than what it
does, tests are named as sentences about behaviour, and visual changes are
verified by rendering them in a browser rather than by asserting HTML. It is a
consistent style, and consistency is most of what makes a small codebase
readable. A patch that is perfectly good on its own terms can still cost more to
absorb than it saves.

So:

- **A fix under about twenty lines** — a typo, a wrong path, an off-by-one, a
  broken link — just send it. Please match the surrounding style.
- **Anything with a design decision in it** — new syntax, a new block form, a
  change to how something is laid out or rendered — **open an issue first** and
  describe what you want to be able to write in a deck. The syntax is the part
  that is expensive to get wrong, because every deck written against it becomes
  a thing that must keep rendering.

An issue costs you far less than a rejected pull request does, and the answer
may be "yes, and here is the shape it should take", which is worth having before
you write anything.

**Branch from `main`, but expect it to move.** The author commits to `main`
directly — see [How this repository is
developed](README.md#how-this-repository-is-developed) — so it is the working
branch rather than a stable one. Rebase before you open a pull request, and do
not be surprised if the file you touched looks different by then. Your changes
still come in as pull requests; the direct-push policy covers the author's own
commits only.

## The bar

Everything below runs in CI on every push, and a pull request is expected to
pass all of it:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets     # with RUSTFLAGS="-D warnings"
cargo fmt --all -- --check
node scripts/check-layout.mjs --build examples/pitch.md   # for visual changes
```

Two of these are unusual and worth knowing about before you start:

- **`commonmark_compat.rs`** enforces the project's central promise: every
  extension must degrade to a harmless code block or readable text in a plain
  CommonMark parser, so a deck still reads on GitHub. A new fenced block goes
  on `mirzam_syntax::BLOCK_KINDS` in the same change, and that test walks the
  list.
- **`check-layout.mjs`** renders every sample deck in a real browser and fails
  on clipped content, an undrawn annotation, an element left in its entrance
  state, and other things HTML snapshots cannot see. If you changed something
  visual, run it — and look at the result.

[AGENTS.md](AGENTS.md) is the fuller version of the working agreement: the
non-negotiables, where each crate's responsibility ends, and what "done" means.
It is written for coding agents, but it is the same bar for everyone, and it is
the most accurate description of the house style there is.

[docs/development.md](docs/development.md) covers the build, the crate layout
and the versioning policy.

## What is planned

[docs/workstreams.md](docs/workstreams.md) is the plan of record. Each stream is
a brief with the reasoning behind it, not just a title — if you want to know why
something is the way it is, or what is coming, that is the file to read.

Issues are the inbox; the workstreams file is what has been accepted and thought
through. An issue that turns into work becomes a stream there.

## License

By contributing you agree that your contribution is licensed under the MIT
license, the same as the rest of the project.
