# Mirzam syntax

Mirzam decks are CommonMark files. Everything below is an addition that a plain
Markdown parser still renders as readable text — that rule is enforced by
`crates/mirzam-cli/tests/commonmark_compat.rs`.

> **In a hurry, or handing this to a model?** [llms.md](llms.md) is the same
> markup compressed to one page — every block, every frontmatter field, one
> example each, and the traps called out. This page is the reference behind it.

| Extension | What a plain Markdown parser shows |
|---|---|
| Fenced blocks (`pane`, `shape`, `connect`, `chart`) | A code block |
| Those same blocks inside a longer fence (````` ```` `````) | Quoted, not executed — this is how a document shows Mirzam syntax |
| Fenced divs (`::: pane main`) | A paragraph of text |
| Inline attributes `{#id .class k=v}` | Literal text (Pandoc reads them as attributes) |
| Marks `==highlight==`, `++underline++`, `:tada:` | Literal text (many renderers, GitHub included, read them) |
| Term lists (`Term`, then `: meaning`) | The two lines, as written |
| Variables `{{ price * 12 }}` | Literal text |
| Transclusion `![[file.md]]` | An image-like link (Obsidian embeds it) |
| Speaker notes `<!-- note: ... -->` | Nothing; it is an HTML comment |

## Chapters

This file runs long; jump straight to the one you need.

| Chapter | Covers |
|---|---|
| [Deck and slides](#deck-and-slides) | Frontmatter, how a document splits into slides, speaker notes |
| [Layout](#layout) | The `pane` grid and `::: pane` — see also the dedicated [layout guide](layout.md) |
| [Inline syntax](#inline-syntax) | Plain CommonMark on a slide, headings, `{#id .class}` attributes, the marks a slide reaches for (`==highlight==`, term lists, …), syntax-highlighted code, variables, transclusion |
| [Charts](#charts) | The `chart` block: types, CSV data, per-mark ids for `connect` |
| [Shapes](#shapes) | The `shape` block — page coordinates at slide top level, pane coordinates inside a `::: pane` |
| [Connectors](#connectors) | The `connect` block: arrows between text anchors, shapes and chart marks |
| [When a slide has too much on it](#when-a-slide-has-too-much-on-it) | `--fit shrink`, `<!-- next -->`, and when to just make another pane |
| [Table of contents](#table-of-contents) | The `toc` block — an agenda slide that links to itself |
| [Citations](#citations) | Footnotes with `[^key]` — **the definition must be on the same slide as the reference** |
| [References](#references) | `[@key]` against a `bibliography:` file, listed and linked by the `bibliography` block |
| [Presentation effects](#presentation-effects) | The `effects` block: flourishes bound to a key, fired while presenting |
| [Annotations](#annotations) | The `annotate` block: circle, underline, box, arrow, pointing at a live element |
| [Animations](#animations) | The `anim` block: entrances, click steps, exits |
| [Driving the viewer](#driving-the-viewer) | Keyboard shortcuts, presenter mode, the `/` shortcut sheet, [the Markdown behind a slide](#the-markdown-behind-a-slide) |
| [Theming](#theming) | `theme:` — a built-in name or [a theme of your own](#a-theme-of-your-own-in-a-file) — `mode:`, [a theme on one pane](#a-theme-smaller-than-a-deck), and the tokens a theme writes in |

## Deck and slides

### Frontmatter

```yaml
---
title: Quarterly review
author: Your Name
aspect: "16:9"        # or "4:3"
theme: mirzam         # a built-in name, a .css path, or a list of both
transition: fade      # how pages turn; see Animations below
masters: masters.md   # named slide shapes; see Slide masters below
layout: body          # the master a slide takes when it draws no grid
footer: Internal      # drawn on every slide, and in the PDF
slide-number: "{n} / {total}"
bibliography: refs.bib   # references `[@key]` can cite; see References below
citation-style: numeric  # `[1]`, or `author` for `[Vaswani+17]`
grid-pad-x: 60px      # the grid's margin and gutter; see Shapes below for
grid-pad-y: 44px      # why these are deck settings rather than CSS
grid-gap: 20px
vars:
  product: Mirzam
  price: 1200
---
```

### Slide breaks

Slides are separated by a horizontal rule (`---`) outside code fences.

A document written without slide breaks — a README, a set of notes — becomes a
deck by starting a new slide at every heading:

```bash
mirzam build README.md --split h2      # or h1, h3
```

or, for a file you own, in frontmatter:

```yaml
split: h2
```

`---` still breaks slides either way. Content before the first heading becomes the
opening slide. A section longer than a slide will overflow; the layout checker
reports it and [the layout guide](layout.md) says what to do about it.

### Splitting a deck across files

```markdown
![[sections/method.md]]
```

The file is expanded in place, slide breaks included. Frontmatter in the included
file is ignored, and circular includes are reported rather than followed.

Put a `---` on each side of the include unless you mean the section to continue
the slide before it: the file is pasted in where the line was, so without one
its first slide joins its neighbour.

Sections written by different people usually want the same slide shapes; see
[masters across a deck split into several files](#masters-across-a-deck-split-into-several-files).

### Speaker notes

```markdown
<!-- note: Skip the derivation if time is short. -->
```

Press `N` in the viewer to show them.

## Layout

For how space is allocated and what to do when content does not fit, see the
[layout guide](layout.md).

A slide's layout is one `pane` block. Without it the slide is a single pane.

````markdown
```pane
+--------------------+-------------+
|  head                            |
+--------------------+-------------+
|                    |             |
|  main              |  fig        |
|                    |             |
+--------------------+-------------+
```
````

- `+ - |` draw the borders; the identifier inside a cell names the pane.
- Column widths come from the character widths between borders; row heights from
  the number of lines. Draw a taller band to give it more of the slide.
- Repeat a name in adjacent cells to merge them. Merged regions must be
  rectangular, the same constraint CSS Grid areas have.
- Use `.` or leave a cell blank to leave it empty.

Assign content with a fenced div:

```markdown
::: pane main
Ordinary Markdown goes here.
:::

::: pane fig {align=center valign=middle}
![Result](img/result.svg){fit=contain}
:::
```

Pane attributes: `align=left|center|right`, `valign=middle|bottom`,
`columns=2` (balance the pane's content across that many columns — see
[the layout guide](layout.md#columns-inside-a-pane)),
`theme=`/`mode=` ([a palette for one pane](#a-theme-smaller-than-a-deck)), and
any extra `.class` names your own theme or a `<style>` block defines. Content
that is not assigned to a pane flows into `main`, or the first pane if there
is none.

### Slide masters

Most decks use three or four shapes over and over, and redrawing the same ASCII
on every slide is the part nobody enjoys. Name the shapes once, and slides pick
one instead.

The shapes usually go in a file of their own, named the way a theme file is:

```yaml
---
masters: masters/cookbook.md   # relative to this deck
layout: body                   # what a slide takes when it names nothing
---
```

That file is ordinary Markdown. A heading names a master, the `pane` block
under it is the drawing, and the prose between them is where the master says
what it is for:

````markdown
# Deck masters

## two-up

Two columns under a full-width heading band.

```pane
+----------------+-----------------+
|  head                            |
+----------------+-----------------+
|                |                 |
|  main          |  fig            |
|                |                 |
+----------------+-----------------+
```
````

A section with no `pane` block is not a master, which is what lets the file
open with a title and an introduction. A longer fence quotes a drawing without
defining it, the same rule a deck follows. A name defined twice is a warning,
and the last one wins.

[`examples/masters/cookbook.md`](../examples/masters/cookbook.md) is a working
one; `examples/03-layout.md` is drawn on it.

A short set can stay in frontmatter instead, as a mapping:

```yaml
---
masters:
  two-up: |
    +----------------+-----------------+
    |  head                            |
    +----------------+-----------------+
    |  main          |  fig            |
    |                |                 |
    +----------------+-----------------+
---
```

One key, two forms: a string is a path, a mapping is the shapes themselves.
Prefer the file once there is more than one shape — the drawings have to be
indented inside a YAML block scalar here, which is exactly what a `pane` fence
in a file of its own avoids, and a set in a file can be shared by every deck
beside it.

A master's value is exactly the drawing a `pane` block holds, and it is read by
exactly the same parser — everything in [the layout guide](layout.md) about
column widths and band heights applies unchanged.

A slide picks one with an HTML comment, the same form a per-slide theme takes,
so a plain Markdown reader shows nothing:

```markdown
<!-- layout: two-up -->

::: pane fig
![Result](img/result.svg)
:::
```

The order, innermost first:

1. **The slide's own `pane` block**, if it has one. A master is what a slide
   falls back to, never something that overrides the grid you drew.
2. **`<!-- layout: name -->`** on the slide.
3. **`layout:`** in frontmatter, the deck's default.
4. Otherwise a single pane, as always.

`<!-- layout: none -->` opts one slide out of the deck's default and gives it
the whole surface back — a title slide or a full-bleed photograph is usually
the reason.

An unknown name is a warning naming the slide, never a failed build, and the
slide keeps what it would have had without the name: its deck's default. An
unknown `layout:` in frontmatter is reported once for the deck rather than once
per slide. A masters file that cannot be read is a warning too, and the deck
builds with every slide as a single pane — the warning says so, because
"cannot read" on its own does not explain why the layouts vanished.

One thing a master is not: it is **a shape, not content**. Pane names come from
the drawing, so a deck's masters and its `::: pane` blocks have to agree on
`head`/`main`/`fig`, and a name in one that is not in the other is the usual
first mistake.

#### Masters across a deck split into several files

A deck assembled from [transcluded section files](#splitting-a-deck-across-files)
is where a shared set of shapes earns the most, and it works — with one rule to
know:

**`masters:` goes in the root deck.** Only the root's frontmatter is read; a
transcluded file's is ignored, the way it is for every other setting. Sections
still pick shapes freely with `<!-- layout: two-up -->`, since that is a slide
setting rather than a deck one.

```yaml
# deck.md — the root
---
masters: masters/deck.md
layout: body
---
```

```markdown
<!-- sections/method.md -->
<!-- layout: two-up -->

::: pane fig
…
:::
```

A section file can still carry frontmatter of its own naming the same file, so
its author can build and preview that section alone with the right shapes:

```yaml
# sections/method.md — ignored when transcluded, used when built directly
---
masters: ../masters/deck.md
layout: body
---
```

The path is relative to the file that declares it, which is why the root says
`masters/deck.md` and a section one directory down says `../masters/deck.md`.
Both spellings are the same file, and saying it twice costs nothing.

Naming a **different** file there is a warning, because it is the one case
where ignoring a transcluded file's frontmatter changes the slides rather than
costing nothing: the section is drawn on the deck's shapes, not the ones its
author previewed it on, and if the two sets share pane names nothing else in
the build would ever say so.

```
⚠ sections/method.md: its `masters:` names different shapes from the deck's; a
  transcluded file's frontmatter is not read, so these slides are drawn on the
  deck's masters, not the ones this file was previewed on
```

Everything else behaves as one deck. Slide numbers run across the files, the
footer is the root's, and `serve` watches the masters file along with every
section — so editing a shared shape updates the slides that use it wherever
they were written, in one hot reload.

The coordination cost is the pane names: a master fixes them, so everyone
writing a section has to call their panes what the shape calls them. A slide
that names a master the root never declared is a warning saying which file it
is in, and it says `(this deck defines none)` when the root has no `masters:`
at all — which is the mistake this arrangement makes, and it is fixed in the
root rather than in the section the warning points at.

### A footer and a slide number on every slide

```yaml
---
footer: "Quarterly review — internal"
slide-number: "{n} / {total}"
---
```

Both are drawn along the bottom of every slide, in the band the grid already
holds back as its margin, so they take nothing from the content. `{n}` is the
slide's own number and `{total}` is the deck's; both work in either field, and
`{{ }}` variables are substituted as they are anywhere else. The footer sits
against the left margin and the number against the right, on the same margin as
the words above them.

They are part of the document, so they are in the PDF too — unlike the viewer's
own counter in the corner of the window, which is an aid for whoever is driving
and never prints.

A slide drops both with a comment:

```markdown
<!-- chrome: none -->

# Quarterly review {.title-slide}
```

Use it on a title slide, on a section divider, and on any slide whose
`.bleed` background covers the whole surface — the photograph is then painted
over the margin the footer was going to sit in. There is no automatic
suppression: a rule that silently dropped a confidentiality notice would be
worse than one that draws it somewhere you can see and fix.

A deck that sets neither carries neither; the element is not in the markup at
all.

### Background images

A pane can carry a photograph behind its text, with the treatments that make the
text readable over it.

```markdown
::: pane hero {.bleed bg=media/bg/city.jpg dim=0.4 blur=3 scrim=bottom}
# Ship the story
Plain Markdown in. Presentation-grade decks out.
:::
```

| Attribute | Values | Effect |
|---|---|---|
| `bg=` | a path | The image. Local files are inlined like any other asset. |
| `bg-light=`, `bg-dark=` | a path | A different image for that colour mode, overriding `bg=` there. Naming one leaves `bg=` as the other mode's image. |
| `bg-fit=` | `cover` (default), `contain` | How the image fills the pane. |
| `bg-pos=` | a CSS position, e.g. `top`, `20% 40%` | Which part survives the crop. |
| `dim=` | `0`–`1` | Darkens the whole image. `0.4` is a good starting point. |
| `blur=` | pixels | Pushes the photo out of focus so text reads first. |
| `scrim=` | `bottom` (default), `top`, `left`, `right` | Fades that edge to black, leaving the rest of the photo visible. |
| `text=` | `light`, `dark` | Overrides the text colour. Light is chosen automatically whenever `dim` or `scrim` is set. |
| `.bleed` | class | Takes the background out to the slide edge — the edges this pane reaches. A pane that is the whole slide covers it; a pane drawn down one half bleeds on three sides and leaves the pane beside it alone. See [the layout guide](layout.md#text-over-a-photograph). |

`dim` and `scrim` combine: `dim` sets the floor, `scrim` adds the gradient on top
of it. If you set only `scrim`, the gradient runs from 0.75 to transparent.

A deck that is read in both colour modes can name a photograph for each:

```markdown
::: pane hero {.bleed bg-light=media/bg/dawn.jpg bg-dark=media/bg/night.jpg dim=0.35}
```

Both images are inlined, and the deck shows whichever matches the mode it is in
— including after the reader presses `D`, which a `<picture>` element could not
follow: its `media` query can only ask the operating system. The treatments
(`dim`, `blur`, `scrim`, `text`) apply to both, so pick a pair that wants the
same handling. `text=dark` is often the right one here: it takes the theme's own
foreground colour, which flips with the mode the way the photo does.

A PDF has no reader to ask, so the export follows the deck's `mode:` and prints
the light image when there is none. A deck whose theme is dark by default
should say `mode: dark`, or its PDF will pair the light photo with light text.

Photographs are the one asset that can dominate a deck's file size. A
1600px-wide JPEG at quality 70 is around 100 KB; a 4000px original is several
megabytes, and it is inlined into every build. Downscale before you commit.

To pull photos from Unsplash, with the attribution the API requires:

```bash
export UNSPLASH_ACCESS_KEY=...
./scripts/fetch-backgrounds.sh mountains "city at night"
```

The images in `examples/media/bg/` are drawn by
`scripts/make-sample-backgrounds.py`, not downloaded, so the repository builds
with no network access.

## Inline syntax

### The Markdown you already write

All of CommonMark works on a slide, plus GitHub's tables, strikethrough and task
lists. Listing it may look redundant; it is not. This reference described only
what Mirzam *adds* for two releases, so "does it do tables?" had no answer here,
and both strikethrough and task lists shipped working and undocumented.

| | |
|---|---|
| `**bold**`, `*italic*`, `inline code` | as anywhere |
| `~~text~~` | strikethrough (GFM) |
| `# ` to `###` | headings; `#` is the deck's title |
| `- ` and nested `  - ` | bullets |
| `1.` numbered lists | the renderer counts, so inserting an item renumbers the rest |
| `- [ ]` / `- [x]` | task lists (GFM) |
| `> ` | a quotation |
| `***` or `___` | a horizontal rule — **not** `---`, which breaks the slide |
| `[text](url)`, bare URLs | links, kept clickable, printed beside the words in the PDF |
| `| a | b |` | tables; `---` left, `---:` right, `:---:` centred |
| ` ``` ` fences and indents | code blocks, [syntax highlighted](#syntax-highlighting) when the fence names a language |
| `[^key]` | footnotes, landing on the slide that cites them |
| `[@key]` | a reference from the deck's bibliography, listed at the back |
| `<!-- -->` | comments; `<!-- note: -->` is a speaker note |
| raw HTML | passed through, so `<div class="box">` works |

`crates/mirzam-cli/tests/markup_coverage.rs` holds this table, the renderer and
`examples/02-writing.md` to each other: a mark that renders but is missing from
either fails the build.

### Syntax highlighting

A fence that names a language is coloured:

````markdown
```rust
fn main() {
    println!("hello");
}
```
````

**36 languages**, via [synoptic](https://crates.io/crates/synoptic) — Rust,
Python, JavaScript, TypeScript, Go, C, C++, C#, Java, Kotlin, Swift, Ruby,
PHP, Haskell, Scala, Lua, R, SQL, HTML, CSS, XML, JSON, YAML, TOML, Markdown,
shell and diff among them. Common aliases work: `py`, `js`, `ts`, `rs`,
`c++`, `golang`, `bash`, `zsh`, `yml`, `latex`.

**A language nobody recognises stays a plain block**, exactly as it rendered
before highlighting existed — and so does a fence with no language at all, and
so do Mirzam's own block kinds (`chart`, `shape`, `pane`, …) when they appear
somewhere that leaves them as code. Nothing to switch off: an unhighlighted
block is not a degraded one.

Highlighting happens **at build time**. The deck carries `<span>` runs and no
highlighter, so it stays one self-contained file with no client-side
JavaScript, and the PDF export — which never runs a script — is coloured too.

**The colours are the deck's, not the highlighter's.** Six theme tokens carry
them, so code in a `nord` deck reads Nord and code follows the deck through
`D`:

| Token | Colours |
|---|---|
| `--mz-code-keyword` | reserved words, macros, tags, attributes |
| `--mz-code-string` | strings, characters, links |
| `--mz-code-comment` | comments, block quotes |
| `--mz-code-function` | calls, types, namespaces, keys, headings |
| `--mz-code-number` | numbers, booleans, other literal constants |
| `--mz-code-operator` | operators, punctuation, list and table markup |

Every built-in theme sets all six, in both modes, and the contrast test holds
them to 4.5:1 against the code block's background. Override one in a theme of
your own like any other token:

```css
:root { --mz-code-comment: #7a8b9a; }
```

A deck whose theme sets none of them still gets coloured code: each token
falls back to a colour the palette already defines.

### Attributes

```markdown
## Heading {#intro .center}
[a phrase]{#anchor .u}
![Figure](img/a.png){#fig1 fit=contain w=80%}
```

`#id` names an element so `connect` and (later) `anim` can target it.

The classes the renderer brings, before any theme adds its own:

| Class | |
|---|---|
| `.u` | an accent-coloured rule under the words |
| `.center` `.right` | alignment |
| `.small` `.big` `.huge` | size |
| `.muted` `.accent` `.accent2` `.danger` | colour |
| `.box` | a bordered aside *inside* a pane, sized in `em` so it tracks the text it interrupts |
| `.card` | a pane raised off the slide, sized in `px` so a row of them agrees |
| `.eyebrow` | the small tracked label above a heading |
| `.metric` | one number, at the size a number is worth saying — with `.metric-up` and `.metric-label` |

Every colour here is a theme token, so it moves with the palette and survives
`D`. That is also why there is no syntax for writing a colour directly: a hex
value picked against a white slide is the one thing that cannot follow the
deck into dark mode.

**A `[span]{...}` has to fit on one source line.** The transform runs line by
line, so a span whose text is wrapped across two lines is left alone and the
brackets and braces reach the slide as literal characters. Rewrap the sentence,
or split it into two spans.

### Marks beyond CommonMark

```markdown
==marked== and ++underlined++ and :tada:

Term
: What the term means.
```

| Written | Becomes | In a plain Markdown reader |
|---|---|---|
| `~~text~~` | struck through | struck through — this one is GFM, not ours |
| `==text==` | a marker-pen wash in the accent colour | literal `==text==` |
| `++text++` | an underline | literal `++text++` |
| `:tada:` | 🎉 | literal `:tada:` |
| a line, then `: definition` | a term list | the two lines, as written |

A term list sets the definition **beside** its term rather than under it, the
way Typst sets one:

```markdown
Apple
: A red fruit.

Orange
: A mandarin.
```

```
Apple   A red fruit.
Orange  A mandarin.
```

**No colon is drawn.** The `:` is how a definition line is written — the marker,
the way `-` starts a bullet — and a renderer that echoes its own syntax back
puts punctuation on the slide that the author never typed. Weight, colour and
the gap separate the term from its meaning.

The definition follows immediately rather than lining up in a column with its
neighbours — a column has to be as wide as the longest term, so one long entry
maroons every short one from its own definition. A definition that wraps is
given a hanging indent instead, so its second line clears the terms above it.

Which shape is right is a per-list question rather than a house style, so two
classes on the **pane** change it:

| On the pane | The list becomes |
|---|---|
| *(nothing)* | `Apple   A red fruit.` — the definition beside the term, wrapping to a hanging indent |
| `{.terms-aligned}` | every definition in one column, for definitions meant to be read against each other |
| `{.terms-stacked}` | the definition on its own line, indented, for definitions long enough that the term reads as a heading |

```markdown
::: pane glossary {.terms-aligned}
Apple
: A red fruit.
:::
```

Three lengths tune all three shapes, and can be set on the pane, the deck or a
theme:

| Custom property | Default | Sets |
|---|---|---|
| `--mz-terms-hang` | `2em` | the hanging indent — `0` turns it off, and it is also the stacked indent |
| `--mz-terms-gap` | `.6em` | the space between a term and its definition |
| `--mz-terms-col` | `38%` | the widest the term column may get under `.terms-aligned` |

### The list marker

Bullets are most of what a slide is made of, so the kind of bullet is a choice
rather than something the renderer settles for you. Six more custom properties,
set the same three places:

| Custom property | Default | Sets |
|---|---|---|
| `--mz-bullet` | `disc` | the top-level bullet |
| `--mz-bullet-2` | `circle` | the bullet one level in |
| `--mz-bullet-3` | `square` | the bullet two levels in |
| `--mz-number` | `decimal` | the top-level number |
| `--mz-number-2` | `decimal` | the number one level in |
| `--mz-number-3` | `decimal` | the number two levels in |
| `--mz-marker` | the text colour | the colour of every marker |

Each takes anything `list-style-type` does: a keyword (`square`, `upper-roman`,
`lower-alpha`, `decimal-leading-zero`, `none`) or a quoted string.

```css
.plan {
  --mz-bullet: "→  ";
  --mz-number: upper-roman;
  --mz-marker: var(--mz-accent2);
}
```

**A string marker carries its own trailing space.** The browser adds none after
one, so `"→"` sets the arrow flush against the first word; `"→  "` is what you
want. Keywords are spaced for you.

Each depth reads its own property, so setting `--mz-bullet` changes the top
level and leaves the hollow circle under it alone. Footnote numbering ignores
all of this — its marker has to match the `[^a]` that cites it, and a citation
is always a numeral.

**Underline is `++`, not `__`.** Some editors take double underscores for an
underline; CommonMark and GFM both read them as **bold**, and Mirzam's whole
premise is that the same file renders on GitHub. Taking `__` would mean a
document written anywhere else silently changes meaning when it becomes a
deck — with no way to warn, since `__bold__` is perfectly valid markup.

Typing the emoji character directly always worked and still does; the
shortcode is for the keyboards that make that hard.

Task lists work too — `- [ ]` and `- [x]` — and always have. Mirzam draws the
box itself rather than leaving the browser's: the native one is about 13px
whatever the type around it does, and takes its colour from the operating
system, which is the one mark on a slide that would not follow the theme or
`D`.

### Variables and arithmetic

```markdown
{{ product }} costs {{ price * 12 }} per year, or {{ round(price / 30) }} per day.
```

Values come from frontmatter `vars`. Arithmetic, parentheses, and `round`, `ceil`,
`floor` are supported. Anything that fails to evaluate is left as written, so a
typo never silently deletes text.

### Math

```markdown
Inline $E = mc^2$, and display style:

$$
\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
$$
```

LaTeX is converted to MathML when the deck is built, so nothing runs in the
browser. Decks containing math bundle the STIX Two Math font (~540 KB) so they
render identically on machines without one installed. If a formula fails to
convert, its source is shown in red with the error in the tooltip.

#### Typst-flavoured math

LaTeX is hard to write from memory; [Typst's math syntax] is not. Setting
`math: typst` in frontmatter switches what `$...$` and `$$...$$` hold — for
the whole deck, so a deck reads as one language. The default is `latex`, and
every existing deck renders exactly as it always did.

```markdown
---
math: typst
---

The roots are $x = (-b pm sqrt(b^2 - 4a c))/(2a)$, and

$$
sum_(i=1)^n i = (n(n+1))/2
$$
```

The supported surface, all lowered to LaTeX and rendered through the same
MathML path:

| Form | Written as |
|---|---|
| Fractions | `a/b` — binds the adjacent term; `(a + b)/c` groups without printing the parens |
| Scripts | `x^2`, `x_(i+1)`, `x_i^2`, `x^-1`; `'` and `!` stay with their base |
| Roots | `sqrt(x)`, `root(3, x)` |
| Bars and fences | `abs(x)`, `norm(v)`, `floor(x)`, `ceil(x)`; `[a, b]` and the mixed pairs intervals need — `[0, oo)`, `(0, 1]` — all growing with their contents; `angle.l u, v angle.r` for ⟨u, v⟩ |
| Big operators | `sum_(i=1)^n`, `product`, `integral_0^oo` and `integral.double`/`.triple`/`.cont`, `union.big`, `lim_(x -> 0)` |
| Greek | by name: `alpha`, `pi`, `Omega` — following Typst's glyphs, so `epsilon` is ε and `epsilon.alt` is ϵ |
| Accents | `hat(x)`, `dot(x)`, `ddot(x)`, `tilde(x)`, `macron(x)`, `arrow(v)`, `overline(x)`, `underline(x)`; `hat` and `arrow` widen over more than one glyph — `hat(A B)` |
| Letter styles | `bb(R)`, `cal(F)`, `frak(g)`, `bold(v)`, `upright(d)`, `sans(A)`, `mono(m)` |
| Functions | `sin`, `cos`, `log`, `min`, … set upright, gluing to their `(...)`; `op("argmax")` for one the tables lack |
| Arrows and relations | `->` `=>` `<-` `!=` `<=` `>=` `\|->` `<<` `>>`, the long forms `-->` `<--` `<->` `<-->` `==>` `<==` `<=>` `<==>`, the tailed `->>` `<<-` `>->`, `in`, `subset`, `union`, `approx`, `perp`, `parallel`, `divides`, `models`, `tack.r`/`tack.l`, and dotted variants: `subset.eq`, `in.not`, `arrow.l.r`, … |
| Symbols | `infinity` (or `oo`), `partial`, `nabla`, `hbar`, `times`, `dot`, `pm`, `and`, `or`, `dots`, `...`, `dots.c`, `ell`, `Re`, `Im`, `aleph`, `angle`, `degree`, `star`, `dagger`, `compose`, `convolve`, `without`, `therefore`, `because`, `top`, `bot` |
| Spacing | `thin`, `med`, `thick`, `quad`, `wide` — from `\,` up to `\qquad`; `space` for an ordinary one |
| Blackboard | `NN`, `ZZ`, `QQ`, `RR`, `CC`, `EE`, `PP` — the doubled capital Typst uses; `bb(X)` for any other letter |
| Differentials | `dif` sets an upright d: `integral_0^t f(s) dif s` |
| Matrices | `mat(1, 2; 3, 4)` — `,` separates cells, `;` rows; `mat(delim: "[", …)` picks the brackets |
| Vectors | `vec(1, 2)` is a column; `binom(n, k)` a binomial |
| Cases | `cases(x^2 &"if" x > 0, 0 &"otherwise")` |
| Braces | `underbrace(a + b, "label")`, `overbrace`, `cancel(x)` — keep the *base* under about eight em and put the words in the label; see below |
| Text | `"km/h"` renders upright, spaces kept |
| Alignment | `&` lines up equations, `\` breaks the line |
| Escapes | `#` strips the next character of its meaning: `a #/ b` is a slash, not a fraction |

Two things to know that are not the parser's doing:

- **A brace stops growing at about eight em.** `underbrace` and `overbrace`
  draw a stretchy character the browser assembles out of pieces, and it stops
  extending that assembly at roughly eight em — past which the brace is drawn
  shorter than the base and flush left, so the last characters of the base
  have nothing under them. It is a rendering-engine limit rather than a markup
  error, and it reaches the PDF for the same reason. Write
  `underbrace(P, "the over-confident term")`, not
  `underbrace(P "is over-confident", "…")`: the label has no brace to outgrow.
  A build warns, naming the slide, when a base looks wide enough to hit it.
- **`EE` is 𝔼 here, where Typst reads it as ∃.** The doubled capitals are
  blackboard letters in this subset — `EE`, `PP`, `RR`, `NN`, `ZZ`, `QQ`, `CC`
  — because a deck writing `EE[x]` means an expectation far more often than it
  means "there exists". Write `exists` for ∃.

It is a deliberate subset — a parser of our own rather than a dependency on
Typst, whose own math goes through its layout engine to SVG, not MathML. A
formula using something outside the subset shows its source in red, the same
way a broken LaTeX formula does — including an unknown dotted name, an unknown
word used like a function, and an unknown bare name of three letters or more,
all of which refuse to render rather than quietly becoming a run of italic
letters that merely resembles the formula. Two letters side by side are still
the product a LaTeX author writes the same way, so `dx` and `dt` are variables;
for anything longer the error says how to ask for each reading — `d i f` for
variables, `"dif"` for upright text, `op("dif")` for an operator.

[Typst's math syntax]: https://typst.app/docs/reference/math/

### Media

```markdown
![Demo](media/demo.webm){.autoplay .loop .controls poster=media/first.png fit=contain}
![Animation](media/loop.gif){w=60%}
```

`mp4`, `webm`, `ogv`, `mov` become `<video>`; everything else stays an image.
`autoplay` implies `muted`, since browsers block audible autoplay. In PDF output a
video is replaced by its poster, or a placeholder if none was given.

Prefer `webm` for distribution: Chromium builds without proprietary codecs cannot
play H.264.

#### A `<picture>` that picks art by colour scheme

The markup a README uses so its logo survives GitHub's dark theme is rewritten
into one image per mode:

```html
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo-dark.svg">
  <img src="logo-light.svg" alt="Mirzam" width="340">
</picture>
```

Written as-is it would follow the **machine**, while the deck's mode follows
`mode:`, `?mode=` or the reader pressing `D` — so a light deck on a dark phone
showed the pale logo on a white slide. Both images still ship; which one is
displayed now follows the deck. Every other attribute you wrote, `alt` and
`width` included, is carried into both copies.

A `<picture>` whose sources select on anything else — a width breakpoint, a
`webp` fallback — is left alone, because there the element is doing a job this
would break.

## Charts

````markdown
```chart
type: bar          # bar | line | area | pie
id: latency        # optional; ids of individual marks derive from it
title: p95 latency by region (ms)
y_label: ms
highlight: after   # dim every other series
data: |
  region, before, after
  us-east, 210, 120
  ap-ne, 380, 180
```
````

The first column holds categories; every other column is a series. `data` may
instead name a `.csv` file, which is resolved like any other asset (and watched by
`mirzam serve`). Values may contain `%` or thousands separators.

Each mark gets an id of the form `<chart-id>-<series>-<row>`, so the second bar of
the first series above is `#latency-0-1`. That is what makes it possible to point
an arrow at one bar. For a bar chart the id names a group holding the bar *and*
its value label, so animating a mark moves the number with the bar.

## Shapes

Shapes are drawn on a layer above the panes. Where the block is written
decides what its percentages mean:

- **At slide top level**, coordinates are page coordinates — percentages of
  the whole slide. The drawing ignores the grid deliberately; this is the form
  for a figure that spans panes or annotates the slide as a whole.
- **Inside a `::: pane`**, coordinates are percentages of *that pane's
  rectangle*: `at(50%, 50%)` is the centre of the pane, and resizing the pane
  in the ASCII grid moves the whole drawing with it. Nothing clips at the
  pane's edges — `at(110%, …)` deliberately reaches out of it, the way a
  page-level shape may reach across one.

````markdown
```shape
rect    #cache at(72%, 30%) size(30%, 14%) label="Cache" fill=@shape-fill stroke=@accent2
ellipse #db    at(72%, 70%) size(26%, 16%) label="Database"
arrow   from(#cache.s) to(#db.n) style=dashed
line    from(10%, 90%) to(40%, 90%)
text    #cap   at(72%, 88%) "95% hit rate" .small
```
````

````markdown
::: pane fig
```shape
rect #in  at(50%, 20%) size(70%, 22%) label="Input"
rect #out at(50%, 80%) size(70%, 22%) label="Output" stroke=@accent2
arrow from(#in.s) to(#out.n)
```
:::
````

- Shapes: `rect`, `ellipse`, `text`, `arrow`, `line`.
- Edges for endpoints: `.n`, `.s`, `.e`, `.w`, `.c`.
- Colors: `@accent1`, `@accent2`, `@shape-fill`, … resolve to theme variables, so
  shapes follow a theme change. Literal CSS colors also work.
- Both forms draw into one layer, and ids resolve across it: an arrow in a
  page-level block may end on a shape a pane block drew.
- Labels, stroke widths and arrowheads keep their size in either form — a
  pane's frame scales coordinates, not typography.

Pane rectangles are computed at build time from the grid's ratios and its
margin and gutter, which is why those two numbers are deck settings
(`grid-pad-x`, `grid-pad-y`, `grid-gap` in frontmatter) rather than something
a stylesheet is free to move: a theme that overrides the `--mz-grid-*` custom
properties in CSS moves the panes without telling the build, and pane-anchored
shapes drift by the difference. Declare the metrics in frontmatter and the
build emits the CSS itself, so the browser and the shape layer always agree.
Decks that never anchor a shape to a pane can keep adjusting the custom
properties in CSS, exactly as before.

## Connectors

```markdown
The [edge cache]{#t-edge .u} answers first.

```connect
#t-edge -> #cache.w : color=@accent2 style=dashed
#a <-> #b
#a -- #c : curve=0
```
```

- Operators: `->` (arrow), `<->` (both ends), `--` (plain line).
- Either endpoint may be a text anchor, a shape, or a chart mark.
- Omit the edge and Mirzam picks the natural one from relative position.
- Attributes: `color=`, `style=dashed`, `curve=` (0 for a straight line).

Connector endpoints are resolved in the browser *after* layout, on every show,
resize and hot reload. That is why arrows keep pointing at the right thing when
the window changes size or the theme changes metrics.

### When to reach for one, and when not to

**A connector is at its best between two boxes in a diagram**: both ends are
shapes, the route is short, and the line is part of the picture rather than
something laid over it.

**From a sentence to a figure, prefer a [paired
annotation](#tying-a-phrase-to-a-figure).** An arrow from prose has to leave the
text without striking through it, cross the slide without colliding with
anything, and arrive somewhere meaningful — three problems, none of which the
audience asked for. Marking the phrase and the target *at the same moment, in
the same colour* says the same thing with nothing travelling between them, and
it survives an edit to the sentence.

The connector syntax is not going anywhere; it is simply the wrong tool for
that particular job.

## Audio, and video that lives somewhere else

```markdown
![Interview with the author](media/talk.mp3)
![The paper's own talk](https://www.youtube.com/watch?v=…)
```

- An audio file becomes a player with the alt text as its label, inlined like
  any other asset — a deck with a recording in it is still one file.
- A YouTube or Vimeo page URL becomes an embed, served from
  `youtube-nocookie.com`. **This is the one thing in a deck that is not
  self-contained:** the frame is fetched when the slide is shown, so it needs
  the network and it cannot be printed. The PDF gets a placeholder carrying
  the link instead, and audio gets its label without the transport.
- What a reference *is* follows from what it points at, so the attribute block
  is optional: `![clip](talk.mp4)` is a video whether or not you wrote `{}`.

## When a slide has too much on it

By default a pane **clips** what does not fit. That keeps the layout you drew
and `scripts/check-layout.mjs` reports the overflow before anyone presents it —
but nothing warns you while you are writing, and text that silently disappears
is a bad way to find out.

```yaml
---
fit: shrink        # every pane on every slide
---
```

```markdown
::: pane body {fit=shrink}
```

```bash
mirzam build README.md --split h2 --fit shrink   # for a document with no frontmatter
```

`fit=shrink` gives up the type size to keep the words: the pane's contents are
scaled down in small steps until they fit, to a floor of 55%, and re-measured
on every page turn and window resize. It runs in the PDF too — it only ever
makes content smaller than a box it is already overflowing, so a page that runs
it shows strictly more than one that does not. Without JavaScript you get the
clipping default, which is the documented fallback rather than a broken state.

If a pane is shrinking a lot, that is the deck telling you the slide has two
slides' worth on it.

### Carrying one pane on to the next slide

When shrinking is the wrong answer — a prose pane you would rather break at a
sentence you chose — put `<!-- next -->` where the break belongs:

```markdown
::: pane body
The estimator is unbiased under the stated conditions.

<!-- next -->

The variance, though, is where the argument actually happens.
:::
```

That slide becomes **two slides**, identical except for `body`. Every other
pane — the figure, the heading, the citations — is the same markup rendered
into the same place, and the viewer *cuts* between the parts instead of turning
the page, so the audience sees only the text change. `<!-- more -->` is accepted
as the same marker, and both are HTML comments, so a plain Markdown parser
shows nothing.

The expansion happens before a slide is parsed, so the parts are ordinary
slides: they animate, annotate and export like any other, and the PDF gets one
page per part.

Two rules follow from what this is:

- **One pane per slide may break.** Two panes breaking at once is a cross
  product nobody can predict. Mirzam reports it and renders the slide whole.
- **`<!-- next -->` outside every pane** breaks the slide body itself, which is
  what you want on a slide with no `pane` layout at all.

## Table of contents

````markdown
```toc
from: 2        # skip the deck's `#` title
depth: 2       # deepest heading listed
current: true  # mark the section being presented
```
````

Collects every heading in the deck, links each entry to the slide it is on, and
draws a leader out to the page number. Clicking an entry goes there; the address
is the slide number the viewer already keeps in the URL, so an entry works with
JavaScript switched off.

- **`from`** (default `1`) is the shallowest level listed, **`depth`** (default
  `2`) the deepest. `from: 2` is the usual setting: the title of the talk is not
  an item on its own agenda.
- **`current: true`** marks the last entry at or before the slide on screen —
  the section you are *inside*, not the heading you last passed. That is what
  turns an agenda slide into a progress indicator you can return to.
- A heading appears once, at the first slide that carries it, so a slide broken
  by `<!-- next -->` contributes one entry rather than three.
- Headings written inside speaker notes stay out: a note is what you say, not
  part of the structure.
- The slide carrying the list is not in it.
- **In the PDF** each entry shows its page number instead of a link, since a
  link to slide 7 means nothing on paper.

This is the first block that needs to know about slides other than its own. It
resolves in a second pass once the whole deck has rendered, which is why a
`toc` block previewed on a single slide renders as nothing rather than as a
guess.

## Citations

`[^key]` marks a claim and the note lands at the foot of **that slide** — a
reference belongs on the slide that made the claim, not in a bibliography at
the end that nobody will be looking at.

```markdown
Attention replaced recurrence[^vas], and the same block pretrains[^dev].

[^vas]: Vaswani et al., *Attention Is All You Need*, NeurIPS 2017.
[^dev]: Devlin et al., *BERT*, NAACL 2019. https://arxiv.org/abs/1810.04805
```

A bare DOI or arXiv URL becomes a link on its own. See
[`examples/seminar.md`](../examples/seminar.md) for the shape of a reading-group
talk: a figure quoted from the paper, annotated and pointed at from the prose,
with its citation at the foot of the same slide.

For a source cited on several slides — where the note would have to be repeated
on each of them, or written once and unreachable from the others — use
[references](#references) instead: `[@key]` against a bibliography, collected
into a list at the back. The two are unrelated and a deck can use both.

**The `[^key]: …` definition has to be on the same slide as its `[^key]`
reference** — each slide renders on its own, so a definition left on another
slide (or, in a grid layout, a pane other than the reference's own) never
reaches it. Nothing catches this at the Markdown level: a reference with no
matching definition is just left as literal `[^key]` text, which reads as a
typo rather than a broken feature. `mirzam build` warns when this happens
(`--strict` fails the build on it), so the fix is to move the definition onto
the citing slide rather than to hunt for what silently did not link.

## References

A footnote answers *what is this claim resting on, here*. The other question —
*what has this talk read* — is what a bibliography answers, and it is a
different shape: one source cited on four slides, whose details are worth
writing down exactly once.

Name a bibliography in the frontmatter and `[@key]` becomes a citation:

```yaml
---
bibliography: refs.bib
citation-style: numeric      # or `author`
---
```

```markdown
Attention replaced recurrence[@vaswani2017], and the same block
pretrains[@devlin2019]. Both at once reads [@vaswani2017; @devlin2019].
```

`refs.bib` is a plain BibTeX file — what a reference manager exports, read as
it is. A deck citing three papers can skip the second file and write them in
frontmatter instead, with the same field names:

```yaml
bibliography:
  vaswani2017:
    author: Vaswani, Ashish and Shazeer, Noam
    title: Attention Is All You Need
    booktitle: NeurIPS
    year: 2017
```

**Without `bibliography:` in the frontmatter, `[@key]` is ordinary text.** So is
anything that is not a bare key — `[@handle said so]`, an address in brackets,
a citation inside `` ` `` or a fence, and `\[@key]` when you want the brackets
themselves.

### The list

A `bibliography` block puts the references somewhere, usually on the last
slide:

````markdown
```bibliography
show: cited      # `cited` (default) or `all`
back: true       # show which slides cited each entry
```
````

| Key | Default | Does |
|---|---|---|
| `show` | `cited` | `all` lists every entry in the file, cited or not |
| `back` | `true` | prints the slides each entry was cited on, each a link |

Every `[@key]` links to the slide the list is on, and every entry links back to
each slide that cited it — a slide citing one reference three times is one
backlink. In the PDF the backlink is still the slide number, so it says
something on paper where there is nothing to click.

A block with nothing to list renders as nothing, the way an empty `toc` does.

[`examples/seminar.md`](../examples/seminar.md) shows both halves of the
question in one talk: the quoted figure's source stays in a footnote on the
slide that shows the figure, and the papers the argument leans on are cited
with `[@key]` from three different slides and listed at the back. Its `.bib`
carries a Japanese-authored entry, which is where `[山田+24]` comes from.

### What the mark says

| `citation-style:` | Mark | Listed in |
|---|---|---|
| `numeric` (default) | `[1]` | order of first citation |
| `author` | `[Vaswani+17]` | alphabetical order |

`author` builds the label from the entry: the first author's surname, `+` if
there are others, and the last two digits of the year. Two entries that would
print the same label get `a`, `b`, … so a mark always names one reference.

An entry reads as *authors — title — where it appeared, year — link*, with
surnames only and at most three of them before `et al.` The link is the DOI if
the entry has one, else its URL, else its arXiv identifier.

**`--mz-bib-size`** (default `1.05em`) sets how large the list is, on the pane,
the deck or a theme — a talk citing thirty papers needs it smaller than one
citing three.

### When a key is wrong

A `[@key]` naming no entry stays on the slide exactly as it was written, and
`mirzam build` says which slide it is on (`--strict` fails the build on it).
That is deliberate: a mark that silently became `[7]` would point at the wrong
paper, and one that silently vanished would take the claim's source with it.

Citing with no `bibliography` block anywhere in the deck warns too. The marks
still read; they simply have nowhere to go.

## Presentation effects

Flourishes the speaker fires with a key, bound per slide:

````markdown
```effects
1 : flash
2 : shake
3 : lines
4 : boom
e : burst 🎉
c : confetti
m : danmaku "this bit matters"
```
````

| Effect | |
|---|---|
| `flash` | one bright pulse over the slide |
| `shake` | the slide shakes |
| `lines` | speed lines converging on the middle |
| `boom` | an explosion out of the centre |
| `burst <emoji>` | emoji thrown upward |
| `confetti` | paper instead of emoji |
| `danmaku "<text>"` | a comment sweeps across, Nico-Nico style |

**This is not animation, and the difference is the point.** An `anim` block
belongs to the document: ordered, deterministic, and present in the PDF. An
effect belongs to the *performance* — it happens because someone pressed a key
in front of an audience, it never reaches the exported file, and a talk where
none of them fire is the same talk. Nothing here can change what the deck says.

- One key per line, one character. `Esc` clears anything still on screen, and
  turning the page cancels it.
- `← → Space PageUp PageDown Home End N F L D Esc` belong to the viewer;
  binding one is a build warning, not a silent shadowing of navigation.
- No effect may reflow the slide — they animate transforms and opacity only,
  in a throwaway layer above the page.
- Under `prefers-reduced-motion` the movement is dropped and the flash is brief.

[`examples/05-motion.md`](../examples/05-motion.md) has a slide bound to all seven.

## Annotations

Circle the part you are talking about, point at it, label it. An `annotate`
block sits beside the pane it decorates, the way `connect` does:

````markdown
::: pane shot
![p95 by region](img/latency.png)
:::

```annotate
target: shot
circle 62,38 34x34 : label="the hot corner"
rect   10,10 20x14 : color=@accent2 style=dashed
arrow  18,86 -> 55,48
text   6,90 "coordinates are percentages of the picture"
```
````

- **`target:`** is a pane name, or a `#id`. **A pane holding one picture means
  that picture** — a photo, a video or a chart. That matters: a picture is
  centred in its pane and rarely fills it, so measuring the pane would put
  every mark somewhere you did not point.
- **Coordinates are percentages of the target**, and `x,y` is the *centre* of
  a `rect` or `circle`, the way `shape` reads. `WxH` is its size. So the
  annotation stays put when the pane is resized, the deck is projected at a
  different aspect, or the picture is replaced with a bigger one.
- **An anchored item needs no coordinates at all.** Write `circle #latency-1-2`
  and the mark is taken from that element's live box — a chart mark, a shape,
  anything with an id. `pad=` in pixels gives it room to breathe. This survives
  a data change, which coordinates do not.
- **Attributes:** `label=`, `color=` (a `@token` or a literal), `style=dashed`,
  `pad=` for anchored items, `id=` to name the mark, and `step=N` to hold an
  item back until the Nth click. Attributes always come after ` : `, including
  on a `text` item: `text 6,90 "…" : step=2`.
- **`id=` makes the mark itself a target.** A `connect` arrow can run from a
  phrase in the prose to the circle drawn over the photograph — the only way
  to point at something that does not exist until the page is laid out. The
  connector appears with the mark and is re-routed whenever it moves.
- **`step=` counts as a click for the slide**, so `→` reveals the annotation
  before it turns the page — and a page with no viewer, the PDF included,
  shows every item regardless. An annotation waits for a click; it does not
  depend on one.
- Either end of an `arrow` may be an anchor: `arrow 12,70 -> #latency-1-2`
  stops at the edge of the mark rather than in the middle of it.

### Tying a phrase to a figure

An annotation may mark **words** as well as part of a picture, and that is what
replaces an arrow running from a sentence across the slide:

````markdown
::: pane note
Origin traffic keeps falling — [by Q3 it is the smaller half]{#c-q3}
:::

```annotate
highlight #c-q3     : color=@accent2 step=1
rect      #cook-1-2 : color=@accent2 step=1 pad=6
```
````

Both halves are ordinary annotation items with the **same `step`**, so they
arrive together and in one colour. A room reads that as a pairing instantly,
and nothing crosses the slide to say it.

| Mark | What it does |
|---|---|
| `highlight #id` | A wash behind the words, like a marker pen |
| `underline #id` | A rule under them |
| `box #id` | A rounded outline around them; `pad=` gives it room |

- These three take an **`#id` and nothing else**. Where the words are is the
  browser's business; a percentage would be a guess that goes stale the moment
  the sentence is edited.
- **They follow the lines the words are on.** A phrase that wraps is two line
  boxes, not one rectangle with the middle of the sentence inside it.
- A block whose items are *all* anchored needs no `target:` line — there are no
  percentages to measure against anything.

A block whose `target:` or anchor matches nothing on the slide is a warning,
not a build failure: the slide renders unannotated and the warning names it.

Annotations are resolved in the browser after layout, like connectors — and,
unlike everything else that runs there, the overlay is inlined into the PDF
export too, so the marks survive the export. See
[architecture.md](architecture.md#annotations-and-the-pdf) for why that is the
one script the print page carries.

## Animations

````markdown
```anim
[enter]   .title       : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[after #latency-0-2 +200ms] .caption : fade-in 300ms
[exit]    slide        : iris-out 500ms
```
````

One line is one track: `[trigger] target : effect duration attributes...`.

- **Triggers:** `enter`, `click N` (the Nth click-to-advance within the
  slide), `exit`, and `after #id [+Nms]` (relative to another track's target,
  the offset optional and possibly negative).
- **Targets:** a `#id` or `.class`, or the literal `slide` for the whole
  section. A `shape` with an id is one group — a box and its label, an arrow
  and its head — so animating it moves the whole thing. An optional `chars`, `words` or `lines` keyword before the effect
  name splits the target's text at build time — the wrapping spans are already
  in the HTML, so the runtime only ever selects them, never mutates the DOM to
  make them. Splitting never breaks inline markup (`<strong>` and friends stay
  intact), a multi-byte character, or an HTML entity.
- **Effects:** `fade-in`, `fade-out`, `slide-in` / `slide-out` and `wipe-in` /
  `wipe-out` (all four require `dir=left|right|up|down`), `zoom-in`,
  `zoom-out`, `blur-in`, `grow-x`, `grow-y`, `pop`, `draw`, `iris-out`.
  A `slide` travels; a `wipe` stays put while an edge uncovers it. `draw`
  runs the strokes tip-first over the full duration and inks the fills —
  an arrow's head, a label's glyphs — in over the last stretch, once the
  line has arrived at them.
- **Attributes:** a bare `400ms` sets the duration; `delay=`, `stagger=` (for a
  split target) and `ease=` are otherwise `key=value`. `ease` is a named curve
  (`out-cubic`, `in-out-back`, …) or `spring(mass,stiffness,damping)`, resolved
  to a sampled curve at build time so nothing simulates physics in the browser.

A line that points at nothing — a target that matches no element, or an
`after` reference to a missing id — is a warning, not a build failure: the
slide renders unanimated and the warning names the offending line.

### Presenting an animated slide

`→` advances to the slide's next `click` step; once the steps run out it turns
the page. `←` steps back, then goes to the previous slide. The page counter
shows the step alongside the slide (`3 / 12 · 1/2`) when there is one. Arriving
at a slide from a later one shows it with every step already played, since it is
a slide the room has already seen.

Stepping back within a slide snaps rather than playing in reverse: going back is
a correction, and a correction should be immediate.

### Slide transitions

How pages turn is a deck-wide setting, because it is the same pair of
whole-slide tracks repeated on every slide:

```yaml
---
transition: slide-left 400ms ease=out-cubic
---
```

`none`, `fade`, `slide-left`, `slide-right`, `slide-up`, `slide-down`,
`wipe-left`, `wipe-right`, `wipe-up`, `wipe-down`, `zoom` and `iris`, each
optionally with a duration and an `ease=`. Going backwards plays the
directional ones the other way.

[`examples/05-motion.md`](../examples/05-motion.md) demonstrates all of this: text
entrances, a chart whose bars grow one click at a time, a diagram that assembles
itself box by box, and a slide that overrides the deck's page turn.

A slide that declares its own whole-slide track overrides the matching half —
`[enter] slide : …` replaces the incoming transition for that slide, `[exit]
slide : …` the outgoing one. There is no separate per-slide `transition:`,
because that is what those two tracks already are.

### What animation never changes

Elements are laid out in their **final** state, and the runtime is the only
thing that ever puts one in its starting state. So a deck read without
JavaScript, and the PDF export — which ships no scripts at all — both show every
slide fully revealed. Animation is something a deck gains in a browser, never
something it depends on.

Under `prefers-reduced-motion` the reveals still happen, and stepping still
works, but nothing travels to get there: an element appears instead of arriving.

## Driving the viewer

Press **`/`** and the deck tells you. The overlay lists every key, and — the
reason it exists — the `effects` keys *this slide* binds, which are the ones
nobody can guess. `Esc` or `/` closes it.

| | |
|---|---|
| `→` `Space` `PageDown` | Next click step, then the next slide |
| `←` `PageUp` | Back a step, then the previous slide |
| `Home` `End` | First / last slide |
| `N` | Speaker notes |
| `V` | The Markdown this slide was written as ([see below](#the-markdown-behind-a-slide)) |
| `P` | Presenter window |
| `F` | Fullscreen |
| `D` | Dark / light |
| `L` | Outline the layout |
| `/` | The cheat sheet |
| `Esc` | Close the sheet; clear any effect in flight |

Clicking the left third of the slide goes back and anywhere else goes forward,
which is what a presenter with a clicker or a trackpad is using. A drag that
ends on the deck is a text selection, not a page turn.

A quiet control cluster sits below the bottom-right corner — previous, next and
the cheat sheet — and fades in when you move the pointer or touch the screen.
It is outside the deck, so it never covers slide content, and it is never
printed.

### The Markdown behind a slide

A deck built with `--embed-source` carries the document it was built from:

```bash
mirzam build deck.md -o out --embed-source
```

`V` then opens the current slide's Markdown *beside* the slide — the deck makes
room for the panel instead of being covered by it, because the point is reading
the two together. `Copy` takes that slide's source to the clipboard, `Esc` or
`Close` puts the slide back to full width. In a deck built without the flag the
key does nothing and the cheat sheet does not offer it.

On a phone there is no `V` to press, so the panel has a control: `</>` in the
cluster below the bottom-right corner, which the `/` sheet also names. The
panel docks along the bottom there rather than at the side, the cluster moves
clear of it, and a swipe inside the panel scrolls the panel — a pane drawing is
wider than a phone and deliberately does not wrap, so dragging the end of a
line into view must not turn the page.

A rendered slide is not always an authored one: `<!-- next -->` turns one slide
into several, and each of them shows the source of the slide they were cut
from. What travels is the document **as rendered** — transclusions expanded and
variables substituted — because that is what can be re-rendered on its own,
without the files it was assembled from. A deck that writes `{{price}}`
therefore carries the number.

Add `--editor-url` and the panel also carries the deck out:

```bash
mirzam build deck.md -o out --editor-url ../../try/
```

The link hands **the whole deck** to the
[browser editor](https://ayatough.github.io/Mirzam/try/), opened at the slide
you were looking at: the cursor sits where that slide starts and the preview
shows it. The whole deck rather than the one slide, because a slide is not a
document — it has no frontmatter of its own and its citations are listed
elsewhere in the file — so one on its own would be something you had to paste
back by hand.

Everything the deck reads by name goes with it: the stylesheets `theme:` points
at, the `bibliography:`, the `masters:`. It all rides in the URL's *fragment*,
the part a browser never sends to a server, so nothing is uploaded and a deck
saved to a phone hands itself over exactly like a published one. A deck given
`--theme`, `--mode`, `--fit` or `--split` on the command line hands over
frontmatter saying so, since the deck it was built as is not the one its own
text describes.

Images do not travel: they are inlined in the deck as data URIs with the path
they came from long gone, so a deck that uses one arrives with the reference
intact and the file missing, which the editor reports the way it reports any
missing asset. Drag the picture in and it resolves again.

`--editor-url` implies `--embed-source`. Both are `build` only: `serve` already
has your source open in the editor beside it.

### The presenter window

`P` opens a second window showing the current slide, the next one, that slide's
speaker notes, the time and an elapsed timer — click the timer to restart it.
Put it on your laptop and the audience window on the projector.

It is **the same file**, opened again with `?presenter=1`. There is no second
document, no server and no export step: a deck is one file, and this is that
file rendered differently.

The two windows stay in step over a `BroadcastChannel`, falling back to the
window handles when the deck is opened from `file://`, where two windows have no
shared origin to meet on. Neither window is privileged — turn the page in either
and both move. What crosses the link is the *position*, not a keystroke, so a
window opened halfway through a talk adopts the slide already on screen instead
of starting from the beginning, and closing or reloading either one strands
nothing.

The audience window is unchanged: no extra chrome appears on it. `N` still
opens the notes panel there, for a talk given on one screen.

`D` and `L` travel across the link too. Dark mode and the layout outline are
properties of the deck rather than of one window — a presenter who switches to
light mode means the projector as well.

The next-slide preview is a still: it is built from the slide as authored, so
it never inherits the current window's animation state, and it does not run
animations, annotations or connectors of its own.

### On a phone

There is no keyboard, so every control has a gesture:

| | |
|---|---|
| Swipe left / right | Next / previous |
| Swipe up / down | Show / hide speaker notes |
| Two-finger tap | The cheat sheet |
| Tap left third / elsewhere | Back / forward |
| Long press | Select text, as anywhere else |

The deck claims horizontal swipes from the browser, so swiping right turns the
page instead of navigating away from the deck. On a touch device the cheat
sheet leads with these gestures rather than with the keys.

**The long press is not bound to anything**, because on a phone that gesture is
how you select text, and a deck a reader cannot quote from is a worse deck. For
the same reason, a drag that starts or ends with a selection on screen is
treated as adjusting the selection, never as a page turn.

## Theming

### Named themes

```yaml
---
theme: nord
---
```

| Name | Source |
|---|---|
| `mirzam` | ours, from [the brand sheet](brand/palette.md) — and what a deck that names no theme gets, so a file with no frontmatter is already in the project's colours |
| `nord` | [Nord](https://www.nordtheme.com/), MIT |
| `solarized` | [Solarized](https://ethanschoonover.com/solarized/), MIT |
| `vscode` | VS Code Light+/Dark+, MIT |
| `wuwei` | ours — warm greyscale and roman type, minimal, deliberately low contrast |

Leaving `theme:` out is the same as writing `theme: mirzam`, so the key is a
choice to look like something else. An unknown name is a warning, not a build
failure, and falls back to `mirzam`. There was a `default` theme until it was
found to be that same palette under a second name; writing it now warns and
tells you to write `mirzam`. See
[`themes/CREDITS.md`](../crates/mirzam-render/src/theme/themes/CREDITS.md) for
where each palette comes from and how it maps to Mirzam's tokens.

A named theme is a **token set**, and a token set is more than a palette:
`theme: mirzam` gives a deck Mirzam's colours *and* its identity — Space
Grotesk over Inter, the weight ladder that gets heavier as the type gets
smaller, the short violet rule under a section heading instead of a full-width
border. See [the vocabulary](#the-vocabulary-a-theme-writes-in) for the whole
list of dials.

`theme: wuwei` is the other one, and it is the demonstration that a theme is an
identity rather than a palette: warm greys **and roman type** — an old-style
serif, Mincho named after it so Japanese stays roman too, one face for headings
and text alike, and a ladder that separates them by size and space rather than
by weight. Put the two side by side and the difference is legible before a
colour registers.

A built-in theme is still tokens and nothing else, because it is loaded
*before* the layout stylesheet: a rule written there would be overridden by the
very stylesheet it is meant to sit on. What changed is how much a token can
say. `nord`, `solarized` and `vscode` set colours only, so a deck that names one
of them keeps the built-in type.

**Look at them rather than reading about them.** The
[themes gallery](https://ayatough.github.io/Mirzam/themes/) puts one slide —
a heading, body text, a list, a code block, a metric and a chart — through all
five built-ins and the sample theme-in-a-file, in light and dark. Every picture
on it is generated from the stylesheets by `scripts/make-theme-gallery.mjs`, so
it shows what the themes do today rather than what they did when somebody last
took a screenshot.

### A theme of your own, in a file

`theme:` takes a **path** as readily as a name. An entry ending in `.css` is a
stylesheet of yours, resolved relative to the deck the way `masters:` and
`bibliography:` are; anything else is a built-in name. A list is cascade order,
and a scalar is a list of one:

```yaml
theme: mirzam                        # a built-in
theme: themes/acme.css               # a theme of your own, beside the deck
theme: [mirzam, themes/tweaks.css]   # a built-in, then your file over it
```

The order the page is assembled in is: the built-in's tokens, then the shared
stylesheet, then each `.css` entry in turn. Your file loads **after** the
shared stylesheet — that is what lets it override the type and not only the
colours, and it is why a built-in cannot: a built-in is a token set loaded
*before* the sheet that reads it.

A deck that names only its own theme wears it; a deck that names a built-in as
well has said which palette it is in, and its own file is then there for the
slides and panes that ask for it by name.

`--theme` takes a path too, and repeating it is a list:
`mirzam build deck.md --theme mirzam --theme house.css`. (`css:` was the old
spelling of a one-entry `theme:`; it warned for one release and is gone.)

#### A file theme gets a name — if it scopes its tokens to it

`themes/acme.css` registers under its filename stem, `acme`, which a slide or a
pane can then write in `theme=`. That only means something if the file says so:

```css
[data-theme="acme"] { --mz-accent1: #6557d9; }   /* usable in a pane's theme= */
:root            { --mz-accent1: #6557d9; }      /* the deck, and nothing smaller */
```

Custom properties set on `:root` are set on the *document*. A pane carrying
`data-theme="acme"` picks up nothing from them, and nothing on the page says
why — so the rule is: **a file theme is usable in a `theme=` if, and only if,
it scopes its tokens to its own stem.** `mirzam check` reports a `theme=` that
names a file which cannot answer to it, because a pane that silently stays in
the deck's palette is exactly the kind of failure a checker is for.

That selector is the one the built-ins use, minus the `:where()` — they need
that so your stylesheet can outrank them, and yours is the one doing the
outranking.

**A scope starts from the defaults, not from the deck.** Every element that
carries a `data-theme` — the page, a slide, a pane — begins with the whole
derived vocabulary undefined, and then that theme's own declarations run. So a
token your theme sets is yours, and a token it does not set falls back to the
shared stylesheet's default resolved in *your* palette and *your* mode, rather
than to whatever the deck around it happened to say. Custom properties inherit,
so without that a pane wearing your theme would take the deck's subheading
colour, its faces and its margins for everything you left alone — in the deck's
mode, which put a colour mixed for a dark slide on a light pane. This costs a
theme nothing: your values are emitted after the reset and win over it.

The palette itself is not reset, because there is nothing to fall back to: the
colour tokens are the contract every theme keeps in both modes. The rest —
type, weights, tracking, marks, margins — is what a theme may leave unsaid.

Two more rules worth knowing:

- A stem that collides with a built-in (`themes/nord.css`) does **not** take
  the name: `theme=nord` keeps meaning the built-in, and the collision warns.
  A file in one directory quietly redefining what `theme: nord` means
  everywhere is worse than a name being taken.
- A deck-specific class does not need a file at all. A raw `<style>` block in
  the deck reaches the page untouched, which is the right home for the one or
  two classes a single deck invents; `examples/06-theming.md` does that for its
  list-marker demonstration. `theme: [mirzam, tweaks.css]` is where it goes
  when it outgrows a block.

[`examples/themes/blueprint.css`](../examples/themes/blueprint.css) is a
complete one: a whole identity — one mono hand for the whole sheet, pale blue
paper in light and an ink-blue night in dark, a hairline rule under a section
heading, square cards, an em-dash bullet — written in tokens and
scoped to its stem, so `examples/06-theming.md` can wear `mirzam` and still
hand one pane to it.

#### Tokens travel; rules do not

The asymmetry above is worth stating on its own, because it decides how to
write a theme rather than only where to put it:

| Written as | Applies to | Works with a pane's `theme=` |
|---|---|---|
| tokens (`--mz-*`) | deck, slide, pane | yes |
| rules (`h1 { }`, `.foo { }`) | the deck | no |

Custom properties **inherit**, so setting them on an element re-themes
everything inside it — a pane's `theme=` resolves inwards for free. Plain rules
cascade by specificity and source order, which have nothing to do with where
the element is, so a rule that styles `h1` styles every `h1` in the deck. A
theme written in tokens works at every scale; the same theme written as rules
works at one.

### Dark mode

Every built-in theme defines both a light and a dark palette. Which one shows:

1. `mode: dark` (or `mode: light`) in frontmatter, if set - baked into the
   deck, so there is no flash of the wrong palette on load.
2. `?mode=dark` in the URL, read by the viewer before it draws anything.
3. `D` in the viewer, which toggles for that reading session only.
4. Otherwise, the reader's `prefers-color-scheme` - a deck with no explicit
   `mode:` just follows the system, live, with no reload.

### A theme smaller than a deck

`theme:` and `mode:` are the deck's, and a slide or a single pane can answer
differently. A pane in a theme of its own is how you put two palettes on one
slide — the same screenshot in light and in dark, a quotation on its own paper,
one figure in the palette it was designed for.

On a pane, as attributes:

```markdown
::: pane before {theme=wuwei mode=light}
The quiet version.
:::

::: pane after {theme=wuwei mode=dark}
The same words, after dark.
:::
```

On a whole slide, as an HTML comment — the same form as a speaker note, and
invisible in a plain Markdown reader for the same reason:

```markdown
<!-- theme: nord -->
<!-- mode: dark -->

# One cold slide in a warm deck
```

The rules:

- Either attribute can be given alone. `theme=` with no `mode=` follows the
  deck's mode, so a re-themed pane still flips with `D`; `mode=` with no
  `theme=` shows the surrounding theme's other half, and stays there.
- Following the deck means following what the deck *declares* — `mode:` in
  frontmatter, `?mode=`, `D`, and otherwise the reader's `prefers-color-scheme`.
  A theme of your own that pins a palette at a bare `:root` has chosen a mode
  without declaring one, and a pane that follows will follow the reader's
  machine instead. **Write `mode:` in frontmatter when your theme has already
  decided**; `examples/06-theming.md` does exactly that.
- The palette is set **on that element**, and custom properties inherit, so
  everything inside it — headings, code, tables, chart series — is drawn from
  the other theme's tokens.
- A re-themed pane paints its own background and rounds its corners, because a
  patch of another palette needs an edge to read as one.
- Nesting resolves inwards: a pane's theme beats its slide's, which beats the
  deck's.
- An unknown name is a warning naming the slide and the pane, and that element
  simply keeps what it inherited. A deck never fails to build over a palette.
- A theme of your own can be named this way too, under its filename stem —
  but only if it [scopes its tokens to that stem](#a-file-theme-gets-a-name--if-it-scopes-its-tokens-to-it).
  A file that sets them at `:root` is loaded once for the whole page, and a
  pane naming it picks up nothing; `check` says so.

A deck carries the tokens of the themes it actually names and no others, so
none of this costs anything to a deck that uses one palette.

### What a theme sets

Name a `.css` file in `theme:` and override the tokens — this layers on top of
whichever built-in the deck named, or on top of `mirzam` if it named none:

```css
[data-theme="acme"] {
  --mz-slide-bg: #0d1117;
  --mz-fg: #e9edf5;
  --mz-accent1: #5b8cff;
  --mz-accent2: #2dd4bf;
  --mz-chart3: #f6c177;   /* chart series 3-6 */
}
```

`.card`, `.eyebrow` and `.metric` are **not** something a theme has to define:
they come with the renderer, like `.box` and `.small`, so a deck that names no
theme at all still has an eyebrow and a card to lay a slide out with. What a
theme changes about them is [their tokens](#the-vocabulary-a-theme-writes-in).
See [`examples/themes/blueprint.css`](../examples/themes/blueprint.css) for a
complete one.

#### Margins, padding and borders

Spacing is a token too, so moving a deck's margins does not mean restating the
rules that position them:

| Token | Default | What it moves |
|---|---|---|
| `--mz-grid-pad-y` | `44px` | The slide's top and bottom margin |
| `--mz-grid-pad-x` | `60px` | Its left and right margin, and the footer's |
| `--mz-grid-gap` | `20px` | The space between panes |
| `--mz-columns-gap` | the grid gap | The gutter between a pane's `columns=` columns |
| `--mz-pane-pad` | `2px 4px` | Padding inside every pane |
| `--mz-pane-border` | `none` | A border on every pane |
| `--mz-pane-radius` | `0` | Its corner radius |
| `--mz-slide-chrome-size` | `.62em` | The footer and slide number |
| `--mz-slide-chrome-fg` | `var(--mz-muted)` | Their colour |

```css
:root {
  --mz-grid-pad-y: 56px;
  --mz-grid-pad-x: 72px;
  --mz-grid-gap: 28px;
}
```

These are **not** palette tokens: no built-in theme sets one, every use carries
the built-in value as its fallback, and a deck that sets none renders exactly as
it did before they existed. `theme:` remains a choice of colour only.

Custom properties inherit, which makes them the one dial that works at every
scale. Set on `:root` they move the whole deck; set under a class you put on a
pane, they move that pane:

```css
.tight  { --mz-pane-pad: 0; }
.framed { --mz-pane-border: 1px solid var(--mz-border); --mz-pane-radius: 8px; }
```

```markdown
::: pane fig {.framed}
```

Writing the rule directly still works and always did — `.grid { padding: 48px
64px; gap: 24px }` — but the tokens are the better route, because the footer
reads `--mz-grid-pad-x` to stay on the same margin as the words above it, and a
`padding` shorthand it cannot see leaves it on the old one.

#### The vocabulary a theme writes in

Type is a token too, and so is every mark that carries an identity rather than
a colour. All of it follows the same rule as the margins above: **every use
carries today's value as its fallback**, no theme has to set any of it, and a
deck that sets none renders exactly as it did before the dials existed.

The faces. Nothing is fetched — a deck is one self-contained file, and a
projector at a venue may have no network — so name a stack that ends in a
system face, and name the Japanese faces yourself if the deck has any CJK in
it:

| Token | Default | What it sets |
|---|---|---|
| `--mz-font` | Helvetica Neue / Arial + the CJK stack | The deck's text face |
| `--mz-font-display` | inherited | Headings, `.eyebrow` and `.metric` |
| `--mz-font-mono` | SF Mono / Consolas / Menlo | Code, `kbd`, an unparsed formula |

The ladder. Each level has the same four dials, and *inherited* means the rule
declared nothing at all before — so setting `letter-spacing` on a pane still
reaches the headings inside it:

| Token | Default | What it sets |
|---|---|---|
| `--mz-h1-size` `-weight` `-tracking` `-leading` | `2.6em`, `bold`, `.01em`, inherited | `h1` |
| `--mz-h2-size` `-weight` `-tracking` `-leading` | `1.85em`, `bold`, inherited, `1.25` | `h2` |
| `--mz-h3-size` `-weight` `-tracking` `-leading` | `1.3em`, `bold`, inherited, inherited | `h3` |
| `--mz-h3-color` | `var(--mz-accent1)` | `h3`'s colour |
| `--mz-body-size` `--mz-body-leading` | `1.35em`, `1.65` | Paragraphs, list items, term lists |
| `--mz-title-size` | `3.4em` | `.title-slide` |
| `--mz-title-weight` `--mz-title-tracking` | the `h1` pair | `.title-slide`, when it differs from `h1` |

`--mz-title-*` fall through to the `h1` dials, so a theme with one answer for
display type gives it once.

The marks. A section heading gets a full-width border **or** a short rule under
it, and the two are the same choice: `--mz-h2-rule-w` is `0`, so nothing is
drawn until a theme asks, and a theme that asks usually sets the border to
`none`:

| Token | Default | What it sets |
|---|---|---|
| `--mz-h2-border` | `3px solid var(--mz-accent1)` | The rule under every `h2` |
| `--mz-h2-pad` | `.25em` | The space above that border |
| `--mz-h2-rule-w` `-h` `-gap` | `0` | The short rule's width, height and the gap above it — set all three, or nothing is drawn |
| `--mz-h2-rule-radius` | `2px` | Its corners |
| `--mz-h2-rule-a` `-b` | accent 1 → accent 2 | The gradient it runs |
| `--mz-strong-color` `-weight` | `var(--mz-accent1)`, `bolder` | Bold text inside a sentence |
| `--mz-quote-border` | `4px solid var(--mz-accent2)` | A quotation's edge |
| `--mz-quote-fg` | `var(--mz-muted)` | Its text |
| `--mz-code-bg` | `var(--mz-surface)` | The paper under a code block *and* an inline span |
| `--mz-code-fg` | inherited | The inline span's colour only — a block takes the highlighter's `--mz-code-*` colours |
| `--mz-th-fg` | inherited | A table's header row |

The furniture — the three classes a deck lays a slide out with:

| Token | Default | What it sets |
|---|---|---|
| `--mz-card-bg` | `var(--mz-surface)` | `.card`'s fill |
| `--mz-card-border` | `1px solid var(--mz-border)` | Its edge |
| `--mz-card-radius` | `12px` | Its corners |
| `--mz-card-pad` | `24px 26px` | Its padding |
| `--mz-card-shadow` | `none` | Whether it is raised |
| `--mz-eyebrow-size` `-weight` `-tracking` `-color` | `.82em`, `500`, `.12em`, `var(--mz-accent1)` | `.eyebrow` |
| `--mz-metric-size` `-weight` `-tracking` `-color` | `3.2em`, `bold`, inherited, inherited | `.metric` |

`.metric-up` takes `--mz-chart3` and `.metric-label` takes `--mz-muted`, both
straight from the palette: "up" is a meaning, not a decoration, and a label
under a number is secondary text like any other.

One thing about a theme's own identity is **not** a token, because it is
selector logic rather than a value: **the short rule follows the heading's
alignment**. It is a block box of a fixed width and `text-align` cannot move
one, so the alignment has to be read off a selector and answered with a margin.
`base.css` carries that one rule, so a heading in a pane with `align=center`
gets its mark centred under the words and one with `align=right` gets it at the
right edge — both for free, and neither needing a theme of your own.

```css
/* A theme in tokens: the faces, the ladder, the mark under a heading. */
:root {
  --mz-font: Inter, "IBM Plex Sans", sans-serif;
  --mz-font-display: "Space Grotesk", sans-serif;
  --mz-h1-weight: 300;
  --mz-h1-tracking: -0.03em;
  --mz-h2-weight: 400;
  --mz-h2-border: none;
  --mz-h2-pad: 0;
  --mz-h2-rule-w: 64px;
  --mz-h2-rule-h: 4px;
  --mz-h2-rule-gap: 14px;
}
```

Because custom properties inherit, that block works at any scale — on `:root`
it moves the deck, on `[data-theme="acme"]` it moves whatever carries that
name, on a class you put on one pane it moves that pane. It is also what a
pane's `theme=` carries, which is why the type now travels with a re-themed
pane and a rule never could.

#### A custom theme needs both modes, or it has none

The built-in tokens are wrapped in `:where()` and carry no specificity, which
is what lets a plain `:root` in your stylesheet override them. The same thing
makes a one-palette custom theme **pin the deck to one mode**: your `:root`
beats the built-in light *and* dark tokens, so `D` changes `data-mode` and
nothing on screen moves. Give the second mode a selector that outranks your own
`:root`:

```css
:root                     { --mz-slide-bg: #0d1117; --mz-fg: #e9edf5; }
:root[data-mode="light"]  { --mz-slide-bg: #ffffff; --mz-fg: #10151f; }
```

A theme scoped to its own stem writes the same pair one level in:

```css
[data-theme="acme"]                    { --mz-slide-bg: #ffffff; --mz-fg: #10151f; }
[data-theme="acme"][data-mode="dark"]  { --mz-slide-bg: #0d1117; --mz-fg: #e9edf5; }
```

Three rules follow, and **`mirzam check` reports all three against your own
theme** — they used to be a test that only the sample themes could fail:

- **Every colour set in one mode must be set in the other.** A colour you set
  once keeps its other-mode value — which is how a dark panel ends up on a
  white slide.
- **A theme with one palette and no second mode is reported as such**, with
  the block to add. Type, sizes and spacing are not colours and need saying
  only once.
- **Text has to be legible on its own background**: the same contrast floors
  the built-in themes are held to — 4.5:1 for text, 3:1 for a chart mark — are
  measured on the colours your theme actually resolves to in each mode.

One consequence: **name a colour once**. A literal buried in a rule
(`p { color: #c7cede }`) cannot have a second mode and cannot be checked. Put
it in a token of your own and set that token twice.

A theme that deliberately only ever appears one way is fine; write `mode:` in
the deck's frontmatter and say so.
