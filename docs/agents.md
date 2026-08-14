# Mirzam for agents

Two files make Mirzam usable by a model that writes decks:
[`llms.md`](llms.md) — the whole markup on one page, small enough to hand to a
model as context, also published as
[`/llms.txt`](https://ayatough.github.io/Mirzam/llms.txt) — and
`mirzam check --format json`, which is this page.

> Not to be confused with [AGENTS.md](../AGENTS.md) at the repository root.
> That is for an agent changing *Mirzam*; this is for an agent writing a *deck*.

## The loop

A deck fails in ways a diff cannot show: a heading clipped by its band, an
arrow pointing at an id that was renamed, a `shape` block written inside a
pane so it renders as source code. The [usability
evaluation](reports/2026-08-usability-eval.md) found that every first-time
author shipped one of these without noticing. An agent will not — but only if
it can read the answer:

```bash
mirzam check deck.md --format json
```

Write the deck, run that, fix what it names, run it again. The exit code is
still `0` when the deck has no layout problems and non-zero when it has, so the
loop works from a shell script that never parses anything; the document is for
the caller that wants to *act* on a finding rather than only stop.

`stdout` carries the JSON document and nothing else — no progress lines, no
warnings, no verdict. Errors and the exit message go to `stderr`, so the
output is safe to pipe.

## The document

```json
{
  "schema": "mirzam-check",
  "version": 1,
  "deck": "examples/pitch.md",
  "slides": 9,
  "ok": false,
  "diagnostics": [
    {
      "kind": "layout.clipped",
      "severity": "error",
      "message": "content is 31px taller than the pane",
      "slide": 4,
      "pane": "head",
      "file": "examples/pitch.md",
      "line": 88
    },
    {
      "kind": "build.shape",
      "severity": "warning",
      "message": "slide 2: shape line 1: unknown shape kind `boxx`",
      "slide": 2,
      "file": "examples/sections/method.md",
      "line": 14
    }
  ],
  "notes": [
    "fonts: measured with Arial; not on this machine: Hiragino Sans, and 5 more. A reader who has them sees different line breaks"
  ]
}
```

| Field | |
|---|---|
| `schema` | Always `mirzam-check`. Check it before anything else: it is how a caller knows the document is one of these |
| `version` | The integer below. `1` today |
| `deck` | The input path, exactly as it was given on the command line |
| `slides` | How many slides were rendered and measured |
| `ok` | `true` when no diagnostic has severity `error` — the same verdict the exit code carries |
| `diagnostics` | Every finding, from both passes. Always present, `[]` when there are none |
| `notes` | What the run was measured *with*: the fonts this machine actually had, and how little room the tightest pane had left. Prose, not records — a clean run is a statement about one machine, and this says which |

### A diagnostic

| Field | Always? | |
|---|---|---|
| `kind` | yes | A stable machine-readable name; the list is below |
| `severity` | yes | `error` or `warning`. `error` fails the run |
| `message` | yes | The sentence the text form prints. Prose, written for a person, and free to be reworded — branch on `kind`, never on this |
| `slide` | when known | 1-based slide number, counting the way the deck does: a slide broken by `<!-- next -->` counts as each of its parts |
| `pane` | when known | The pane's name from the `pane` drawing |
| `file` | when known | The file the slide was written in — the *included* file, if the slide came in through `![[…]]` |
| `line` | when known | 1-based line in `file` |

**An absent field means "not known", never "none".** A warning about the
frontmatter belongs to no slide; a line that variable substitution rewrote
belongs to no file, because the text on the slide is not text anyone typed
there. Treat a missing `line` as "open the file and look", not as line 0.

`file` and `line` point at the **slide**, or at the pane's `::: pane` line when
the diagnostic names a pane — not at the exact character. The deck's source map
follows transclusion, so a slide that arrived through `![[sections/method.md]]`
names that file rather than the deck that included it.

### Kinds

Two families, by the pass that found the problem.

**`layout.*` — the deck rendered in a browser and measured.** These are the
`error` severity and the ones that decide the exit code. Each is described in
[the layout guide](layout.md#checking-a-deck).

| Kind | |
|---|---|
| `layout.clipped` | Content is taller or wider than its pane, or an element inside one hides part of itself by scrolling |
| `layout.overlap` | An overflowing pane runs into its neighbour |
| `layout.nesting` | A nested list or paragraph is set larger than the item it sits inside |
| `layout.connector` | A `connect` line was declared but no arrow was drawn — usually a typo in an id |
| `layout.annotation` | An `annotate` mark could not be drawn, usually because the `#id` it names was renamed |
| `layout.animation` | An element is still in its entrance state after that entrance has played, so nobody ever sees it |
| `layout.slack` | A pane fits, but by less than `--min-slack <px>` asked for |
| `layout.debug` | The pane overlay is baked into this build (`--debug-layout`) |

**`build.*` — the build's own warnings**, the `⚠` lines the text form prints.
They are `warning` severity: they do not fail the run here, and
`mirzam build --strict` is what fails a build on them.
[troubleshooting.md](troubleshooting.md#build-warnings-and-what-they-mean) says
what each one does to the slide.

| Kind | About |
|---|---|
| `build.layout` | The `pane` grid: a pane not in the layout, a non-rectangular merge, a malformed drawing |
| `build.master` | `masters:` and `<!-- layout: … -->` |
| `build.shape` | A `shape` block — including one written inside a pane, where it does not run |
| `build.connect` | A `connect` block or an endpoint matching nothing |
| `build.annotate` | An `annotate` block, its target or its anchors |
| `build.anim` | An `anim` block, its targets, its triggers and its splits |
| `build.effects` | An `effects` block and its key bindings |
| `build.chart` | A `chart` block, its data file or its CSV |
| `build.toc` | A `toc` block |
| `build.footnote` | A `[^key]` with no definition on its slide |
| `build.bibliography` | `bibliography:`, `[@key]`, and the `bibliography` block |
| `build.span` | An attribute span left on the slide as literal text — almost always one broken across two lines |
| `build.math` | The math dialect, and a brace too wide to stretch |
| `build.asset` | An image, audio or video file that is missing or too large to inline |
| `build.theme` | An unknown `theme:` or `mode:`, on the deck, a slide or a pane |
| `build.transition` | An unparsable `transition:` |
| `build.css` | An unreadable `css:` |
| `build.continuation` | `<!-- next -->` in more than one pane on a slide |
| `build.deck` | The deck as a whole — a file with no slides in it |
| `build.other` | A warning this list has not been taught yet. Read `message` |

**Handle an unknown kind.** `build.other` exists so a new warning is never
mislabelled, and a `layout.*` name not on this list can appear the moment the
in-page check learns a new failure mode. Fall back to `severity` and `message`
rather than dropping the record.

## The version, and what it promises

`version` is `1`.

- **A field may be added at any time, and does not move the version.** Ignore
  fields you do not know.
- **A field is never renamed, removed, or given a different meaning without the
  version going up.** So a consumer written against `1` keeps working, or is
  told plainly that it does not.
- `kind` values are part of the contract in the same way: a new one may appear,
  an existing one does not change meaning.

## What this is not

Mirzam renders and checks. It does not draft, call a model, or hold an API key
— the agent is somebody else's process consuming a stable interface. That
separation is deliberate: see
[W21 in the work streams](workstreams.md#w21--an-authoring-contract-for-agents).
