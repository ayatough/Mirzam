// Builds the themes gallery: one specimen slide, rendered in every theme and
// both modes, photographed, and written out as a page.
//
//   node scripts/make-theme-gallery.mjs -o site/themes
//
// Generated rather than curated, and that is the whole point. A gallery
// assembled by hand is a claim about what the stylesheets looked like the
// afternoon somebody assembled it; the next token that moves makes the page
// quietly untrue, and nothing fails. This builds `scripts/gallery/specimen.md`
// once per theme with `--theme` and `--mode`, so the page cannot say anything
// the CSS does not currently do.
//
// It also *checks* each rendering before photographing it, with the same check
// `mirzam check` runs. A theme is a type identity now, not a palette - a face
// a size too large clips a heading in one theme and in no other - so the
// gallery is the one place where all twelve renderings are laid out, and it
// would be a waste to look at all twelve and measure none of them.
//
// `examples/themes/blueprint.css` is in the list beside the built-ins on
// purpose. The theme contract says a stylesheet of your own is a first-class
// identity; a gallery that showed the five built-ins and mentioned the sixth
// in a footnote would be making a weaker claim than the contract does.

import { existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "fs";
import { join, resolve } from "path";
import { buildDeck, checkDeck, launch, openDeck, shootSlides, REPO_ROOT } from "./lib/deck-browser.mjs";

const argv = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = argv.indexOf(name);
  return i === -1 ? fallback : argv[i + 1];
};

const out = resolve(opt("-o", "site/themes"));
const width = +opt("--width", 1280);
const height = +opt("--height", 720);
const specimen = opt("--deck", join(REPO_ROOT, "scripts", "gallery", "specimen.md"));

// The order is the order the page reads in: ours first because it is the
// fallback, the four borrowed identities next, and the file theme last -
// last because it is the answer to "and if none of these is yours", not
// because it is an afterthought.
const THEMES = [
  {
    id: "mirzam",
    arg: "mirzam",
    name: "mirzam",
    kind: "built-in",
    note: "The default, and what an unnamed deck gets. A light grotesque, a violet accent, and a short rule under a section heading.",
  },
  {
    id: "nord",
    arg: "nord",
    name: "nord",
    kind: "built-in",
    note: "The Nord palette: cool blue-greys and a muted frost accent, in the same grotesque voice.",
  },
  {
    id: "solarized",
    arg: "solarized",
    name: "solarized",
    kind: "built-in",
    note: "Solarized, whose two modes are one palette measured from opposite ends — the accents do not move between them.",
  },
  {
    id: "vscode",
    arg: "vscode",
    name: "vscode",
    kind: "built-in",
    note: "VS Code Light+ and Dark+, so a code-heavy deck is set in the colours its reader already reads code in.",
  },
  {
    id: "wuwei",
    arg: "wuwei",
    name: "wuwei",
    kind: "built-in",
    note: "無為: warm greys and a roman serif. Paper and ink, with no accent asking for attention — the clearest proof that a theme is a voice and not a colour scheme.",
  },
  {
    id: "blueprint",
    arg: "examples/themes/blueprint.css",
    name: "themes/blueprint.css",
    kind: "your own",
    note: "The sample theme in a file: a drawing office, lettered throughout in a mono hand, with square cards and an em dash for a bullet. Nothing in the renderer knows it exists.",
  },
];

const MODES = ["light", "dark"];

const imgDir = join(out, "img");
rmSync(out, { recursive: true, force: true });
mkdirSync(imgDir, { recursive: true });

if (!existsSync(specimen)) {
  console.error(`error: ${specimen} not found`);
  process.exit(2);
}

const browser = await launch();
let problems = 0;
let bytes = 0;

for (const theme of THEMES) {
  for (const mode of MODES) {
    const deck = buildDeck(specimen, ["--theme", theme.arg, "--mode", mode]);

    // Measured before it is photographed: a clipped heading is exactly the
    // failure a themes gallery is likeliest to introduce and least likely to
    // be noticed in, since nobody knows what the slide was supposed to look
    // like in a theme they have never seen.
    const page = await browser.newPage({
      viewport: { width: 1440, height: 810 },
      colorScheme: mode,
    });
    const report = await checkDeck(page, deck.file);
    for (const p of report.problems) {
      problems++;
      console.log(`✗ ${theme.name} / ${mode}: [${p.kind}] pane "${p.pane}": ${p.detail}`);
    }
    await page.close();

    const shot = await browser.newPage({
      viewport: { width, height },
      colorScheme: mode,
      reducedMotion: "no-preference",
    });
    await openDeck(shot, deck.file);
    const [file] = await shootSlides(shot, imgDir, {
      name: `${theme.id}-${mode}`,
      slides: [0],
      hideChrome: true,
    });
    await shot.close();

    bytes += statSync(file).size;
    if (report.problems.length === 0) console.log(`✓ ${theme.name} / ${mode}`);
  }
}

await browser.close();

// ---- the page --------------------------------------------------------------

const esc = (s) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

// The two modes sit side by side rather than behind a toggle. A theme's claim
// is that it is an identity in *both*, and a toggle lets a reader check one and
// assume the other - which is the assumption `blueprint.css` was written to
// break, its light mode having once been indistinguishable from any other.
const cards = THEMES.map((t) => {
  const shots = MODES.map(
    (m) => `      <figure>
        <img src="img/${t.id}-${m}.png" width="${width}" height="${height}"
             loading="lazy" alt="The specimen slide in the ${esc(t.name)} theme, ${m} mode">
        <figcaption>${m}</figcaption>
      </figure>`
  ).join("\n");
  const how =
    t.kind === "built-in"
      ? `theme: ${t.id}`
      : `theme: ${t.name}`;
  return `  <section class="theme" id="${t.id}">
    <h2>${esc(t.name)} <span class="tag">${esc(t.kind)}</span></h2>
    <p>${esc(t.note)}</p>
    <pre><code>---
${esc(how)}
---</code></pre>
    <div class="shots">
${shots}
    </div>
  </section>`;
}).join("\n\n");

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Mirzam themes</title>
<meta name="description" content="Every Mirzam theme on the same slide, in both modes: five built-ins and a theme of your own in a file.">
<link rel="icon" href="../brand/mirzam-icon-light.svg">
<style>
  :root {
    --bg: #080C18; --surface: #0E1425; --raised: #161E33; --fg: #F4F6FF;
    --muted: #8F9AB8; --line: #252E47; --accent: #9B8CFF;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0; background: var(--bg); color: var(--fg);
    font-family: Inter, system-ui, 'Hiragino Sans', 'Noto Sans CJK JP', sans-serif;
    line-height: 1.65;
  }
  .wrap { max-width: 1180px; margin: 0 auto; padding: 48px 20px 96px; }
  a { color: var(--accent); }
  h1 { font-size: clamp(28px, 5vw, 42px); margin: 0 0 12px; letter-spacing: -0.02em; }
  .lede { color: var(--muted); max-width: 78ch; margin: 0 0 10px; }
  .toc { display: flex; flex-wrap: wrap; gap: 10px; margin: 24px 0 8px; padding: 0; list-style: none; }
  .toc a {
    display: inline-block; padding: 5px 12px; border-radius: 999px; font-size: 14px;
    background: var(--raised); border: 1px solid var(--line); text-decoration: none;
  }
  .toc a:hover { border-color: var(--accent); }
  .theme { margin-top: 56px; border-top: 1px solid var(--line); padding-top: 28px; }
  .theme h2 { font-size: 22px; margin: 0 0 6px; font-family: 'IBM Plex Mono', ui-monospace, monospace; }
  .tag {
    font-family: Inter, system-ui, sans-serif; font-size: 12px; font-weight: 500;
    color: var(--muted); border: 1px solid var(--line); border-radius: 999px;
    padding: 2px 9px; margin-left: 8px; vertical-align: 2px; letter-spacing: .04em;
  }
  .theme p { color: var(--muted); max-width: 74ch; margin: 0 0 14px; }
  pre {
    background: var(--surface); border: 1px solid var(--line); border-radius: 8px;
    padding: 10px 14px; overflow-x: auto; margin: 0 0 18px;
  }
  code { font-family: 'IBM Plex Mono', ui-monospace, Consolas, monospace; font-size: 13px; }
  .shots { display: grid; grid-template-columns: repeat(auto-fit, minmax(330px, 1fr)); gap: 16px; }
  figure { margin: 0; }
  figure img {
    width: 100%; height: auto; display: block; border-radius: 8px;
    border: 1px solid var(--line); background: var(--surface);
  }
  figcaption {
    font-size: 12px; color: var(--muted); margin-top: 6px;
    text-transform: uppercase; letter-spacing: .12em;
  }
  footer { margin-top: 64px; color: var(--muted); font-size: 14px; }
</style>
</head>
<body>
<div class="wrap">
  <p><a href="../">← Mirzam</a></p>
  <h1>Themes</h1>
  <p class="lede">The same slide, rendered by each theme in both modes. A theme
  is a token set rather than a palette — it carries the face, the ladder of
  sizes, the bullet, the rule under a heading and the six chart series — so the
  specimen puts one of each on a single slide and lets the stylesheets answer.</p>
  <p class="lede">Every picture here is generated: <code>scripts/make-theme-gallery.mjs</code>
  builds <code>scripts/gallery/specimen.md</code> once per theme with
  <code>--theme</code> and <code>--mode</code>, checks each rendering for
  clipped content, and photographs it. Nothing on this page can drift from the
  CSS, because nothing on it was typed by hand.</p>
  <ul class="toc">
${THEMES.map((t) => `    <li><a href="#${t.id}">${esc(t.name)}</a></li>`).join("\n")}
  </ul>

${cards}

  <footer>
    <p>Pick one in a deck's frontmatter, or on the command line with
    <code>mirzam build deck.md --theme nord</code>. A theme of your own is a
    <code>.css</code> file naming the same tokens — see
    <a href="https://github.com/ayatough/Mirzam/blob/main/examples/themes/blueprint.css">blueprint.css</a>
    and <a href="https://github.com/ayatough/Mirzam/blob/main/docs/syntax.md#theming">the theming section</a>
    of the syntax reference.</p>
  </footer>
</div>
</body>
</html>
`;

writeFileSync(join(out, "index.html"), page);

const kb = (n) => `${(n / 1024).toFixed(0)} KB`;
console.log(
  `\n${THEMES.length} themes × ${MODES.length} modes → ${out} ` +
    `(${THEMES.length * MODES.length} images, ${kb(bytes)})`
);
if (problems) {
  console.error(
    `\n${problems} layout problem(s) in the specimen. A theme whose type does ` +
      `not fit the specimen does not fit a real deck either — fix the theme, or ` +
      `give scripts/gallery/specimen.md the room.`
  );
  process.exit(1);
}
