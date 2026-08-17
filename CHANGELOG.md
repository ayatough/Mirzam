# Changelog

All notable changes to Mirzam are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
`0.MINOR.PATCH` while the project is pre-1.0: **a minor bump may change the
markup**. See [docs/development.md](docs/development.md#versioning) for the policy.

## [Unreleased]

Nothing yet.

## [0.7.0] - 2026-08-16

### Removed
- **`css:` and `--css` are gone**, one release after they were retired, exactly
  as the warning said. `theme:` has been the same key with more room since
  `v0.6.0`: `theme: themes/house.css` in frontmatter, `--theme house.css` on
  the command line. A deck still writing `css:` now builds as if the key were
  not there — like any unknown frontmatter key — so the stylesheet it named no
  longer loads; the fix is the one-line rename the `v0.6.0` warning printed.

### Added
- **The README shows the thing working.** A twenty-second recording at
  the top of it: a deck being typed into the browser editor, with the preview
  rebuilding beside it — a title, an ASCII grid becoming a two-column layout,
  three lines of CSV becoming a bar chart, and one `theme:` line changing the
  deck's whole face. Watching a finished deck is something every slide tool
  can show; watching source become slides as it is typed is the thing that is
  actually different, so that is what the recording is of. It is regenerated
  by CI whenever the editor, the recording script, the WASM bindings or the
  shared theme CSS change, which is the complete list of things that could
  make it show a version that no longer exists.
- **A themes gallery**, at
  [`/themes/`](https://ayatough.github.io/Mirzam/themes/) on the site. Since
  a theme became a token set rather than a palette — `mirzam`'s grotesque,
  `wuwei`'s roman serif — no page anywhere showed what that meant. One
  specimen slide carrying a heading, body text, a list, a code block, a metric
  and a chart is now rendered in all five built-ins **and** in
  `examples/themes/blueprint.css`, in both modes, and photographed. The custom
  theme sits beside the built-ins as an equal, which is the claim the theme
  contract makes. Every picture is generated from the stylesheets during the
  site build, and each rendering is layout-checked on the way past, so a theme
  whose type stopped fitting fails the build rather than shipping a clipped
  heading.
- **The browser editor's preview follows the cursor.** Typing on the fourth
  slide and watching the first is not an edit loop; everything past the
  opening slide was being written blind. The core has been able to answer
  "which slide is this cursor in" since the source map landed, and the VS Code
  preview has asked it on every keystroke all along — the browser editor now
  does too, so a `---` typed above the cursor carries the preview along with
  it, and clicking into a slide in the source shows that slide.
- **A per-slide screenshot pass**: `node scripts/shoot-slides.mjs --build
  deck.md -o shots` writes one PNG per slide, in the resting state a reader
  ends on rather than mid-animation. It shares its browser plumbing with the
  layout checker and the gallery.
- **Mermaid diagrams, drawn while the deck builds.** A ```mermaid fence inside
  a pane becomes inline SVG — flowchart, sequence, state, whatever Mermaid
  draws — sized by the pane it sits in the way a chart is, and **coloured in
  the deck's own theme**: Mermaid's palette is mapped onto the `--mz-*` tokens,
  so a diagram follows the deck into dark mode instead of staying white when
  the reader presses `D`. A colour you set yourself stays yours.

  It needs mermaid-cli (`npm install -g @mermaid-js/mermaid-cli`), which
  Mirzam does not ship and does not require: no `mmdc` on `PATH` — or named by
  `MIRZAM_MMDC` — and the fence renders as a code block **and the build says
  so**, as a `build.mermaid` warning, rather than quietly shipping a diagram
  as source code. `build` still needs no browser. And the fallback is the one
  place Mirzam's plain-Markdown promise pays a dividend rather than a tax:
  GitHub draws a ```mermaid fence as a diagram itself, so the deck's source
  shows the picture even where Mirzam did not.
- **A pane can balance its content across columns.** `columns=2` (up to 6) on
  a `::: pane` splits the pane's content by amount: a list of ten short items
  becomes five and five instead of running down the left edge with the right
  half of the pane empty. The browser does the balancing, so adding an item
  redistributes the rest, and an item never breaks across the fold. The gutter
  matches the grid gap unless `--mz-columns-gap` moves it. Rule 11 in the
  layout cookbook (`examples/03-layout.md`, last slide at `/next/`) shows it.
- **A published slide can show the Markdown it was written as, and hand the
  deck to the browser editor.** `mirzam build --embed-source` carries the
  deck's own document inside the deck; `V` then opens the current slide's
  source beside the slide — the deck makes room for the panel rather than
  being covered by it — with a button that copies it. Add `--editor-url <url>`
  and the panel also carries the deck out: one click opens **the whole deck**
  in the browser editor, positioned at the slide you were looking at, where
  the same core re-renders it as you change it. The whole deck rather than the
  one slide, because a slide has no frontmatter of its own and its citations
  are listed elsewhere in the document.

  The handover travels in the URL's fragment, which a browser never sends to a
  server, so nothing is uploaded and a deck saved to a phone hands itself over
  exactly like a published one. Every file the deck reads by name goes with it
  — the stylesheets `theme:` points at, the `bibliography:`, the `masters:` —
  so those keys resolve over there the way they resolved here, and a deck given
  `--theme`, `--mode`, `--fit` or `--split` on the command line hands over
  frontmatter saying so, since the deck it was built as is not the one its own
  text describes. Images do not travel: inlined as data URIs, they have lost
  the path they came from, and a deck that uses one arrives with the reference
  intact and the file missing.

  **The site is built this way**, which is what it was for: it showed a
  rendering and prose about it, and nothing on the page said which eight lines
  produced the slide in front of you.

  On a phone the panel has a control rather than a key — `</>` in the cluster,
  named in the `/` sheet — and it docks along the bottom, with the cluster
  moving clear of it. A swipe inside any panel now scrolls the panel instead
  of turning the page: the source does not wrap, because a pane drawing that
  reflows is not a drawing, and dragging the end of a line into view was
  losing the slide. Two more things a touchscreen was owed regardless: **the
  control cluster is always visible on a coarse pointer** rather than fading
  in on pointer movement a phone has no way to produce, and **its buttons are
  42px** instead of 30px. The shortcut sheet says "tap anywhere to close"
  there, instead of naming two keys a phone does not have.

  `v` is now a viewer key, so an `effects` block can no longer bind it — the
  same warning the other reserved keys give.

### Fixed
- **The light hero now spends its violet the way the dark one does.** The
  dark art keeps space and the planet dark and puts the violet on what glows
  — the limb, the flare, the drawn arcs. The light art had it inverted: the
  glow was white and the violet sat on the planet itself, which is why its
  accents read as a pale wash however much they were strengthened. Both
  heroes are regenerated at the source as a matched pair — same composition
  in both modes, pale space and white light, the violet in the glowing limb
  and the linework, the ground quiet. Look at the site header at `/next/`
  in either mode.
- **The rule under a section heading is a gradient in light mode too.** Its
  pale end was `#8b7cff`, close enough to the `#6557d9` it starts from that
  the ramp read as one flat colour; it now ends at `#c0b7ff`, the same pale
  violet the dark mode's rule already ran to. Any deck on `theme: mirzam` in
  light mode shows it — `02-writing` slide 2 under "A contents page that
  writes itself".

## [0.6.0] - 2026-08-14

### Added
- **A `shape` block written inside a `::: pane` draws in that pane's
  coordinate space.** `at(50%, 50%)` is the centre of the pane, not of the
  slide, so a diagram written where its content lives resizes with the pane
  that holds it — the way a chart already does. The pane's edges do not clip:
  past 100% deliberately hangs out, the same freedom page-level shapes keep.
  Both forms draw into the slide's one shape layer and ids resolve across it,
  so an arrow in a page-level block can end on a box a pane drew. Previously
  the fence was a warning and rendered as a literal code block.

  Pane rectangles are computed at build time, which needs the grid's margin
  and gutter as numbers the build can see: **`grid-pad-x`, `grid-pad-y` and
  `grid-gap` are now frontmatter keys**, emitted as the matching `--mz-grid-*`
  CSS so the browser lays the grid out from the same values. Decks that
  adjust the custom properties in CSS keep working as before — but a deck
  that anchors shapes to panes should declare the numbers in frontmatter,
  because a margin moved only in CSS moves the panes out from under the
  build's arithmetic.
- **`mirzam skill install` teaches Claude Code to write Mirzam decks.** One
  command writes `.claude/skills/mirzam/` into your deck repository (`--user`
  for `~/.claude/skills/` instead): the loop — write the deck, run
  `mirzam check --format json`, fix what it names — and the whole syntax card
  beside it. Both come out of the binary, so the markup the model reads is the
  markup that binary implements, which matters while the syntax is still `0.x`.

  Nothing versions a local skill, so Mirzam does. The installed card is stamped
  with the version that wrote it, and `build` and `check` compare stamps: a card
  older than the binary asks for `mirzam skill install`, a card newer says to
  upgrade the binary. It arrives as an ordinary warning — `build.skill` in the
  JSON — so an agent repairs the drift in the loop it already runs. A skill you
  have edited is never overwritten without `--force`.

  For claude.ai, the desktop app and phones, where no binary can run,
  `mirzam skill install --zip` writes the archive those upload: the same syntax
  card, and instructions to hand the finished `.md` to you for the browser
  editor to render. Each release attaches it.
- The `--format json` document now carries a `mirzam` field naming the binary
  that produced it. Additive — the schema is still `version: 1`.

### Changed
- **`theme:` takes a theme of your own, in a file — and gives it a name.**
  A custom theme was a stylesheet named in a second key, which meant it could
  repaint a deck and nothing more. It is a supported artefact now:

  ```yaml
  theme: mirzam                        # a built-in
  theme: themes/acme.css               # your own, beside the deck
  theme: [mirzam, themes/tweaks.css]   # a built-in, then yours over it
  ```

  An entry ending in `.css` is a path, resolved relative to the deck the way
  `masters:` and `bibliography:` are; anything else is a built-in name. A list
  is cascade order, and a scalar is a list of one — so every deck that already
  wrote `theme: nord` is unchanged. Your file loads *after* the shared
  stylesheet, which is what lets it set the type and not only the colours.

  **`themes/acme.css` also registers as `acme`**, a name a slide or one pane
  can wear: `::: pane fig {theme=acme}`. That works only if the file scopes its
  tokens to its own stem — `[data-theme="acme"] { … }` — because tokens written
  at `:root` are set on the document and a pane asking for the name would pick
  up nothing. `mirzam check` says so when a deck gets it wrong, rather than
  leaving a pane silently in the deck's palette.

  Two more things `check` now says about a theme of your own, both of them
  gates the built-in themes have always been held to: a theme that paints in
  **one palette** pins the deck to one mode and makes `D` in the viewer look
  broken, and a colour pair under the **contrast floor** is text an audience
  cannot read. `examples/themes/blueprint.css` is a complete example to copy —
  deliberately not Mirzam's identity, so it shows what a theme is free to
  change.
- **`theme: mirzam` sets the type, not just the colours.** A theme was a
  palette and nothing else, so the part of a deck's look people recognise — the
  faces, the weight ladder, the short violet rule under a section heading —
  could only be had by writing a stylesheet and naming it in a second
  frontmatter key. Those are now tokens: `--mz-font`, `--mz-font-display`,
  `--mz-font-mono`, a size, weight, tracking and leading for each heading
  level, the body pair, and the marks a theme signs its name with
  (`--mz-strong-*`, `--mz-quote-*`, `--mz-code-bg`, `--mz-th-fg`,
  `--mz-h2-rule-*`). Every one carries today's value as its fallback, so **a
  deck that sets none renders exactly as it did**; the full list is in
  [docs/syntax.md](docs/syntax.md#the-vocabulary-a-theme-writes-in).

  `theme: mirzam` now carries Mirzam's identity rather than its colours, so a
  deck that names it — or names nothing, since it is the fallback — gets the
  type as well. **`examples/seminar.md` is the one sample deck that moves for
  this**: it loads no stylesheet, so this is the first thing it has ever had
  beyond a palette. The other eight said the same thing as rules, in
  `examples/themes/mirzam.css`; they now write `theme: mirzam` and that file is
  gone (below).

  Because custom properties inherit and rules do not, the type now travels with
  a pane that carries `theme=` — a re-themed pane used to take the other
  theme's colours and keep the deck's face.
- **`theme: wuwei` is now set in roman type.** The quiet greyscale theme was a
  palette; it is an identity now — an old-style serif for headings and text
  alike, more air between the lines, and a ladder that tells a heading from a
  paragraph by size and space rather than by weight. Put it beside `mirzam`,
  which is Inter and Space Grotesk, and you can see which theme a deck is in
  before you notice a colour: slide 5 of `examples/06-theming.md` shows all
  five side by side.

  Nothing is downloaded — a deck is one file and a venue may have no network —
  so the theme names faces a machine is likely to have (Charter, Iowan Old
  Style, Palatino, Georgia, Noto Serif) and ends in the generic `serif`. Mincho
  and Song faces are named after them, so Japanese stays roman instead of
  falling back to a gothic. Code keeps the monospaced face it had.

  Two marks change with the type: bold text is drawn in the ink colour rather
  than the accent, and a quotation's bar is half the weight it was. `wuwei`
  still draws no short rule under a section heading — a theme with no accent
  colour has nothing to sign its name with, so the plain border stays.
- **The rule under a section heading follows the heading's alignment.** A theme
  that signs its name with a short rule rather than a full-width border drew it
  at the left edge whatever the heading did, so a centred or right-aligned `##`
  had its mark stranded across the pane from the words. Centre the heading and
  the rule centres; put it on the right and the rule goes right. Only
  `examples/themes/mirzam.css` knew this before, so it took a stylesheet to get
  it; now `theme: mirzam` — or any theme setting `--mz-h2-rule-w` — is enough.
- **`.card`, `.eyebrow` and `.metric` come with the renderer now**, next to
  `.box`, so a slide copied out of a sample deck keeps its shape without a
  stylesheet behind it. `examples/seminar.md` wrote `[先行研究]{.eyebrow}` and
  rendered it as plain text; it no longer does. `.box` and `.card` are both
  bordered blocks and the difference between them is now written down: `.box`
  is an aside *inside* a pane, measured in `em` so it tracks the text it
  interrupts; `.card` is the pane, measured in `px` so a row of them agrees,
  and it is the one with dials.
- **A photograph can take one half of a slide.** `.bleed` used to be a statement
  about the whole slide: it dropped the grid's margin, and the margin belonged to
  every pane, so putting a bleeding photo in one column left the words in the
  next column pressed against the slide edge. It is now a statement about one
  pane — the background runs out only on the edges that pane actually reaches,
  and everything beside it keeps the margin it was drawn with. A half-and-half
  slide needs no stylesheet of its own any more; Rule 7 in
  `examples/03-layout.md` is one.

  A `.bleed` pane that *is* the whole slide — a title, a section divider — is
  unchanged: it reaches all four edges and covers the surface exactly as before,
  so decks like `examples/pitch.md` render the same. The one difference to know
  about is the grid gap: it now survives, so a half-width photo stops one gutter
  short of the words instead of running up against them. Set `--mz-grid-gap: 0`
  if you want the two halves to meet.

### Deprecated
- **`css:` is retired. Write `theme:` instead — the same path, in the list.**
  It still works for **this release**, and every build that sees it prints the
  exact line to write:

  ```yaml
  css: themes/house.css                  # before
  theme: [mirzam, themes/house.css]      # after
  ```

  `--css` on the command line goes the same way; `--theme` takes a path now,
  and repeating it is a list. Both are removed in the next release, so a deck
  or a script that writes either has one release to move. Nothing changes about
  what is loaded or in what order in the meantime.

### Removed
- **`examples/themes/mirzam.css` is gone**, and the eight sample decks that
  loaded it write `theme: mirzam`. Everything in it is in the built-in theme's
  tokens now, so nothing repaints — with two exceptions worth knowing if you
  copied from those decks: `.foot` was `.small` under another name and is now
  written `{.small}`, and `.markers`, one deck's list-marker demonstration, is
  a `<style>` block in that deck, which is where a class or two belongs.
  `examples/themes/` still ships a custom theme, `blueprint.css`, because a
  theme of your own is now a documented feature and a feature wants a sample.
- **The `default` theme is gone. Write `theme: mirzam`, or nothing at all.**
  There were six built-in themes and only five palettes: `default` and `mirzam`
  were the same sheet under two names — 66 token declarations, every value
  identical — kept in step by a test whose whole job was to notice when they
  drifted. One palette now has one name, and a deck that names no theme gets
  `mirzam`, exactly as it already did.

  **Nothing repaints.** A deck with `theme: default` renders in the same
  colours it always has; a deck that named no theme is untouched. Only the name
  is a breaking change — `theme: default` is now an unrecognised name, so it
  warns instead of being silently accepted. The warning says what to write
  rather than "unknown theme": deleting the key is the better fix, since the
  key was only ever choosing the palette you would have got anyway. The same
  goes for a slide's `<!-- theme: default -->` or a pane's `{theme=default}`.

  `examples/06-theming.md`'s palette gallery now shows all five built-ins with
  no repeat.
- **The landing page opens dark, like the deck beside it.** It followed the
  machine's `prefers-color-scheme` when nothing was stored, while the README
  deck linked from it is built `--mode dark` — so on a light-preferring machine
  the front door and the demonstration disagreed. Dark is now the page's own
  default, reached without asking the system anything. The switch in the corner
  still wins in both directions, so a reader who chose light keeps light, and
  the page's mode now rides on the deck links as `?mode=` whether it was chosen
  or defaulted: what you click looks like what you were looking at. The page
  and the decks go on sharing one stored preference — pressing `D` in a deck is
  the same reader saying the same thing.
- **The sample custom theme looks like a theme in light mode too.**
  `examples/themes/blueprint.css` is the repository's demonstration that a
  theme of your own can carry a real identity, and in light it was near-white
  paper and a sans body — a deck you could not tell from any other. It is a
  drawing office now in both modes: pale blue paper on a blue-grey desk in
  light, the ink-blue night it already had in dark, and one mono hand for the
  whole sheet rather than only for the headings, since a face survives a
  projector and a photocopy in a way a colour does not. Sizes come down a step
  and leading goes up to pay for mono's width. Same contrast floors, same
  system fonts, nothing fetched.

### Fixed
- **A pane wearing a theme no longer borrows the deck's type and colour.** On
  the "Two palettes on one slide" slide of `examples/06-theming.md`, the `###
  Day` heading in the `wuwei` pane came out a pale violet on wuwei's cream
  paper — very nearly invisible — and flipping the deck to light did the mirror
  image to `### Night`. The pane was taking a colour mixed for the deck's theme
  *and the deck's mode*, which is the opposite of what asking for a theme by
  name is supposed to mean.

  It was never one token or one slide. A theme is a token set, custom
  properties inherit, and 36 of them are set by some built-ins and not others —
  a subheading's colour, bold's colour and weight, a table header, a quotation,
  code's paper and ink, both faces, the whole `h1`/`h2` ladder, the mark under
  a section heading, a title, a metric, body leading, the grid's margins. Every
  one of those leaked into any pane or slide whose theme happened not to set
  it.

  Every theme scope now opens by undefining the whole derived vocabulary, so it
  starts from the same defaults as every other scope and falls back to its
  *own* palette in its *own* mode for anything it does not set. That covers a
  theme you wrote as well: a file theme scoped to its stem gets the same block,
  ahead of your declarations, so your values still win. Two consequences worth
  knowing: a pane wearing a theme that names no face now shows the shared
  default face rather than the deck's, and a pane wearing a theme with no
  signature rule no longer inherits the deck's.
- **The pitch deck's "How it works" diagram stays inside the deck margin.** The
  WASM box reached 99% of the slide width, 47px past the right margin every
  other slide aligns to. Shapes are free to cross pane borders — that layer
  ignores the grid deliberately — but nothing in this composition meant to
  break the margin line.
- **The VS Code preview reads every file a deck names.** Open
  `examples/pitch.md` with **Mirzam: Open Preview** and it came up with two
  missing photographs, an empty chart and a connector pointing at a mark that
  was never drawn — while the same deck built cleanly from the CLI. The preview
  renders in a webview, which has no filesystem, so the extension reads the
  deck's files and hands them over; it knew about transclusions, images,
  `masters:` and `bibliography:`, and about nothing else. A pane's background
  photograph (`bg=`, `bg-light=`, `bg-dark=`), a video's `poster=`, the CSV a
  chart names in `data:`, and the sources inside raw HTML — a `<picture>`
  picking artwork by colour scheme — are now read too, and markup a deck merely
  *quotes* inside a code fence is no longer mistaken for markup it writes.

  **The stylesheet in `css:` reaches the preview as well**, which it never had:
  the preview was showing every deck with a custom stylesheet stripped of its
  own type and colour and saying nothing about why. A deck whose host cannot
  supply it now warns instead, the way a missing `masters:` file already did.

  `examples/pitch.md` now previews byte-for-byte identical to what `mirzam
  build` produces for it.

## [0.5.0] - 2026-08-13

### Added
- **Code blocks are syntax highlighted.** A fence that names a language comes
  out coloured — 36 of them, Rust, Python, JavaScript, TypeScript, Go, C, C++,
  Java, shell, SQL, HTML, CSS, JSON, YAML, TOML, Markdown and diff among them,
  with the usual aliases (`py`, `js`, `rs`, `c++`, `bash`, `yml`). Uncoloured
  code was the most visible thing separating a Mirzam deck from the tools a
  developer talk is usually written in, and it is gone.

  The colouring happens while the deck is built, so a deck is still one file
  with no JavaScript doing the work, and the PDF export is coloured too. **The
  colours are the theme's, not a highlighter's**: six new `--mz-code-*` tokens
  carry them, so code in a `nord` deck reads Nord, code follows the deck when
  a reader presses `D`, and every one of them is held to the same contrast
  floor as body text in both light and dark. Override any of them in a deck's
  own stylesheet like any other token.

  **A language nobody recognises is still a plain block** — as is a fence with
  no language, and one carrying `chart`, `shape` or another Mirzam block that
  landed somewhere it renders as code. Nothing to turn on, nothing to turn
  off, and existing decks change in exactly one way: their code has colour in
  it. The browser build grows 36 KB gzipped, a third of what the emoji table
  already costs.
- **`mirzam check --format json`, so something other than a person can read
  the answer.** The checker already found the failures a diff cannot show — a
  clipped heading, an arrow pointing at an id that was renamed, a `shape` block
  written inside a pane so it ships as source code — but it said so in prose,
  which meant a tool that could fix them had to guess at what it had been told.
  Now the same run comes back as one JSON document: every build warning and
  every in-page finding as a record with a stable `kind`, a severity, the slide
  and pane, and the source file and line, followed through `![[…]]` to the file
  the slide was actually written in. Exit codes are unchanged and `--format
  text` is still the default, so nothing that already runs the checker moves;
  the document goes to stdout on its own, with errors on stderr, so it is safe
  to pipe. The schema is versioned in `docs/agents.md` and a field may be added
  but never renamed.
- **`docs/llms.md`, the whole markup on one page** — every fenced block, every
  frontmatter field, the attribute syntax, one example each, and the traps that
  fail silently called out at the top: an attribute span cannot cross a line
  break, `shape` and `connect` only work at slide top level, a footnote
  definition has to be on the slide that cites it. It is written to be handed
  to a model as context, and the site publishes it at `/llms.txt`, where the
  emerging convention says to look. `docs/agents.md` ties the two together: the
  card to write a deck with, the JSON schema to check it against.
- **A deck can cite a bibliography, and the references list themselves.**
  Footnotes always covered a remark belonging to one slide; the other half —
  a paper cited on four slides, whose details are worth writing down once — had
  no answer, and the reference had to be repeated on every slide that made a
  claim from it. Now `bibliography: refs.bib` in the frontmatter turns `[@key]`
  into a citation against a plain BibTeX file (the one a reference manager
  already exports; a deck citing three papers can write them in frontmatter
  instead). A `bibliography` block, usually on the last slide, lists what was
  cited: every mark links to it and every entry links back to each slide that
  cited it, and in the PDF that backlink is still the slide number. Marks read
  `[1]` by default, or `[Vaswani+17]` with `citation-style: author`; the list
  is ordered to match. Nothing is silent — a `[@key]` naming no entry stays on
  the slide exactly as it was written and says which slide it is on, citing
  with no list anywhere warns, and a deck with no `bibliography:` leaves
  `[@anything]` as the text somebody typed. `--mz-bib-size` sets how large the
  list is. See `examples/04-components.md`, slides 16 and 17, and
  `examples/seminar.md` — the Japanese research talk now cites three papers
  from three slides and lists them on slide 11, with the quoted figure's
  source left in the footnote where it belongs, which is the two halves of
  the question side by side. The VS Code preview reads the `.bib` out of the
  same file table it already reads a masters file and a transcluded section
  from, so it cites and lists exactly as the CLI does.
- **`examples/research.md`: a research report, in English.** The whole-deck
  samples were a sales pitch and a Japanese seminar talk, so anyone wanting a
  report to start from had to read the one deck deliberately written in the
  language they might not read. This is that shape in English — background,
  prior work as a table, method in maths, a chart, what still fails — citing
  four references from four different slides, with one of them cited from
  three so its entry carries three backlinks. `seminar.md` stays Japanese: it
  is the CJK typography sample, and having both is the point.
- **Cutting a release is one command: `./scripts/release.sh <version>`.** It
  writes the version into the five files that carry it — the root `Cargo.toml`,
  `Cargo.lock`, `editors/vscode/package.json`, and the status sentence in both
  `README.md` and `docs/roadmap.md` — closes `[Unreleased]` into a dated
  section with a fresh empty one above it, and runs the gate: formatting,
  lints, tests, the eight sample decks built and checked, the benchmark, and
  the extension package. It refuses a version below the current one, an
  `[Unreleased]` section with nothing in it, and a dirty working tree; it reads
  which digit should move off the changelog's own headings (`Added`, `Changed`
  or `Removed` is a minor bump, `Fixed` alone is a patch) and says so when the
  version it was given disagrees. `--dry-run` shows the edits and keeps none of
  them. It stops before committing, pushing and tagging, and prints those
  steps, because they are the ones worth looking at.
- **CI checks that every version agrees** (`./scripts/check-versions.sh`). The
  README's status section spent the whole of `v0.3.0` claiming `v0.2.0` was the
  current release, and nothing could have caught it: no test was failing,
  because nothing was broken — only untrue. This is the kind of check
  `build-site.sh`'s dead-link pass already is, applied to the version number.
- **The layout check now measures what scrolls, not only what overflows.** An
  element that scrolls internally holds its overflow instead of passing it up
  to the pane, so the pane measured clean while its content was out of sight —
  and nobody in an audience can scroll a slide. Any element inside a pane that
  hides part of itself is reported with the element and how much is missing.
- **Typst maths knows `dif`, `space`, and the blackboard shorthands** `NN`,
  `ZZ`, `QQ`, `RR`, `CC`, `EE` and `PP`. `integral f(s) dif s` now sets an
  upright differential, and `EE[x]` the 𝔼 a probability deck wants, instead of
  spelling those names out in italic letters.
- **A build warns when an attribute span reaches the slide as text.** A span
  that did not become a span is literal `[text]{.small}` on the slide, which is
  indistinguishable from Markdown somebody meant literally — and the layout
  check cannot see it either, because the box is the right size and merely has
  punctuation in it. `--strict` fails on it.
- **`mirzam check` says what it measured with.** A deck embeds no text font —
  only the maths face is inlined — so the layout it checks is the deck set in
  whatever fonts the checking machine happens to have, and a clean run was a
  statement about that one machine with nothing saying so. It now prints the
  families it found, the ones the deck asks for and this machine lacks, and how
  much room the tightest pane had left. On the Japanese sample that last number
  is 3px: less than one wrapped line, which is what a font substitution costs.
- **`mirzam check --min-slack <px>`** reports any pane with less than that much
  room left, even though it fits here. A deck that will be shown somewhere else
  can be held to a margin instead of to "it fitted on the machine that ran
  this", which is the only thing a fit alone can promise.
- **The long arrows.** `-->` `<--` `<->` `<-->` `==>` `<==` `<=>` `<==>`, and
  the tailed `->>` `<<-` `>->`, are single operators in Typst maths now.

### Changed
- **`mirzam check` is now the layout check the contributor docs ask for**, in
  place of `node scripts/check-layout.mjs`. Both run the same in-page checks
  from the same source, but `check` needs only the binary and a browser, where
  the script needs `npm i playwright-core` and leaves three paths to clean up
  before committing. The script stays the right tool where a tab has to remain
  open — screenshots, and `scripts/record-demo.mjs`.
- **The release checklist says where the commit has to live and how an agent
  cuts the tag.** It assumed both: that the version bump was already on `main`
  (a release cannot be cut from a branch) and that whoever read it could press
  a button in the Actions tab. Neither is true for an agent working on a branch
  through an API, which is now the case the checklist is written for. The
  benchmark step also says to read the *second* run — the first pays for a cold
  cache and can report a full build several times slower than the roadmap's
  table with nothing wrong.
- **An unknown name of three letters or more in Typst maths is an error.** It
  used to become a run of italic letters, so `dif s` in an integral rendered as
  `difs` — which reads as a typo the author made rather than as a word the
  parser does not know. Two letters side by side are still the product a LaTeX
  author writes the same way (`dx`, `dt`), and the message says how to get each
  of the other readings: `d i f` for variables, `"dif"` for upright text,
  `op("dif")` for an operator. This is the rule the parser already applied to
  unknown dotted names and to unknown words used like functions.
- **A code block in a pane no longer scrolls; it overflows.** `overflow: auto`
  also makes a flex item's automatic minimum size zero, so a code block in a
  centred pane shrank below its own content: three lines of four invisible,
  with `mirzam check` reporting nothing and `fit=shrink` finding nothing to
  shrink. Overflowing is what every other element in a pane does, and it is
  what both of those already act on. Four sample slides were hiding content
  this way and now show all of it — `01-start.md` 6, `04-components.md` 10 and
  15, `05-motion.md` 3, `06-theming.md` 7; the last three have a taller band to
  hold it.

### Fixed
- **The warning table in `docs/troubleshooting.md` lists every warning again.**
  It says it is the full list, and a reader who cannot find a message there
  reasonably concludes the build is telling them something undocumented. Three
  families had been added to the code without reaching it: slide masters
  (`no master named …`, a masters file that cannot be read or defines none, a
  transcluded section naming its own), a malformed `toc` or `bibliography`
  block, and the attribute-span warning added above.
- **Changing `theme:` did nothing to the live preview.** The preview patches
  the slides that changed into a page it assembled earlier, and a theme is not
  in a slide: swapping it changed no slide's HTML, so nothing was patched and
  the deck kept the palette it opened with until the preview was closed and
  reopened. The same held for `title:`, `aspect:`, `transition:`, `fit:` and
  for a slide reaching for a palette the page was never assembled with. The
  renderer now hands out a fingerprint of everything the page carries around
  the slides, and a host that sees it move rebuilds the page. `serve` uses the
  same fingerprint, which closes the case it also missed: a page-level setting
  changed in the same save as a slide.
- **The preview followed the cursor to the wrong slide in a deck split across
  files.** Which slide the cursor was on was counted by the `---` rules in the
  file being edited — right for a deck of one file, and wrong for every slide
  a `![[…]]` brings in, so the preview landed further and further ahead of the
  cursor the further down the deck it went. The count now happens in the core,
  on the expanded document, and the source map carries the cursor across; a
  cursor resting on the `![[…]]` line itself shows the first slide of the file
  it names. Non-ASCII text no longer shifts the answer either — the offset is
  counted in bytes on both sides now instead of UTF-16 units on one.
- **Editing a section of a split deck did nothing to the preview.** The preview
  watched exactly one document — the file it was opened on — so the whole point
  of splitting a deck was lost the moment you started writing in a section:
  nothing re-rendered, and the cursor there moved nothing. A preview now
  follows every file its deck was assembled from, sections and masters alike,
  re-rendering as they are typed in rather than waiting for a save, and the
  cursor in a section scrolls the preview to that section's slides. Two open
  previews also stop cancelling each other's updates: the debounce is per
  preview now instead of one shared timer.
- **`split: h2` made no slides in the preview or the browser build.** The
  frontmatter setting that turns an ordinary document into a deck was read by
  `mirzam build` and ignored by the WASM core, so the two disagreed about what
  a slide even is: the preview showed one long slide for a deck the CLI split
  into a dozen.
- **`\/` inside `mat()`, `cases()` or `vec()` crashed the build.** `\` is a
  line break here, so a Typst author's escaped slash parsed as break-then-
  divide and put a row separator inside a fraction, which panicked the MathML
  renderer — no output at all, and a message naming a file in a dependency
  rather than the line that caused it. A line break or an alignment point where
  a value belongs is now the ordinary red source with a tooltip, and the
  tooltip says to write `#/` for a literal slash.
- **`%` in Typst maths discarded the rest of the formula.** It went out as a
  bare `%`, which opens a LaTeX comment: `99% "of the mass"` rendered as `99`,
  with no error, no warning and nothing in `mirzam check`. A sentence about a
  99% interval simply lost its percent sign, which looks like a typo rather
  than a tool that dropped it.
- **`**+ text**` showed its asterisks instead of going bold.** `+` is the
  delimiter of the `++inserted++` extension, and the scan for what follows `**`
  stepped over it onto the space, so the emphasis never opened — in plain
  Markdown that every other parser bolds. `| **+ wheel odometry** |` is a
  natural way to write a table row, and it was reaching the audience as
  punctuation. `=` and `~` had the same hole. There is now a test comparing
  plain Markdown through Mirzam against a reference parser, so the next
  extension cannot quietly change how ordinary text reads.
- **An attribute span broke on any `]` inside it.** `[an aside[^a]]{.small}`,
  `[a [word]{.accent} in it]{.small}` and `[maths $x[i]$]{.small}` all reached
  the slide as raw `[…]{.small}`: the closing bracket was found by refusing to
  allow one at all. Brackets are matched with nesting now, and the content is
  read with the rest of the slide rather than on its own, which is what lets a
  footnote inside a span find its definition.
- **A failed formula was shown back without its backslashes.** The red source
  is raw HTML in a Markdown document, so `\/` was read as an escape and shown
  as `/` — the author was pointed at a line they had not written.

## [0.4.0] - 2026-08-12

### Added
- **Slide masters: draw a shape once, not once per slide.** `masters:` names
  slide shapes — the same ASCII drawing a `pane` block holds, read by the same
  parser — and a slide picks one with `<!-- layout: two-up -->`, an HTML
  comment, so a plain Markdown reader still shows nothing. `layout:` sets the
  deck's default for every slide that names none. The shapes normally live in
  a Markdown file of their own, named like `css:` is
  (`masters: masters/cookbook.md`): a heading names a master, the `pane` block
  under it is the drawing, and the prose between them says what it is for. That
  keeps ASCII art out of frontmatter, where it has to be indented inside a YAML
  block scalar and pushes the first slide off the screen — and it lets several
  decks share one set of shapes. A mapping written in frontmatter still works
  for a deck with one or two. The file is read through the same `FileProvider`
  a `![[…]]` transclusion uses, so the editor extension and the browser build
  draw the deck on the same shapes the CLI does rather than falling back to
  single panes, and it joins the watch set, so editing shared shapes re-renders
  the decks on them. A deck assembled from `![[…]]` section files declares
  `masters:` in the root — only the root's frontmatter is read — while each
  section still picks shapes with `<!-- layout: -->` and can carry frontmatter
  of its own so its author previews that section alone with the right shapes.
  Editing a shared shape then updates the slides that use it wherever they were
  written, in one hot reload. A section naming a *different* masters file is a
  warning: that is the one case where ignoring a transcluded file's frontmatter
  changes the slides rather than costing nothing, and when both sets share pane
  names nothing else in a build would say so. Redrawing the same grid on the twelfth slide that has the same shape
  was the part of writing a deck nobody enjoyed, and it was also where decks
  drifted: two slides meant to match ended up a character apart. A slide that
  needs a different shape simply draws one — its own `pane` block always wins,
  so an exception costs nothing — and `<!-- layout: none -->` gives a title
  slide the whole surface back. An unknown name is a warning naming the slide,
  never a failed build, and the slide keeps what it would have had without it;
  an unknown deck-wide `layout:` is reported once rather than once per slide.
  The catch to know going in: a master fixes the pane names, so the deck's
  masters and its `::: pane` blocks have to agree on `head`/`main`/`fig`.
  `examples/03-layout.md` now uses one for the shape it drew three times, and
  renders byte-identically to the version that drew them by hand — which is the
  test that a master is a shape moved, not a shape changed. Rule 9 in that deck
  shows it.
- **A footer and a slide number on every slide.** `footer:` and
  `slide-number:` in frontmatter draw along the bottom of every slide, in the
  band the grid already holds back as its margin, so they take nothing from the
  content. `{n}` and `{total}` are substituted, `{{ }}` variables work, and the
  footer sits on the same margin as the words above it however that margin is
  set. They are part of the document rather than the viewer, so they are in the
  PDF too — unlike the counter in the corner of the window, which is an aid for
  whoever is driving and never prints. `<!-- chrome: none -->` drops both on one
  slide, which is what a title slide, a section divider, or a `.bleed`
  photograph wants; there is no automatic suppression, because a rule that
  silently dropped a confidentiality notice would be worse than one that draws
  it somewhere you can see. A deck that sets neither carries neither.
  `examples/06-theming.md` now carries its own footer from slide 2 on.
- **Margins, padding and borders are tokens.** `--mz-grid-pad-y`,
  `--mz-grid-pad-x`, `--mz-grid-gap`, `--mz-pane-pad`, `--mz-pane-border` and
  `--mz-pane-radius` replace the literals that used to be spelled out in the
  layout stylesheet, each carrying its built-in value as a fallback, so a deck
  that sets none renders exactly as it did before. Writing `.grid { padding: …
  }` yourself still works and always did, but the tokens are the better route:
  custom properties inherit, so the same six names move the whole deck from
  `:root` or one pane from a class you put on it, and the footer reads
  `--mz-grid-pad-x` to stay on the deck's margin rather than the built-in one.
  They are deliberately **not** palette tokens — no built-in theme defines one,
  and `theme:` stays a choice of colour.
- **A theme can now be smaller than a deck.** `theme=` and `mode=` are pane
  attributes (`::: pane figure {theme=wuwei mode=dark}`) and slide settings
  (`<!-- theme: nord -->`, an HTML comment, so a plain Markdown reader shows
  nothing), alongside the deck-wide `theme:` they have always been. That is
  what puts two palettes on one slide: the same screenshot in light and in
  dark, a quotation on its own paper, a figure kept in the colours it was
  drawn for. The palette is set on that element and custom properties
  inherit, so everything inside it — headings, code, tables, chart series —
  is drawn from the other theme's tokens, and a re-themed pane paints its own
  background rather than leaving its words a different colour on the slide's
  paper. Innermost wins: a pane's theme beats its slide's, which beats the
  deck's. `theme=` alone still follows the deck's colour mode, so a re-themed
  pane keeps flipping with `D`; `mode=` alone shows the surrounding theme's
  other half and stays there. An unknown name is a warning naming the slide
  and the pane, never a failed build, and that element simply keeps what it
  inherited. A deck carries the tokens of the themes it actually names, so
  none of this costs anything to a deck that uses one palette.
- **A theme gallery, in the theming deck.** One slide of
  `examples/06-theming.md` puts every palette side by side, each pane holding
  the same three lines of markup, so the only thing that differs between the
  samples is the palette. No pane names a mode, so `D` turns all five at once
  along with the slide around them — the gallery is a live deck rather than a
  page of screenshots, which is the whole point of a theme a pane can carry.
  `examples/06-theming.md` also declares `mode: dark` now, which is what its
  stylesheet already meant: a deck whose `css:` pins a mode without saying so
  in frontmatter leaves a pane that follows the mode following the reader's
  machine instead of the deck.
- **A sixth built-in theme, `wuwei`.** Warm greyscale, minimal, and
  deliberately low contrast: body text sits at about 9:1 against its paper
  rather than the 12-14:1 a black-on-white theme reaches — quiet enough to
  read for an hour, and still well clear of the 4.5:1 the contrast test
  enforces. No accent colour; the six chart series keep a whisper of hue
  (taupe, sand, near-black, olive, rose, slate) so they can be told apart
  without leaving the grey family. Dark mode is drawn from scratch — warm
  near-black paper, warm bone ink — not inverted from light.
- **A troubleshooting/FAQ page** ([docs/troubleshooting.md](docs/troubleshooting.md)), linked from the README and quickstart: the decision order for a slide that doesn't fit, what to suspect when markup shows up as literal text, the current PDF steps, and a full table of every build warning with what it means. `docs/syntax.md` (1,000+ lines) now opens with a chapter list, and `docs/ja/README.md` gets a chapter-by-chapter map into it plus a mention of `math: typst` and the built-in size/colour classes, none of which had reached the Japanese docs yet.
- **`mirzam check <deck.md>`: the layout checker without `cargo`, `playwright-core`, or a checkout.** `scripts/check-layout.mjs` catches a clipped pane, an unresolved connector, an animation left mid-entrance, the debug overlay baked in — but needed all three, so a binary install never had it. `check` builds the deck and drives the exact same in-page check (now shared between both tools as `crates/mirzam-cli/src/check.js`, not duplicated) through a one-shot headless Chromium process instead of a kept-open browser tab, takes the same deck-shaping flags `build` does, and exits non-zero on anything it finds — a CI gate that doesn't need the repository.
- **Three silent degradations now warn at build time, and `mirzam build --strict` fails on any warning (for CI)**: a `shape` block written inside a `::: pane` (shape only parses at slide top level, so it rendered as a plain code block with nothing said about it), a footnote reference with no `[^key]:` definition on the same slide (left as literal bracket text), and a `connect` endpoint id matching no text anchor, shape, or chart element on the slide (no arrow drawn, and only the browser-side checker used to notice). Every one of these still renders exactly as before — this only adds the warning.
- **`mirzam export pdf` takes `--split`, `--theme`, `--css`, `--fit` and
  `--mode`, the same flags `build` does**, so a deck assembled with
  `--split h2` exports to PDF with the same slide breaks in one command
  instead of two. It also now refuses anything but a `.md` source: pointing
  it at a built `out/index.html` used to "succeed" with a title-only PDF,
  silently dropping the whole deck, because the HTML was re-parsed as
  Markdown. That is an error now, and says the right command to run instead.
- **Typst-flavoured math covers more of what LaTeX can say.** Brackets are
  delimiters now: `[a/b]` grows with its contents like parens always did, and
  the mixed pairs intervals need — `[0, oo)`, `(0, 1]` — parse instead of
  erroring on the unmatched half. Spacing words (`thin`, `med`, `thick`,
  `quad`, `wide`) map to LaTeX's `\,` through `\qquad`, so `integral f(x)
  thin d x` gets its breath. `hat` and `arrow` widen over more than one
  glyph (`hat(A B)` is `\widehat`, `arrow(A B)` is `\overrightarrow`).
  `|->`, `<<` and `>>` lex as `\mapsto`, `\ll` and `\gg`; and a batch of
  everyday names joins the tables: `ell`, `Re`, `Im`, `aleph`, `angle` (and
  `angle.l`/`angle.r` for ⟨ ⟩), `degree`, `star`, `dagger`, `compose`,
  `convolve`, `without`, `perp`, `parallel`, `divides`, `therefore`,
  `because`, `top`, `bot`, `models`, `tack.r`, `tack.l`, `arrow.r.bar`,
  `lt.double`, `gt.double`, `brace.l`, `brace.r`. Every entry is verified
  against hand-written LaTeX at the MathML level, so a name the renderer
  cannot draw cannot join the table.
- Text areas in the browser editor hold 16px type on phones, because iOS
  Safari force-zooms the page into any smaller input the moment it is
  focused — which read as "editing is broken on a phone" while being,
  precisely, a font size.

## [0.3.0] - 2026-08-11

### Changed
- **The default theme is Mirzam's own palette.** A deck that names no theme is
  the common case — a quick note, a README turned into slides, a sample showing
  one piece of markup — and those came out in a generic blue-and-teal that
  looked like nobody's. `theme:` is now a choice to look like something *else*
  rather than a choice to look like anything at all. `theme: mirzam` still
  works and is the same palette; a test compares the two token sets so they
  cannot drift into two slightly different Mirzams.
- **The sample decks stopped claiming to be a tutorial.** "Learn it, in order"
  was a promise nothing kept: 02 to 06 are subject areas, and nobody reads
  Components because they finished Layout. The site now says **Start here**
  (a first deck, and a README turned into one by `--split h2`), **The markup,
  deck by deck** (a reference to look things up in), and **Whole decks**
  (`pitch` and `seminar`, written for an audience rather than as
  documentation) — which is also what the author meant by the research talk
  being one use case and the README deck being one feature, rather than both
  sitting on the front page as equals.

### Added
- **Typst-flavoured math: `math: typst` in frontmatter.** LaTeX is hard to
  write from memory; Typst's `sum_(i=1)^n i = (n(n+1))/2` is not. The setting
  switches what `$...$` holds for the whole deck — fractions with `/`, `sqrt()`
  without backslashes, Greek by name with `.alt` variants, accents (`dot(x)`,
  `hat(p)`, `arrow(v)`), letter styles (`bb(R)`, `cal(F)`), dotted symbol
  variants (`subset.eq`, `in.not`, `integral.cont`), `mat()` with `delim:`,
  `vec()`, `cases()`, `binom()`, `floor()`/`ceil()`, `underbrace()` with a
  label, `op("argmax")`, `"upright text"`, `&` alignment and `#` escapes. It
  is a subset parser of our own (`mirzam-tmath`) that lowers to LaTeX and
  renders through the existing MathML path, not a dependency on Typst itself.
  The default stays `latex`, so no existing deck changes; anything outside the
  subset shows its source in red like any broken formula — deliberately
  including unknown dotted names and unknown words used like functions, which
  would otherwise render as a run of italic letters that merely resembles the
  formula. `docs/syntax.md` has the full table, and the theming deck shows it
  on a slide.
- **`mirzam new <file.md>` writes the first file.** Every way in started from a
  deck that already existed: `build` and `serve` both error on a path that is
  not there, so the actual first step — create a file, guess the frontmatter —
  was the one step nothing helped with. `new` writes frontmatter, a title slide
  and a slide break, and never overwrites an existing file. `--empty` writes a
  blank file instead, which is the case that had no expression at all: starting
  from nothing rather than from somebody's template. Pointing `build` or
  `serve` at a missing file now names the command instead of only reporting the
  failed read.
- **New and Sample in the browser editor.** It opened on the sample deck and
  kept exactly one draft, so there was no way to start an empty deck — the
  first thing you want on a phone, where the editor is the whole toolchain —
  and no way back to the sample once you had typed over it. Both buttons ask
  before replacing a draft that has anything in it, since that draft lives in
  the browser and nowhere else. An emptied draft now survives a reload rather
  than reopening on the sample.
- **`examples/02-writing.md` is the reference for writing a slide**, rebuilt
  around what a reader actually asks: a contents page, headings, the marks for
  a phrase, colour and size, quotes and asides and code, four kinds of list,
  tables, mathematics, emoji, footnotes and the two kinds of comment. It used
  to be a deck arguing that plain CommonMark works, which is one slide's worth
  of point stretched over eleven.
- **`markup_coverage.rs`, so "is X supported?" has an answer that cannot rot.**
  Every inline mark is listed once and held to three conditions: it renders, it
  is in `docs/syntax.md`, and some deck shows it. Adding it caught six marks the
  reference had never mentioned — bold, italic, inline code, ordered lists,
  quotations and tables — because the reference described only what Mirzam
  *adds*, so "does it do tables?" could not be answered from it. Strikethrough
  and task lists had already shipped that way for two releases.
- **The marks a slide reaches for, which CommonMark has none of**:
  `==highlight==`, `++underline++`, `:tada:`, and term lists (a line, then
  `: its meaning`) — which decks had been faking with a two-column table, so a
  screen reader announced a data grid and a rule sat between a word and its
  definition. `<mark>`'s browser default is a fixed yellow, the one colour on a
  slide that would not move when the theme or the mode did, so the wash is
  drawn from the accent instead. The emoji table costs the browser build 103 KB
  gzipped, which is on the record here because it is a real price for a
  convenience: typing the character directly always worked.
- **Classes for colour, size and a bordered aside**: `.big`, `.huge`, `.muted`,
  `.accent`, `.accent2`, `.danger`, `.box`. Inline colour and size had no
  syntax at all, and `.card` existed only in the sample themes, so a deck that
  picked no stylesheet had nowhere to put a caveat. Each takes a theme token,
  which is also the reason there is still no way to write a colour literal: a
  hex value chosen against a white slide is exactly what cannot follow the deck
  into dark mode.
- Task lists (`- [ ]`) are documented. They have worked all along and appeared
  in no sample and no reference, which is the kind of gap this release exists
  to close.
- **Three shapes for a term list, because which one is right is a per-list
  question and a renderer that picks for you is wrong a third of the time.**
  `{.terms-aligned}` on the pane holds every definition to one column, for
  definitions meant to be read against each other; `{.terms-stacked}` puts the
  definition on its own line and indents it, for definitions long enough that
  the term reads as a heading over them; the default sets the definition beside
  its term. Underneath, `--mz-terms-hang` (`0` turns the hanging indent off),
  `--mz-terms-gap` and `--mz-terms-col` tune all three, and are settable on a
  pane, a deck or a theme — each is read as a `var()` fallback rather than
  declared on the `dl`, because a default written onto the element would beat
  the value it should be inheriting. No new markup: pane classes already pass
  through, so this is a stylesheet and a slide.
- **The list marker is a choice now.** Bullets are most of what a slide is made
  of, and the kind of bullet was whatever the browser settled on.
  `--mz-bullet`, `--mz-bullet-2`, `--mz-bullet-3`, `--mz-number`,
  `--mz-number-2`, `--mz-number-3` and `--mz-marker` take anything
  `list-style-type` does — `square`, `upper-roman`, `decimal-leading-zero`,
  `none`, or a quoted string like `"→  "`. Each depth reads its own property and
  defaults to what the browser would have drawn, so a deck that asks for a dash
  at the top level keeps the hollow circle beneath it instead of flattening
  three depths into one mark. A string marker carries its own trailing space,
  which the reference says out loud because the browser adds none and `"→"`
  otherwise sits flush against the first word. Footnotes are pinned to
  `decimal`: their marker has to match the `[^a]` citing them, and a citation is
  always a numeral.
- **A `<picture>` that picks art by colour scheme follows the deck, not the
  machine.** The markup every README uses so its logo survives GitHub's dark
  theme is rewritten at build time into one image per mode, switched the way
  `bg-light=`/`bg-dark=` already switched. Left as written it consults
  `prefers-color-scheme`, which can only ask the operating system, while a
  deck's mode is `mode:`, `?mode=` or the reader pressing `D` — so Mirzam's own
  README, published as a deck, showed a pale wordmark on a white slide whenever
  the reader's phone was dark and the deck was not. `alt`, `width` and every
  other attribute are carried into both copies; a `<picture>` selecting on a
  width breakpoint or a format fallback is left alone.
- **A colour-mode button in the viewer.** The mode was bound to `D` and nothing
  else, so on a phone — which has no keyboard, and which is where a deck
  arriving from a share is read — there was no way to reach it at all, and a
  deck baked `mode: dark` could not be read in sunlight. The control cluster
  gains a fourth button showing where it takes you. `every_control_has_a_touch_equivalent`
  used to check only that the file mentioned touch events, which is how this
  shipped; it now checks that every button in the cluster is bound.

### Fixed
- **A second tab of the browser editor could undo your writing.** The draft is
  one `localStorage` slot shared by every tab of the site, and the editor wrote
  it again on `beforeunload` — so a tab left open on an older copy overwrote the
  newer one on the way out, silently and after the fact. Nothing is written on
  unload now: typing, adding an image, opening a file, New and Sample all save
  as they happen, so the only thing that write could still save was something
  older. It leaves the tabs unsynchronised rather than destructive — a tab that
  is already open still shows what it loaded, and picks up the current draft
  when it reloads.
- **A deck with no slides said nothing about it.** An empty file, or one whose
  content never made it past the frontmatter, built successfully and opened as
  a blank page — the same picture as a failure, with no way to tell which. The
  build still succeeds, because an empty file is where a new deck starts and
  `serve` has to watch one while the first slide is typed, but it warns and
  says which of the two happened. The browser editor shows an empty-deck page
  in the preview instead of an unexplained black rectangle.
- **A list inside a list came out bigger than the list.** Type sizes are written
  in `em`, which multiplies down a nesting, so `p, li { font-size: 1.35em }` made
  the qualification under a point 1.35 × 1.35 — half again as large as the point
  it qualified. It reads as a styling choice rather than a bug, which is how it
  survived a release; `check-layout.mjs` now measures every nested item against
  its parent, so it cannot come back quietly.
- **A task list showed a bullet *and* a checkbox** — two markers for one idea,
  because comrak writes `- [x]` as an ordinary `<li>` with an `<input>` inside
  and nothing dropped the marker. The box is now drawn by the stylesheet: it
  scales with the type around it and takes the accent colour, where the native
  one is ~13px whatever the slide does and is coloured by the operating system —
  the one mark that would not move when the theme or `D` did. The input stays,
  hidden, so a screen reader still reports the state.
- **A term list stacked the definition under its term**, which turned four
  entries into eight lines on a slide with room for four. The definition sits
  beside the term now, the way Typst sets one: it follows immediately rather
  than lining up in a column with its neighbours, since a column has to be as
  wide as the longest term and one long entry maroons every short one from its
  own definition. A definition that wraps gets a hanging indent instead, so its
  second line clears the terms above it. No colon is drawn between the two: the
  `:` is how a definition line is *written*, the way `-` starts a bullet, and a
  renderer that echoes its own syntax back puts punctuation on the slide that
  nobody typed.
- Lists carried a browser's blank line above and below — right for a page,
  wasteful on a slide, where three of them cost a bullet's worth of height.
- **One sample stylesheet, not two.** `examples/themes/pitch.css` was described
  as `mirzam.css` "with a sales deck's furniture on top"; it had in fact become a
  full copy — identical in all twelve palette tokens, to the character — that
  differed only in numbers nobody had chosen: a 64px heading rule against 56, 4px
  tall against 3, a 26px grid gap against 24, an eyebrow at `.85em` against
  `.82em`. Two copies of one identity drifting apart is not two themes, and the
  file named for the pitch deck was setting the look of five reference decks. It
  is deleted; every sample deck now names `themes/mirzam.css`. The gradient rule
  is the one thing carried over rather than dropped — deep violet to pale reads
  as a mark that was drawn, where a flat bar reads as a border that happens to be
  short.
- **A `--split h2` deck lost its heading rule.** `examples/themes/*.css` drop
  the built-in full-width border under an `h2` and draw a short violet rule
  instead — but scoped to `.pane-head h2`, which the sample decks all have and a
  split document never does, since every slide it makes is one `main` pane. So
  the README published as a deck got the removal and not the replacement. The
  rule is on `h2` now: "one violet rule per section heading" was never a claim
  about what the pane is called. A centred heading gets it centred, which the
  narrower selector had never had to handle.
- **`mirzam build --mode light|dark`**, the sibling `--theme`, `--css` and
  `--fit` already had. A stylesheet may rest in either mode — `themes/mirzam.css`
  is dark by default and says so in its header — but nothing in the CSS tells the
  renderer which, and an unset mode means "follow the reader's machine". A
  dark-resting deck left unset therefore painted dark while every per-mode asset
  in it picked its *light* copy, which is how the README deck ended up with a
  light pipeline diagram on a dark slide. The deck that most needs this is the
  one that cannot carry frontmatter at all.
- **The pipeline diagram was dark whatever it was sitting on.** It was the one
  brand asset with no light twin — `docs/brand/README.md` said so, in the row
  describing it — so the README read as a deck put a black slab in the middle of
  a white slide, and the landing page did the same. There is a light version
  now, in the light palette from `docs/brand/palette.md`, and the pair is
  referenced the way the wordmark already was. On the landing page it is a
  background image rather than a `<picture>`, for the reason written beside the
  wordmark: `prefers-color-scheme` can only ask the operating system, so the
  page's own switch would have turned everything dark except the diagram. A deck
  needs no such care — the build rewrites a `<picture>` into one image per mode
  and follows `D`.
- Publishing a release left the *previous* one on the front page. Cutting a
  release pushes the version bump and then tags it, so the Pages run that CI
  triggers for that commit starts before the tag exists and rebuilds the root
  from the release before last — which is how v0.2.0 shipped with v0.1.0's
  decks at the root for nine minutes. Pages now also runs on `release:
  published`, and takes the default branch rather than `github.ref`, since on
  that event the ref is the tag and `/next/` would have stopped being `main`.
- The landing page's deck links carry the build they came from. Each deck is its
  own file, so a phone that had opened one before kept serving that copy while a
  deck visited for the first time arrived fresh — which reads as the new control
  being missing from one deck rather than as a stale page, and cost an
  afternoon working out which of the two sites was being looked at.
- The preview banner separated its `DEV` tag from the sentence with a CSS
  margin and nothing else, so it looked right and copied out — and read aloud —
  as `DEVUnreleased build of main`. The gap is a space in the markup now.

## [0.2.0] - 2026-08-10

### Added
- `examples/01-start.md`: the deck that was missing. The smallest file that
  builds, where a page breaks, `--split h2`, the three commands and the viewer
  keys — six slides between reading about Mirzam and having a deck.
- `examples/02-writing.md`: plain CommonMark on a slide, with no Mirzam
  extension anywhere in it and no theme chosen. Headings, emphasis, links,
  nested lists, quotes, `***` rules, tables, code blocks and footnotes. It is
  also the control group: anything that renders badly here is the default
  theme's fault, not a feature's.
- `examples/06-theming.md`: named themes, the four ways a colour mode is
  chosen, every frontmatter field in one place, `{#id .class key=value}`, custom
  CSS and the both-modes rule, and `fit: shrink`. `theme:`, `split:` and `fit:`
  had no sample deck at all before this.
- `bg-light=` and `bg-dark=` on a pane: one photograph per colour mode. Both
  are inlined and the deck shows the one that matches — including after the
  reader presses `D`, which a `<picture>` element cannot follow, since its
  `media` query can only ask the operating system. Naming one leaves `bg=` as
  the other mode's image; naming one *without* a partner warns, because the
  other mode would show a bare pane with photo-coloured text on it.
- `mirzam build --theme <name>`, `--css <file>` and `--fit shrink`: the
  frontmatter's theme, stylesheet and overflow behaviour, chosen from the
  command line. This is what lets a document that cannot carry frontmatter — a
  README, where it would surface as a stray table on GitHub — still be published
  as a deck with an identity, and without four of its sections cut off at the
  bottom of the slide.
- The landing page has a light/dark switch instead of only following the
  machine, and stores the choice where a deck's viewer reads it, so a deck
  opened from a light page opens light. The viewer's own `D` writes the same
  key, which also makes that toggle stick from one deck to the next.

### Changed
- **The site is published in two channels.** The root is built from the latest
  tag; `/next/` is built from the tip of `main`, carries a `DEV` banner naming
  the commit (`v0.1.0 +11 · 72433fb`), lists this file's `[Unreleased]` section,
  and asks not to be indexed. Landing a change and releasing it were the same
  action before this, which is a poor arrangement for a repository where work
  goes straight to `main`: the only way to look at a change was to publish it to
  everyone. Now a push moves `/next/` and only a tag moves the root.
- **Pages runs after CI, not beside it.** Both workflows triggered on the same
  push and raced, so a commit whose tests were red still reached the site. With
  no pull request in the way, CI is the only gate there is, and the deployment
  now waits for it.
- The sample decks are a numbered series rather than six files at the same
  level with three different jobs between them. `cookbook.md` → `03-layout.md`,
  `showcase.md` → `04-components.md`, `motion.md` → `05-motion.md`; `pitch.md`
  and `seminar.md` keep their names, because they are decks rather than
  documentation and are not read in an order. The site, the README and the
  Japanese README present the two groups separately.
- Each feature now has one home. `media.md` was two slides and a whole file, so
  its video and GIF material moved into `04-components.md` and the file is
  gone; the animation slide `04-components.md` was still carrying moved to
  `05-motion.md`, where the rest of the animation material already lived. The
  closing "that is the whole vocabulary" slide is now actually the last one.
- `main` is documented as the working branch rather than a stable one, in the
  README, its Japanese translation, `CONTRIBUTING.md` and `AGENTS.md`. It is
  where development lands directly, since a change held on a branch cannot be
  reviewed where it counts — on the deployed site. Depend on a release, not on
  `main`.
- The pitch deck's title slide carries Mirzam's own hero art, one image per
  mode, in place of the stock city photograph.
- The README deck on the site is built with Mirzam's theme rather than
  `default`, which is the one deck there that looked like someone else's.

### Fixed
- The landing page's "See a deck running" and "Source on GitHub" buttons were
  unclickable: the hero's scrim is a positioned sibling that came after the
  content, so it painted over both and swallowed every click aimed at them.

## [0.1.0] - 2026-08-09

First tagged release. Prebuilt binaries, a browser editor, and a deck you can
present from — animation, effects, annotations, a presenter window and a
contents page that writes itself.

### Added
- `scripts/record-demo.mjs`: records a deck being presented, by driving it in a
  browser rather than by anyone operating one. Writes a `.webm` with no extra
  tooling — and a GIF when a full ffmpeg is on the machine, which the one
  Playwright ships is not. Keypresses appear on screen, because a deck that
  advances by itself demonstrates nothing.
- `theme: mirzam` — Mirzam's own palette as a built-in theme, in both modes,
  so a deck gets the identity's colours from one word of frontmatter. It is the
  token half of `examples/themes/mirzam.css`; the typography stays in that file,
  because a built-in theme is loaded before the layout stylesheet and can only
  set tokens. `css: themes/mirzam.css` is still how you get the whole thing.
- **Prebuilt binaries.** `.github/workflows/release.yml` builds `mirzam` for
  x86-64 and arm64 Linux, Intel and Apple-silicon macOS, and x86-64 Windows on
  every `v*` tag, smoke-tests each native one by building a deck with it, and
  publishes the archives with checksums and that version's changelog section as
  the release notes. `scripts/install.sh` picks the right archive, verifies the
  checksum and drops the binary in `~/.local/bin` — using Mirzam no longer
  requires a Rust toolchain, which was the largest thing standing in front of
  anyone who just wanted to make a deck.
- `LICENSE`: the MIT text the README has always claimed, plus a note that the
  bundled STIX Two Math font travels under the SIL Open Font License wherever a
  deck goes.
- `docs/quickstart.md` and `docs/ja/quickstart.md`: four ways in — browser, CLI,
  VS Code, Obsidian — with an honest table of what the browser build cannot do
  and why.
- **Per-pane continuation.** `<!-- next -->` inside a pane carries that pane on
  to the next slide while every other pane holds still. The viewer recognises
  the two slides as one and cuts between them instead of turning the page, so a
  chart you are still talking about does not move. A build expands the marker
  into real slides, which means the PDF and a no-JavaScript reader get it too.
- **A contents page that writes itself.** A `toc` block collects the deck's
  headings, links each one to its slide, marks the section you are in, and
  prints page numbers instead of links in the PDF. `from:`, `depth:` and
  `current:` choose what it covers.
- **A presenter window.** `P` opens a second window with the next slide, your
  speaker notes, a clock and a talk timer. The two windows stay in step through
  `BroadcastChannel`, including dark mode and the layout overlay, so the screen
  the audience sees never disagrees with the one you are reading.
- **Viewer chrome.** A page counter and controls, and `/` for a cheat sheet that
  lists this particular deck's effect keys rather than a generic table. On a
  phone: swipe to turn the page, swipe up for notes, two-finger tap for the same
  sheet.
- **Marking a phrase and the thing it refers to, together.** `highlight`,
  `underline` and `box` take an `#id` and nothing else, and follow the line
  boxes that phrase actually occupies — a sentence that wraps gets one mark per
  line, not one box over both. Paired with a mark on a chart bar under the same
  `step`, they say "this phrase, that bar" in one colour without drawing
  anything across the slide. `target:` is now optional, because a block whose
  items are all anchored measures nothing against a box.
- **A browser editor**, published at [ayatough.github.io/Mirzam/try](https://ayatough.github.io/Mirzam/try/):
  open and save `.md`, attach, drop or paste images, and download the finished
  self-contained deck. The same Rust core as the CLI, compiled to WebAssembly;
  nothing is uploaded. It works on a phone.
- `mirzam_syntax::BLOCK_KINDS`, the canonical list of fenced forms the language
  claims. `commonmark_compat.rs` walks it, so a new block form is checked
  against a plain CommonMark parser the moment it is added — the promise that a
  deck still reads on GitHub is now kept by construction rather than by memory.
- `scripts/check-layout.mjs` learned three failure modes that HTML snapshots and
  the eye both miss: an annotation that could not be drawn (its anchor was
  renamed), an element still holding its entrance state after that entrance has
  played, and a `--debug-layout` overlay baked into a published build. The
  runtime answers for the first two through `MZAnnot.missing` and `MZAnim.armed`,
  and a test keeps those two names from drifting out from under the checker.
- `rust-version = "1.91"` in the root manifest, with a CI job that builds the
  workspace on exactly that toolchain. The README had claimed 1.75, which had
  not been true since `math-core` landed.

- `docs/brand/`: the mark, palette and type used to present Mirzam — wordmark and
  icon in light and dark, hero backgrounds, the pipeline diagram, a 1200×630
  social card, and `mirzam-theme.css` carrying the Mirzam Light / Mirzam Dark
  tokens. Documented in [docs/brand/README.md](docs/brand/README.md) and
  [docs/brand/palette.md](docs/brand/palette.md); the rasters rebuild with
  `node scripts/make-brand-raster.mjs`.
- `examples/themes/mirzam.css`: the identity as a deck theme — `css:
  themes/mirzam.css` in a deck's frontmatter. Both modes, chart series that can
  be told apart, and the brand type ladder. `examples/themes/pitch.css` keeps
  its name and its pitch-deck furniture but is redrawn in the same palette, so
  every published sample deck now looks like Mirzam.
- `srcset` is now inlined alongside `src` and `poster`, so a `<picture>` that
  offers one image for a light background and another for a dark one still
  makes a self-contained deck. Previously the source the reader's theme
  selected was the one left pointing at a relative path.
- `chart` blocks: `bar`, `line`, `area` and `pie` charts rendered to SVG at build
  time from inline CSV or a `.csv` file. Individual marks get stable ids
  (`<chart-id>-<series>-<row>`) so `connect` can point at a single bar or point.
- `examples/pitch.md` and `examples/showcase.md`, with the `themes/pitch.css`
  theme, demonstrating metric tiles, charts, diagrams and connectors.
- Pane attributes `align=` and `valign=`, plus extra classes on `::: pane`.
- VS Code extension with live preview, cursor-to-slide sync and HTML export.
- WebAssembly bindings (`mirzam-wasm`) and a browser playground under
  `web/wasm-demo`.
- Video and GIF embedding, with poster-frame substitution in PDF export.
- Quality gates in CI: CommonMark compatibility, golden snapshots, incremental
  build equivalence, and a standing performance benchmark.
- `docs/layout.md` and `examples/cookbook.md`: a layout guide whose every rule is
  demonstrated by a deck that CI renders and checks.
- `scripts/check-layout.mjs`: renders decks in a browser and fails on clipped or
  overlapping content and undrawn connectors — problems HTML snapshots cannot see.
- Documentation site published to GitHub Pages: the guides plus every sample deck
  rendered as a live page (`scripts/build-site.sh`, `.github/workflows/pages.yml`).
- `AGENTS.md` and `CLAUDE.md`: working agreement for coding agents, including how
  to split work across several agents without colliding.
- Pane background images: `bg=` with `dim=`, `blur=`, `scrim=`, `bg-fit=`,
  `bg-pos=` and `text=`, plus a `.bleed` class for a full-slide background. The
  photo is inlined like any other asset, so a deck is still one file.
- `scripts/fetch-backgrounds.sh` downloads photographs from Unsplash and records
  the attribution the API requires; `scripts/make-sample-backgrounds.py` draws
  the sample backgrounds in `examples/media/bg/` so the repository builds offline.
- Heading-based slide splitting: `mirzam build doc.md --split h2`, or `split: h2`
  in frontmatter, turns an ordinary document into a deck without editing it. The
  project README is published as a deck on the docs site to demonstrate it.
- Layout debug overlay: `L` in the viewer (or `mirzam build --debug-layout`)
  outlines every pane, labels it with its band name, and tints the grid gaps.
  Off by default and never in print.
- `anim` blocks: `mirzam-anim` compiles triggers (`enter`, `click N`, `exit`,
  `after #id`), targets (ids, classes, the whole slide, and `chars`/`words`/
  `lines` splitting), a standard effect set and easing (including `spring(...)`
  resolved to a sampled curve at build time) into the timeline JSON embedded
  per slide. Text splitting happens at build time so the wrapping spans are
  already in the HTML. A target that matches nothing is a warning, not a
  build failure.
- The animation runtime plays those timelines. `→` steps through a slide's
  `click` triggers before turning the page and the counter shows the step;
  arriving from a later slide shows every step already played. The runtime is
  inlined only into decks that animate something, and it is the only thing that
  ever puts an element in its starting state — so a deck read without
  JavaScript, and the PDF export, still show every slide fully revealed. Under
  `prefers-reduced-motion` the reveals happen without the movement.
- Slide transitions: `transition: fade | slide-left | slide-right | slide-up |
  slide-down | iris | none` in frontmatter, with an optional duration and
  `ease=`. A slide overrides its half of the page turn with an ordinary
  whole-slide `[enter] slide` / `[exit] slide` track.
- Named themes: `theme: nord | solarized | vscode` in frontmatter, alongside
  the existing `default`. Every theme defines both light and dark tokens
  explicitly (dark is never derived from light by inversion), verified by a
  unit test that computes the WCAG contrast ratio for every token against
  `--mz-slide-bg` in every theme and mode. See
  `crates/mirzam-render/src/theme/themes/CREDITS.md` for each palette's origin
  and licence.
- Dark mode: `mode: dark`/`mode: light` in frontmatter, `?mode=` in the
  viewer's URL, `D` to toggle for the session, and `prefers-color-scheme`
  when nothing is set - in that priority order, with no reload needed for the
  OS-preference case.
- `examples/motion.md`: the animation sample. Text entrances, a chart whose bars
  grow one click at a time, a diagram that assembles itself box by box and
  arrow by arrow, photos that fade in and out (and come back when you step
  back), and a slide that overrides the deck's page turn.
- More effects: `wipe-in` / `wipe-out` (an edge uncovers the content instead of
  moving it), `zoom-in` / `zoom-out` and `blur-in`. More transitions:
  `wipe-left|right|up|down` and `zoom`.
- Audio: `![Interview](talk.mp3)` becomes a player, inlined like any other
  asset. `.mp3`, `.m4a`, `.wav`, `.ogg`, `.flac`, `.opus` and friends, each
  served with the media type a browser will actually play.
- YouTube and Vimeo page URLs become embeds, from `youtube-nocookie.com`. This
  is the one thing in a deck that is not self-contained: the frame is fetched
  when the slide is shown, and the PDF gets a placeholder carrying the link.
- Media is recognised by what it points at rather than by whether attributes
  were written, so a bare `![clip](talk.mp4)` is a video instead of a broken
  image.
- `fit: shrink` in frontmatter, or `{fit=shrink}` on a pane: content that would
  overflow is scaled down until it fits rather than clipped, to a floor of 55%,
  re-measured on every page turn and resize. Runs in the PDF too, for the same
  reason the annotation overlay does — it only ever reveals what a clipped pane
  would have swallowed.
- Citations: `[^key]` footnotes render at the foot of the slide that cites them,
  and a bare DOI or arXiv URL becomes a link. `examples/seminar.md` gains a
  slide quoting a figure from the paper under discussion, annotated, pointed at
  from the prose, with its references beneath it.
- `effects` blocks: presenter-triggered flourishes bound to a key — `flash`,
  `shake`, `lines` (speed lines), `boom`, `burst 🎉`, `confetti` and a Nico-Nico-style
  `danmaku`. These are part of the performance rather than the document: they
  never reach the PDF, `Esc` clears them, a page turn cancels them, and binding
  a key the viewer already uses is a build warning. Nothing they draw can
  reflow the slide.
- `annotate` blocks: circle, box, arrow and label anything on a slide. An item
  is placed either in percentages of what the target *paints* — a pane holding
  one picture means that picture, letterboxing excluded — or by naming another
  element's id, which needs no coordinates and survives a data change. The
  overlay is re-measured on every resize, and it is the one script the print
  page carries, so the marks reach the PDF. `step=N` holds an item back until
  the Nth click, counting towards the slide's steps like any other build —
  and a page with no viewer still shows every mark. `id=` names a mark so a
  `connect` arrow can run from a sentence to the circle drawn over a
  photograph; the connector is routed once the mark exists and re-routed
  whenever it moves.
- `mirzam build --base-url <url>` says where the input file's directory lives
  once published, so a deck served from somewhere other than beside its source
  still resolves its links to other documents.

### Changed
- The release profile enables thin LTO and strips symbols: the binary went from
  6.5 MB to 4.5 MB, at a link cost paid once per tag rather than once per edit.
- Documentation no longer recommends an arrow from a sentence to a figure. An
  arrow has to leave the prose, cross the slide and land somewhere meaningful,
  and none of that was ever what the audience asked for; `connect` is now
  presented as the tool for two boxes *inside* a diagram, and text-to-figure
  goes to the paired annotation above.
- The viewer chrome takes its colours from the deck's own paper tokens instead
  of a fixed dark palette, so it is legible whatever the theme does. This is
  what made the presenter window wrong in light mode, and it was never only the
  presenter window.
- Benchmark re-measured at this release: a 500-slide deck builds in 76 ms and a
  single-slide edit re-renders in 3.2 ms, up from 2.3 ms. A build now expands
  `<!-- next -->`, resolves the contents page against the finished deck and only
  then hashes, so the whole-document pass grew while the per-slide render did
  not.

- A warning raised on a slide that came from an included file now names that
  file: `mirzam-syntax` keeps a source map from the expanded document back to
  the files it was assembled from, through nested includes, a file included
  twice, CRLF line endings and variable substitution.
- A `shape` with an id is emitted as one group: a box and its label, an arrow
  and its head. Animating `#box` now moves the whole shape rather than leaving
  its label behind, and connectors resolve against the group's box.
- A bar chart mark's id likewise names a group holding the bar *and* its value
  label, so a bar animated with `wipe-in dir=up` rises with its number on top
  instead of leaving it hanging in the air.
- `draw` no longer fades the whole shape in alongside the stroke, which showed
  an arrow's head at half strength before the line had reached it. Strokes draw
  tip-first over the full duration; fills — the head, a label's glyphs, a box's
  wash — ink in over the last stretch.
- Documentation is English-first; Japanese translations live under `docs/ja/`.
- All source comments, CLI output and UI strings are English.
- Math conversion moved from `latex2mathml` to `math-core`, fixing sub/superscript
  placement; decks containing math now bundle STIX Two Math.
- Upgraded comrak to 0.54 and enabled CJK-friendly emphasis.

- The published landing page and the browser editor now use the Mirzam palette
  and type — Space Grotesk for headings, Inter for text, IBM Plex Mono for code
  — and follow the reader's `prefers-color-scheme` instead of being dark only.
  The page carries a favicon and a social card, so a link to it unfurls.

### Fixed
- **Text selection on a phone.** The cheat sheet was bound to a long press, and
  a long press is how you select text — so a deck on a phone could not be quoted
  from. The binding is gone (two-finger tap and the `?` button open the sheet),
  and a selection drag is no longer read as a page swipe.
- Swiping right walked out of the deck: Chrome reads horizontal overscroll as
  browser-back. The deck now claims vertical panning only.
- Dark mode and the layout overlay were independent between the presenter and
  audience windows, so the two could disagree about what the audience was
  looking at. Both now travel over the same link as the slide and step.
- `--mz-muted` on `default/light` and `solarized/light` sat at 4.19:1 and 4.39:1
  against a surface — below the 4.5:1 the contrast guard requires. Caught by
  extending that guard to the muted-on-surface pair, which the chrome change
  above made load-bearing.
- A `draw` animation left a saved style snapshot on the element around the
  painted parts, which nothing ever restored; a later arming of that element
  would then quietly keep the stale one.

- `D` appeared to do nothing on the sample decks. It was working — `data-mode`
  flipped — but all four decks share `examples/themes/pitch.css`, which set its
  palette once, on a plain `:root` that outranks the built-in tokens for both
  modes. The theme now defines light and dark, and names its own shades as
  tokens instead of burying literals in rules (a literal cannot have a second
  mode). Two tests hold every theme under `examples/themes/` to the rule: each
  token set for one mode must be set for the other, and both modes must meet
  the same WCAG ratios the built-in themes do.
- Slides were transparent, so a page turn showed the departing slide through
  the arriving one and the previous layout appeared to linger. A stray `*/` had
  closed a comment early in `base.css`; the prose after it became CSS, and the
  parser's error recovery swallowed the rule that paints a slide opaque. Two
  tests now stand where the comment was: one rejects a `*/` outside a comment
  in any shipped stylesheet, the other asserts the rule itself is present.
- Turning back to a slide that declares its own `[enter] slide` track played no
  arrival at all — the custom track replaced the page turn, and a backwards
  entrance is deliberately not replayed — so the departing slide slid away with
  nothing covering it. Going backwards now always plays the deck's page turn,
  reversed.
- Arriving at a slide whose exit transition was still running left it stranded
  off-screen: the guard that stops a repaint from cancelling an animation in
  flight also skipped staging the slide being arrived at, so it kept the
  transform its exit had left behind. Reachable from the editor's cursor sync
  and from live reload, both of which repaint during a page turn.
- Advancing past the last slide (or retreating before the first, or pressing
  `End` while already there) replayed the current slide's entrance: the viewer
  clamped the index and treated the result as an arrival. Navigating to the
  slide already showing is now a no-op.
- An element faded out with a click could not be brought back: stepping back
  only re-armed the track, while the finished animation kept holding the hidden
  end state. Stepping back now cancels it and restores the element — and
  arriving from a *later* slide correctly keeps it hidden, since that exit has
  already played.
- Links inside a deck published away from its source 404'd: the README rendered
  as a deck at `/decks/readme/` still pointed at `docs/layout.md`, which does
  not exist there. The site now builds every deck with `--base-url`.
- A deck's own `css:` stopped overriding the palette: named themes moved the
  tokens behind `:root[data-theme="…"]`, which outranks the plain `:root` a
  custom stylesheet uses, so every deck with a custom theme silently reverted
  to the built-in one — dark styling on a light background in light mode, and
  text lost against the background in dark mode. Built-in theme selectors are
  wrapped in `:where()` now, so they carry no specificity and an author's
  `:root` always wins.
- The sample background used to demonstrate `blur=` was already out of focus,
  so blurring it showed nothing. It carries a crisp grid now, and the mountain
  photo a sharp treeline.
- The documentation site linked its guides as `docs/*.html`, which the static
  Pages deployment never produced — no Jekyll runs on an uploaded artifact, so
  every one of those links 404'd. The prose is now linked to GitHub, and
  `scripts/build-site.sh` fails the build if the landing page points at a file
  the artifact does not contain.
- Inline code, `pre` blocks, table headers and the parse-error box kept a
  hard-coded light background in dark mode, so their text was light on light
  and unreadable. Those surfaces are now theme tokens (`--mz-surface`,
  `--mz-danger-*`), defined per theme and mode, and a test rejects any
  hard-coded color in the shared stylesheet unless it carries a comment saying
  why it does not belong to a theme.
- `End` in the viewer went to the first slide instead of the last: it read the
  arity of the slide-list function rather than calling it.
- Clicking to select text in the viewer turned the page. A click is only a page
  turn when it is not a drag, no text is selected, and it did not land on a
  control.
- Connectors from a text anchor left sideways and struck through their own
  sentence. They now leave from the centre of the underline, through the edge
  facing the target, and follow direction-aware curves.
- A heading band drawn too short silently hid its heading behind the pane below;
  heading panes now stay legible and the layout checker reports the overflow.
- `history.replaceState` threw inside srcdoc iframes, aborting preview updates in
  embedded viewers such as the VS Code webview.
- Multi-line `$$...$$` blocks were not converted.
- TeX like `\sqrt[3]{x}` was mangled by the span attribute rule.
- Asset-only changes (replacing an image file) now reach connected clients.
- `scripts/build-wasm.sh` read the wasm-bindgen version from `Cargo.toml` instead
  of the resolved version in `Cargo.lock`, producing a confusing schema mismatch.
- A mistyped subcommand (`mirzam server`) printed the usage text with no
  explanation, which read as if the input file were at fault. It now names the
  mistake and suggests the nearest command. `--help` prints to stdout and exits
  0, and the usage text no longer loses its indentation.
- An image alone in a pane sat on a text baseline, so its descender space pushed
  it a few pixels past the band. Such an image is now laid out as a block.
- Fenced blocks were matched without regard to fence length, so a `pane` or
  `chart` block quoted inside a longer fence was executed instead of shown. This
  is how documentation about Mirzam is written, and it also fixed the README's own
  example block.
- The pipeline diagram's ASCII layout panel drew as `| | |`: XML collapses runs
  of whitespace in `<text>` unless told otherwise, which flattened the one part
  of the illustration whose point was its alignment.

## [0.0.1] - never tagged

Initial spike: CLI (`build`, `serve`, `export pdf`), ASCII pane layout, file
transclusion, variables, math, shapes and live-routed connectors.
