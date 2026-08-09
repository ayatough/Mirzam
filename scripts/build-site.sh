#!/usr/bin/env bash
# Build the documentation site: a landing page plus every sample deck rendered
# as a live, self-contained HTML file.
#   ./scripts/build-site.sh [out_dir]
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="${1:-site}"
DECKS=(pitch showcase cookbook seminar media motion)

rm -rf "$OUT"
mkdir -p "$OUT/decks"

# A deck published under /decks/<name>/ cannot resolve the links its source
# wrote relative to its own directory, so each build is told where that
# directory lives on GitHub.
REPO_BLOB="https://github.com/ayatough/Mirzam/blob/main"

echo "==> building decks"
cargo build --release --bin mirzam
for deck in "${DECKS[@]}"; do
  ./target/release/mirzam build "examples/$deck.md" -o "$OUT/decks/$deck" \
    --base-url "$REPO_BLOB/examples/"
done
# The README with no Mirzam syntax at all, split at its own headings.
./target/release/mirzam build README.md -o "$OUT/decks/readme" --split h2 \
  --base-url "$REPO_BLOB/"

# The browser editor: the same Rust core compiled to WebAssembly, so someone
# with no toolchain - or no laptop - can still write a deck and download it.
# Skipped when the WASM toolchain is not available, and the landing page then
# simply does not offer the card, so a site built without it has no dead link.
TRY_CARD=""
echo "==> building the browser editor"
if [ "${MIRZAM_SKIP_WASM:-}" != "1" ] && ./scripts/build-wasm.sh web/wasm-demo/pkg; then
  mkdir -p "$OUT/try"
  cp -R web/wasm-demo/. "$OUT/try/"
  cp docs/brand/mirzam-icon-light.svg "$OUT/try/favicon.svg"
  TRY_CARD='<a class="card" href="try/"><b>Write one in the browser</b><span>The Rust core as WebAssembly: no install, and it runs on a phone</span></a>'
else
  echo "  (skipped; the landing page will not link to it)"
fi

# The brand assets the landing page and the link preview are built from. Copied
# rather than inlined so the social card has a stable absolute URL - a scraper
# fetches that image separately, and cannot follow a data: URI.
echo "==> copying brand assets"
mkdir -p "$OUT/brand"
cp docs/brand/*.svg docs/brand/*.png docs/brand/*.webp "$OUT/brand/"

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

<link rel="icon" href="brand/mirzam-icon-light.svg">
<link rel="apple-touch-icon" href="brand/mirzam-icon-512.png">

<meta property="og:type" content="website">
<meta property="og:url" content="https://ayatough.github.io/Mirzam/">
<meta property="og:title" content="Mirzam - presentation decks that live in your repository">
<meta property="og:description" content="Write plain Markdown, draw the layout as ASCII, and get a deck with real charts, diagrams, video and math - as one self-contained HTML file or a PDF.">
<meta property="og:image" content="https://ayatough.github.io/Mirzam/brand/mirzam-social-card.png">
<meta name="twitter:card" content="summary_large_image">

<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300;400;500&family=Inter:wght@300;400&family=IBM+Plex+Mono:wght@400&display=swap">
<style>
  /* Mirzam Light and Mirzam Dark, straight from docs/brand/mirzam-theme.css. The
     page has no theme switch: it follows the reader's, and both are drawn. */
  :root {
    --bg:#F7F8FC; --surface:#EEF0F7; --fg:#17203A; --muted:#68708A; --line:#D9DDEB;
    --accent:#6557D9; --accent-2:#8B7CFF; --cyan:#4F8CC9;
    /* A button's label has to clear AA on the violet it sits on, and white
       does not clear it on the light violet - so the ink flips with the theme
       and the hover shade moves away from the label, not towards it. */
    --btn-bg:#6557D9; --btn-bg-hover:#5347B8; --btn-ink:#FFFFFF;
    --hero: url("brand/mirzam-hero-light.webp");
    --code-bg:#EEF0F7; --shadow: 0 1px 2px rgba(23,32,58,.06), 0 8px 24px rgba(23,32,58,.06);
    --lead-weight: 300;
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg:#080C18; --surface:#0E1425; --fg:#F4F6FF; --muted:#8F9AB8; --line:#252E47;
      --accent:#9B8CFF; --accent-2:#C0B7FF; --cyan:#72B5E8;
      --btn-bg:#9B8CFF; --btn-bg-hover:#C0B7FF; --btn-ink:#080C18;
      --hero: url("brand/mirzam-hero-dark.webp");
      --code-bg:#11192D; --shadow: none;
      /* Light type on a dark ground reads a step thinner than it measures. */
      --lead-weight: 400;
    }
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
  .hero::before {
    content: ""; position: absolute; inset: 0;
    background: var(--hero) 20% 100% / cover no-repeat;
  }
  .hero::after {
    content: ""; position: absolute; inset: 0;
    background: linear-gradient(180deg, var(--bg) 0%, transparent 34%, transparent 62%, var(--bg) 100%);
  }
  .hero .wrap { position: relative; padding-top: 88px; padding-bottom: 104px; }
  .hero img { display: block; width: min(340px, 78vw); height: auto; }
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
  @media (max-width: 620px) {
    .hero .wrap { padding-top: 56px; padding-bottom: 72px; }
    h2 { font-size: 1.45rem; }
  }
</style>
</head>
<body>

<header class="hero">
  <div class="wrap">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="brand/mirzam-wordmark-dark.svg">
      <img src="brand/mirzam-wordmark-light.svg" alt="Mirzam" width="340" height="135">
    </picture>
    <p class="tag">Presentation decks that live in your repository. Plain Markdown in,
    charts, diagrams, video and math out — as one HTML file or a PDF.</p>
    <p class="cta">
      <a class="btn" href="decks/pitch/">See a deck running</a>
      <a class="btn ghost" href="https://github.com/ayatough/Mirzam">Source on GitHub</a>
    </p>
  </div>
</header>

<main class="wrap">
  <h2>See it running</h2>
  <p>Each deck below was built by Mirzam from the Markdown in <code>examples/</code>.
  <span class="kbd">←</span> <span class="kbd">→</span> to navigate,
  <span class="kbd">N</span> for speaker notes, <span class="kbd">/</span> for the rest.</p>
  <div class="cards">
    <a class="card" href="decks/pitch/"><b>Pitch deck</b><span>Metric tiles, charts from CSV, a dark theme</span></a>
    <a class="card" href="decks/showcase/"><b>Component gallery</b><span>Every feature beside its source</span></a>
    <a class="card" href="decks/cookbook/"><b>Layout cookbook</b><span>One layout rule per slide</span></a>
    <a class="card" href="decks/seminar/"><b>Research talk</b><span>Math, tables, Japanese typography</span></a>
    <a class="card" href="decks/motion/"><b>Motion</b><span>Entrances, click-through builds, page turns</span></a>
    <a class="card" href="decks/media/"><b>Media</b><span>Video and GIF embedding</span></a>
    <a class="card" href="decks/readme/"><b>This README, as a deck</b><span>No Mirzam syntax: <code>--split h2</code> on an ordinary document</span></a>
  </div>

  <h2>How it works</h2>
  <p>Four stages, all at build time. The charts are SVG, the math is MathML, the
  images are inlined — the deck is one file you can email.</p>
  <p><img src="brand/mirzam-concept-workflow.svg" alt="Markdown becomes an ASCII layout, then real components, then a self-contained HTML or PDF deck" style="width:100%;height:auto;border-radius:14px;margin-top:26px"></p>

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
  </footer>
</main>
</body>
</html>
HTML

# Every local link on the landing page must resolve in the artifact, since
# nothing rewrites paths after this point.
sed -i.bak "s|<!--TRY-->|$TRY_CARD|" "$OUT/index.html" && rm -f "$OUT/index.html.bak"

echo "==> checking links"
# Images count: a missing wordmark or social card is as broken as a dead link,
# and the one that fails silently - the link preview - is the one nobody sees.
missing=0
for ref in $(grep -oE '(href|src|srcset)="[^"]*"' "$OUT/index.html" | sed 's/^[a-z]*="//;s/"$//'); do
  case "$ref" in
    *://* | '#'* | data:*) continue ;;
  esac
  [ -e "$OUT/${ref%/}" ] || [ -e "$OUT/${ref}index.html" ] || { echo "  ✗ dead link: $ref"; missing=1; }
done
[ "$missing" = 0 ] || { echo "error: the landing page links to files the site does not contain"; exit 1; }

echo "✓ site in $OUT/ ($(du -sh "$OUT" | cut -f1))"
echo "  preview: python3 -m http.server -d $OUT 8080"
