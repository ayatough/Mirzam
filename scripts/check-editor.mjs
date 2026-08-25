// Opens the browser editor, types a deck with mistakes in it, and checks that
// the editor says where they are.
//
//   node scripts/check-editor.mjs
//   node scripts/check-editor.mjs --pkg pkg          # WASM built elsewhere
//
// Why a browser and not a unit test: the placement itself is Rust and is
// tested there, and the row-building is a dozen lines of DOM. What neither can
// see is the half of this that only exists in a real browser — a warning row
// is a button, pressing it moves focus off the editor, and a caret set on a
// textarea that no longer has focus is one the browser puts back at the end
// the moment the click finishes. That bug was written, and it looked perfect
// in every other kind of test: the handler ran, the offset was right, and a
// person clicking the row watched nothing happen.
//
// So this presses the row the way a person does, and asks where the caret
// ended up. `playwright-core` is not a repository dependency; it is installed
// for the job, like the layout check's.

import { chromium } from "playwright-core";
import { createServer } from "http";
import { existsSync, readFileSync } from "fs";
import { dirname, extname, join } from "path";
import { fileURLToPath } from "url";

const REPO = dirname(dirname(fileURLToPath(import.meta.url)));
const PAGE = join(REPO, "web/wasm-demo");
const argv = process.argv.slice(2);
const at = (name, fallback) => {
  const i = argv.indexOf(name);
  return i === -1 ? fallback : argv[i + 1];
};
// The WASM package lives beside the page in a working tree and in `pkg/` on a
// machine that just built it; either is served at `/pkg/`.
const PKG = join(REPO, at("--pkg", "web/wasm-demo/pkg"));

if (!existsSync(join(PKG, "mirzam_wasm.js"))) {
  console.error(`✗ no WASM package at ${PKG} - run ./scripts/build-wasm.sh first`);
  process.exit(1);
}

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

const server = createServer((request, response) => {
  const url = decodeURIComponent(request.url.split("?")[0]);
  const file = url.startsWith("/pkg/")
    ? join(PKG, url.slice("/pkg/".length))
    : join(PAGE, url === "/" ? "index.html" : url);
  if (!existsSync(file) || !file.startsWith(file.includes("/pkg/") ? PKG : PAGE)) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.writeHead(200, { "content-type": TYPES[extname(file)] || "application/octet-stream" });
  response.end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, ok));
const origin = `http://localhost:${server.address().port}/`;

// A deck whose mistakes are all of the invisible kind: it builds, it previews,
// and two things are quietly not what was written.
const BROKEN = `---
title: A deck with mistakes
theme: nosuchtheme
---

\`\`\`pane
+--------+
|        |
| text   |
|        |
+--------+
\`\`\`

::: pane figure
This pane is not in the grid.
:::
`;

const problems = [];
const check = (ok, said) => {
  if (!ok) problems.push(said);
};

const browser = await chromium.launch({ executablePath: process.env.MIRZAM_CHROMIUM });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
page.on("pageerror", (e) => problems.push(`the page threw: ${e.message}`));
await page.goto(origin);
await page.waitForSelector("#src", { timeout: 30000 });

await page.fill("#src", BROKEN);
await page.waitForFunction(() => document.querySelectorAll("#warnings button").length > 0, null, {
  timeout: 20000,
});

const rows = await page.$$eval("#warnings button", (found) =>
  found.map((row) => ({
    at: row.querySelector(".at")?.textContent ?? "",
    kind: row.querySelector(".kind")?.textContent ?? "",
  }))
);
check(rows.length === 2, `expected two warnings, got ${rows.length}`);
check(
  rows[0]?.at === "L3" && rows[0]?.kind === "build.theme",
  `the theme warning should be on line 3: ${JSON.stringify(rows[0])}`
);
check(
  rows[1]?.at === "L14" && rows[1]?.kind === "build.layout",
  `the pane warning should be on line 14: ${JSON.stringify(rows[1])}`
);

// Pressed the way a person presses it, mouse and all.
await page.click("#warnings button >> nth=1");
const caret = await page.$eval("#src", (el) => {
  const upto = el.value.slice(0, el.selectionStart);
  return {
    line: upto.split("\n").length,
    word: el.value.slice(el.selectionStart, el.selectionStart + 6),
  };
});
check(
  caret.line === 14 && caret.word === "figure",
  `clicking the row should put the caret on \`figure\` on line 14, not ${JSON.stringify(caret)}`
);

// And a deck with nothing wrong says nothing at all, which is the half that
// stops this becoming a strip nobody reads.
await page.fill("#src", "---\ntitle: Fine\n---\n\n# One\n\nProse.\n");
await page.waitForTimeout(600);
const left = await page.$$eval("#warnings *", (found) => found.length);
check(left === 0, `a clean deck still showed ${left} warning element(s)`);

await browser.close();
server.close();

if (problems.length) {
  for (const said of problems) console.error(`  ✗ ${said}`);
  console.error(`✗ the browser editor did not report its warnings as expected`);
  process.exit(1);
}
console.log("✓ the browser editor places its warnings, and a click goes there");
