# Troubleshooting

Four things people actually get stuck on, in the order they tend to hit them.
If your problem isn't here, `mirzam check deck.md` (or, for contributors,
`node scripts/check-layout.mjs`) renders the deck and tells you exactly which
slide and what's wrong — see [Layout guide § Checking a
deck](layout.md#checking-a-deck).

## A slide doesn't fit

Do these in order. Each one costs more than the last, so stop as soon as the
slide looks right.

1. **`--fit shrink`.** Costs nothing to try — it needs no edit to the deck at
   all.
   ```bash
   mirzam build deck.md --fit shrink        # or --split h2 --fit shrink for a converted document
   ```
   or `fit: shrink` in frontmatter (every pane), or `{fit=shrink}` on one
   pane. It scales the pane's text down in small steps, to a floor of 55%,
   re-measured on every resize — and it runs in the PDF too. **If a pane is
   shrinking a lot, that's the deck telling you the slide has two slides'
   worth on it** — treat a large shrink as a sign to do the next step, not as
   the fix.
2. **You're actually restructuring — pick one:**
   - **Widen the band, or move the content to another pane.** The ASCII
     `pane` grid is the specification: if a pane needs more room, redraw it
     wider or taller. See [Layout guide § When content does not
     fit](layout.md#when-content-does-not-fit).
   - **`<!-- next -->` inside the pane**, at the sentence or bullet where the
     break belongs. That pane's content becomes two slides; every other pane
     — the figure, the heading — renders in place on both, and the viewer
     *cuts* between them, so the audience sees only the one pane's text
     change. Full rules: [syntax reference § Carrying one pane on to the next
     slide](syntax.md#carrying-one-pane-on-to-the-next-slide).

Without `fit: shrink`, a pane clips silently — the layout you drew is kept
exactly, but the overflow is invisible until you look or run `mirzam check`.
That's the documented fallback, not a bug: nothing about the syntax warns you
while you're typing.

## Something rendered as literal text instead of the feature you wrote

Every Mirzam extension is designed to degrade to plain text in a parser that
doesn't know it — which means a *typo* degrades exactly the same way a
*correct plain-Markdown-on-purpose* passage would. Before assuming a feature
is broken, check the three limits that produce this most often:

- **An attribute span has to be on one source line.** `[text]{.small}` split
  across a line break is not recognised; rewrap the sentence or split it into
  two spans. This applies to `{#id .class}` on headings, images and spans
  alike.
- **`shape` only parses at slide top level**, never inside `::: pane`. Written
  inside a pane, the fence reaches the Markdown renderer untouched and shows
  as an ordinary code block. `mirzam build` warns about this one (see below)
  — the fix is to move the block out of the pane, after the `:::` that closes
  it.
- **A footnote's `[^key]:` definition has to be on the same slide as its
  `[^key]` reference** — each slide renders on its own, so a definition left
  on another slide (or, in a `pane` grid layout, a *different pane* of the
  same slide) never reaches it, and the reference is left as literal bracket
  text. `mirzam build` warns about this one too.

If none of those explain it, check whether the block is nested inside a
*longer* fence (four backticks around three): that's how a document quotes
Mirzam syntax as an example rather than using it, and it's meant to render as
a code block.

## Getting a PDF

```bash
mirzam export pdf deck.md -o deck.pdf
```

`export pdf` takes the same `--split`, `--theme`, `--css`, `--fit` and
`--mode` flags `build` does, so a deck assembled with one of them — most
commonly a document turned into a deck with `--split` — exports with the
same slide breaks and identity in one command:

```bash
mirzam export pdf notes.md --split h2 --theme mirzam --fit shrink -o notes.pdf
```

It needs headless Chromium (`--chromium <path>`, or set `MIRZAM_CHROMIUM`, or
have `chromium`/`google-chrome`/`chrome` on `PATH`). It always reads the
Markdown **source** — never point it at a built `out/index.html`: that used
to "succeed" by re-parsing the HTML as Markdown, silently producing a
title-only PDF, so it's a hard error now, naming this same command.

A deck whose stylesheet rests in dark mode should set `mode: dark` in
frontmatter (or pass `--mode dark`): the PDF has no reader to ask, so it
follows the deck's declared mode and falls back to light, pairing a
dark-resting stylesheet's own colours with a `bg-light=` image otherwise.

## Build warnings, and what they mean

`mirzam build`, `mirzam export pdf` and `mirzam check` all print every
warning the build produced, one line each: `  ⚠ <message>`. A warning from a
slide that came in through `![[included.md]]` gets that file's name appended
— `(in included.md)` — so you know which document to open. **A build that
prints warnings still succeeds**; add `--strict` to `mirzam build` to fail
instead (non-zero exit), for a CI gate that catches these before they ship.

Three kinds of thing happen when a warning fires — worth knowing which,
since it changes how urgent the fix is:

| | Meaning |
|---|---|
| **Note only, nothing else changes** | The slide renders exactly as it would without the warning; the message is purely informational. |
| **The one block is dropped** | That `anim`, `annotate` or `effects` block does not run for this slide at all — the content is still there, just unanimated/unannotated/without its key bindings. |
| **Shown on the slide too** | A `<div class="mz-error">⚠ …</div>` appears where the broken block would have rendered, *and* the same message is printed by the CLI — you'll see it twice. |

| Warning (paraphrased) | Category | Cause | What happens |
|---|---|---|---|
| pane "x" contains a shape block, but shape only renders at slide top level | Shape | A `shape` fence sits inside `::: pane` | Note only — still renders as a code block |
| footnote reference "[^key]" has no definition on this slide | Citations | `[^key]` with no `[^key]:` on the same slide | Note only — bracket text stays literal, once per key |
| connect endpoint "#id" matches nothing on this slide | Connectors | A `connect` id matches no text anchor, shape, `annotate` mark, or chart on the slide | Note only — the connector is still emitted; the viewer just draws no arrow |
| pane "x" is not in the layout | Layout | `::: pane x` names a pane the `pane` grid doesn't define | Shown on the slide too |
| a pane block needs … / the merged region for pane "x" is not rectangular | Layout | Malformed ASCII `pane` grid | Shown on the slide too |
| `bg-light`/`bg-dark` needs `bg=` … alongside it | Layout | Only one per-mode background given, with no `bg=` fallback for the other mode | Shown on the slide too (no `slide N:` prefix — the only structural error that lacks one) |
| shape line N: … (unknown kind, bad `at()`/`size()`, unclosed paren, unknown id) | Shape | Malformed top-level `shape` DSL | Shown on the slide too |
| connect line N: … (missing operator, endpoint not written as `#id`) | Connectors | Malformed `connect` DSL line | Shown on the slide too |
| chart: cannot parse block / cannot read data file / no data rows / row-level CSV errors | Charts | Malformed `chart` YAML, an unreadable `data:` file, or bad CSV | Shown on the slide too |
| anim line N: … (missing target, bad step number, unknown ease, …) | Animations | Malformed `anim` DSL — the message names the exact problem | Whole `anim` block dropped |
| anim target "…" matches nothing on this slide / anim trigger references an id that doesn't exist | Animations | Target or `[after #id]` doesn't resolve | Whole `anim` block dropped |
| cannot split … / a target is split by more than one track | Animations | `target.split` used on the whole slide, on something with no closing tag, or twice on one element | Whole `anim` block dropped |
| annotate line N: … (empty target, bad coordinates, unknown attribute, …) | Annotations | Malformed `annotate` DSL — the message names the exact problem | That `annotate` block dropped (others on the slide still run) |
| annotate target "…" matches nothing on this slide / annotate anchors an id that doesn't exist | Annotations | `target:` or an anchored `#id` doesn't resolve | That `annotate` block dropped |
| effects line N: … (key bound twice, not a single key, taken by the viewer, unknown effect, needs an argument, …) | Effects | Malformed `effects` DSL — the message names the exact problem | Whole `effects` block dropped |
| path: file not found / larger than 20MB, not inlined | Assets | An image/audio/video `src=` doesn't resolve, or exceeds the inline size limit | Note only — a placeholder "missing" graphic is substituted (no `slide N:` prefix) |
| unknown theme "x"; using default | Theme | Frontmatter `theme:` isn't a built-in name | Note only — falls back to `default` |
| unknown mode "x"; expected light or dark | Theme | Frontmatter `mode:` isn't `light`/`dark` | Note only — falls back to following the reader's machine |
| slide N, pane "x": unknown theme "y"; keeping the surrounding theme | Theme | A pane's `{theme=y}` — or a slide's `<!-- theme: y -->` — isn't a built-in name | Note only — that pane or slide keeps the theme it inherits |
| slide N, pane "x": unknown mode "y"; expected light or dark | Theme | A pane's `{mode=y}` or a slide's `<!-- mode: y -->` isn't `light`/`dark` | Note only — that pane or slide follows the deck's mode |
| math: unknown dialect "x"; latex and typst are supported | Math | Frontmatter `math:` isn't `latex`/`typst` | Note only — renders as `latex` |
| transition: … | Frontmatter | Frontmatter `transition:` doesn't parse | Note only — deck falls back to plain cuts |
| css: cannot read path | Frontmatter | Frontmatter `css:` file can't be read | Note only — builds without the custom stylesheet |
| no slides: file is empty / … has nothing outside its frontmatter | Frontmatter | Nothing to render | Note only — builds as a blank page |
| `<!-- next -->` appears in more than one pane | Frontmatter | Two panes on one slide both try to break | Note only — the slide renders whole, unsplit |

Two messages appear **on the slide itself as a quoted line**, never as a
`⚠` line from the CLI: `circular include, not expanded: <target>` and
`include failed: <error>`, from `![[...]]` transclusion gone wrong. Look at
the slide, not the terminal, for those two.

This list moves when the checks do; if a message doesn't match anything
above, trust what it says over this page and open an issue.
