# Market survey: Mirzam against Marp, Typst and the field — August 2026

Three parallel research passes (Marp; Typst with Touying/Polylux; the wider
text-based presentation market) run in August 2026, condensed into what it
means for the roadmap. The prioritised outcome is at the end; the roadmap
carries the schedule.

## The competitors, briefly

**Marp** (Marpit / Marp Core / Marp CLI / VS Code). CommonMark-first,
CSS-only themes, deliberately thin. 2026 shipped Core v5 (release candidate):
Shiki becomes the default code highlighter, Mermaid diagrams become native —
closing its most-upvoted request after six years. Exports HTML, PDF, PPTX —
but the PPTX is one full-page image per slide; the editable variant is
experimental and needs LibreOffice. Top user complaints, in order: no native
multi-column layout (the team pushes users to raw CSS), the PPTX story,
vertical overflow silently clipping content, Chromium dependency breakage in
CI, fragments that vanish on export, no table of contents.

**Typst** (Touying is the de-facto slide framework; Polylux is the older
one). PDF-first and the PDF is excellent: tagged/accessible (PDF/UA-1),
first-class math, CSL bibliographies, programmable slides — loops that
generate content, which no Markdown tool has. Commercially real: profitable
company, on-premises contracts, a funded full-time engineer. Its structural
ceiling is the format: no video and no animation in PDF (the video issue is
three years old with no owner), HTML export is experimental and Touying
closed its HTML issue as *not planned*, PPTX/HTML exports are rasterised
snapshots. The web app's presentation mode is a paid Pro feature.

**The field.** Slidev owns developer talks but drags a Node toolchain
(400 MB installs, OOM on large decks) and its PPTX is also images-only.
Quarto is absorbing academia. presenterm (Rust, terminal) is the notable
riser. Gamma proved AI generation monetises (~$100M ARR); Tome proved it
doesn't without a wedge (shut down 2025). The visible trend that matters
here: **"AI drafts, human reviews the diff" is becoming a real workflow, and
comparison articles now score slide tools on LLM-friendliness.**

## What the market asks for that Mirzam lacks

In decreasing order of observed demand:

1. **Syntax highlighting in code blocks.** Shiki is now the default in Marp
   and Slidev; presenterm and Typst highlight too. Mirzam rendering code
   uncoloured is the single most visible gap for the developer-talk audience.
2. **Mermaid/D2 diagrams.** Table stakes since GitHub rendered Mermaid
   natively. Every major competitor has it; Mirzam defers it to a plugin
   system that does not exist yet.
3. **PPTX export.** Already on the roadmap as Later. The market's actual
   unmet want is *editable text*, which nobody ships well — Marp and Slidev
   emit images, Touying emits PNGs.
4. **A theme gallery.** Marp and Touying have one; default-design quality is
   the top reason users bounce to Gamma/Canva. (Already Next on the roadmap.)
5. Smaller, recurring: notes-in-handout PDF (Marp CLI's most-reacted issue),
   an overview/grid view (Marp added one in 2026), data-driven slide
   generation (Typst's structural advantage), tagged/accessible PDF (Typst
   0.14 leads), PPTX import.

Bibliographies were on this list when the survey ran; they landed the same
week (`references:` + `[@key]`), so they appear here only as evidence the
academic audience is being taken seriously.

## Where Mirzam already wins, and nobody can follow quickly

- **The source renders on GitHub.** Marp/Slidev/Typst sources degrade into
  directive soup or code in a README. A deck that is also a readable
  document in a PR diff is an unoccupied position — and it compounds with
  the AI-review trend.
- **ASCII grid layout** answers the market's #1 Marp complaint (layout
  without leaving Markdown) declaratively — and an LLM can *see* the layout
  it is emitting.
- **Live connectors and phrase-to-element pairing** exist nowhere else.
- **Animation + video + self-contained HTML together.** Typst structurally
  cannot (PDF); Marp's fragments die on export. Mirzam's "HTML moves, PDF
  shows every step" answer is already better than either.
- **Speed at scale.** 500 slides in 76 ms against Slidev's OOM reputation
  and Marp's >100-slide preview breakage. Large decks (courses, training)
  are an underserved segment.
- **Zero-install editing, phone included.** Typst's presentation mode is
  paywalled; Marp's editor story is VS Code only.
- **CJK typography** is a pile of workarounds in every competitor.

Marketing note, costing no engineering: the comparison table above is the
landing-page story. Multi-column, `fit: shrink`, `toc`, video-in-deck and
phone editing each answer a named, linkable competitor pain point.

## Decision (approved 2026-08)

| Priority | Item | Demand | Effort |
|---|---|---|---|
| P0 | Syntax highlighting, build-time, after the size measurement | strong | small–medium |
| P0 | An authoring contract for agents: machine-readable `check`, a syntax the model can be taught | strong, rising | small–medium |
| P1 | Mermaid (then D2) rendered to SVG at build time, via the CLI, not in core | strong | medium |
| P1 | Theme gallery and a documented theme contract | strong | medium |
| P1 | PPTX export, staged: images+notes first, editable text as the differentiator | strong | large |
| P2 | Handout PDF with notes; overview grid view | medium | small–medium |
| P2 | Data-driven slides (a CSV row per slide) | medium | medium |
| P3 | Tagged (accessible) PDF | medium | large |
| P3 | PPTX import | medium | large |

Explicit non-goals, so the list above stays a list: real-time collaboration
and viewer analytics (Gamma's ground), WYSIWYG editing, executable code
cells (Quarto/Slidev's ground). Mirzam's identity is the version-controlled,
offline, reproducible deck; features that require a server or an account
work against it.
