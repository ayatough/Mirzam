# Third-party usability evaluation — August 2026

> **Re-verified against v0.3.0** (see addendum at the end): the release
> resolves several documentation and onboarding findings, but every problem
> in "Reproduced problems" below still reproduces, and recommendations 1–3
> remain open.

Four first-time-user personas (subagents restricted to README, `docs/` and
`examples/`) each built a deck with the prebuilt CLI, then reported on the
experience. Every finished deck was then verified independently: rendered,
screenshotted slide by slide, and run through `scripts/check-layout.mjs`.
Problems below were reproduced from minimal cases, not taken from the
personas' reports on faith.

| Persona | Task | Features exercised |
|---|---|---|
| Business developer | 9-slide investor pitch | panes, charts, shapes, metric tiles |
| Engineer (Marp user) | 8-slide tech talk | code, shape+connect, anim, notes |
| Grad student (LaTeX user) | 10-slide research talk in Japanese | toc, math, citations, charts, CJK |
| Non-technical PM | Weekly memo, unedited, via `--split` | `--split`, `--theme`, `--fit`, PDF |

## Headline

Everyone reached a working deck in about ten minutes and rated learnability
and expressiveness highly — and **all four shipped a deck containing a visual
defect they had not noticed.** Where a warning existed (orphan pane, anim
target, `bg-dark` alone) every persona fixed the problem unaided; the gap is
warning *coverage*, not warning quality. "Build succeeded" is not "the deck
is right", and nothing in the product closes that gap for a user who does not
inspect every slide.

## Reproduced problems

**Silent degradations (no warning at build time):**

1. A `shape` block inside `::: pane` renders as a literal code block —
   shapes only work at slide top level, which no doc states. One persona
   shipped its "roadmap diagram" as source code on the slide.
2. A footnote definition on a different slide leaves the `[^key]` reference
   as literal text. `docs/syntax.md` shows same-slide definitions but never
   says they are required; the LaTeX-habit persona put them at the end.
3. A `connect` referencing an id that does not exist draws nothing — only
   the browser-side checker notices. Two decks shipped underlined phrases
   whose arrows never appear.
4. An attribute span broken across lines (known in AGENTS.md) rendered a
   whole image reference as literal text; no user-facing doc mentions it.
5. `.metric` / `.card` / `.eyebrow` live in `examples/themes/pitch.css`, but
   nothing distinguishes theme classes from renderer classes, so copying
   them from the examples without `css:` yields silently unstyled text.

**PDF export is asymmetric with build:**

- `export pdf` accepts none of `--split` / `--theme` / `--fit`, so a deck
  produced with `--split` has no supported path to a PDF at all.
- `export pdf out/index.html` is accepted and "succeeds", writing a
  one-slide PDF containing only the title — silent loss of the whole deck.
  The PM persona believed this had worked.

**The layout checker is out of users' reach.** It caught every clipped pane
and undrawn connector above, but it needs `cargo run`, a hand-installed
`playwright-core` and the repository root; two personas tried and failed to
run it. The one safety net exists only for contributors.

## Recommendations, in order

1. **Give silent degradation a voice**: build warnings for shape-in-pane,
   undefined footnote keys, unresolved connect ids, and attribute spans that
   never closed; plus `--strict` to fail on warnings in CI.
2. **PDF parity**: teach `export pdf` the build options, and reject `.html`
   input instead of reparsing it as Markdown.
3. **Promote layout checking into the CLI** (`mirzam check deck.md`,
   reusing the `export pdf` Chromium discovery) so binary-install users get
   the same net CI has.
4. **Docs**: state the three constraints above explicitly; add a class
   catalogue separating renderer classes from theme classes; a themes
   gallery (already on the roadmap); a short troubleshooting/FAQ page; a
   table of contents at the top of `syntax.md` (1,000+ lines). One persona
   requested Typst math as a missing feature — it exists and is documented,
   which is a discoverability failure, not a feature gap.
5. **Japanese reach**: `docs/ja/` covers README/quickstart/roadmap only; the
   research-talk persona had to fall back to English for chart/connect
   syntax. Even a chapter-level mapping into `syntax.md` would help, and the
   landing page's Japanese entry point is a single footer link despite
   `seminar.md` being a strong CJK showcase.

## What already works

Sub-10ms rebuilds made trial-and-error painless for every persona; the
"four ways in" quickstart table and the examples-written-in-their-own-markup
scheme were the primary learning path for all four; existing warnings and
the `serve` startup message rated 4–5/5 for clarity. Expressiveness averaged
4.6/5 with no persona blocked by a missing capability. The site's
stable-plus-`/next/` split and CI-gated deploy are sound; its weak points
are prose docs living only as GitHub links and the `/try/` card vanishing
silently when the WASM build fails.

## Addendum: re-verified against v0.3.0 (2026-08-11)

Each finding was re-run against a fresh `origin/main` build (`mirzam 0.3.0`)
using the minimal cases above.

**Resolved by v0.3.0:**

- Typst math is promoted, documented, and renders (`math: typst` → MathML).
- Built-in classes exist for the "styled aside / big number" need
  (`.big`, `.huge`, `.box`, `.accent`, …), and `pitch.css` is gone — the
  theme-class confusion this report described no longer arises in that form.
- The one-line attribute-span limit is documented in the reference.
- An empty deck now warns (`⚠ no slides: <file> is empty`).
- `mirzam new` closes the missing-first-file gap.

**Still reproduce, unchanged:**

1. `shape` block inside `::: pane` → literal code block, no warning.
2. Footnote definition on another slide → literal `[^key]`, no warning.
3. `connect` to a nonexistent id → no warning, arrow silently absent.
4. `export pdf` rejects `--split` (and `--theme`/`--fit`); a `--split` deck
   still has no supported route to PDF.
5. `export pdf out/index.html` still "succeeds" with a one-slide PDF.

There is no `check` subcommand in v0.3.0. Recommendations 1–3 (warn on
silent degradation + `--strict`; PDF parity + `.html` rejection;
`mirzam check`) are the remaining implementation work; recommendation 4 is
partly done (span limit documented, class catalogue improved via built-ins;
still missing: the shape/footnote constraints, a PDF-flow doc, a FAQ, a
`syntax.md` table of contents) and recommendation 5 (Japanese reach) is
untouched.
