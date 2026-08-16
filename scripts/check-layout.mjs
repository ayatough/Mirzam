// Layout checker: renders built decks in a headless browser and reports slides
// whose content does not fit, because a clipped heading or an overflowing pane
// is invisible to HTML-level snapshot tests.
//
//   node scripts/check-layout.mjs out/index.html [more.html ...]
//   node scripts/check-layout.mjs --build examples/pitch.md examples/04-components.md
//
// Exits non-zero when any deck has a violation, so CI can gate on it.
//
// The check itself - what counts as clipped, overlapping, an unresolved
// connector - lives in one place, `crates/mirzam-cli/src/check.js`, run in the
// page exactly as `mirzam check` runs it. This file is the CI-only,
// Playwright-driven way to reach that same check; a binary install reaches it
// through `mirzam check` instead, without Node or playwright-core. Keep the two
// in sync by editing only the shared file.
//
// Building a deck, opening it and running the check in it are shared with the
// screenshot pass and the themes gallery - see `scripts/lib/deck-browser.mjs`.

import { existsSync } from "fs";
import { basename } from "path";
import { buildDecks, checkDeck, launch } from "./lib/deck-browser.mjs";

const args = process.argv.slice(2);
const decks =
  args[0] === "--build"
    ? buildDecks(args.slice(1))
    : args.map((f) => ({ label: basename(f), file: f }));

if (decks.length === 0) {
  console.error("usage: node scripts/check-layout.mjs [--build] <deck...>");
  process.exit(2);
}
for (const d of decks) {
  if (!existsSync(d.file)) {
    console.error(`error: ${d.file} not found`);
    process.exit(2);
  }
}

const browser = await launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 810 } });

let failures = 0;
for (const deck of decks) {
  const { count, problems, notes = [] } = await checkDeck(page, deck.file);
  if (problems.length === 0) {
    console.log(`✓ ${deck.label}: ${count} slides, no layout problems`);
  } else {
    failures += problems.length;
    console.log(`✗ ${deck.label}: ${problems.length} problem(s) across ${count} slides`);
    for (const p of problems) {
      console.log(`    slide ${p.slide} [${p.kind}] pane "${p.pane}": ${p.detail}`);
    }
  }
  // What the run was measured with. A deck embeds no text font, so a clean
  // result is a statement about this machine; the notes say which machine.
  for (const n of notes) console.log(`  · ${n}`);
}

await browser.close();
if (failures) {
  console.log(
    `\n${failures} problem(s). Widen the band in the pane block, shorten the text, ` +
      `or move the content to another pane. See docs/layout.md.`
  );
  process.exit(1);
}
