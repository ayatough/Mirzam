#!/usr/bin/env bash
# Build the documentation site: a landing page plus every sample deck rendered
# as a live, self-contained HTML file.
#   ./scripts/build-site.sh [out_dir]
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="${1:-site}"
DECKS=(pitch showcase cookbook seminar media)

rm -rf "$OUT"
mkdir -p "$OUT/decks"

echo "==> building decks"
cargo build --release --bin mirzam
for deck in "${DECKS[@]}"; do
  ./target/release/mirzam build "examples/$deck.md" -o "$OUT/decks/$deck"
done

# Markdown docs are rendered by GitHub Pages' Jekyll, so copy them as-is.
echo "==> copying docs"
cp -r docs "$OUT/docs"
cp README.md CHANGELOG.md "$OUT/"

cat > "$OUT/index.html" <<'HTML'
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Mirzam - slides that live in your repository</title>
<style>
  :root { --bg:#0d1117; --fg:#e9edf5; --muted:#8b93ad; --a1:#5b8cff; --a2:#2dd4bf; --line:#232a3a; }
  * { box-sizing: border-box; }
  body {
    margin: 0; background: var(--bg); color: var(--fg);
    font: 16px/1.65 system-ui, -apple-system, "Segoe UI", sans-serif;
  }
  .wrap { max-width: 860px; margin: 0 auto; padding: 72px 24px 96px; }
  h1 { font-size: 2.6rem; letter-spacing: -.02em; margin: 0 0 .2em; }
  .tag { color: var(--muted); font-size: 1.15rem; margin: 0 0 2em; }
  h2 { font-size: 1.25rem; margin: 2.5em 0 .8em; }
  h2::after { content:""; display:block; width:56px; height:3px; border-radius:2px; margin-top:10px;
              background: linear-gradient(90deg, var(--a1), var(--a2)); }
  a { color: var(--a1); }
  ul { padding-left: 1.2em; }
  li { margin: .4em 0; }
  .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 14px; }
  .card { display:block; padding: 18px 20px; border:1px solid var(--line); border-radius: 12px;
          background:#131a27; text-decoration:none; color:inherit; }
  .card:hover { border-color: var(--a1); }
  .card b { display:block; color:#fff; margin-bottom:.25em; }
  .card span { color: var(--muted); font-size: .92rem; }
  code { background:#1a2231; color: var(--a2); padding:.12em .38em; border-radius:4px; font-size:.9em; }
  pre { background:#0b0f16; border:1px solid var(--line); border-radius:10px; padding:14px 16px; overflow:auto; }
  footer { color: var(--muted); font-size:.9rem; margin-top:4em; border-top:1px solid var(--line); padding-top:1.4em; }
</style>
</head>
<body>
<div class="wrap">
  <h1>Mirzam</h1>
  <p class="tag">Presentation decks that live in your repository. Plain Markdown in,
  charts, diagrams, video and math out — as one HTML file or a PDF.</p>

  <h2>See it running</h2>
  <p>Each deck below was built by Mirzam from the Markdown in <code>examples/</code>.
  Use <code>←</code> <code>→</code> to navigate, <code>N</code> for speaker notes.</p>
  <div class="cards">
    <a class="card" href="decks/pitch/"><b>Pitch deck</b><span>Metric tiles, charts from CSV, a dark theme</span></a>
    <a class="card" href="decks/showcase/"><b>Component gallery</b><span>Every feature beside its source</span></a>
    <a class="card" href="decks/cookbook/"><b>Layout cookbook</b><span>One layout rule per slide</span></a>
    <a class="card" href="decks/seminar/"><b>Research talk</b><span>Math, tables, Japanese typography</span></a>
    <a class="card" href="decks/media/"><b>Media</b><span>Video and GIF embedding</span></a>
  </div>

  <h2>Read</h2>
  <ul>
    <li><a href="docs/syntax.html">Syntax reference</a> — every block and inline form</li>
    <li><a href="docs/layout.html">Layout guide</a> — sizing, spacing, keeping arrows out of the text</li>
    <li><a href="docs/architecture.html">Architecture</a> — how it is built and why</li>
    <li><a href="docs/roadmap.html">Roadmap</a> — what works today, what is next</li>
    <li><a href="docs/development.html">Development guide</a> — build, test, contribute</li>
    <li><a href="docs/ja/">日本語</a></li>
  </ul>

  <h2>Try it</h2>
<pre>git clone https://github.com/ayatough/Mirzam
cd Mirzam &amp;&amp; cargo build --release

./target/release/mirzam build examples/pitch.md -o out
./target/release/mirzam serve examples/pitch.md</pre>

  <footer>
    MIT licensed · <a href="https://github.com/ayatough/Mirzam">Source on GitHub</a>
  </footer>
</div>
</body>
</html>
HTML

# Jekyll needs front matter to render bare .md; a config is enough here.
cat > "$OUT/_config.yml" <<'YML'
theme: jekyll-theme-primer
title: Mirzam
description: Presentation decks that live in your repository
YML

echo "✓ site in $OUT/ ($(du -sh "$OUT" | cut -f1))"
echo "  preview: python3 -m http.server -d $OUT 8080"
