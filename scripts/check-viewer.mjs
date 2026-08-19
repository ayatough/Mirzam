// Viewer behaviour, checked in a browser, because none of it is reachable from
// a Rust test: pressing a key, the grid `O` builds out of every slide, and
// whether a thumbnail is laid out like the slide it is a picture of.
//
//   node scripts/check-viewer.mjs --build examples/04-components.md [more.md ...]
//   node scripts/check-viewer.mjs out/index.html [more.html ...]
//
// Exits non-zero when a deck fails, so CI can gate on it.
//
// It exists because two bugs got past everything else. `O` did nothing at all
// on the component gallery: a cell in the grid is a `<button>`, that deck had a
// slide with a button of its own inside it, and the parser ends the enclosing
// button when it meets a nested one - so the caption span landed outside the
// item it captioned and building the grid threw. And on every deck the title
// slide was centred and huge on screen but top-left and ordinary in the grid,
// because the rules that give it that look name `section.slide`, which a
// thumbnail deliberately is not.
//
// Both are invisible to the HTML snapshots and to the layout check: the markup
// was right and every slide fitted. Only a browser pressing the key can see
// them.
//
// Building and opening decks is shared with the layout check and the screenshot
// pass - see `scripts/lib/deck-browser.mjs`.

import { existsSync } from "fs";
import { basename } from "path";
import { buildDecks, launch, openDeck, slideCount } from "./lib/deck-browser.mjs";

const args = process.argv.slice(2);
const decks =
  args[0] === "--build"
    ? buildDecks(args.slice(1))
    : args.map((f) => ({ label: basename(f), file: f }));

if (decks.length === 0) {
  console.error("usage: node scripts/check-viewer.mjs [--build] <deck...>");
  process.exit(2);
}
for (const d of decks) {
  if (!existsSync(d.file)) {
    console.error(`error: ${d.file} not found`);
    process.exit(2);
  }
}

/**
 * What the grid has to be, once `O` has opened it.
 *
 * Runs in the page, so it can ask for computed styles - which is the only way
 * to state the second half: a thumbnail of a title slide has to be centred the
 * way the title slide is, and "centred" is a resolved value rather than
 * anything present in the markup.
 */
function inspect() {
  const ov = document.getElementById("overview");
  if (!ov) return { problems: ["the deck has no overview at all"] };
  const problems = [];
  if (ov.hidden) problems.push("`O` did not open the grid");

  const items = [...ov.querySelectorAll(".mz-ov-item")];
  const live = [...document.querySelectorAll("#deck section.slide")];
  if (items.length !== live.length) {
    problems.push(`${live.length} slides, ${items.length} thumbnails`);
  }
  // A caption outside the item it captions is the shape the nested-button bug
  // left behind, and it is worth naming on its own: the grid can look right
  // and still have been taken apart.
  for (const cap of ov.querySelectorAll(".mz-ov-cap")) {
    if (!cap.closest(".mz-ov-item")) {
      problems.push("a caption ended up outside the thumbnail it captions");
      break;
    }
  }

  // A thumbnail is a picture, and every part of it belongs to the cell it is
  // in: a frame that still takes the mouse is a hole in the contents page,
  // which is what a slide with a widget on it left behind.
  for (const f of ov.querySelectorAll(".mz-ov-item iframe")) {
    if (getComputedStyle(f).pointerEvents !== "none") {
      problems.push("a frame in a thumbnail takes the click meant for the thumbnail");
      break;
    }
  }

  // The look of a slide, on the slide and on its picture. Only the title slide
  // has ever differed, but the comparison is written against any slide that
  // carries a `.grid`, because the next rule to be spelled `section.slide`
  // will not announce itself either.
  const keys = ["textAlign", "justifyItems", "alignItems", "fontSize"];
  const of = (el) => {
    if (!el) return null;
    const s = getComputedStyle(el);
    return keys.map((k) => `${k}:${s[k]}`).join(" ");
  };
  for (let i = 0; i < Math.min(items.length, live.length); i++) {
    if (!live[i].querySelector(".title-slide")) continue;
    const a = of(live[i].querySelector(".grid"));
    const b = of(items[i].querySelector(".grid"));
    if (a && b && a !== b) {
      problems.push(`slide ${i + 1} is drawn one way and pictured another:\n` + `        slide: ${a}\n` + `        thumb: ${b}`);
    }
  }
  return { problems, thumbnails: items.length };
}

const browser = await launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 810 } });

// A script that throws takes the feature with it and says nothing on screen,
// which is exactly how `O` came to do nothing: collect them per deck.
let thrown = [];
page.on("pageerror", (e) => thrown.push(String(e.message || e)));

let failures = 0;
for (const deck of decks) {
  thrown = [];
  await openDeck(page, deck.file);
  const slides = await slideCount(page);
  await page.keyboard.press("o");
  // The grid is built on first use; give the page a frame to build it in.
  await page.waitForFunction(
    () => {
      const ov = document.getElementById("overview");
      return !ov || !ov.hidden || document.querySelectorAll(".mz-ov-item").length > 0;
    },
    null,
    { timeout: 5000 }
  ).catch(() => {});
  const { problems = ["the grid could not be inspected"] } = await page.evaluate(inspect);
  const all = [...thrown.map((t) => `the viewer threw: ${t}`), ...problems];
  if (all.length === 0) {
    console.log(`✓ ${deck.label}: ${slides} slides, the grid opens and pictures them`);
  } else {
    failures += all.length;
    console.log(`✗ ${deck.label}: ${all.length} problem(s)`);
    for (const p of all) console.log(`    ${p}`);
  }
}

await browser.close();
if (failures) {
  console.log(`\n${failures} problem(s) in the viewer. See scripts/check-viewer.mjs for what each one means.`);
  process.exit(1);
}
