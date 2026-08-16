// The browser plumbing every script here shares: build a deck, open it in a
// headless Chromium, wait until it has actually settled, and either measure it
// or photograph it.
//
// This used to live only in `check-layout.mjs`, because checking was the only
// thing anybody drove a deck for. The screenshot pass and the themes gallery
// want exactly the same four steps and none of the checking, so the steps moved
// here rather than being typed a second time - a second copy of "how long to
// wait before you believe the layout" is a second answer to that question, and
// the two would diverge on the first deck that loaded slowly.
//
// Nothing in this file decides *what* to look at. It is the tab and the shutter.

import { chromium } from "playwright-core";
import { execFileSync } from "child_process";
import { mkdirSync, mkdtempSync, readFileSync } from "fs";
import { tmpdir } from "os";
import { fileURLToPath } from "url";
import { basename, dirname, join, resolve } from "path";

/** A browser is not on `PATH` in CI, so every caller honours the same variable. */
export const CHROMIUM = process.env.MIRZAM_CHROMIUM || undefined;

export const REPO_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

// The in-page checks - what counts as clipped, overlapping, an unresolved
// connector - live in one file, shared with the `mirzam check` subcommand.
// Reading it here is what keeps the Playwright route and the binary route
// running the *same* check rather than two that agree today.
export const CHECK_JS = readFileSync(
  join(REPO_ROOT, "crates", "mirzam-cli", "src", "check.js"),
  "utf8"
);

/**
 * Builds decks with the CLI into throwaway directories.
 *
 * `args` is appended to every build, which is how the gallery asks for one
 * theme and one mode at a time without knowing anything about how a deck is
 * built.
 */
export function buildDecks(sources, args = []) {
  const out = [];
  for (const src of sources) {
    const dir = mkdtempSync(join(tmpdir(), "mirzam-deck-"));
    execFileSync(
      "cargo",
      ["run", "-q", "--bin", "mirzam", "--", "build", src, "-o", dir, ...args],
      { cwd: REPO_ROOT, stdio: ["ignore", "ignore", "inherit"] }
    );
    out.push({ label: basename(src), file: join(dir, "index.html") });
  }
  return out;
}

/** One deck, one directory. The common case, spelled without the array. */
export function buildDeck(src, args = []) {
  return buildDecks([src], args)[0];
}

export function launch() {
  return chromium.launch({ executablePath: CHROMIUM });
}

/**
 * Opens a built deck and waits for it to be worth looking at.
 *
 * "Worth looking at" is not the `load` event: a deck's images can still be
 * decoding and its fonts still resolving after it, and both move the layout.
 * Measuring or photographing before either settles reads a slide nobody will
 * ever see - an image at its alt-text size, a CJK paragraph in a fallback
 * face - so this waits for the same two things `mzRunCheck` waits for, out of
 * the same file.
 */
export async function openDeck(page, file) {
  await page.goto("file://" + resolve(file));
  await page.evaluate(
    (src) =>
      new Function(
        `${src}\nreturn (async () => {
           await mzWaitImagesLoaded();
           if (document.fonts && document.fonts.ready) await document.fonts.ready;
         })();`
      )(),
    CHECK_JS
  );
}

/** Runs the shared layout check in the page and returns its report. */
export async function checkDeck(page, file) {
  await page.goto("file://" + resolve(file));
  // `mzRunCheck` does its own waiting - images, fonts, click steps,
  // animations - so nothing here has to.
  return page.evaluate((src) => new Function(`${src}\nreturn mzRunCheck();`)(), CHECK_JS);
}

/** How many slides the open deck has. */
export function slideCount(page) {
  return page.$$eval("section.slide", (s) => s.length);
}

/**
 * Shows slide `i` (0-based) in its resting state and returns once it is still.
 *
 * The resting state, not the arriving one: entrance animations are finished
 * rather than waited out, and the connectors are redrawn against where the
 * elements ended up. That is the state a reader without JavaScript sees, the
 * state the PDF export has, and the only one a screenshot can be *about* -
 * catching a slide mid-fade photographs the timing of this script instead.
 */
export async function showSlide(page, i, { steps = true } = {}) {
  await page.evaluate(
    ([src, index, withSteps]) =>
      new Function(
        "index",
        "withSteps",
        `${src}
         if (window.__mirzamGoto) window.__mirzamGoto(index);
         mzFinishAnimations();
         const active = document.querySelector('section.slide.active');
         if (withSteps && active) {
           const n = Math.max(
             window.MZAnim ? window.MZAnim.steps(active) : 0,
             window.MZAnnot ? window.MZAnnot.steps(active) : 0
           );
           for (let s = 0; s < n; s++) {
             dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
             mzFinishAnimations();
           }
         }
         if (window.__mirzamConnectors) window.__mirzamConnectors();`
      )(index, withSteps),
    [CHECK_JS, i, steps]
  );
  // One frame for the browser to paint what the code above set up.
  await page.waitForTimeout(120);
}

/**
 * Photographs slides of an already-open deck into `outDir`.
 *
 * The frame is the viewport rather than the `section` element, deliberately:
 * a theme paints the desk the slide rests on as well as the slide, and a
 * screenshot cropped to the sheet throws away half of what a themes gallery
 * exists to show.
 *
 * `slides` is a list of 0-based indices; omitted, it takes every slide.
 * Returns the paths written, in order.
 */
export async function shootSlides(
  page,
  outDir,
  { name = "slide", slides, steps = true, hideChrome = false } = {}
) {
  mkdirSync(outDir, { recursive: true });
  // The viewer's own cluster - the page counter and the control buttons - is
  // themed, so it belongs in a screenshot of a deck and not in a screenshot of
  // one slide standing for a theme: "1 / 1" in the corner of a specimen is a
  // fact about the specimen file, and it is the brightest thing on the slide.
  if (hideChrome) {
    await page.addStyleTag({ content: "#chrome { display: none !important; }" });
  }
  const count = await slideCount(page);
  const want = slides ?? [...Array(count).keys()];
  const written = [];
  const pad = String(count).length;
  for (const i of want) {
    if (i < 0 || i >= count) throw new Error(`no slide ${i + 1}: the deck has ${count}`);
    await showSlide(page, i, { steps });
    const file =
      want.length === 1 && slides
        ? join(outDir, `${name}.png`)
        : join(outDir, `${name}-${String(i + 1).padStart(pad, "0")}.png`);
    await page.screenshot({ path: file });
    written.push(file);
  }
  return written;
}
