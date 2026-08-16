#!/usr/bin/env bash
# Build the documentation site: a landing page plus every sample deck rendered
# as a live, self-contained HTML file.
#   ./scripts/build-site.sh [out_dir]
#
# Two channels, because the site is read by two different people. `stable` is
# built from the latest tag and published at the root - that is what a stranger
# arriving from a link sees. `dev` is built from the tip of `main` and published
# under /next/, which is how the author checks a change without waiting for a
# release. The dev build says so on the page and asks not to be indexed;
# everything else about the two is identical.
#
#   MIRZAM_SITE_CHANNEL=dev MIRZAM_SITE_VERSION="v0.1.0 +11 · 72433fb" \
#     ./scripts/build-site.sh site/next
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="${1:-site}"
DECKS=(01-start 02-writing 03-layout 04-components 05-motion 06-theming pitch research seminar)

CHANNEL="${MIRZAM_SITE_CHANNEL:-stable}"
# Falls back to `git describe` so a build run by hand is stamped too. The
# workflow passes the string it wants rather than relying on this, because a
# shallow checkout has no tags to describe against.
VERSION="${MIRZAM_SITE_VERSION:-$(git describe --tags --always 2>/dev/null || echo unknown)}"
BUILT="$(date -u +%Y-%m-%d)"

rm -rf "$OUT"
mkdir -p "$OUT/decks"

# A deck published under /decks/<name>/ cannot resolve the links its source
# wrote relative to its own directory, so each build is told where that
# directory lives on GitHub.
REPO_BLOB="https://github.com/ayatough/Mirzam/blob/main"

# The browser editor: the same Rust core compiled to WebAssembly, so someone
# with no toolchain - or no laptop - can still write a deck and download it.
# Skipped when the WASM toolchain is not available, and the landing page then
# simply does not offer the card, so a site built without it has no dead link.
#
# Built before the decks, because each deck is told where it is: a slide on the
# site hands itself to this editor, and a site built without one must not link
# to a page it does not contain.
TRY_CARD=""
EDITOR_ARGS=()
echo "==> building the browser editor"
if [ "${MIRZAM_SKIP_WASM:-}" != "1" ] && ./scripts/build-wasm.sh web/wasm-demo/pkg; then
  mkdir -p "$OUT/try"
  cp -R web/wasm-demo/. "$OUT/try/"
  cp docs/brand/mirzam-icon-light.svg "$OUT/try/favicon.svg"
  TRY_CARD='<a class="card" href="try/"><b>Write one in the browser</b><span>The Rust core as WebAssembly: no install, and it runs on a phone</span></a>'
  # Relative, because both channels publish the same decks: `/decks/x/` and
  # `/next/decks/x/` each reach their own editor by the same three steps up.
  EDITOR_ARGS=(--editor-url ../../try/)
else
  echo "  (skipped; the landing page will not link to it, and the decks"
  echo "   carry their source with no way out of the panel)"
fi
# Expanded below as `${EDITOR_ARGS[@]+"${EDITOR_ARGS[@]}"}` rather than
# `"${EDITOR_ARGS[@]}"`: under `set -u` an empty array counts as unbound in
# bash before 4.4, which is the bash macOS still ships.

echo "==> building decks"
cargo build --release --bin mirzam
# `--embed-source` is what makes a slide legible as *markup*: the site shows
# the rendering and the prose about it, and until now nothing on the page said
# which eight lines produced the slide in front of you. With it, `V` opens the
# Markdown beside the slide, and the editor link hands that Markdown over.
for deck in "${DECKS[@]}"; do
  ./target/release/mirzam build "examples/$deck.md" -o "$OUT/decks/$deck" \
    --base-url "$REPO_BLOB/examples/" --embed-source ${EDITOR_ARGS[@]+"${EDITOR_ARGS[@]}"}
done
# The README with no Mirzam syntax at all, split at its own headings. The theme
# and the fit come from the command line for the same reason: frontmatter would
# show up as a stray table at the top of the README on GitHub, so the one thing
# this deck cannot do is carry its own settings. `--theme mirzam` is redundant
# with the fallback and kept for the reader's benefit - it now carries the
# identity's type as well as its colours, so there is no second stylesheet to
# name. Without `--fit` four of its sections are longer than a slide and the
# viewer simply cuts them off - which is the worst outcome for the deck whose
# whole claim is that an unedited document becomes a deck.
./target/release/mirzam build README.md -o "$OUT/decks/readme" --split h2 \
  --theme mirzam --fit shrink --mode dark \
  --base-url "$REPO_BLOB/" --embed-source ${EDITOR_ARGS[@]+"${EDITOR_ARGS[@]}"}

# The themes gallery: one specimen slide put through every theme in both modes,
# photographed. Generated here rather than committed, for the same reason the
# decks are - a picture of a stylesheet checked into the repository is true on
# the day it is taken and quietly wrong afterwards, and there is no test that
# can see it drift. Twelve screenshots is under a megabyte and takes a few
# seconds, so there is nothing to be gained by keeping them.
#
# Skipped, like the editor above, when the machinery is not here: the gallery
# needs `playwright-core` and a browser, which the layout-check job installs and
# a laptop may not have. A site built without them simply does not offer the
# card, so nothing on the page is a dead link.
GALLERY_CARD=""
echo "==> building the themes gallery"
if [ "${MIRZAM_SKIP_GALLERY:-}" != "1" ] && node scripts/make-theme-gallery.mjs -o "$OUT/themes"; then
  GALLERY_CARD='<a class="card" href="themes/"><b>Themes</b><span>The same slide in all six identities, light and dark — generated from the stylesheets</span></a>'
else
  echo "  (skipped; needs playwright-core and a browser. The landing page will"
  echo "   not link to it.)"
fi

# The brand assets the landing page and the link preview are built from. Copied
# rather than inlined so the social card has a stable absolute URL - a scraper
# fetches that image separately, and cannot follow a data: URI.
echo "==> copying brand assets"
mkdir -p "$OUT/brand"
cp docs/brand/*.svg docs/brand/*.png docs/brand/*.webp "$OUT/brand/"

# The syntax card, at the path the emerging convention puts it: a model handed
# a URL for a tool's markup looks for /llms.txt, and one file it can read
# beginning to end is worth more than a docs tree it has to crawl. Copied
# rather than linked to GitHub, because the point is that the site itself
# serves it - and `.txt` rather than `.md` for the same reason the rest of the
# prose stays on GitHub: nothing here renders Markdown.
echo "==> copying the syntax card"
cp docs/llms.md "$OUT/llms.txt"

# The prose stays on GitHub, linked absolutely from the landing page below.
# `actions/deploy-pages` serves this directory verbatim - no Jekyll runs, so a
# copied `syntax.md` would never become the `syntax.html` the page linked to,
# and every one of those links 404'd. The site's job is showing the decks
# running; GitHub already renders Markdown well, anchors and all.

cat > "$OUT/index.html" <<'HTML'
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Mirzam - presentation decks that live in your repository</title>
<meta name="description" content="Write plain Markdown, draw the layout as ASCII, and get a deck with real charts, diagrams, video and math - as one self-contained HTML file or a PDF.">
<!--NOINDEX-->

<link rel="icon" href="brand/mirzam-icon-light.svg">
<link rel="apple-touch-icon" href="brand/mirzam-icon-512.png">

<meta property="og:type" content="website">
<meta property="og:url" content="https://ayatough.github.io/Mirzam/">
<meta property="og:title" content="Mirzam - presentation decks that live in your repository">
<meta property="og:description" content="Write plain Markdown, draw the layout as ASCII, and get a deck with real charts, diagrams, video and math - as one self-contained HTML file or a PDF.">
<meta property="og:image" content="https://ayatough.github.io/Mirzam/brand/mirzam-social-card.png">
<meta name="twitter:card" content="summary_large_image">

<script>
// The stored colour mode, applied before the first paint. Reading it after the
// body has parsed would show the default palette and swap it a frame later.
// Only a reader who chose light needs stamping: the page is dark with nothing
// stored, so dark is already what the stylesheet below paints.
try {
  var m = localStorage.getItem('mirzam-mode');
  if (m === 'light' || m === 'dark') document.documentElement.dataset.theme = m;
} catch (e) {}
</script>

<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300;400;500&family=Inter:wght@300;400&family=IBM+Plex+Mono:wght@400&display=swap">
<style>
  /* Mirzam Light and Mirzam Dark, straight from docs/brand/mirzam-theme.css.
     Both palettes are written once, as `--l-*` and `--d-*`; what follows only
     decides which set is in force.

     **Dark is the default, and not because a machine asked for it.** This page
     is the front door of a brand whose mark is drawn on a dark ground, and the
     deck linked from it is built `--mode dark` - so a page that followed
     `prefers-color-scheme` disagreed with the deck beside it on every reader
     whose system prefers light. There is no system route here any more: the
     page is dark until somebody says otherwise, which is the one thing that
     cannot drift out of step with the deck.

     The reader's own choice still wins, in both directions, and it is stored
     under the key a deck's viewer reads: press the switch here and a deck
     opens the same way, press `D` in a deck and this page opens the way that
     deck ended up. That key holds a deliberate choice and nothing else -
     neither side writes it on load - so there is no stale value for it to
     carry. */
  :root {
    color-scheme: dark;

    --l-bg:#F7F8FC; --l-surface:#EEF0F7; --l-fg:#17203A; --l-muted:#68708A; --l-line:#D9DDEB;
    --l-accent:#6557D9; --l-accent-2:#8B7CFF; --l-cyan:#4F8CC9;
    /* A button's label has to clear AA on the violet it sits on, and white
       does not clear it on the light violet - so the ink flips with the theme
       and the hover shade moves away from the label, not towards it. */
    --l-btn-bg:#6557D9; --l-btn-bg-hover:#5347B8; --l-btn-ink:#FFFFFF;
    --l-hero: url("brand/mirzam-hero-light.webp");
    --l-wordmark: url("brand/mirzam-wordmark-light.svg");
    --l-workflow: url("brand/mirzam-concept-workflow-light.svg");
    --l-code-bg:#EEF0F7; --l-shadow: 0 1px 2px rgba(23,32,58,.06), 0 8px 24px rgba(23,32,58,.06);
    --l-lead-weight: 300;
    /* The switch shows where it takes you, not where you are, so the glyph and
       the label it carries say the same thing. */
    --l-switch: "\263D\FE0E";  /* a moon on the light page: click for dark */

    --d-bg:#080C18; --d-surface:#0E1425; --d-fg:#F4F6FF; --d-muted:#8F9AB8; --d-line:#252E47;
    --d-accent:#9B8CFF; --d-accent-2:#C0B7FF; --d-cyan:#72B5E8;
    --d-btn-bg:#9B8CFF; --d-btn-bg-hover:#C0B7FF; --d-btn-ink:#080C18;
    --d-hero: url("brand/mirzam-hero-dark.webp");
    --d-wordmark: url("brand/mirzam-wordmark-dark.svg");
    --d-workflow: url("brand/mirzam-concept-workflow-dark.svg");
    --d-code-bg:#11192D; --d-shadow: none;
    /* Light type on a dark ground reads a step thinner than it measures. */
    --d-lead-weight: 400;
    --d-switch: "\2600\FE0E";  /* a sun on the dark page: click for light */

    --bg:var(--d-bg); --surface:var(--d-surface); --fg:var(--d-fg);
    --muted:var(--d-muted); --line:var(--d-line);
    --accent:var(--d-accent); --accent-2:var(--d-accent-2); --cyan:var(--d-cyan);
    --btn-bg:var(--d-btn-bg); --btn-bg-hover:var(--d-btn-bg-hover); --btn-ink:var(--d-btn-ink);
    --hero:var(--d-hero); --wordmark:var(--d-wordmark); --workflow:var(--d-workflow);
    --code-bg:var(--d-code-bg); --shadow:var(--d-shadow);
    --lead-weight:var(--d-lead-weight); --switch:var(--d-switch);
  }
  /* Light, reached one way: the reader asked for it. Only the mapping repeats
     - the colours above are written once - so this block and the one above can
     disagree about which set is in force, never about what a colour is. It is
     stamped on `<html>` before the first paint by the script in the head, so a
     reader who chose light never sees a dark frame first. */
  :root[data-theme="light"] {
    color-scheme: light;
    --bg:var(--l-bg); --surface:var(--l-surface); --fg:var(--l-fg);
    --muted:var(--l-muted); --line:var(--l-line);
    --accent:var(--l-accent); --accent-2:var(--l-accent-2); --cyan:var(--l-cyan);
    --btn-bg:var(--l-btn-bg); --btn-bg-hover:var(--l-btn-bg-hover); --btn-ink:var(--l-btn-ink);
    --hero:var(--l-hero); --wordmark:var(--l-wordmark); --workflow:var(--l-workflow);
    --code-bg:var(--l-code-bg); --shadow:var(--l-shadow);
    --lead-weight:var(--l-lead-weight); --switch:var(--l-switch);
  }
  * { box-sizing: border-box; }
  body {
    margin: 0; background: var(--bg); color: var(--fg);
    font: 400 16px/1.7 Inter, "Hiragino Sans", "Noto Sans JP", system-ui, sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  .wrap { max-width: 880px; margin: 0 auto; padding: 0 24px; }

  /* The hero art is the horizon the mark rises over, so the lockup sits above
     the limb rather than on it, and the scrim keeps the tagline off the flare. */
  .hero { position: relative; overflow: hidden; border-bottom: 1px solid var(--line); }
  /* Both layers are decoration painted behind the lockup. They are positioned
     siblings of `.wrap` and come after it in the box tree, so without the
     stacking order below the scrim covers the content - and, being a real box,
     swallows every click aimed at the buttons underneath it. `pointer-events`
     alone would restore the links but leave the labels washed out by the fade;
     `z-index` alone would fix both, and the pair says why. */
  .hero::before, .hero::after { pointer-events: none; }
  .hero::before {
    content: ""; position: absolute; inset: 0;
    background: var(--hero) 20% 100% / cover no-repeat;
  }
  .hero::after {
    content: ""; position: absolute; inset: 0;
    background: linear-gradient(180deg, var(--bg) 0%, transparent 34%, transparent 62%, var(--bg) 100%);
  }
  .hero .wrap { position: relative; z-index: 1; padding-top: 88px; padding-bottom: 104px; }
  /* The wordmark is a background rather than an <img> so it rides the same
     token as everything else and flips with the switch, not only with the
     operating system. It carries its name for anyone who cannot see it. */
  .wordmark {
    width: min(340px, 78vw); aspect-ratio: 340 / 135;
    background: var(--wordmark) left center / contain no-repeat;
  }
  /* The pipeline diagram, for the same reason and by the same means. As an
     <img> inside a <picture> it followed `prefers-color-scheme`, which can only
     ask the operating system — so the switch turned the page dark and left a
     white diagram sitting in it. A deck build rewrites a <picture> to follow
     the deck's own mode; this page is plain HTML and has to say it itself. */
  .workflow {
    width: 100%; aspect-ratio: 1400 / 420; margin-top: 26px; border-radius: 14px;
    background: var(--workflow) center / contain no-repeat;
  }

  /* The switch, in the corner the eye reaches last. */
  .switch {
    position: absolute; top: 18px; right: 22px; z-index: 2;
    width: 38px; height: 38px; border-radius: 10px; cursor: pointer;
    display: grid; place-items: center; font-size: 1.05rem; line-height: 1;
    background: var(--surface); color: var(--fg); border: 1px solid var(--line);
  }
  .switch::before { content: var(--switch); }
  .switch:hover { border-color: var(--accent); color: var(--accent); }
  .switch:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tag {
    font-size: 1.22rem; font-weight: var(--lead-weight); line-height: 1.55;
    max-width: 30em; margin: 28px 0 32px;
  }
  .cta { display: flex; flex-wrap: wrap; gap: 12px; margin: 0; }
  .btn {
    display: inline-block; padding: 11px 20px; border-radius: 10px; text-decoration: none;
    font-family: "Space Grotesk", system-ui, sans-serif; font-weight: 500; font-size: .96rem;
    background: var(--btn-bg); color: var(--btn-ink); border: 1px solid transparent;
  }
  .btn:hover { background: var(--btn-bg-hover); }
  .btn.ghost { background: transparent; color: var(--fg); border-color: var(--line); }
  .btn.ghost:hover { border-color: var(--accent); color: var(--accent); }

  main { padding-bottom: 96px; }
  h2 {
    font-family: "Space Grotesk", "Hiragino Sans", "Noto Sans JP", system-ui, sans-serif;
    font-weight: 400; font-size: 1.7rem; letter-spacing: -.02em; margin: 3em 0 .1em;
  }
  h2 + p { color: var(--muted); margin-top: .5em; }
  a { color: var(--accent); text-underline-offset: 2px; }
  ul { padding-left: 1.2em; }
  li { margin: .45em 0; }

  .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(238px, 1fr)); gap: 14px; margin-top: 26px; }
  .card {
    display: block; padding: 18px 20px; border: 1px solid var(--line); border-radius: 14px;
    background: var(--surface); text-decoration: none; color: inherit; box-shadow: var(--shadow);
  }
  .card:hover { border-color: var(--accent); }
  .card b {
    display: block; margin-bottom: .3em; color: var(--fg);
    font-family: "Space Grotesk", system-ui, sans-serif; font-weight: 500; font-size: 1.02rem;
  }
  .card span { color: var(--muted); font-size: .92rem; line-height: 1.55; }

  code, pre { font-family: "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, monospace; }
  code { background: var(--code-bg); color: var(--accent); padding: .12em .4em; border-radius: 5px; font-size: .88em; }
  pre {
    background: var(--code-bg); border: 1px solid var(--line); border-radius: 12px;
    padding: 16px 18px; overflow-x: auto; font-size: .9rem; line-height: 1.7; margin-top: 26px;
  }
  pre code { background: none; color: inherit; padding: 0; }

  .kbd {
    font-family: "IBM Plex Mono", ui-monospace, monospace; font-size: .82em;
    border: 1px solid var(--line); border-bottom-width: 2px; border-radius: 5px;
    padding: .1em .45em; background: var(--surface); color: var(--fg);
  }
  footer {
    color: var(--muted); font-size: .9rem; margin-top: 4.5em;
    border-top: 1px solid var(--line); padding-top: 1.6em;
  }

  /* The preview banner. Only the /next/ build carries one, and it is the first
     thing on the page because its whole job is to stop somebody mistaking a
     working copy for the product. Both accents clear AA against the button ink
     they pair with, in either mode - see the palette in docs/brand. */
  .devbar {
    background: var(--accent); color: var(--btn-ink);
    padding: 11px 24px; font-size: .92rem; line-height: 1.6;
  }
  /* The gap after the tag is a real space in the markup, not margin alone:
     margin separates the words on screen and nowhere else, so the banner
     copied out as "DEVUnreleased" and read aloud the same way. */
  .devbar b {
    font-family: "Space Grotesk", system-ui, sans-serif;
    font-weight: 500; letter-spacing: .08em; margin-right: .25em;
  }
  .devbar code { background: rgba(0,0,0,.2); color: inherit; }
  .devbar a { color: inherit; text-underline-offset: 3px; }

  /* The unreleased changelog, on the dev build only: what this site has that
     the released one does not. */
  .unreleased h3 {
    font-family: "Space Grotesk", system-ui, sans-serif;
    font-weight: 500; font-size: 1.02rem; margin: 1.8em 0 .2em;
  }
  .unreleased ul { margin-top: .4em; }
  .unreleased li { color: var(--muted); }
  .unreleased li code { font-size: .84em; }
  @media (max-width: 620px) {
    .hero .wrap { padding-top: 56px; padding-bottom: 72px; }
    h2 { font-size: 1.45rem; }
  }
</style>
</head>
<body>
<!--DEVBAR-->

<header class="hero">
  <button class="switch" id="switch" type="button" aria-label="Switch colour mode"></button>
  <div class="wrap">
    <div class="wordmark" role="img" aria-label="Mirzam"></div>
    <p class="tag">Presentation decks that live in your repository. Plain Markdown in,
    charts, diagrams, video and math out — as one HTML file or a PDF.</p>
    <p class="cta">
      <a class="btn" href="decks/pitch/">See a deck running</a>
      <a class="btn ghost" href="https://github.com/ayatough/Mirzam">Source on GitHub</a>
    </p>
  </div>
</header>

<main class="wrap">
<!--UNRELEASED-->
  <h2>Start here</h2>
  <p>Six slides between reading about Mirzam and having a deck.
  <span class="kbd">←</span> <span class="kbd">→</span> to navigate,
  <span class="kbd">N</span> for speaker notes, <span class="kbd">/</span> for the rest.</p>
  <p>Every deck here carries its own Markdown: press <span class="kbd">V</span>
  on any slide to read the source that produced it beside the slide, and from
  there one click opens it in the browser editor, where you can change it and
  watch it re-render.</p>
  <div class="cards">
    <a class="card" href="decks/01-start/"><b>Your first deck</b><span>The smallest file that works, where a page breaks, the three commands</span></a>
    <a class="card" href="decks/readme/"><b>A README, unedited</b><span>What <code>--split h2</code> does to a document nobody wrote for slides</span></a>
  </div>

  <h2>The markup, deck by deck</h2>
  <p>Not a path to walk — a reference to look things up in. Each deck covers one
  area and is written in the markup it documents, so the source beside the slides
  is the example.</p>
  <div class="cards">
    <a class="card" href="decks/02-writing/"><b>02 · Writing a slide</b><span>Headings, emphasis, lists, tables, maths, footnotes, emoji</span></a>
    <a class="card" href="decks/03-layout/"><b>03 · Layout</b><span>One layout rule per slide</span></a>
    <a class="card" href="decks/04-components/"><b>04 · Components</b><span>Charts, shapes, connectors, media, annotations</span></a>
    <a class="card" href="decks/05-motion/"><b>05 · Motion</b><span>Entrances, click-through builds, page turns, effects</span></a>
    <a class="card" href="decks/06-theming/"><b>06 · Theming</b><span>Themes, frontmatter, attributes, custom CSS</span></a>
    <!--GALLERY-->
  </div>

  <h2>Whole decks</h2>
  <p>Written for an audience rather than as documentation — which is the only
  honest way to show what the markup adds up to.</p>
  <div class="cards">
    <a class="card" href="decks/pitch/"><b>A sales pitch</b><span>Metric tiles, charts from CSV, one hero image per colour mode</span></a>
    <a class="card" href="decks/research/"><b>A research report</b><span>Maths, a chart, and a bibliography cited from four slides</span></a>
    <a class="card" href="decks/seminar/"><b>A research talk, in Japanese</b><span>Maths, a quoted figure, citations, CJK typography</span></a>
  </div>

  <h2>How it works</h2>
  <p>Four stages, all at build time. The charts are SVG, the math is MathML, the
  images are inlined — the deck is one file you can email.</p>
  <div class="workflow" role="img" aria-label="Markdown becomes an ASCII layout, then real components, then a self-contained HTML or PDF deck"></div>

  <h2>Write one</h2>
  <p>Nothing to install: the editor below runs the same Rust core in your browser
  and hands you the finished <code>.html</code>. It works on a phone.</p>
  <div class="cards">
    <!--TRY-->
    <a class="card" href="https://github.com/ayatough/Mirzam/blob/main/docs/quickstart.md"><b>Quick start</b><span>Browser, command line, VS Code, Obsidian, phone</span></a>
  </div>

  <h2>Read</h2>
  <ul>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/syntax.md">Syntax reference</a> — every block and inline form</li>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/layout.md">Layout guide</a> — sizing, spacing, keeping arrows out of the text</li>
    <li><a href="llms.txt">Syntax card</a> — the whole markup on one page, for a model writing a deck</li>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/architecture.md">Architecture</a> — how it is built and why</li>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/roadmap.md">Roadmap</a> — what works today, what is next</li>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/development.md">Development guide</a> — build, test, contribute</li>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/brand/README.md">Brand assets</a> — the mark, the palette, the type</li>
    <li><a href="https://github.com/ayatough/Mirzam/blob/main/docs/ja/README.md">日本語</a></li>
  </ul>

  <h2>Install it</h2>
  <p>No Rust toolchain needed — every release ships a binary for macOS, Linux
  and Windows.</p>
<pre><code>curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh

mirzam build deck.md -o out          # one self-contained HTML file
mirzam serve deck.md                 # live preview, re-rendering as you type
mirzam export pdf deck.md</code></pre>
  <p>Windows, or picking a version by hand:
  <a href="https://github.com/ayatough/Mirzam/releases">the releases page</a>.
  Prefer to build it yourself? <code>cargo install --path crates/mirzam-cli
  --bin mirzam</code> from a clone, with Rust 1.91+.</p>

  <footer>
    MIT licensed · <a href="https://github.com/ayatough/Mirzam">Source on GitHub</a>
    · <!--VERSION-->
  </footer>
</main>

<script>
// The colour mode. Two states that matter - dark, which is what this page is
// until somebody says otherwise, and light, which a reader asks for. The
// choice is stored under the key a deck's viewer reads, so a deck opened from
// a light page opens light - and the page's *effective* mode is put on the
// deck links as ?mode= too, dark included, so what you click looks like what
// you were looking at. That also covers the reader whose browser refuses
// storage. Everything here degrades to "the page is dark", which is what a
// reader with no JavaScript at all gets, because the CSS above says so.
(() => {
  const KEY = 'mirzam-mode';
  const root = document.documentElement;
  // Stamped per build. Each deck is its own file, so a phone that has opened
  // one before keeps serving that copy while a deck visited for the first time
  // arrives fresh — which reads as "the new control is missing from this one
  // slide deck only". Carrying the build in the link makes every deck's URL
  // change when the site does.
  const BUILD = '<!--BUILD-->';
  const read = () => { try { return localStorage.getItem(KEY); } catch (e) { return null; } };
  const write = (m) => { try { localStorage.setItem(KEY, m); } catch (e) {} };

  // Nothing stored means dark here, not "ask the machine" - the one place that
  // decision is written down for the script, matching the stylesheet's own.
  const effective = (mode) => (mode === 'light' ? 'light' : 'dark');

  const apply = (mode) => {
    if (mode) root.dataset.theme = mode; else delete root.dataset.theme;
    const dark = effective(mode) === 'dark';
    const btn = document.getElementById('switch');
    if (btn) {
      // The glyph is the destination, so the label has to be too.
      btn.setAttribute('aria-label', dark ? 'Switch to light mode' : 'Switch to dark mode');
      btn.title = btn.getAttribute('aria-label');
    }
    for (const a of document.querySelectorAll('a[href^="decks/"], a[href^="try/"]')) {
      const base = a.getAttribute('href').split('?')[0];
      const q = [];
      if (BUILD) q.push('v=' + encodeURIComponent(BUILD));
      // The mode the page is actually showing, chosen or not: a reader who has
      // picked nothing is still looking at a dark page, and a deck that opened
      // light underneath them would be the same disagreement one level down.
      q.push('mode=' + effective(mode));
      a.setAttribute('href', q.length ? base + '?' + q.join('&') : base);
    }
  };

  let mode = read();
  if (mode !== 'light' && mode !== 'dark') mode = null;
  apply(mode);

  document.getElementById('switch').addEventListener('click', () => {
    mode = effective(mode) === 'dark' ? 'light' : 'dark';
    write(mode);
    apply(mode);
  });
})();
</script>
</body>
</html>
HTML

# Every local link on the landing page must resolve in the artifact, since
# nothing rewrites paths after this point.
sed -i.bak "s|<!--TRY-->|$TRY_CARD|" "$OUT/index.html" && rm -f "$OUT/index.html.bak"
sed -i.bak "s|<!--GALLERY-->|$GALLERY_CARD|" "$OUT/index.html" && rm -f "$OUT/index.html.bak"

# The channel markers. Python rather than sed because two of the three
# replacements are multi-line, and one of them is the `[Unreleased]` section of
# CHANGELOG.md turned into HTML - so that the question the dev site exists to
# answer, "what does this build have that the last release did not", is on the
# page instead of one tap away on GitHub.
#
# The converter handles the subset the changelog actually uses: `### Heading`,
# `- bullet` with indented continuation lines, and inline code, bold and links.
# Anything else passes through as escaped text, which is wrong-looking rather
# than broken, and is the trade for not carrying a Markdown library here.
CHANNEL="$CHANNEL" VERSION="$VERSION" BUILT="$BUILT" python3 - "$OUT/index.html" <<'PY'
import html, os, re, sys

page = sys.argv[1]
channel, version, built = os.environ["CHANNEL"], os.environ["VERSION"], os.environ["BUILT"]
# A token that changes whenever the site does, safe to put in a query string.
build_id = re.sub(r"[^A-Za-z0-9._-]+", "-", f"{version}-{built}").strip("-")


def inline(text):
    out = html.escape(text)
    out = re.sub(r"`([^`]+)`", r"<code>\1</code>", out)
    out = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", out)
    out = re.sub(r"\*([^*]+)\*", r"<em>\1</em>", out)  # bold is already gone
    return re.sub(r"\[([^\]]+)\]\((https?://[^)]+)\)", r'<a href="\2">\1</a>', out)


def unreleased_html():
    """The `## [Unreleased]` section of the changelog, as HTML."""
    lines, inside = [], False
    for line in open("CHANGELOG.md", encoding="utf-8"):
        if line.startswith("## [Unreleased]"):
            inside = True
            continue
        if inside and line.startswith("## "):
            break
        if inside:
            lines.append(line.rstrip())

    out, items, item = [], [], None

    def flush():
        # A list closes on the first line that is not part of one.
        nonlocal item
        if item is not None:
            items.append(item)
            item = None
        if items:
            out.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
            items.clear()

    for line in lines:
        if line.startswith("### "):
            flush()
            out.append(f"<h3>{inline(line[4:])}</h3>")
        elif line.startswith("- "):
            if item is not None:
                items.append(item)
            item = line[2:]
        elif line.strip() and item is not None:
            item += " " + line.strip()   # an indented continuation of the bullet
        elif not line.strip():
            flush()
    flush()
    return "\n".join(out)


if channel == "dev":
    body = unreleased_html()
    fill = {
        "<!--NOINDEX-->": '<meta name="robots" content="noindex, nofollow">',
        "<!--DEVBAR-->": (
            '<div class="devbar"><b>DEV</b> '
            f"Unreleased build of <code>main</code> — <code>{html.escape(version)}</code>, "
            f"built {built}. "
            '<a href="../">The released site is here.</a></div>'
        ),
        "<!--UNRELEASED-->": (
            "  <h2>What this build has that the release does not</h2>\n"
            "  <p>Straight from the changelog's unreleased section.</p>\n"
            f'  <div class="unreleased">\n{body}\n  </div>'
            if body
            else ""
        ),
        "<!--VERSION-->": f"<code>{html.escape(version)}</code>, built {built}",
        "<!--BUILD-->": build_id,
    }
else:
    # A release is named by its tag and nothing else; there is no unreleased
    # work on it by definition, and it wants to be indexed.
    fill = {
        "<!--NOINDEX-->": "",
        "<!--DEVBAR-->": "",
        "<!--UNRELEASED-->": "",
        "<!--VERSION-->": f"<code>{html.escape(version)}</code>",
        "<!--BUILD-->": build_id,
    }

text = open(page, encoding="utf-8").read()
for marker, replacement in fill.items():
    text = text.replace(marker, replacement)
open(page, "w", encoding="utf-8").write(text)
PY

echo "==> checking links"
# Images count: a missing wordmark or social card is as broken as a dead link,
# and the one that fails silently - the link preview - is the one nobody sees.
# `url("...")` counts for the same reason: the hero art and the wordmark are
# stylesheet backgrounds, so an attribute-only sweep would never look at them.
missing=0
refs=$(
  grep -oE '(href|src|srcset)="[^"]*"' "$OUT/index.html" | sed 's/^[a-z]*="//;s/"$//'
  grep -oE 'url\("[^"]*"\)' "$OUT/index.html" | sed 's/^url("//;s/")$//'
)
for ref in $refs; do
  case "$ref" in
    *://* | '#'* | data:*) continue ;;
  esac
  [ -e "$OUT/${ref%/}" ] || [ -e "$OUT/${ref}index.html" ] || { echo "  ✗ dead link: $ref"; missing=1; }
done
[ "$missing" = 0 ] || { echo "error: the landing page links to files the site does not contain"; exit 1; }

echo "✓ site in $OUT/ ($(du -sh "$OUT" | cut -f1))"
echo "  preview: python3 -m http.server -d $OUT 8080"
