// A live-reload patch, checked against the annotation overlay - a bug the
// layout check cannot see, because it only ever renders a deck once.
//
//   node scripts/check-annot-reload.mjs
//
// `mirzam serve`'s hot-reload client (`crates/mirzam-render/src/theme/mod.rs`)
// and the VS Code preview's `editors/vscode/media/preview.js` both patch a
// changed slide the same way: replace its whole `<section>` via `outerHTML`
// with a freshly rendered one, then call `window.__mirzamRefresh()`. A
// `<script>` written that way never runs - only one the browser itself
// parses does - so `annot.js` cannot mount an annotation once at load and
// keep the `sec`/`target`/`overlay` triple forever: a patch orphans it
// without ever mounting the new `script.mz-annot` the fresh markup carries.
// The symptom was a rectangle that stopped following its own coordinates, or
// vanished outright, after any edit to the slide it sat on - fixed by having
// `annot.js` re-mount from the live DOM before every redraw.
//
// Exits non-zero if a patched mark ends up stale, missing, or duplicated.

import { mkdtempSync, readFileSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { buildDeck, launch, openDeck } from "./lib/deck-browser.mjs";

// A tiny picture, inline, so the fixture has no sibling asset file: a 1x1 PNG
// is not a picture `annotate` can find a paint box for, so this is an SVG the
// core sizes rather than the smallest possible byte string.
const PICTURE =
  "data:image/svg+xml;base64," +
  Buffer.from(
    '<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300">' +
      '<rect width="400" height="300" fill="#eee"/></svg>'
  ).toString("base64");

const source = (coords) => `---
title: T
---

\`\`\`pane
+------+
| shot |
+------+
\`\`\`

::: pane shot
![a picture](${PICTURE})
:::

\`\`\`annotate
target: shot
rect ${coords} 20x20 : id=box color=@accent2
\`\`\`
`;

function build(coords) {
  const dir = mkdtempSync(join(tmpdir(), "mirzam-annot-reload-"));
  const src = join(dir, "deck.md");
  writeFileSync(src, source(coords));
  return buildDeck(src);
}

/**
 * The raw markup the core renders for the slide - no overlay `<div>`, because
 * `annot.js` only appends that once the browser mounts the script. This is
 * the same shape of HTML a slide-changed patch (`res.changes[i]` from
 * `mirzam-wasm`, or `serve`'s own diff) hands the client.
 */
function sectionHtml(file) {
  const html = readFileSync(file, "utf8");
  const start = html.indexOf('<section class="slide"');
  const end = html.indexOf("</section>", start) + "</section>".length;
  return html.slice(start, end);
}

const before = build("30,30");
const patchedSection = sectionHtml(build("60,60").file);

const browser = await launch();
const page = await browser.newPage();
await openDeck(page, before.file);

const marks = () =>
  page.$$eval("#box", (rs) => rs.map((r) => ({ x: +r.getAttribute("x"), y: +r.getAttribute("y") })));

const start = await marks();

await page.evaluate((section) => {
  const sec = document.querySelector('section.slide[data-index="0"]');
  sec.outerHTML = section;
  if (window.__mirzamRefresh) window.__mirzamRefresh();
}, patchedSection);
// The overlay redraws synchronously from `__mirzamRefresh`; nothing here is
// waiting on a frame or an async task.
const end = await marks();

await browser.close();

const problems = [];
if (start.length !== 1) problems.push(`expected one mark before the patch, found ${start.length}`);
if (end.length === 0) problems.push("the mark disappeared after a live-reload patch");
if (end.length > 1) problems.push(`a stale duplicate survived the patch (${end.length} marks)`);
if (end.length === 1 && start.length === 1 && end[0].x === start[0].x && end[0].y === start[0].y) {
  problems.push("the mark did not move to its edited coordinates; the overlay is stale");
}

if (problems.length) {
  console.log(`✗ annotation live-reload: ${problems.length} problem(s)`);
  for (const p of problems) console.log(`    ${p}`);
  process.exit(1);
}
console.log("✓ annotation live-reload: a patched mark's new position is drawn, once");
