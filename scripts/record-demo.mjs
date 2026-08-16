// Records Mirzam working, as a video or a GIF, by driving it in a browser
// rather than by anyone operating one. There are two subjects:
//
//   node scripts/record-demo.mjs --editor -o media/edit-loop --gif
//   node scripts/record-demo.mjs --build examples/pitch.md -o media/pitch
//   node scripts/record-demo.mjs out/index.html -o media/demo --gif
//
// **`--editor` is the one the README carries.** It records the *edit loop* -
// the browser editor at `web/wasm-demo`, a textarea beside a preview that
// rebuilds on every keystroke - while a script types a small deck into it: a
// title, then an ASCII pane grid that becomes a layout as the last `+` lands,
// then a chart forming out of three lines of CSV, then a `theme:` line
// changing the whole identity. A viewing of a finished deck is something every
// slide tool can show. Source becoming slides while you watch is not, and the
// typing is therefore the content rather than the delivery.
//
// The other mode plays a *built* deck: navigation, click-through steps, the two
// viewer toggles. That is a walkthrough, and a walkthrough is worth having -
// but it is minutes long and belongs on a hosted video, linked rather than
// embedded, because GitHub will not render a large GIF and will not render a
// committed `.webm` at all.
//
// Why script it at all: a screen recording of a slide tool is the one piece of
// documentation that cannot be written, and it is also the piece most likely to
// come out badly — a hesitation before a keypress, a cursor crossing the slide,
// a pause of the wrong length. None of that is a recording problem. It is a
// *performing* problem, and a script does not hesitate. It is also
// reproducible: change a theme, re-run it, and the demo is the tool as it is
// today rather than as it was the afternoon someone had time to record it.

import { chromium } from "playwright-core";
import { execFileSync } from "child_process";
import { createReadStream, existsSync, mkdirSync, mkdtempSync, renameSync, rmSync, readdirSync, statSync } from "fs";
import { createServer } from "http";
import { tmpdir } from "os";
import { extname, join, resolve, dirname, normalize } from "path";
import { fileURLToPath } from "url";

const CHROMIUM = process.env.MIRZAM_CHROMIUM || undefined;
const REPO_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

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

const editorMode = flag("--editor");
const buildFrom = flag("--build") ? argv[argv.indexOf("--build") + 1] : null;
const positional = argv.filter((a, i) => !a.startsWith("--") && argv[i - 1] !== "-o" && argv[i - 1] !== "--build" && !["-o"].includes(a));
const out = opt("-o", editorMode ? "media/edit-loop" : "media/demo");
const width = +opt("--width", editorMode ? 1500 : 1280);
const height = +opt("--height", editorMode ? 760 : 720);
// Seconds a slide is held before moving on. Long enough to read a heading, not
// long enough to feel like a pause — a demo is an advertisement for the tool,
// not a talk being given.
const dwell = +opt("--dwell", 2.2) * 1000;
const stepDwell = +opt("--step", 1.1) * 1000;
const wantGif = flag("--gif");
const gifFps = +opt("--fps", editorMode ? 10 : 12);
const gifWidth = +opt("--gif-width", editorMode ? 1000 : 800);
// How many colours the GIF's one palette holds, and whether to dither against
// it. The two modes want opposite answers and the defaults follow the subject
// rather than a house style.
//
// A deck is photographs, gradients and a drop shadow: it needs the full 256 and
// it needs dithering, or the gradients band and the tool looks worse than it
// is. The editor is flat panels and text, so dithering only speckles the type
// it was meant to smooth, and dropping it saves a fifth of the file.
//
// 128 rather than 64, and the difference is not subtlety: at 64 the palette
// cannot hold two near-flat papers at once, and the deck in the preview -
// whose whole job in the last five seconds is to change its face for
// `theme: wuwei` - quantises the two faces toward one, with the near-flat
// paper banding into stripes. A recording of a theme change that does not
// show the theme change is worth none of the megabyte it saves.
const gifColors = +opt("--gif-colors", editorMode ? 128 : 256);
const gifDither = opt("--gif-dither", editorMode ? "none" : "bayer:bayer_scale=3");
const showKeys = !flag("--no-keys");

// Typing tunables for `--editor`. `--cadence` scales every keystroke and every
// beat at once, which is the only honest way to make the recording shorter:
// typing half the deck twice as fast is a different demo, typing all of it at
// 1.2× is the same one.
const cadence = +opt("--cadence", 1);
// The beat at the end of a typed line. It has a floor rather than a taste: the
// editor rebuilds 120 ms after the last keystroke, so a pause shorter than that
// is a pause the preview never notices, and a recording of an editor that
// updates four times in twenty seconds is a recording of the wrong tool.
const linePause = +opt("--line-pause", 150);

if (!editorMode && !buildFrom && positional.length === 0) {
  console.error(
    "usage: node scripts/record-demo.mjs --editor [-o <out>] [--gif]\n" +
      "       node scripts/record-demo.mjs [--build <deck.md> | <built.html>] -o <out>\n" +
      "       [--gif] [--width 1280] [--height 720] [--dwell 2.2] [--step 1.1]\n" +
      "       [--fps 12] [--gif-width 800] [--gif-colors 256] [--gif-dither none]\n" +
      "       [--no-keys]  --editor only: [--cadence 1] [--line-pause 150]"
  );
  process.exit(2);
}

// ---- the overlay -----------------------------------------------------------

// A viewer cannot see a keyboard. Without this the deck appears to advance by
// itself, which demonstrates nothing: the point of the recording is that a
// person is driving, and which key does what.
//
// The editor recording has no overlay and wants none: every keystroke there is
// already on screen, as the character it typed.
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

// ---- a static server, for the editor ---------------------------------------

// `.wasm` cannot be loaded over `file://` — the browser refuses to instantiate
// a module fetched from an opaque origin — so the editor has to be recorded
// over HTTP. That is one small server rather than a dependency; `serve-wasm-
// demo.sh` reaches for python3 for the same job, which a recording script
// cannot rely on being present and cannot easily ask for an unused port.
const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".wasm": "application/wasm",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ts": "text/plain; charset=utf-8",
};

function serveDir(root) {
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://localhost");
    let rel = decodeURIComponent(url.pathname);
    if (rel.endsWith("/")) rel += "index.html";
    // A recording script serving a directory is still a server: a request for
    // `../../.ssh/id_rsa` gets a 403 here rather than a file.
    const file = join(root, normalize(rel));
    if (!file.startsWith(root)) {
      res.writeHead(403).end();
      return;
    }
    if (!existsSync(file) || statSync(file).isDirectory()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": MIME[extname(file)] || "application/octet-stream" });
    createReadStream(file).pipe(res);
  });
  return new Promise((ok) => {
    server.listen(0, "127.0.0.1", () => ok({ server, port: server.address().port }));
  });
}

// ---- the deck being typed --------------------------------------------------

// Four beats, in the order a deck is actually written: say what it is, draw
// where things go, put the data in, decide how it looks. Each beat is one
// uninterrupted burst of typing, and the pause after it is the moment the
// viewer needs to notice what just appeared.
//
// `cps` is characters per second, and the numbers are not decoration. The
// title is slow because it is the first thing on screen and nobody knows yet
// what they are looking at; the grid is fast because it is a shape rather than
// a sentence, and a shape read at reading speed is tedious; the CSV is between
// the two. A single speed for all of them reads as a robot typing, which is
// exactly what this is and exactly what it must not look like.
const BEATS = [
  {
    what: "a title",
    text:
      "---\n" +
      "title: Latency after the cache rollout\n" +
      "---\n" +
      "\n" +
      "# Latency after the cache rollout {.title-slide}\n" +
      "\n" +
      "Four weeks after the rollout\n",
    cps: 60,
    after: 430,
  },
  {
    what: "a pane grid",
    // The ASCII box *is* the layout. This is the beat the whole recording
    // exists for: the moment the last `+` lands, the preview stops being one
    // column of Markdown and becomes a slide with a shape.
    text:
      "\n---\n\n" +
      "```pane\n" +
      "+----------------+----------------+\n" +
      "|  head                           |\n" +
      "+----------------+----------------+\n" +
      "|  main          |  chart         |\n" +
      "+----------------+----------------+\n" +
      "```\n\n" +
      "::: pane head\n" +
      "## Where the time went\n" +
      ":::\n\n" +
      "::: pane main\n" +
      "- p95 dropped in **every region**\n" +
      "- The largest win is `ap-ne`\n" +
      ":::\n\n" +
      "::: pane chart\n" +
      ":::\n",
    cps: 130,
    after: 430,
  },
  {
    what: "a chart",
    // Typed *into* the pane that is already there, which is why the caret is
    // moved first: a chart is data written where the picture goes, not a file
    // imported from somewhere else.
    caretAfter: "::: pane chart\n",
    text:
      "```chart\n" +
      "type: bar\n" +
      "data: |\n" +
      "  region, before, after\n" +
      "  us-east, 210, 120\n" +
      "  ap-ne, 380, 180\n" +
      "```\n",
    cps: 95,
    after: 480,
  },
  {
    what: "a theme",
    // Back to the top for one line. The look of the whole deck is a key in
    // the frontmatter, and the cheapest way to show that is to add it last.
    // The preview follows the caret, so this beat also takes it back to the
    // title slide - which is where a new face reads most clearly anyway.
    caretAfter: "title: Latency after the cache rollout\n",
    text: "theme: wuwei\n",
    cps: 27,
    after: 760,
  },
  {
    what: "back to the slide",
    // No typing: a click back into the slide that was being written, so the
    // recording ends on the deck rather than on its frontmatter. The preview
    // follows, and the last thing on screen is the grid and the chart in the
    // new theme.
    caretAfter: "## Where the time went\n",
    text: "",
    after: 1200,
  },
];

// The editor's own type is sized for a 42%-wide column on a laptop, and a GIF
// scaled to fit a README turns 13px into six. Enlarging it for the recording
// is not a lie about the product - it is the same trade a presenter makes
// zooming their editor before a talk, and an unreadable source pane would make
// the recording about nothing at all.
const RECORDING_STYLE = `
  textarea#src { font-size: 20px !important; line-height: 1.55 !important; width: 46% !important; }
  header h1 { font-size: 18px !important; }
  .stat { font-size: 15px !important; }
  header button, header label.btn { font-size: 14px !important; }
`;

// A deterministic wobble. Human typing is not a metronome and a metronome is
// visible; a seeded generator keeps the recording reproducible anyway, so two
// runs a month apart differ because the tool changed and not because the
// random numbers did.
let seed = 20260816;
const rand = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);

// ---- record ----------------------------------------------------------------

const outDir = dirname(resolve(out));
mkdirSync(outDir, { recursive: true });
const videoDir = mkdtempSync(join(tmpdir(), "mirzam-video-"));

let httpServer = null;
let page_url;

if (editorMode) {
  const demoDir = join(REPO_ROOT, "web", "wasm-demo");
  if (!existsSync(join(demoDir, "pkg", "mirzam_wasm.js"))) {
    console.log("==> the WASM package is not built yet; building it");
    execFileSync(join(REPO_ROOT, "scripts", "build-wasm.sh"), ["web/wasm-demo/pkg"], {
      cwd: REPO_ROOT,
      stdio: "inherit",
    });
  }
  const { server, port } = await serveDir(demoDir);
  httpServer = server;
  page_url = `http://127.0.0.1:${port}/`;
  console.log(`==> serving ${demoDir} at ${page_url}`);
} else {
  if (buildFrom) {
    const dir = mkdtempSync(join(tmpdir(), "mirzam-demo-"));
    execFileSync("cargo", ["run", "-q", "--bin", "mirzam", "--", "build", buildFrom, "-o", dir], {
      stdio: ["ignore", "ignore", "inherit"],
    });
    page_url = "file://" + join(dir, "index.html");
  } else {
    if (!existsSync(positional[0])) {
      console.error(`error: ${positional[0]} not found`);
      process.exit(2);
    }
    page_url = "file://" + resolve(positional[0]);
  }
}

const browser = await chromium.launch({ executablePath: CHROMIUM });
const context = await browser.newContext({
  viewport: { width, height },
  recordVideo: { dir: videoDir, size: { width, height } },
  // A deck-mode demo should show the deck's own default, not this machine's
  // preference - so light, since a deck with no `mode:` rests there. The
  // editor recording is different: it lives at the top of the README, and the
  // README, the site and the editor's own chrome are all dark-first - a light
  // preview inside that frame reads as a hole in the page. The deck in the
  // preview follows the browser preference, so the preference is set to the
  // identity the recording sits in.
  colorScheme: editorMode ? "dark" : "light",
  reducedMotion: "no-preference",
});

// The editor keeps the draft in localStorage and opens on the sample deck when
// there is none. Both are wrong for a recording that is about starting from
// nothing, and the empty string is not the same as no value: it is what the
// editor stores when somebody has emptied the draft on purpose, which is
// exactly the state being staged.
if (editorMode) {
  await context.addInitScript(() => {
    try {
      localStorage.setItem("mirzam-src", "");
      localStorage.removeItem("mirzam-assets");
      localStorage.removeItem("mirzam-files");
    } catch (e) {
      /* a private window with no storage; the sample is then what gets typed over */
    }
  });
}

const page = await context.newPage();
await page.goto(page_url);
await page.waitForTimeout(900);

const wait = (ms) => page.waitForTimeout(ms);

if (editorMode) {
  await performEditor(page);
} else {
  await performDeck(page);
}

await context.close();
await browser.close();
if (httpServer) httpServer.close();

// ---- the two performances --------------------------------------------------

/** Types a deck into the browser editor, one beat at a time. */
async function performEditor(page) {
  await page.addStyleTag({ content: RECORDING_STYLE });
  // The WASM module is fetched and instantiated after the page loads, and the
  // status line is what says it finished. Typing before that would drop the
  // first characters into a textarea nothing is listening to.
  await page.waitForFunction(() => !/Loading/.test(document.getElementById("stat").textContent), null, {
    timeout: 30000,
  });
  await page.click("#src");
  await wait(500);

  for (const beat of BEATS) {
    if (beat.caretAfter) {
      // Moved the way a person moves it: the caret is placed and the textarea
      // is *clicked*, because clicking into the source is what tells the
      // preview to follow. Setting the selection alone fires no event, and the
      // recording would then be of a feature nobody triggered.
      const moved = await page.evaluate((marker) => {
        const el = document.getElementById("src");
        const i = el.value.indexOf(marker);
        if (i === -1) return false;
        el.focus();
        el.setSelectionRange(i + marker.length, i + marker.length);
        el.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        return true;
      }, beat.caretAfter);
      if (!moved) {
        throw new Error(
          `the beat "${beat.what}" is typed after ${JSON.stringify(beat.caretAfter)}, ` +
            `which is not in the deck typed so far - the beats above it changed`
        );
      }
      // Long enough to read as "they clicked there", short enough not to stall.
      await wait(300 / cadence);
    }
    await typeText(page, beat.text, beat.cps);
    await wait(beat.after / cadence);
  }
}

/** Types one string at a human cadence, letting the preview catch up per line. */
async function typeText(page, text, cps) {
  const base = 1000 / (cps * cadence);
  for (const ch of text) {
    await page.keyboard.type(ch);
    await wait(base * (0.55 + rand() * 0.9));
    // The editor rebuilds 120 ms after the last keystroke. Typing a whole deck
    // without ever stopping for that long would show a preview that updates
    // four times: the pause at the end of a line is both what a person does
    // and what makes the loop visible.
    if (ch === "\n") await wait(linePause / cadence);
  }
}

/** Plays a built deck: every slide, every click step, the two viewer toggles. */
async function performDeck(page) {
  if (showKeys) await page.evaluate(KEY_OVERLAY);
  const press = async (key, label) => {
    if (showKeys) await page.evaluate((l) => window.__mzShowKey(l), label || key);
    await page.keyboard.press(key);
  };

  const slides = await page.$$eval("section.slide", (s) => s.length);

  // Every slide is held, every click step is taken, and the two toggles are
  // shown once each — near the end, because they are a feature of the viewer
  // rather than of the deck and would otherwise interrupt the reading.
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
}

// ---- output ----------------------------------------------------------------

// Playwright names the file after the page; move it somewhere predictable.
const produced = readdirSync(videoDir).find((f) => f.endsWith(".webm"));
if (!produced) {
  console.error("error: no video was produced");
  process.exit(1);
}
const webm = resolve(out) + ".webm";
renameSync(join(videoDir, produced), webm);
rmSync(videoDir, { recursive: true, force: true });
console.log(`✓ ${webm} (${(statSync(webm).size / 1024).toFixed(0)} KB)`);

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
        `  ffmpeg -i ${webm} -vf "fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos,palettegen=max_colors=${gifColors}" /tmp/p.png\n` +
        `  ffmpeg -i ${webm} -i /tmp/p.png -lavfi "fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=${gifDither}" out.gif`
    );
    process.exit(1);
  }
  const gif = resolve(out) + ".gif";
  const palette = join(tmpdir(), `mz-palette-${process.pid}.png`);
  const filters = `fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos`;
  execFileSync(
    FFMPEG,
    ["-y", "-i", webm, "-vf", `${filters},palettegen=stats_mode=diff:max_colors=${gifColors}`, palette],
    { stdio: ["ignore", "ignore", "inherit"] }
  );
  execFileSync(
    FFMPEG,
    ["-y", "-i", webm, "-i", palette, "-lavfi", `${filters}[x];[x][1:v]paletteuse=dither=${gifDither}`, gif],
    { stdio: ["ignore", "ignore", "inherit"] }
  );
  rmSync(palette, { force: true });
  const mb = statSync(gif).size / 1024 / 1024;
  console.log(`✓ ${gif} (${mb.toFixed(1)} MB)`);
  // GitHub renders an inline GIF up to 10 MB and links to anything larger,
  // which is the difference between a README that shows the tool working and
  // one that offers to. The budget is enforced rather than advised in
  // `.github/workflows/demo.yml`; here it is a note, because a run by hand is
  // often deliberately larger.
  if (mb > 10) {
    console.log(
      "  GitHub will not render a GIF this large inline. Lower --fps, --gif-width,\n" +
        "  or --cadence the typing along faster."
    );
  }
}
