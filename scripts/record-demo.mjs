// Records a deck being presented, as a video or a GIF, by driving it in a
// browser rather than by anyone operating one.
//
//   node scripts/record-demo.mjs --build examples/pitch.md -o media/pitch
//   node scripts/record-demo.mjs out/index.html -o media/demo --gif
//
// Why this exists: a screen recording of a slide tool is the one piece of
// documentation that cannot be written, and it is also the piece most likely to
// come out badly — a hesitation before a keypress, a cursor crossing the slide,
// a pause of the wrong length. None of that is a recording problem. It is a
// *performing* problem, and a script does not hesitate.
//
// What it buys beyond steadiness: the run is reproducible. Change a theme,
// re-run it, and the demo is the deck as it is today rather than as it was the
// afternoon someone had time to record it.

import { chromium } from "playwright-core";
import { execFileSync } from "child_process";
import { existsSync, mkdirSync, mkdtempSync, renameSync, rmSync, readdirSync } from "fs";
import { tmpdir } from "os";
import { join, resolve, dirname, basename } from "path";

const CHROMIUM = process.env.MIRZAM_CHROMIUM || undefined;

// The webm needs no ffmpeg of ours - the browser records it. A GIF needs a
// *full* ffmpeg, and the one Playwright keeps beside its browsers is not one:
// it is built with two encoders and a dozen filters, enough to mux a screencast
// and nothing more. So this asks what a candidate can do rather than whether it
// exists, because the failure of the other check is a filter-graph error forty
// seconds into a recording that already succeeded.
function findFfmpeg() {
  const candidates = [];
  if (process.env.MIRZAM_FFMPEG) candidates.push(process.env.MIRZAM_FFMPEG);
  candidates.push("ffmpeg");
  const root = process.env.PLAYWRIGHT_BROWSERS_PATH || "/opt/pw-browsers";
  if (existsSync(root)) {
    for (const d of readdirSync(root)) {
      if (!d.startsWith("ffmpeg")) continue;
      for (const f of ["ffmpeg-linux", "ffmpeg-mac", "ffmpeg.exe"]) {
        const p = join(root, d, f);
        if (existsSync(p)) candidates.push(p);
      }
    }
  }
  for (const bin of candidates) {
    try {
      const filters = execFileSync(bin, ["-hide_banner", "-filters"], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      });
      if (["palettegen", "paletteuse", "fps", "scale"].every((f) => filters.includes(` ${f} `))) {
        return bin;
      }
    } catch {
      /* not on this machine, or not runnable */
    }
  }
  return null;
}
const FFMPEG = findFfmpeg();

// ---- arguments -------------------------------------------------------------

const argv = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = argv.indexOf(name);
  return i === -1 ? fallback : argv[i + 1];
};
const flag = (name) => argv.includes(name);

const buildFrom = flag("--build") ? argv[argv.indexOf("--build") + 1] : null;
const positional = argv.filter((a, i) => !a.startsWith("--") && argv[i - 1] !== "-o" && argv[i - 1] !== "--build" && !["-o"].includes(a));
const out = opt("-o", "media/demo");
const width = +opt("--width", 1280);
const height = +opt("--height", 720);
// Seconds a slide is held before moving on. Long enough to read a heading, not
// long enough to feel like a pause — a demo is an advertisement for the tool,
// not a talk being given.
const dwell = +opt("--dwell", 2.2) * 1000;
const stepDwell = +opt("--step", 1.1) * 1000;
const wantGif = flag("--gif");
const gifFps = +opt("--fps", 12);
const gifWidth = +opt("--gif-width", 800);
const showKeys = !flag("--no-keys");

if (!buildFrom && positional.length === 0) {
  console.error(
    "usage: node scripts/record-demo.mjs [--build <deck.md> | <built.html>] -o <out>\n" +
      "       [--gif] [--width 1280] [--height 720] [--dwell 2.2] [--step 1.1]\n" +
      "       [--fps 12] [--gif-width 800] [--no-keys]"
  );
  process.exit(2);
}

// ---- the deck --------------------------------------------------------------

let page_url;
if (buildFrom) {
  const dir = mkdtempSync(join(tmpdir(), "mirzam-demo-"));
  execFileSync("cargo", ["run", "-q", "--bin", "mirzam", "--", "build", buildFrom, "-o", dir], {
    stdio: ["ignore", "ignore", "inherit"],
  });
  page_url = join(dir, "index.html");
} else {
  page_url = positional[0];
}
if (!existsSync(page_url)) {
  console.error(`error: ${page_url} not found`);
  process.exit(2);
}

// ---- the overlay -----------------------------------------------------------

// A viewer cannot see a keyboard. Without this the deck appears to advance by
// itself, which demonstrates nothing: the point of the recording is that a
// person is driving, and which key does what.
const KEY_OVERLAY = `
(() => {
  const box = document.createElement('div');
  box.id = '__mz_keys';
  box.style.cssText = [
    'position:fixed', 'left:50%', 'bottom:38px', 'transform:translateX(-50%)',
    'display:flex', 'gap:8px', 'z-index:2147483647', 'pointer-events:none',
    'font:500 22px/1 ui-sans-serif,system-ui,sans-serif',
  ].join(';');
  document.body.appendChild(box);
  window.__mzShowKey = (label) => {
    const chip = document.createElement('span');
    chip.textContent = label;
    chip.style.cssText = [
      'padding:10px 16px', 'border-radius:10px',
      'background:rgba(14,16,24,.82)', 'color:#fff',
      'border:1px solid rgba(255,255,255,.22)',
      'box-shadow:0 6px 24px rgba(0,0,0,.35)',
      'transition:opacity .28s ease, transform .28s ease',
      'opacity:0', 'transform:translateY(8px)',
    ].join(';');
    box.appendChild(chip);
    requestAnimationFrame(() => {
      chip.style.opacity = '1';
      chip.style.transform = 'translateY(0)';
    });
    setTimeout(() => {
      chip.style.opacity = '0';
      chip.style.transform = 'translateY(-6px)';
      setTimeout(() => chip.remove(), 300);
    }, 900);
  };
})();
`;

// ---- record ----------------------------------------------------------------

const outDir = dirname(resolve(out));
mkdirSync(outDir, { recursive: true });
const videoDir = mkdtempSync(join(tmpdir(), "mirzam-video-"));

const browser = await chromium.launch({ executablePath: CHROMIUM });
const context = await browser.newContext({
  viewport: { width, height },
  recordVideo: { dir: videoDir, size: { width, height } },
  // A demo should show the deck's own default, not this machine's preference.
  colorScheme: "light",
  reducedMotion: "no-preference",
});
const page = await context.newPage();
await page.goto("file://" + resolve(page_url));
await page.waitForTimeout(900);
if (showKeys) await page.evaluate(KEY_OVERLAY);

const press = async (key, label) => {
  if (showKeys) await page.evaluate((l) => window.__mzShowKey(l), label || key);
  await page.keyboard.press(key);
};

const slides = await page.$$eval("section.slide", (s) => s.length);
const wait = (ms) => page.waitForTimeout(ms);

// The performance. Every slide is held, every click step is taken, and the two
// toggles are shown once each — near the end, because they are a feature of the
// viewer rather than of the deck and would otherwise interrupt the reading.
await wait(dwell);
for (let i = 0; i < slides; i++) {
  const steps = await page.evaluate(() => {
    const sec = document.querySelector("section.slide.active");
    if (!sec) return 0;
    return Math.max(
      window.MZAnim ? window.MZAnim.steps(sec) : 0,
      window.MZAnnot ? window.MZAnnot.steps(sec) : 0
    );
  });
  for (let s = 0; s < steps; s++) {
    await press("ArrowRight", "→");
    await wait(stepDwell);
  }
  if (i < slides - 1) {
    await press("ArrowRight", "→");
    await wait(dwell);
  }
}

// The two viewer toggles, shown on the slide with the most panes — the layout
// overlay outlines panes, so demonstrating it on a title slide demonstrates
// nothing. The deck then returns to its last slide, which is where a demo
// should end: on whatever the author wanted the viewer left looking at.
const busiest = await page.evaluate(() => {
  let best = 0;
  let bestN = -1;
  for (const sec of document.querySelectorAll("section.slide")) {
    const n = sec.querySelectorAll(".pane").length;
    if (n > bestN) {
      bestN = n;
      best = +sec.dataset.index;
    }
  }
  return best;
});
const paper = () =>
  page.evaluate(() => {
    const sec = document.querySelector("section.slide.active");
    return sec ? getComputedStyle(sec).backgroundColor : "";
  });

if (busiest !== slides - 1) {
  await page.evaluate((n) => window.__mirzamGoto(n), busiest);
  await wait(700);
}
await press("KeyL", "L  layout");
await wait(1800);
await press("KeyL", "L");
await wait(900);

// A deck pinned to one palette is already in the mode `D` reaches for first,
// so the first press changes nothing. That is right for the deck and wrong for
// a recording of it — so press again, once, when nothing moved.
const before = await paper();
await press("KeyD", "D  dark / light");
await wait(700);
if ((await paper()) === before) {
  await press("KeyD", "D");
  await wait(700);
}
await wait(1500);

if (busiest !== slides - 1) {
  await page.evaluate((n) => window.__mirzamGoto(n), slides - 1);
  await wait(1800);
}

await context.close();
await browser.close();

// Playwright names the file after the page; move it somewhere predictable.
const produced = readdirSync(videoDir).find((f) => f.endsWith(".webm"));
if (!produced) {
  console.error("error: no video was produced");
  process.exit(1);
}
const webm = resolve(out) + ".webm";
renameSync(join(videoDir, produced), webm);
rmSync(videoDir, { recursive: true, force: true });
console.log(`✓ ${webm}`);

// ---- GIF -------------------------------------------------------------------

// Two passes: one to learn the colours actually used, one to quantise against
// them. A single-pass GIF of a deck bands the gradients and speckles the type,
// which makes the tool look worse than it is - the opposite of the point.
if (wantGif) {
  if (!FFMPEG) {
    console.error(
      "The video is written; the GIF is not. A GIF needs a full ffmpeg, and\n" +
        "the one Playwright ships is a stripped build without the palette\n" +
        "filters. Install one (`apt install ffmpeg`, `brew install ffmpeg`),\n" +
        "point MIRZAM_FFMPEG at it, and re-run — or convert by hand:\n" +
        `  ffmpeg -i ${webm} -vf "fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos,palettegen" /tmp/p.png\n` +
        `  ffmpeg -i ${webm} -i /tmp/p.png -lavfi "fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos[x];[x][1:v]paletteuse" out.gif`
    );
    process.exit(1);
  }
  const gif = resolve(out) + ".gif";
  const palette = join(tmpdir(), `mz-palette-${process.pid}.png`);
  const filters = `fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos`;
  execFileSync(FFMPEG, ["-y", "-i", webm, "-vf", `${filters},palettegen=stats_mode=diff`, palette], {
    stdio: ["ignore", "ignore", "inherit"],
  });
  execFileSync(
    FFMPEG,
    ["-y", "-i", webm, "-i", palette, "-lavfi", `${filters}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3`, gif],
    { stdio: ["ignore", "ignore", "inherit"] }
  );
  rmSync(palette, { force: true });
  const { statSync } = await import("fs");
  const mb = (statSync(gif).size / 1024 / 1024).toFixed(1);
  console.log(`✓ ${gif} (${mb} MB)`);
  if (+mb > 10) {
    console.log(
      "  GitHub will not render a GIF this large inline. Lower --fps, --gif-width,\n" +
        "  or record fewer slides."
    );
  }
}
