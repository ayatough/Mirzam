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
// connector - lives in one place, `crates/mirzam-cli/src/check.js`, loaded
// below and run in the page exactly as `mirzam check` runs it. This file is
// the CI-only, Playwright-driven way to reach that same check; a binary
// install reaches it through `mirzam check` instead, without Node or
// playwright-core. Keep the two in sync by editing only the shared file.

import { chromium } from "playwright-core";
import { execFileSync } from "child_process";
import { existsSync, mkdtempSync, readFileSync } from "fs";
import { tmpdir } from "os";
import { fileURLToPath } from "url";
import { join, resolve, basename, dirname } from "path";

const CHROMIUM = process.env.MIRZAM_CHROMIUM || undefined;
const REPO_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const CHECK_JS = readFileSync(join(REPO_ROOT, "crates", "mirzam-cli", "src", "check.js"), "utf8");

function buildDecks(sources) {
  const out = [];
  for (const src of sources) {
    const dir = mkdtempSync(join(tmpdir(), "mirzam-check-"));
    execFileSync("cargo", ["run", "-q", "--bin", "mirzam", "--", "build", src, "-o", dir], {
      stdio: ["ignore", "ignore", "inherit"],
    });
    out.push({ label: basename(src), file: join(dir, "index.html") });
  }
  return out;
}

/** Collects layout problems for every slide of one deck, via the shared check. */
async function checkDeck(page, file) {
  await page.goto("file://" + resolve(file));
  // `mzRunCheck` (from CHECK_JS) does its own waiting - images, fonts, click
  // steps, animations - so nothing here has to.
  return page.evaluate((src) => new Function(`${src}\nreturn mzRunCheck();`)(), CHECK_JS);
}

const args = process.argv.slice(2);
const decks =
  args[0] === "--build" ? buildDecks(args.slice(1)) : args.map((f) => ({ label: basename(f), file: f }));

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

const browser = await chromium.launch({ executablePath: CHROMIUM });
const page = await browser.newPage({ viewport: { width: 1440, height: 810 } });

let failures = 0;
for (const deck of decks) {
  const { count, problems } = await checkDeck(page, deck.file);
  if (problems.length === 0) {
    console.log(`✓ ${deck.label}: ${count} slides, no layout problems`);
    continue;
  }
  failures += problems.length;
  console.log(`✗ ${deck.label}: ${problems.length} problem(s) across ${count} slides`);
  for (const p of problems) {
    console.log(`    slide ${p.slide} [${p.kind}] pane "${p.pane}": ${p.detail}`);
  }
}

await browser.close();
if (failures) {
  console.log(
    `\n${failures} problem(s). Widen the band in the pane block, shorten the text, ` +
      `or move the content to another pane. See docs/layout.md.`
  );
  process.exit(1);
}
