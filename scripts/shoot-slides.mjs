// Photographs a deck, one PNG per slide.
//
//   node scripts/shoot-slides.mjs --build examples/pitch.md -o media/pitch
//   node scripts/shoot-slides.mjs out/index.html -o shots --slide 1 --mode dark
//   node scripts/shoot-slides.mjs --build deck.md -o shots --theme wuwei --name wuwei
//
// Why a script and not a screenshot key: a slide photographed by hand is a
// slide photographed once, at whatever window size that laptop had, and it
// stops being true the next time the theme moves. This takes the deck as it is
// today, at a size the caller names, in the resting state a reader ends on -
// so the pictures in the docs are regenerated rather than curated.
//
// Everything about driving the browser is shared with the layout checker; see
// `scripts/lib/deck-browser.mjs`.

import { existsSync } from "fs";
import { basename, extname } from "path";
import { buildDeck, launch, openDeck, shootSlides, slideCount } from "./lib/deck-browser.mjs";

const argv = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = argv.indexOf(name);
  return i === -1 ? fallback : argv[i + 1];
};
// `--theme` cascades in the CLI, so it has to survive being repeated here too.
const all = (name) =>
  argv.reduce((acc, a, i) => (argv[i - 1] === name ? [...acc, a] : acc), []);
const flag = (name) => argv.includes(name);

const buildFrom = flag("--build") ? opt("--build") : null;
const takesValue = new Set(["-o", "--build", "--width", "--height", "--mode", "--theme", "--slide", "--name", "--split", "--fit"]);
const positional = argv.filter((a, i) => !a.startsWith("--") && !takesValue.has(argv[i - 1]));
const out = opt("-o", "media/slides");
const width = +opt("--width", 1440);
const height = +opt("--height", 810);
const mode = opt("--mode", null);
const themes = all("--theme");
const name = opt("--name", null);
const noSteps = flag("--no-steps");
// The viewer's page counter and control cluster are in the frame by default,
// because they are part of the deck as anybody opening it sees it. A picture
// standing in for something else - one slide as a theme specimen - wants them
// out of the way.
const noChrome = flag("--no-chrome");
// 1-based on the command line, because that is how a slide is numbered
// everywhere else: in the viewer, in the checker's output, in a bug report.
const only = opt("--slide", null);

if (!buildFrom && positional.length === 0) {
  console.error(
    "usage: node scripts/shoot-slides.mjs [--build <deck.md> | <built.html>] -o <dir>\n" +
      "       [--slide 1,3] [--width 1440] [--height 810] [--mode light|dark]\n" +
      "       [--theme <name|file.css>]... [--split h1|h2|h3] [--fit shrink]\n" +
      "       [--name <prefix>] [--no-steps] [--no-chrome]"
  );
  process.exit(2);
}

let deck;
if (buildFrom) {
  const args = [];
  for (const t of themes) args.push("--theme", t);
  if (mode) args.push("--mode", mode);
  if (opt("--split", null)) args.push("--split", opt("--split"));
  if (opt("--fit", null)) args.push("--fit", opt("--fit"));
  deck = buildDeck(buildFrom, args);
} else {
  deck = { file: positional[0] };
}
if (!existsSync(deck.file)) {
  console.error(`error: ${deck.file} not found`);
  process.exit(2);
}

const prefix =
  name || basename(buildFrom || deck.file, extname(buildFrom || deck.file)).replace(/^index$/, "slide");

const browser = await launch();
// A deck pinned with `--mode` still asks the browser what it prefers for the
// per-mode images inside it, so the tab is told the same thing the deck was.
const page = await browser.newPage({
  viewport: { width, height },
  colorScheme: mode === "dark" ? "dark" : "light",
  reducedMotion: "no-preference",
});

await openDeck(page, deck.file);
const count = await slideCount(page);
const slides = only
  ? only.split(",").map((s) => {
      const n = +s.trim();
      if (!Number.isInteger(n) || n < 1) {
        console.error(`error: --slide takes slide numbers from 1, not "${s.trim()}"`);
        process.exit(2);
      }
      return n - 1;
    })
  : undefined;

const written = await shootSlides(page, out, {
  name: prefix,
  slides,
  steps: !noSteps,
  hideChrome: noChrome,
});
await browser.close();

for (const f of written) console.log(`✓ ${f}`);
console.log(`${written.length} of ${count} slide(s) → ${out}`);
