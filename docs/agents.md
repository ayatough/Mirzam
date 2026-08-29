# Mirzam for agents

Three things make Mirzam usable by a model that writes decks:
[`llms.md`](llms.md) — the whole markup on one page, small enough to hand to a
model as context, also published as
[`/llms.txt`](https://ayatough.github.io/Mirzam/llms.txt) —
`mirzam check --format json`, which is most of this page, and
[`mirzam skill install`](#the-skill), which writes both of them into an agent's
own conventions so nobody has to remember to paste anything.

> Not to be confused with [AGENTS.md](../AGENTS.md) at the repository root.
> That is for an agent changing *Mirzam*; this is for an agent writing a *deck*.

## The loop

A deck fails in ways a diff cannot show: a heading clipped by its band, an
arrow pointing at an id that was renamed, a diagram drawn over the words it
was meant to sit beside. The [usability
evaluation](reports/2026-08-usability-eval.md) found that every first-time
author shipped one of these without noticing. An agent will not — but only if
it can read the answer:

```bash
mirzam check deck.md --format json
```

Write the deck, run that, fix what it names, run it again. What the loop
costs in context tokens against inspecting screenshots — measured, per
review round and per deck size — is in [the agent context
report](reports/2026-08-agent-context.md). The exit code is
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
  "mirzam": "0.5.0",
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
| `mirzam` | The version of the binary that produced this document, as a string. Not the schema version: it is what a caller repairs a [drifted skill card](#the-skill) *to*, and what to name when reporting a bug |
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
| `build.shape` | A `shape` block: a line naming no known kind, or an endpoint id matching nothing |
| `build.connect` | A `connect` block or an endpoint matching nothing |
| `build.annotate` | An `annotate` block, its target or its anchors |
| `build.anim` | An `anim` block, its targets, its triggers and its splits |
| `build.effects` | An `effects` block and its key bindings |
| `build.chart` | A `chart` block, its data file or its CSV |
| `build.mermaid` | A `mermaid` block that was not drawn — no renderer installed, or `mmdc` rejected the diagram. The fence is on the slide as a code block; install mermaid-cli, or set `MIRZAM_MMDC` |
| `build.toc` | A `toc` block |
| `build.footnote` | A `[^key]` with no definition on its slide |
| `build.bibliography` | `bibliography:`, `[@key]`, and the `bibliography` block |
| `build.span` | An attribute span left on the slide as literal text — almost always one broken across two lines |
| `build.math` | The math dialect, and a brace too wide to stretch |
| `build.asset` | An image, audio or video file that is missing or too large to inline |
| `build.theme` | An unknown `theme:` or `mode:`, on the deck, a slide or a pane; and what a theme of your own has to say about itself — a stem colliding with a built-in, one palette where two are needed, a colour pair under the contrast floor |
| `build.transition` | An unparsable `transition:` |
| `build.css` | A stylesheet named by `theme:` that cannot be read |
| `build.continuation` | `<!-- next -->` in more than one pane on a slide |
| `build.deck` | The deck as a whole — a file with no slides in it |
| `build.skill` | The installed [skill card](#the-skill) and this binary are different versions |
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

## The skill

The loop above only runs if the agent knows about it. So the binary installs
the instructions, rather than this repository holding a skill for people to
find and copy:

```bash
mirzam skill install          # .claude/skills/mirzam/ in this repository
mirzam skill install --user   # ~/.claude/skills/mirzam/, in every directory
```

It writes two files: `SKILL.md`, which is the loop — check `mirzam --version`
and say how to install it if it is missing, write the deck, run
`mirzam check --format json`, fix what it names, repeat — and
`references/llms.md`, the syntax card, which `SKILL.md` points at rather than
inlining, so the instructions stay short and the card is opened when markup is
about to be written.

**Both are embedded in the binary.** The card is `docs/llms.md` itself, compiled
in, so the markup a model reads is the markup the binary beside it implements.
That is the whole reason this is a command and not a file in a repository:
while Mirzam is `0.x`, a card copied once is a card that will be wrong later.

**A project install beats a user install** when you have both — Claude Code
resolves the project skill first — so `--user` is the one to reach for if you
write decks in directories that are not repositories, and the plain form is the
one to commit, so a teammate cloning the repository gets the same instructions.

### The stamp, and the drift it catches

The generated `SKILL.md` ends with the version that wrote it, in a sentence and
again in a comment:

```html
<!-- mirzam-skill version="0.5.0" hash="…" card="…" -->
```

`build` and `check` look for that comment: up from the deck's directory, one
`.claude/skills/mirzam/SKILL.md` at a time, stopping at a `.git` directory
(the repository is where "near the deck" ends), then `~/.claude/skills/`. When
the version they find is not their own, they emit a `build.skill` warning — an
ordinary one, in the same list as the rest, because the agent is already
reading that list:

- **card older than the binary** — "run `mirzam skill install`". The agent can
  do this itself; it is the repair the diagnostic is asking for.
- **card newer than the binary** — "this binary is older than the skill card;
  upgrade the binary". Nothing an agent should fix by downgrading the card.

So the update story is: upgrade the binary, run `mirzam skill install` again,
and if you forget, the next `check` says so.

`hash` is the file's own contents, `card` is the syntax card's. A reinstall over
a skill somebody has *edited* refuses, and says how to go ahead anyway:

```
error: .claude/skills/mirzam/SKILL.md has been edited since it was installed.
       `mirzam skill install --force` overwrites it (your edits are lost); …
```

An unmodified card from an older version is not somebody's work, and is
replaced without a word.

### Where no binary can run

claude.ai, the desktop app and phones execute a skill in a sandbox with no
filesystem and no network to fetch a release into, so the checking half of the
loop is simply not available there. The honest degradation is a second,
smaller skill that says so:

```bash
mirzam skill install --zip            # mirzam-writing-skill.zip
mirzam skill install --zip out/s.zip  # or wherever you want it
```

`mirzam-writing` carries the same syntax card and tells the model to write
correct Mirzam markdown, hand the `.md` back, and point the person at the
[browser editor](https://ayatough.github.io/Mirzam/try/) — which renders the
deck with nothing installed and works on a phone — or at the CLI for checking
and PDF. Upload the archive under Settings → Capabilities → Skills; phones
receive it by account sync. Each release attaches it, so it can also be
downloaded rather than built.

## What this is not

Mirzam renders and checks. It does not draft, call a model, or hold an API key
— the agent is somebody else's process consuming a stable interface. That
separation is deliberate: see
[W21 in the work streams](workstreams.md#w21--an-authoring-contract-for-agents).
