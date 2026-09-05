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

**A slide that fits here may not fit where you present it.** A deck embeds no
text font, so `check` measures it in whatever the checking machine resolved the
deck's font stack to — and it now says so, along with how little room the
tightest pane had. If the deck is going to be shown on another machine, ask for
a margin rather than for a fit: `mirzam check deck.md --min-slack 24`. See
[Layout guide § What a clean run does and does not promise](layout.md#what-a-clean-run-does-and-does-not-promise).

## The slide is tiny on my phone

A slide is a landscape rectangle; a phone held upright is not. The deck is
fitted into whatever the browser leaves after its address bar and toolbar, and
in portrait that is a strip across the middle of the screen.

| What to do | What it gives you |
|---|---|
| Tap **⛶** in the controls (`F` on a keyboard) | The whole screen, no margin, and a phone asked to turn sideways with the deck |
| Turn the phone sideways | The slide's shape and the screen's agree, and both mobile browsers shrink their own bars in landscape |
| Add the deck to the home screen and open it from there | No address bar, no toolbar — the only full screen Safari on an iPhone has |

On a phone held upright the row keeps the page turns and **⛶**; **⋯** opens the
rest. The **⛶** button is only there when the browser offers full screen at all, so
on an iPhone the third row is the answer; it needs the deck at an `http(s)`
address rather than as a file — publish it, or run
`mirzam serve deck.md --host 0.0.0.0` and type the address it prints. The
shortcut sheet — two-finger tap, or **?** — names whichever of these the
browser you are holding can do.

## Something rendered as literal text instead of the feature you wrote

Every Mirzam extension is designed to degrade to plain text in a parser that
doesn't know it — which means a *typo* degrades exactly the same way a
*correct plain-Markdown-on-purpose* passage would. Before assuming a feature
is broken, check the three limits that produce this most often:

- **An attribute span has to be on one source line.** `[text]{.small}` split
  across a line break is not recognised; rewrap the sentence or split it into
  two spans. This applies to `{#id .class}` on headings, images and spans
  alike. `mirzam build` warns about this one, naming the slide. Brackets
  *inside* a span are fine — a footnote reference, a nested span and inline
  maths all work — so a span that failed is nearly always the line break.
- **`shape` changes coordinate space with where it is written.** At slide top
  level its percentages are of the whole slide; inside a `::: pane` they are
  of that pane's rectangle. A diagram that lands somewhere unexpected is
  usually in the other space from the one its coordinates were written for —
  moving the block in or out of the pane is the fix, not rescaling every
  number. (In earlier releases the in-pane form was a warning and rendered as
  a code block.)
- **A footnote's `[^key]:` definition has to be on the same slide as its
  `[^key]` reference** — each slide renders on its own, so a definition left
  on another slide (or, in a `pane` grid layout, a *different pane* of the
  same slide) never reaches it, and the reference is left as literal bracket
  text. `mirzam build` warns about this one too.
- **`[@key]` is only a citation when the deck has a `bibliography:`.** Without
  one in the frontmatter there is nothing a key could name, so the brackets are
  left exactly as typed rather than turning someone's prose into a reference.
  Same for a key the bibliography does not define — the mark stays as written,
  and the build says which slide it is on.

If none of those explain it, check whether the block is nested inside a
*longer* fence (four backticks around three): that's how a document quotes
Mirzam syntax as an example rather than using it, and it's meant to render as
a code block.

## Getting a PDF

```bash
mirzam export pdf deck.md -o deck.pdf
```

`export pdf` takes the same `--split`, `--theme`, `--fit` and
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
| grid-pad-x: `wide` is not a pixel length | Layout | A `grid-pad-x`/`grid-pad-y`/`grid-gap` value that is not `64px` or `64` | Note only — that key keeps the stylesheet default |
| footnote reference "[^key]" has no definition on this slide | Citations | `[^key]` with no `[^key]:` on the same slide | Note only — bracket text stays literal, once per key |
| slide N: "[text]{.class}" is still on the slide as text | Spans | An attribute span split across a line break | Note only — it stays literal bracket text |
| slide N: the brace over "…" will stop short of it | Math | An `underbrace`/`overbrace` base wider than about 8em, where the browser stops stretching the brace | Note only — the formula renders, with the end of the base uncovered. Move the words into the label |
| `[@key]` is in no bibliography entry | References | A citation key the deck's `bibliography:` does not define | Note only — the mark stays on the slide exactly as written |
| citations: N reference(s) are cited and no `bibliography` block lists them | References | The deck cites but never places a `bibliography` block | Note only — each mark reads, and links nowhere |
| bibliography: nothing to list | References | A `bibliography` block on a deck where no `[@key]` cited an entry | Note only — the block renders as nothing |
| bibliography: cannot read … | References | `bibliography:` names a file that is not there | Note only — every `[@key]` is left as written |
| connect endpoint "#id" matches nothing on this slide | Connectors | A `connect` id matches no text anchor, shape, `annotate` mark, or chart on the slide | Note only — the connector is still emitted; the viewer just draws no arrow |
| pane "x" is not in the layout | Layout | `::: pane x` names a pane the `pane` grid doesn't define | Shown on the slide too |
| a pane block needs … / the merged region for pane "x" is not rectangular | Layout | Malformed ASCII `pane` grid | Shown on the slide too |
| `bg-light`/`bg-dark` needs `bg=` … alongside it | Layout | Only one per-mode background given, with no `bg=` fallback for the other mode | Shown on the slide too (no `slide N:` prefix — the only structural error that lacks one) |
| masters: … / masters: "file" defines none / master "x" is defined more than once | Masters | The `masters:` file can't be read, holds no heading with a `pane` block under it, or names one shape twice | Note only — slides that draw no grid of their own render as a single pane; a duplicate name keeps the last |
| layout: no master named "x" (known: …) | Masters | Deck-wide `layout:` names a shape the masters file doesn't define | Note only — reported once, not once per slide |
| slide N: no master named "x" (known: …) | Masters | `<!-- layout: x -->` names a shape the masters file doesn't define | Note only — that slide renders as it would with no master |
| file.md: its `masters:` names different shapes from the deck's | Masters | A `![[…]]` section declares its own `masters:` | Note only — a transcluded file's frontmatter is not read, so the deck's shapes are used |
| shape line N: … (unknown kind, bad `at()`/`size()`, unclosed paren, unknown id) | Shape | Malformed top-level `shape` DSL | Shown on the slide too |
| connect line N: … (missing operator, endpoint not written as `#id`) | Connectors | Malformed `connect` DSL line | Shown on the slide too |
| chart: cannot parse block / cannot read data file / no data rows / row-level CSV errors | Charts | Malformed `chart` YAML, an unreadable `data:` file, or bad CSV | Shown on the slide too |
| slide N: mermaid: no diagram renderer found, so the block is shown as code | Diagrams | No mermaid-cli on this machine — `npm install -g @mermaid-js/mermaid-cli`, or set `MIRZAM_MMDC` | Note only — the fence renders as a code block, which is what GitHub draws as a diagram anyway |
| slide N: mermaid: mmdc failed (…) | Diagrams | The renderer is installed and rejected the diagram — the message quotes its first line | Note only — the fence renders as a code block, with the diagram's source intact |
| anim line N: … (missing target, bad step number, unknown ease, …) | Animations | Malformed `anim` DSL — the message names the exact problem | Whole `anim` block dropped |
| anim target "…" matches nothing on this slide / anim trigger references an id that doesn't exist | Animations | Target or `[after #id]` doesn't resolve | Whole `anim` block dropped |
| cannot split … / a target is split by more than one track | Animations | `target.split` used on the whole slide, on something with no closing tag, or twice on one element | Whole `anim` block dropped |
| annotate line N: … (empty target, bad coordinates, unknown attribute, …) | Annotations | Malformed `annotate` DSL — the message names the exact problem | That `annotate` block dropped (others on the slide still run) |
| annotate target "…" matches nothing on this slide / annotate anchors an id that doesn't exist | Annotations | `target:` or an anchored `#id` doesn't resolve | That `annotate` block dropped |
| effects line N: … (key bound twice, not a single key, taken by the viewer, unknown effect, needs an argument, …) | Effects | Malformed `effects` DSL — the message names the exact problem | Whole `effects` block dropped |
| toc: unknown key … / from … / depth … must be 1 to 6 / current … must be true or false / … is not `key: value` | Contents | Malformed `toc` block | Shown on the slide too |
| bibliography: unknown key … / show … must be `cited` or `all` / back … must be true or false / … is not `key: value` | References | Malformed `bibliography` block | Shown on the slide too |
| path: file not found / larger than 20MB, not inlined | Assets | An image/audio/video `src=` doesn't resolve, or exceeds the inline size limit | Note only — a placeholder "missing" graphic is substituted (no `slide N:` prefix) |
| unknown theme "x"; using mirzam | Theme | Frontmatter `theme:` isn't a built-in name | Note only — falls back to `mirzam` |
| "default" is no longer a theme name … | Theme | Frontmatter `theme: default` — the name was retired; it was `mirzam` under a second name | Note only — renders in `mirzam`, which is the same palette. Write `theme: mirzam` or drop the key |
| unknown mode "x"; expected light or dark | Theme | Frontmatter `mode:` isn't `light`/`dark` | Note only — falls back to following the reader's machine |
| slide N, pane "x": unknown theme "y"; keeping the surrounding theme | Theme | A pane's `{theme=y}` — or a slide's `<!-- theme: y -->` — is neither a built-in name nor a theme file this deck loads | Note only — that pane or slide keeps the theme it inherits |
| slide N, pane "x": "y" is loaded from "themes/y.css", but that file sets its tokens outside [data-theme="y"] | Theme | The name is registered, but the file scopes nothing to it, so the pane picks up nothing | Note only — wrap the token block in `[data-theme="y"] { … }` and the pane takes the theme |
| theme: "themes/y.css" paints in one palette | Theme | A theme of your own sets colours and defines no light/dark variant of them | Note only — but `D` in the viewer will appear dead. Add the second block the message names |
| theme: "themes/y.css" sets "--mz-x" for dark but not for light | Theme | A colour named in one mode only keeps that value in the other | Note only — a dark panel on a white slide is the usual symptom |
| theme: "themes/y.css" in light: … is 3.1:1, under the 4.5:1 floor | Theme | Two of your theme's colours are not legible together | Note only — the same floor the built-in themes are held to |
| theme: "themes/y.css" registers as "y", which is a built-in theme | Theme | A theme file's stem collides with a built-in name | Note only — `theme=y` keeps meaning the built-in; rename the file |
| slide N, pane "x": unknown mode "y"; expected light or dark | Theme | A pane's `{mode=y}` or a slide's `<!-- mode: y -->` isn't `light`/`dark` | Note only — that pane or slide follows the deck's mode |
| math: unknown dialect "x"; latex and typst are supported | Math | Frontmatter `math:` isn't `latex`/`typst` | Note only — renders as `latex` |
| transition: … | Frontmatter | Frontmatter `transition:` doesn't parse | Note only — deck falls back to plain cuts |
| theme: cannot read path | Frontmatter | A stylesheet named by `theme:` can't be read | Note only — builds without it |
| no slides: file is empty / … has nothing outside its frontmatter | Frontmatter | Nothing to render | Note only — builds as a blank page |
| `<!-- next -->` appears in more than one pane | Frontmatter | Two panes on one slide both try to break | Note only — the slide renders whole, unsplit |

Two messages appear **on the slide itself as a quoted line**, never as a
`⚠` line from the CLI: `circular include, not expanded: <target>` and
`include failed: <error>`, from `![[...]]` transclusion gone wrong. Look at
the slide, not the terminal, for those two.

This list moves when the checks do; if a message doesn't match anything
above, trust what it says over this page and open an issue.
