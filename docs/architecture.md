# Architecture

How Mirzam is put together, and why. For build instructions and conventions see
[development.md](development.md).

## Principles

1. **Never break plain Markdown.** Every extension must degrade to something
   harmless — a code block, a paragraph, literal text — when read by a plain
   CommonMark parser. A deck stays readable on GitHub and in Obsidian. This is
   automated as a test rather than left to discipline.

2. **One engine, everywhere.** The core is Rust, compiled both to a native binary
   and to WebAssembly. The CLI, the VS Code extension and the browser playground
   run the same parsing, layout and rendering code, so output cannot drift between
   them.

3. **Work per slide, not per deck.** Parsing, layout and rendering are cached per
   slide. Editing one slide re-renders one slide, whatever the deck size.

4. **The core computes geometry; the browser does typography.** Rust resolves pane
   rectangles, shape coordinates and chart geometry. Text layout — line breaking,
   font metrics, hyphenation — is left to the browser. This keeps the core small
   and avoids reimplementing a text engine.

5. **I/O is injected.** The core never touches the filesystem directly. Callers
   supply file and asset access, which is what lets the identical code run in a
   browser sandbox.

## Pipeline

```
 .md files
   │  parse (per slide, cached)
   ▼
 slide sources ── frontmatter, transclusion, pane grid, fenced blocks
   │  resolve: variables, includes, assets
   ▼
 geometry ── pane rectangles, shape coordinates, chart marks
   │  render
   ├──▶ HTML + viewer runtime  (preview, presenting, distribution)
   ├──▶ print HTML → Chromium → PDF
   └──▶ PPTX (planned)
```

Connectors are the exception: they are *not* resolved here. Rust emits the
declarations as JSON and the viewer resolves endpoint coordinates after the browser
has laid the slide out, re-running on every show, resize and hot reload. That late
binding is what makes an arrow keep pointing at the right word when the layout
changes.

## Crates

| Crate | Responsibility |
|---|---|
| `mirzam-syntax` | Frontmatter, `![[transclusion]]`, slide splitting, `::: pane` divs, fenced blocks. Knows nothing about rendering. |
| `mirzam-core` | Deck metadata and the `{{ }}` expression evaluator. |
| `mirzam-layout` | ASCII pane grid → proportional grid (areas and `fr` ratios). |
| `mirzam-shape` | Shape DSL → SVG layer, resolving shape-to-shape endpoints. |
| `mirzam-chart` | Chart DSL + CSV → SVG, assigning stable ids to every mark. |
| `mirzam-cite` | BibTeX → entries, each with a citation label and a formatted reference. |
| `mirzam-connect` | Connector DSL → JSON for the runtime. |
| `mirzam-anim` | `anim` DSL → the timeline IR, with easing curves resolved at build time. |
| `mirzam-annot` | `annotate` DSL → the annotation model drawn over a picture or a chart mark. |
| `mirzam-figure` | A laid-out page → its captioned figures: which line is a caption, and which ink belongs to it. Knows nothing about PDF. |
| `mirzam-render` | Assembles slides into HTML; owns the theme, viewer runtime and asset inlining. |
| `mirzam-cli` | `build` / `serve` / `export pdf` / `import pdf`, the caching build pipeline, the benchmark. |
| `mirzam-wasm` | wasm-bindgen bindings over the same pipeline, with host-injected files and assets. |

## Key decisions

### ASCII layout means CSS Grid

A `pane` block maps directly onto `grid-template-areas`: identical merge rules,
identical rectangularity constraint. Column widths come from character counts and
row heights from line counts, so drawing a taller band gives it more of the slide.
Reusing an existing model keeps the implementation small and makes the behavior
predictable to anyone who has used CSS Grid.

### HTML is the primary target

Video, animation and presenter tooling all have mature browser implementations.
Building on them costs far less than reinventing them, and PDF comes free by
printing the same HTML through Chromium — so the PDF matches the preview by
construction rather than by effort.

### Incremental rendering, and what it actually costs

Each slide is cached under a hash of its source and index. Cache entries also
record the mtimes of the assets a slide references, so replacing an image
invalidates exactly the slides that use it.

Change detection hashes the *rendered output*, not the source. This matters: an
image can change while the Markdown does not, and clients still need the update.

Per-edit cost is not strictly O(1). Re-reading, splitting and hashing the source is
linear in deck size — 2.3 ms for 500 slides — while re-rendering is limited to the
changed slides. That is the next thing to optimize if multi-thousand-slide decks
become a target.

### Math is converted at build time

LaTeX becomes MathML during the build, so nothing runs client-side even for decks
with hundreds of formulas, and the same output prints straight to PDF. Because
MathML quality depends on having a font with a MATH table, decks containing math
bundle STIX Two Math (~540 KB, only when math is present).

An earlier converter (`latex2mathml` 0.2, unmaintained) mis-nested sub/superscripts
into a visible staircase; `math-core` produces correct MathML Core.

### Charts are data, not pictures

A `chart` block renders SVG from CSV at build time. Every mark carries an id
derived from the chart id, series index and row index, which is what allows a
`connect` arrow to point at one bar. A screenshot cannot offer that, and neither
can an embedded image.

### A figure comes out of a paper before the build, not during it

`import pdf` writes files and stops. It could have been an asset reference —
`![Fig. 3](paper.pdf#fig=3)`, resolved while building — and that was rejected
twice over. The core may not open a file, so the browser and the editor
extension would show a placeholder where the terminal shows a figure; and a
deck would stop building when the paper moved. A command that writes an SVG
next to the deck leaves the deck ordinary: no new syntax, nothing to resolve,
and the same Markdown everywhere.

The parsing splits the way the rest of the project does. `mirzam-figure` is
given a page that has already been measured — lines with boxes, ink with boxes —
and decides what a figure is; `mirzam-cli` opens the file and measures it,
because opening files is what a process may do and a browser may not. That is
also why the geometry can be tested against pages written by hand rather than
against a committed PDF.

**Rendering the crop is where the licence line runs.** Turning a cropped page
into an image means a PDF renderer, and the two that do it well — MuPDF and
Poppler — are AGPL and GPL. Linking either would relicense an MIT project, so
neither is a dependency. They are *invoked*, when installed, exactly as
Chromium is for the PDF export and `mmdc` is for a `mermaid` fence: a separate
program, discovered on `PATH` or named by an environment variable, never
shipped. Without one the command still answers — an image stored in the page
comes out whole, and a drawn figure comes out as a one-page PDF — which is the
same shape of promise the `mermaid` fence makes when no renderer is installed.

### Annotations and the PDF

An annotation is positioned as a percentage of the box its target *paints*, and
an anchored one takes the live box of the element it names. Neither is known
until the browser has laid the slide out, so — like connectors — the overlay is
drawn at runtime rather than baked into the HTML.

That leaves the export with a choice, since the print page otherwise ships no
JavaScript at all: drop the annotations from the PDF, or let the print page run
the one script that draws them. **It runs the script.** The rule the no-script
print page protects is that a deck read without JavaScript shows every slide in
full; an annotation is drawn *over* the slide and hides nothing, so a scriptless
read is the deck minus its marks, not a deck with something missing from the
middle of it. Chromium loads and runs the page before printing, so the marks
land in the PDF at exactly the coordinates the viewer shows.

`theme/annot.js` is therefore written to stand alone: it never reaches for the
viewer, the active slide or the animation runtime, and a unit test enforces
that.

## Runtime

The viewer shipped inside each deck handles navigation, scaling, speaker notes,
video and connector routing. It is deliberately small and framework-free.

`serve` adds a hot-reload client: it long-polls for a diff and replaces only the
`<section>` elements that changed. The VS Code extension does the same thing
through the WASM core instead of HTTP.

## Risks

| Risk | Mitigation |
|---|---|
| ASCII layout is too rigid for complex slides | Semantics limited to CSS Grid; anything else belongs on the shape layer |
| Stale output after an incremental build | Equivalence with a full rebuild is a test, not an assumption |
| PDF diverging from the preview | Both come from the same HTML through Chromium |
| PPTX export losing fidelity | Elements that do not map will be rasterized; planned from the start |
| Scope growth | The roadmap states explicitly what each phase excludes |
