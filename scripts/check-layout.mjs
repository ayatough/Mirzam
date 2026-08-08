// Layout checker: renders built decks in a headless browser and reports slides
// whose content does not fit, because a clipped heading or an overflowing pane
// is invisible to HTML-level snapshot tests.
//
//   node scripts/check-layout.mjs out/index.html [more.html ...]
//   node scripts/check-layout.mjs --build examples/pitch.md examples/showcase.md
//
// Exits non-zero when any deck has a violation, so CI can gate on it.

import { chromium } from "playwright-core";
import { execFileSync } from "child_process";
import { existsSync, mkdtempSync } from "fs";
import { tmpdir } from "os";
import { join, resolve, basename } from "path";

const CHROMIUM = process.env.MIRZAM_CHROMIUM || undefined;
const TOLERANCE = 2; // px of sub-pixel slack before calling it an overflow

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

/** Collects layout problems for every slide of one deck. */
async function checkDeck(page, file) {
  await page.goto("file://" + resolve(file));
  await page.waitForTimeout(400);
  const count = await page.$$eval("section.slide", (s) => s.length);

  const problems = [];
  for (let i = 0; i < count; i++) {
    await page.evaluate((n) => window.__mirzamGoto && window.__mirzamGoto(n), i);
    await page.waitForTimeout(60);
    const found = await page.evaluate(
      ([index, tol]) => {
        const sec = document.querySelector(`section.slide[data-index="${index}"]`);
        if (!sec) return [];
        const issues = [];
        const panes = [...sec.querySelectorAll(".pane")];

        for (const pane of panes) {
          const name = (pane.className.match(/pane-([\w-]+)/) || [])[1] || "?";
          // Content taller or wider than the pane is clipped by overflow:hidden,
          // which is exactly the "the heading disappeared" failure.
          if (pane.scrollHeight - pane.clientHeight > tol) {
            issues.push({
              kind: "clipped",
              pane: name,
              detail: `content is ${pane.scrollHeight - pane.clientHeight}px taller than the pane`,
            });
          }
          if (pane.scrollWidth - pane.clientWidth > tol) {
            issues.push({
              kind: "clipped",
              pane: name,
              detail: `content is ${pane.scrollWidth - pane.clientWidth}px wider than the pane`,
            });
          }
          // Panes allowed to overflow (headings) must not run into a neighbour.
          if (getComputedStyle(pane).overflow === "visible") {
            const r = pane.getBoundingClientRect();
            const bottom = [...pane.children].reduce(
              (m, c) => Math.max(m, c.getBoundingClientRect().bottom),
              r.top
            );
            for (const other of panes) {
              if (other === pane) continue;
              const o = other.getBoundingClientRect();
              const overlapX = Math.min(r.right, o.right) - Math.max(r.left, o.left);
              if (overlapX > 0 && bottom - o.top > tol && r.top < o.top) {
                issues.push({
                  kind: "overlap",
                  pane: name,
                  detail: `overflows ${Math.round(bottom - o.top)}px into pane below`,
                });
                break;
              }
            }
          }
        }

        // A connector whose endpoint could not be resolved is silently dropped;
        // report the count so a typo in an id does not go unnoticed.
        if (sec.dataset.connectors) {
          const declared = JSON.parse(sec.dataset.connectors);
          const drawn = sec.querySelectorAll("svg.mz-connect path").length;
          if (drawn < declared.length) {
            issues.push({
              kind: "connector",
              pane: "-",
              detail: `${declared.length - drawn} connector(s) not drawn (unknown id?)`,
            });
          }
        }
        return issues;
      },
      [i, TOLERANCE]
    );
    for (const f of found) problems.push({ slide: i + 1, ...f });
  }
  return { count, problems };
}

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
