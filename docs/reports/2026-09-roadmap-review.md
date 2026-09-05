# Roadmap review: where to differentiate after v0.10 — September 2026

A month after the first commit, with ten releases behind it and the author using
it at work, Mirzam is at the point where the question changes. Until now the
roadmap was "match what Marp and Typst ship as standard; widen the lead where
Mirzam is already ahead" (the [post-v0.9 plan](2026-08-v0.10-plan.md)). Most of
that list is done. This report asks the next question: **against a field that
moved a great deal in four weeks, where is the differentiation that lasts, and
what should the next three releases be?**

Three research passes were run for it in early September 2026 — the
Markdown/Typst tools, the AI-native and agent ecosystems, and the Japanese and
academic communities — alongside an audit of the repository itself. The
[August survey](2026-08-market-survey.md) still stands; this is the delta and
the decisions proposed from it. Sources are linked inline; where a page was
unreachable from the research sandbox and only a search summary was read, the
text says so.

## Where Mirzam stands

**The product is further along than its audience.** The repository was created
on 2026-08-08. Since then: ten tagged releases, 119 commits, about 44,000 lines
of Rust across fifteen crates, and a feature table in
[roadmap.md](../roadmap.md) where every row but three reads Done. The
August survey's two P0s and four P1s have all landed, and its largest item —
PPTX export with native text boxes — landed in both stages, which no Markdown
slide tool ships. Measured performance holds the design goal at 500 slides.

Against that, the audience numbers:

| Signal | Mirzam | For scale |
|---|---|---|
| GitHub stars / forks / issues | 0 / 0 / 0 | presenterm 8.8k, Marp CLI 3.8k, Touying 2.3k, Slidev 48k |
| VS Code Marketplace | not published (`.vsix` built by a script) | Marp for VS Code: 2M+ installs |
| Open VSX (Cursor, Windsurf, VSCodium) | not published | required to reach those editors at all |
| crates.io / Homebrew / Scoop / winget / npm | none | Marp, Slidev, presenterm all on at least two |
| Mentions in 2026 tool round-ups (Zenn, Qiita, gist lists, comparison blogs) | none found | Marp, Slidev, Typst in every one |

Nothing found in any of the three research passes mentions Mirzam. The one
external reference is the author's own sister project,
[Barnard](https://github.com/ayatough/Barnard), which reuses three Mirzam
crates as git dependencies pinned to `v0.10.0`.

This is the finding the rest of the report is shaped by. The tool competes on
features with projects that have had years and hundreds of contributors, and
on most of the axes the August survey chose, it now wins. But **differentiation
nobody can find is not differentiation**, and the roadmap has so far treated
distribution as a non-topic. The next phase has to carry a distribution track
beside the engineering one, or the engineering compounds into nothing.

## What moved in the field since August

### The Markdown tools are converging on Mirzam's bet

- **Marp** has Core 5 at release-candidate (Shiki, Mermaid rendered
  deterministically by `beautiful-mermaid`, MathJax 4, a core 84 % smaller;
  not yet in the CLI or the VS Code extension —
  [discussion #625](https://github.com/orgs/marp-team/discussions/625)). The
  VS Code extension carries an opt-in **`slide-content-overflow` diagnostic**
  since August 2025 ([marp-vscode #519](https://github.com/marp-team/marp-vscode/issues/519)),
  a single `scrollHeight > clientHeight` check that lives in the editor and
  not the CLI — and an open exploration of rendering slides inside VS Code's
  AI chat "for a faster feedback loop for both humans and language models"
  ([#525](https://github.com/marp-team/marp-vscode/issues/525)). That is the
  same idea as `mirzam check`, approached from the editor side. Editable PPTX
  still needs LibreOffice, drops presenter notes and breaks tables into loose
  shapes ([marp-cli](https://github.com/marp-team/marp-cli),
  [discussion #82](https://github.com/orgs/marp-team/discussions/82)); the
  handout-with-notes issue is still open after six years.
- **Slidev** now ships a **built-in MCP server** — eight tools to list, read,
  update, insert, move and navigate slides — plus an official skill
  (`npx skills add slidevjs/slidev`) and an `llms.txt`
  ([MCP docs](https://github.com/slidevjs/slidev/blob/main/docs/features/mcp.md),
  [Work with AI](https://github.com/slidevjs/slidev/blob/main/docs/guide/work-with-ai.md)).
  None of the eight tools reports layout; the "verify" step is an agent
  looking at the dev server. PPTX is still one image per slide
  ([discussion #2417](https://github.com/slidevjs/slidev/discussions/2417));
  a community pull request for an editable `pptx-editable` format via
  pptxgenjs has been open since 26 August 2026
  ([PR #2722](https://github.com/slidevjs/slidev/pull/2722)).
- **A cluster of small new entrants** sits exactly on Mirzam's agent+PPTX
  ground: [MdPr](https://github.com/ch040602/MdPr) (TypeScript, "deterministic
  Markdown to editable PowerPoint", a "polish gate" that fails on overflow and
  a 16 pt floor, and a Codex review skill),
  [markdown-pptx](https://github.com/pseudosavant/markdown-pptx) (Python,
  `uvx markdown-pptx skill install`, `--json` with stable error codes),
  [md2any](https://github.com/javaperformance/md2any) (a 5 MB Rust binary:
  Markdown to PPTX, ODP, PDF, DOCX with native text and no browser),
  [Deckrun](https://github.com/arpitbbhayani/deckrun) (local-first, "deck
  lint" in CI), [marp2pptx](https://github.com/ebibibi/marp2pptx).
  Each has a dozen stars or fewer; together they say what the market is
  asking for, and they say it in Mirzam's vocabulary — *skill install*,
  *JSON output*, *deterministic layout*, *editable PPTX*, *no LibreOffice*.
- **presenterm** (Rust, terminal) is the riser of the lane: 8.8k stars, 58
  contributors, Mermaid and D2 rendered, code that executes
  ([repo](https://github.com/mfontanini/presenterm)). A different room, but
  proof that a Rust slide tool can gather a community in this window.
- **MDV** ([repo](https://github.com/drasimwagan/mdv)) is the closest thing
  to Mirzam's chart idea from outside: CommonMark plus fenced `chart` and
  KPI blocks and `:::` layout containers, one self-contained HTML file with
  inline SVG. Its Show HN in April 2026 took 152 points and 55 comments and
  it has about 500 stars — evidence that this exact category gets a hearing
  when it is put in front of people.
- **Typst 0.15 / Touying 0.7.4**: still PDF-first. Typst 0.15's HTML export
  now emits MathML for equations but stays behind a feature flag, "not
  intended for production use", with 271 reactions on the tracking issue
  ([#721](https://github.com/typst/typst/issues/721)); Touying's PPTX is PNGs.
  Typst 0.14 emits **tagged PDF/UA-1 by default**
  ([Typst blog](https://typst.app/blog/2025/typst-0.14/)). The telling item
  is Touying [#337](https://github.com/touying-typ/touying/issues/337) (April
  2026), "towards better AI agent workflow integration": a `breakable: false`
  to force one source slide per page **and machine-readable overflow
  warnings for agents** — a request for what `mirzam check` is. Quarto 1.9
  added experimental PDF/A and PDF/UA for Typst and LaTeX, axe-core
  accessibility reports for reveal decks, offline bundling of Typst packages,
  and `llms-txt: true`
  ([Quarto 1.9](https://quarto.org/docs/blog/posts/2026-03-24-1.9-release)).
  reveal.js 6.0 shipped in March 2026 (Vite, MathJax 4, alt-text
  announcements); Pandoc 3.11 made MathML its default math method and 3.8
  gained a PPTX *reader*.
- **marimo** launched interactive slides from notebooks in July 2026, with a
  speaker view and `export pdf --as=slides`
  ([marimo blog](https://marimo.io/blog/slides)) — the notebook lane now has a
  slide story of its own.

### The AI-native products own "draft from nothing", and their export is the complaint

Gamma raised $68M, runs a public API and an MCP server, and is credit-metered;
the most repeated complaint in independent reviews is PPTX export that
overlaps and substitutes fonts ([review](https://deckary.com/blog/gamma-review)).
Tome shut down in April 2025 and users who had not exported lost their decks
([timeline](https://deckary.com/blog/tome-review)). NotebookLM (renamed Gemini
Notebook in July) exports PPTX as image layers, and a cottage industry exists to
make those editable. Claude Design (April 2026) exports PPTX that flattens text
to images per reviewers
([critique](https://claudedesign.substack.com/p/the-claude-design-mistake-you-dont)).
The two that got native output right are the platform owners: **Google Slides
generates fully native, editable decks from a prompt as of June 2026**
([Workspace update](https://workspaceupdates.googleblog.com/2026/06/create-fully-native-and-editable-presentations-with-Gemini-in-Google-Slides.html)),
and **PowerPoint's Copilot Agent Mode is GA with Brand Kits**
([Microsoft 365 blog](https://www.microsoft.com/en-us/microsoft-365/blog/2026/04/22/copilots-agentic-capabilities-in-word-excel-and-powerpoint-are-generally-available/)).
The widely shared failure of the agent-in-PowerPoint route is that the agent
"never touched the underlying template, master slides, theme"
([post](https://www.smithstephen.com/p/i-tried-to-build-one-powerpoint-template)).

The pattern across the category: credit pricing, export paywalls, and the
export itself — overlap, substitution, text that is a picture. None of that
is Mirzam's fight to pick. The deck that is a readable file in a repository,
with an export that is real text, is the position those products leave open
by construction.

### The agent ecosystem has decided what "good" looks like, and it is measurement

- Anthropic's `pptx` skill mandates a **visual pass** — render every slide to
  JPEG with LibreOffice and look at it — and names text overflow "the most
  common defect and always user-visible"
  ([SKILL.md](https://github.com/anthropics/skills/blob/main/skills/pptx/SKILL.md)).
- OpenAI's built-in Codex `slides` skill bundles **programmatic checks**:
  a per-slide renderer, an overflow test, a font-substitution detector, and
  overlap / out-of-bounds warnings
  ([officialskills.sh](https://officialskills.sh/openai/skills/slides)).
  PPTX only.
- [archforge](https://pypi.org/project/archforge/) 0.11.0 (1 September 2026)
  is a "preflight linter for AI-generated PowerPoint": error and warning
  codes, CJK font-fallback rules, **JSON, SARIF and JUnit output**, shipped
  with an agent skill for the build-lint-fix loop. PPTX only.
- The research literature moved the same way: **AeSlides** (April 2026)
  trains with programmatically verifiable layout rewards — collisions,
  whitespace, aspect compliance — and reports collisions down 43 %
  ([repo](https://github.com/ympan0508/aeslides)); **SlideForge** (EMNLP
  2026) verifies against rendered state
  ([arXiv](https://arxiv.org/abs/2609.03109)); PPTArena benchmarks agents on
  structural diffs.
- The 2026 comparison articles score on the same axes: LLM-friendliness
  ("near-zero LLM error rate" is credited to Marp), PPTX editability, setup
  friction, build speed past 100 slides, and **the review loop after an AI
  first draft** (PkgPulse, Pi Stack, dasroot, youngju.dev — summaries only).
  Mirzam has a measured answer to every one and appears in none.
- Distribution has consolidated on **SKILL.md and MCP**, not `llms.txt`:
  Vercel's skills.sh (January 2026, cross-agent, leaderboard), officialskills.sh,
  Anthropic's marketplace, Codex's installer. Paper2Poster shipped a
  Claude/Codex skill in June; Remotion rebranded as "video tools for the agent
  era" with official skills.

Put together: the agent world has independently arrived at *layout you can
measure is layout you can fix*, and is building it for PPTX from the outside
with LibreOffice renders and heuristics. **No Markdown slide tool exposes
rendered-geometry diagnostics from the source** — the research pass looked for
one and did not find it. That slot is Mirzam's, and it is open today, not
forever: Marp's #519 is the first step in, Touying's #337 is the request
written down, and the agents' own skills are the second step.

### Japan: the loudest needs are typographic and institutional

The 2026 Zenn/Qiita/note stream is dominated by "Claude Code drives Marp or
Slidev". A March 2026 round-up of every way to make PowerPoint with generative
AI concludes: the Claude PPTX skill for a one-off, Marp when the deck lives in
Markdown, PptxGenJS code when design consistency matters — and "パワポでの提出が
求められる場面は今後も続く", PowerPoint submission is not going away
([Zenn](https://zenn.dev/ncukondo/articles/ai-generate-pptx-methods-2026),
summary only). The recurring pain, in the community's own words:

1. **見切れ・はみ出し** — overflow. The most-discussed defect; the community's
   fixes are screenshot loops bolted onto Marp
   ([marp-slide-studio](https://github.com/ovrsa/marp-slide-studio),
   [Playwright-based checking](https://ai.giftx.co.jp/blog/claude-code-slide-creation/)).
2. **日本語フォント** — a container without Japanese fonts prints 中華フォント
   glyphs; variable Noto Sans JP falls to its Thin weight in several PDF
   pipelines; embedding Japanese fonts made a Quarto+reveal deck too heavy and
   pushed an academic author to Quarto+Typst
   ([Zenn](https://zenn.dev/nicetak/articles/quarto-typst-slides)).
3. **PowerPoint 提出必須** — and Marp's `--pptx-editable` is what people
   have, with its narrow text boxes and no notes.
4. **会社テンプレート** — companies rebuild their official `.pptx` template as a
   shared Marp CSS ([Qiita](https://qiita.com/hnz/items/4e6d536c056ff6a9ae35)).
   Nobody applies an existing `.potx` master from Markdown. An open gap.
5. **禁則処理** — Chromium's `word-break: auto-phrase` (BudouX) is the state
   of the art for Japanese line breaking and is Chromium-only; Typst has an
   open request for the same.

Academically, **Typst with Touying is the rising choice for Japanese talks**:
Pepabo's research lab produced both its JSAI 2026 talk and its A0 poster from
one Touying-based template, with a coding agent doing the authoring
("Typstのすゝめ：コーディングエージェント時代のスライド・ポスター作成",
[Pepabo](https://rand.pepabo.com/article/2026/06/15/typst/), summary only).
The poster ecosystem is Typst's ([pollux](https://github.com/taka255/pollux),
`peace-of-posters`, `postercise`); no Markdown-native poster tool surfaced.
Accessibility became institutional: the EU Accessibility Act has applied since
June 2025, the US ADA Title II deadline was April 2026, and 合理的配慮 has been
mandatory for every Japanese higher-education institution since April 2025.

## The thesis

Three things are true at once.

1. **The bet was right.** "AI drafts, human reviews the diff, the tool
   measures" is now the consensus of the agent ecosystem, the research
   literature and the Japanese practitioner community. Mirzam built the
   measuring half first and measured its cost in tokens
   ([agent-context report](2026-08-agent-context.md)); nobody in the Markdown
   lane has followed yet.
2. **The moat is narrowing from three directions**: Marp adding an overflow
   diagnostic, Slidev adding an MCP server and a skill, and PPTX-side linters
   (archforge, Codex's checks) doing from the outside what Mirzam does from the
   source.
3. **Nobody has found it.** The features are ahead; the reach is zero. Every
   item below is judged on both.

So the proposal is not "more features". It is: **make the measurement the
product, make it reachable from every agent and editor people already use, and
ship the three outputs institutions require that no Markdown tool ships —
template-faithful PPTX, an accessible PDF, and a poster.** Then freeze the
markup.

## Where the differentiation is now

Five positions, ordered by how defensible they are.

**1. The verifiable deck.** `mirzam check --format json` is the only
source-level, rendered-geometry diagnostic in the Markdown lane, and the
agent world has just decided that is the thing that matters. Widen it in the
two directions the field is pointing: *more kinds* (the "polish gate" family —
text below a readable floor, contrast, a slide too dense, a font that was not
present so the line breaks are not the author's — the archforge and MdPr rule
sets are a ready checklist), and *more channels* (SARIF so a GitHub Action
annotates the PR; a per-slide before/after screenshot pair as a CI artefact for
the human reviewer, since the JSON is for the agent). A further step none of
the competitors can take: `check` as a reward. AeSlides trained a model on
verifiable layout rewards computed on PPTX; Mirzam can be the environment for
that on Markdown, which is a research-community entry point that costs a
script and a README.

**2. The agent's tool, wherever the agent is.** Today the skill installs into
Claude Code. Slidev reaches every agent through MCP and skills.sh. The parity
items are small: an `mirzam mcp` subcommand exposing render, check, outline,
and one slide's screenshot (the JSON-first loop with a picture on demand); the
skill published to skills.sh and officialskills.sh under the agentskills
spec so Codex, Gemini CLI and Cursor install it; a GitHub Action. None of
this is new capability — it is the existing loop with doors on it.

**3. Output the institution accepts.** The three asks that recur across the
Japanese corporate, the academic, and the accessibility sources, none served
by a Markdown tool:
- **PPTX that respects the house template.** Stage three of the exporter:
  `export pptx --template house.potx` — carry the master, layouts, theme
  fonts and colours, and place Mirzam's text boxes in the template's
  placeholders where the pane names map. That is the exact failure the
  Copilot-in-PowerPoint story is famous for, the gap Japanese companies fill
  by hand-porting `.pptx` to Marp CSS, and the "full Slide Master fidelity"
  enterprise buyers list first. Then the refinements the roadmap already
  names: shapes and chart marks as DrawingML, block math as Office math.
- **An accessible PDF.** Typst emits PDF/UA-1 by default; Quarto has it
  experimentally; Marp has nothing. Chromium's `printToPDF` takes a
  `generateTaggedPDF` flag and Mirzam's HTML is already semantic (headings,
  lists, MathML, alt text). The research task the August survey deferred is
  smaller than it looked: turn the flag on, run the result through a
  validator, fix the tag tree where it is wrong. Certified PDF/UA is a
  further step; "tagged, and the checker says so" is the deliverable.
- **A poster.** One page at A0 from the same pane grid: no slide split, a
  page size, print CSS, `fit` doing the work it already does. Typst owns this
  in the academic lane; nothing Markdown does. It compounds with `import pdf`,
  BibTeX and `check` — a poster is exactly the artefact where an overflow
  found on the print shop's proof is the most expensive kind.

**4. CJK typography as a first-class claim.** Mirzam already treats it
seriously (`seminar.md`, `cjk_friendly_emphasis`). The Japanese pain list is
specific and mostly on Mirzam's side of the fence: report the fonts a machine
does *not* have (already in `check`'s notes — promote it to a warning when a
deck names a Japanese face the machine lacks), pin a variable font's weight so
Noto Sans JP does not print Thin, `word-break: auto-phrase` where the browser
has it, and a documented, tested Japanese-font path for the CI container. The
competitors' answer to every one of these is a workaround blog post.

**5. The moving deck.** Animation with an intact PDF, video export, autoplay,
effects — no competitor has the set, and Barnard proves the crates travel. It
is the least contested position and the least in demand; it stays a
differentiator to *show* (the site, the demo) rather than to build further
right now. Narration and Ken Burns keep their place in the queue behind the
items above.

## Decision proposed

Each item scored on the author's criteria from the v0.10 plan — dissatisfaction
today, fit with what Mirzam is, human+agent affinity, feasibility, licence,
difficulty class, audience widening — with one axis added: **whether it is
findable**, meaning whether a person or an agent who does not know Mirzam
exists would meet it through this item.

| Priority | Item | Answers | Effort | Findable |
|---|---|---|---|---|
| **P0** | Publish: VS Code Marketplace **and** Open VSX; crates.io for the core crates (Barnard already depends on them); Homebrew tap, Scoop and winget manifests; the skill on skills.sh and officialskills.sh | zero reach; Cursor/Windsurf users cannot install the extension at all | small, mostly CI | yes — this *is* findability |
| **P0** | `mirzam mcp`: render, check, outline, one slide's screenshot on demand | Slidev parity, with the diagnostic Slidev lacks | small–medium | yes — MCP registries |
| **P0** | `check --format sarif` and a published GitHub Action (`mirzam check` annotating the PR) | the "deck in a PR" story made literal; CI without a script | small | yes — Marketplace listing |
| **P0** | The comparison page and the Japanese entry point: a one-page "Mirzam against Marp, Slidev, Typst" with the measured numbers, and `docs/ja/` covering syntax and the agent loop; one article each on Zenn and the English web | absent from every 2026 round-up | small, writing | yes |
| **P1** | `export pptx --template house.potx`: master, layouts, theme, placeholders by pane name | the loudest corporate ask in JP sources; the Copilot failure mode | large (Opus-class: OOXML inheritance, placeholder geometry) | partly — it is what gets a corporate user to stay |
| **P1** | `check` widened: readable-size floor, contrast, density, missing-font-as-warning, a per-slide screenshot pair as CI artefact | the polish-gate family the field converged on | medium (Sonnet-class: rules over existing measurement) | yes — the JSON is what agents read |
| **P1** | Tagged PDF: `generateTaggedPDF` on, validated, tag tree fixed | EAA / ADA / 合理的配慮; Typst-only today | medium, research first | partly — accessibility lists |
| **P1** | CJK path: weight pinning, `auto-phrase`, Japanese font warning, tested CI container recipe | the JP community's #2 complaint | small–medium | yes — it is the article to write |
| **P2** | Poster mode: one page, a paper size, the same grid | academic lane, Typst-only | medium (print CSS, `fit`, `check` for print) | yes — a gallery poster |
| **P2** | Slide- and section-level attributes | unchanged from the roadmap | medium | no |
| **P2** | `check` as a reward: a script that scores a deck for an RL or eval loop, with a README | research-community entry | small | yes — a paper's related-work section |
| **P3** | W18 carry, W8 annotation write-back, router, Ken Burns, narration, plugins | as before | large each | no |
| **Then** | **1.0**: freeze the markup, version `llms.txt` and the check schema, a migration note per breaking change since | the promise agents and institutions need before adopting | — | yes — "stable" is a feature |

Ordering within P0 and P1 is by cost, because the P0s are cheap and the reach
they buy is what makes the P1s worth building.

## What to say no to

The August non-goals hold and the research strengthened them: no real-time
collaboration, no analytics, no WYSIWYG, no executable cells. Two are added:

- **No generation.** Gamma, Google, Microsoft, Anthropic's own Claude Design
  and a dozen others generate decks from a prompt; the agent the user already
  runs generates Markdown. Mirzam's role is what happens *after* generation —
  measure, diff, export — and a "generate a deck" button would put it in a
  race it cannot win and does not need to.
- **No hosted anything.** Every export paywall and every dead product in the
  AI-native lane (Tome's users lost their decks) is the argument. A deck is a
  file; the checker runs on the user's machine; the MCP server is a local
  process.

And one to hold: **no direct PDF without Chromium**, still. md2any shows a
5 MB Rust binary can write PDF and PPTX with no browser, and that is
attractive for the air-gapped container; but it means a text engine and the
end of "the browser does typography", which is the principle that keeps CJK
right. Revisit when a Rust text-layout stack matures; do not build one.

## Release shape

- **v0.11 — findable.** The P0 row entire: marketplaces, registries, `mirzam
  mcp`, SARIF and the Action, the comparison page, the Japanese docs, the two
  articles. Nothing in the markup changes. Success is measured by the first
  issue opened by someone who is not the author.
- **v0.12 — accepted.** `--template`, tagged PDF, the widened `check`, the CJK
  path. The corporate and the academic user each get the one export that was
  blocking them.
- **v0.13 — the poster, and the scope between deck and pane.** Plus the
  reward script.
- **1.0 — frozen.** Markup, `llms.txt`, and the check schema versioned and
  promised; a deprecation policy; today's decks keep rendering.

## Open questions for the author

- **Which corporate template first?** `--template` is large; building it
  against one real `.potx` the author has to submit to will find the geometry
  the spec hides. The design should start from that file.
- **The site is the comparison.** The landing page could not be read from the
  sandbox; if it does not yet state the measured comparison against Marp,
  Slidev and Typst in a table, that is the highest-leverage page on it.
- **Barnard and the crates.** Publishing `mirzam-syntax`, `mirzam-shape` and
  `mirzam-anim` to crates.io is a small change that makes both projects easier
  to depend on; it also commits their public API a little earlier than 1.0.
  Worth deciding whether they are `0.x` crates with the same breakage policy
  as the binary.
- **Discussions.** GitHub Discussions is off. With the P0 row landing, a
  place for questions that is not an issue tracker is worth turning on.

## Sources

Read directly unless marked *summary only* (page blocked from the sandbox;
search-engine summary used).

- Marp: [Core v5 RC](https://github.com/orgs/marp-team/discussions/625),
  [marp-vscode #519 overflow diagnostic](https://github.com/marp-team/marp-vscode/issues/519),
  [marp-vscode #525 chat renderer](https://github.com/marp-team/marp-vscode/issues/525),
  [marp-cli](https://github.com/marp-team/marp-cli),
  [editable PPTX discussion #82](https://github.com/orgs/marp-team/discussions/82),
  [Marp for VS Code](https://marketplace.visualstudio.com/items?itemName=marp-team.marp-vscode)
- Slidev: [MCP](https://github.com/slidevjs/slidev/blob/main/docs/features/mcp.md),
  [Work with AI](https://github.com/slidevjs/slidev/blob/main/docs/guide/work-with-ai.md),
  [PPTX discussion #2417](https://github.com/slidevjs/slidev/discussions/2417),
  [editable PPTX PR #2722](https://github.com/slidevjs/slidev/pull/2722)
- New entrants: [MdPr](https://github.com/ch040602/MdPr),
  [markdown-pptx](https://github.com/pseudosavant/markdown-pptx),
  [md2any](https://github.com/javaperformance/md2any),
  [Deckrun](https://github.com/arpitbbhayani/deckrun),
  [presenterm](https://github.com/mfontanini/presenterm),
  [MDV](https://github.com/drasimwagan/mdv)
- Typst / Quarto / marimo: [Typst 0.14](https://typst.app/blog/2025/typst-0.14/),
  [Typst 0.15](https://github.com/typst/typst/releases/tag/v0.15.0),
  [HTML export tracking #721](https://github.com/typst/typst/issues/721),
  [Touying](https://github.com/touying-typ/touying),
  [Touying #337 agent workflow](https://github.com/touying-typ/touying/issues/337),
  [touying-exporter](https://github.com/touying-typ/touying-exporter),
  [reveal.js 6.0](https://github.com/hakimel/reveal.js/releases/tag/6.0.0),
  [Pandoc 3.11](https://github.com/jgm/pandoc/releases/tag/3.11),
  [Quarto 1.9](https://quarto.org/docs/blog/posts/2026-03-24-1.9-release),
  [marimo slides](https://marimo.io/blog/slides),
  [pollux](https://github.com/taka255/pollux)
- AI-native: [Gamma review](https://deckary.com/blog/gamma-review),
  [Tome timeline](https://deckary.com/blog/tome-review),
  [Google Slides native generation](https://workspaceupdates.googleblog.com/2026/06/create-fully-native-and-editable-presentations-with-Gemini-in-Google-Slides.html),
  [Copilot Agent Mode GA](https://www.microsoft.com/en-us/microsoft-365/blog/2026/04/22/copilots-agentic-capabilities-in-word-excel-and-powerpoint-are-generally-available/),
  [Claude Design](https://www.anthropic.com/news/claude-design-anthropic-labs),
  [Claude Design export critique](https://claudedesign.substack.com/p/the-claude-design-mistake-you-dont),
  [template failure post](https://www.smithstephen.com/p/i-tried-to-build-one-powerpoint-template)
- Agents: [Anthropic pptx SKILL.md](https://github.com/anthropics/skills/blob/main/skills/pptx/SKILL.md),
  [Codex slides skill](https://officialskills.sh/openai/skills/slides),
  [archforge](https://pypi.org/project/archforge/),
  [AeSlides](https://github.com/ympan0508/aeslides),
  [SlideForge](https://arxiv.org/abs/2609.03109),
  [PPTArena](https://github.com/michaelofengenden/PPTArena),
  [Paper2Poster](https://github.com/paper2poster/paper2poster),
  [Remotion](https://github.com/remotion-dev/remotion),
  [Open VSX and Cursor](https://cursor.com/help/customization/extensions)
- Japan: [生成AIでパワポを作る方法一覧 2026年3月版](https://zenn.dev/ncukondo/articles/ai-generate-pptx-methods-2026) *summary only*,
  [4大ツール比較](https://zenn.dev/mjinia/articles/cef5337a4f177f) *summary only*,
  [Typstのすゝめ (Pepabo)](https://rand.pepabo.com/article/2026/06/15/typst/) *summary only*,
  [Quarto + Typst slides](https://zenn.dev/nicetak/articles/quarto-typst-slides) *summary only*,
  [PPTX template as Marp CSS](https://qiita.com/hnz/items/4e6d536c056ff6a9ae35) *summary only*,
  [marp-slide-studio](https://github.com/ovrsa/marp-slide-studio),
  [見切れ問題](https://note.com/yaoyoroztech/n/n58e13df410c7) *summary only*,
  [Noto Sans JP Thin in PDF](https://learn.microsoft.com/ja-jp/answers/questions/4377755/power-point-pdf-noto-sans-jp-thin),
  [word-break: auto-phrase](https://developer.chrome.com/blog/css-i18n-features)
- Accessibility: [EAA and PDF](https://pdfix.net/european-accessibility-act-2025-are-your-pdfs-ready/),
  [Chrome DevTools Protocol printToPDF](https://chromedevtools.github.io/devtools-protocol/tot/Page/)
