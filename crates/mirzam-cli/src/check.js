// Layout-check algorithm, run inside a built deck's own page. This is the one
// place the check lives: `scripts/check-layout.mjs` (Playwright, for CI) and
// `mirzam check` (a raw headless Chromium process, driven from Rust) both
// load this exact file rather than keeping their own copy, so the two tools
// can never drift into checking different things.
//
// Nothing here talks to Node, Playwright, or Rust - it only touches the DOM,
// through the same globals the viewer runtime exposes (`window.__mirzamGoto`,
// `window.__mirzamConnectors`, `window.MZAnim`, `window.MZAnnot`). The one
// entry point is `mzRunCheck(opts)`, an async function returning
// `{ count, problems, notes }` - the notes being what the run was measured
// with, which is what says how far a clean result can be trusted.
//
// Waiting for images and fonts, and forcing animations to their end state
// with `Animation.finish()` rather than polling `playState`, is not just
// belt-and-braces here: `mirzam check` runs Chromium under
// `--virtual-time-budget` so it can drive an async check from a one-shot CLI
// process, and under that mode `requestAnimationFrame` never fires and a
// freshly-started Web Animation never advances past `currentTime: 0` - so
// this file calls `window.__mirzamConnectors()` directly instead of waiting
// on the runtime's own `requestAnimationFrame`-scheduled redraw, and finishes
// animations instead of waiting for them to play out. Both are exact,
// deterministic substitutes for what a real, interactively-driven browser
// would eventually settle to on its own - not an approximation - so a
// Playwright-driven run sees the same thing.

async function mzWaitImagesLoaded() {
  const imgs = [...document.images].filter((img) => !img.complete);
  await Promise.all(
    imgs.map(
      (img) =>
        new Promise((resolve) => {
          img.addEventListener("load", resolve, { once: true });
          img.addEventListener("error", resolve, { once: true });
        })
    )
  );
}

/**
 * Which of the families the deck asks for this machine actually has.
 *
 * A deck embeds no text font — only the maths face is inlined — so every
 * measurement here is of the deck set in whatever the checking machine
 * resolved its stack to. On a box with no Japanese family installed but one
 * fallback, a CJK deck is measured in a font no reader will ever see, and a
 * font substitution moves text extent by far more than the slack a tight pane
 * has. The check cannot fix that, but it must not stay quiet about it: a green
 * run is a statement about one machine, and the output should say which.
 *
 * Measured rather than asked, because a page has no API for "what did you
 * actually rasterise" — a family that is missing falls back, and the fallback
 * is the same width as the generic it fell back to.
 */
function mzFontsPresent(families) {
  const probe = document.createElement("span");
  probe.textContent = "MWiil漢字あア";
  probe.style.cssText =
    "position:absolute;left:-9999px;top:0;font-size:72px;white-space:nowrap;visibility:hidden";
  document.body.appendChild(probe);
  const width = (stack) => {
    probe.style.fontFamily = stack;
    return probe.getBoundingClientRect().width;
  };
  const generics = ["monospace", "serif", "sans-serif"];
  const base = generics.map(width);
  const present = [];
  const missing = [];
  for (const family of families) {
    const has = generics.some((g, i) => width(`"${family}",${g}`) !== base[i]);
    (has ? present : missing).push(family);
  }
  probe.remove();
  return { present, missing };
}

/** The families a deck names for its body text, generics dropped. */
function mzRequestedFamilies() {
  const generic = /^(sans-serif|serif|monospace|cursive|fantasy|system-ui|ui-[\w-]+|math|emoji)$/;
  return getComputedStyle(document.body)
    .fontFamily.split(",")
    .map((f) => f.trim().replace(/^["']|["']$/g, ""))
    .filter((f) => f && !generic.test(f));
}

function mzFinishAnimations() {
  for (const a of document.getAnimations()) {
    try {
      a.finish();
    } catch (e) {
      // An animation that cannot be finished (e.g. infinite iterations) is
      // left running; nothing here depends on one existing.
    }
  }
}

/**
 * Whether a pane is held to the rows it sits in, rather than sized to what is
 * in it. Only the first kind can overflow, so only the first kind has slack
 * worth reporting: a title slide's pane grows with its content and would
 * otherwise report 0px of room left however little is on it, making every
 * deck's tightest pane the one slide that cannot fail.
 *
 * Boxes come back through the deck's scale transform and track sizes do not,
 * so the pane is measured back into the grid's own units before comparing.
 */
function mzHeldToItsTrack(pane, grid) {
  const cs = getComputedStyle(grid);
  const rows = cs.gridTemplateRows.split(" ").map(parseFloat).filter((n) => !isNaN(n));
  if (!rows.length || !grid.clientHeight) return false;
  const gap = parseFloat(cs.rowGap) || 0;
  const scale = grid.getBoundingClientRect().height / grid.clientHeight || 1;
  const h = pane.getBoundingClientRect().height / scale;
  for (let i = 0; i < rows.length; i++) {
    let span = 0;
    for (let j = i; j < rows.length; j++) {
      span += rows[j] + (j > i ? gap : 0);
      if (Math.abs(span - h) < 2) return true;
    }
  }
  return false;
}

/**
 * How much room is left under a pane's content, in px, or `null` when the pane
 * holds nothing to measure.
 *
 * Measured from the boxes rather than from `scrollHeight`, which cannot answer
 * this: a scroll height is never smaller than the client height, so a pane with
 * half a slide of room to spare reports exactly the same 0 as one filled to the
 * pixel. Absolutely positioned children — a background photo, a scrim — are
 * skipped, since they are sized to the pane by definition and would report
 * every decorated pane as full.
 */
function mzSlack(pane, wrap) {
  const flow = wrap || pane;

  const cs = getComputedStyle(pane);
  const bottom =
    pane.getBoundingClientRect().bottom -
    parseFloat(cs.paddingBottom) -
    parseFloat(cs.borderBottomWidth);
  let content = null;
  for (const el of flow.children) {
    const p = getComputedStyle(el).position;
    if (p === "absolute" || p === "fixed") continue;
    const r = el.getBoundingClientRect();
    if (!r.height && !r.width) continue;
    content = content === null ? r.bottom : Math.max(content, r.bottom);
  }
  if (content === null) return null;
  // A figure standing on the pane's edge fills it by definition: a chart or an
  // image is sized to its box and scales, where text wraps and grows. Left in,
  // a chart pane would be every deck's tightest one and say nothing. This
  // number is about the text, which is what a font substitution moves.
  for (const fig of pane.querySelectorAll("svg, img, video, canvas")) {
    if (Math.abs(fig.getBoundingClientRect().bottom - content) < 2) return null;
  }
  return bottom - content;
}

/**
 * Collects layout problems for one already-shown slide. Every pane's slack —
 * how much room is left under its content — is pushed onto `slack`, because
 * "it fits" and "it fits by a pixel" are different answers to the question an
 * author is really asking, and only the second one predicts what a font
 * substitution will do.
 */
function mzSlideIssues(sec, tol, slack) {
  const issues = [];
  const panes = [...sec.querySelectorAll(".pane")];
  const grid = sec.querySelector(".grid");

  for (const pane of panes) {
    const name = (pane.className.match(/pane-([\w-]+)/) || [])[1] || "?";
    // Content taller or wider than the pane is clipped by overflow:hidden,
    // which is exactly the "the heading disappeared" failure.
    //
    // A background pane deliberately overflows: the photo is scaled up so
    // its blurred edges stay off screen. Measure the content wrapper against
    // the pane's content box instead, so the decoration is ignored but
    // clipped text is still caught.
    //
    // `fit=shrink` needs the same treatment for the opposite reason. It moves
    // the pane's children into a wrapper and scales *that*, so the pane keeps
    // the scroll height of the type size it started at: a pane the fit had
    // already rescued was still reported as overflowing, and turning on the
    // safety net cost you the tool that tells you whether you need it. This is
    // the same measurement `fit.js` makes to decide when to stop shrinking, so
    // the two now agree by construction — including when they should both
    // fail, on a pane still overflowing at the 55% floor.
    const wrap = pane.querySelector(":scope > .mz-fit-inner, :scope > .mz-bg-content");
    let overY, overX;
    if (wrap) {
      const cs = getComputedStyle(pane);
      const pad = (a, b) => parseFloat(cs[a]) + parseFloat(cs[b]);
      overY = wrap.scrollHeight - (pane.clientHeight - pad("paddingTop", "paddingBottom"));
      overX = wrap.scrollWidth - (pane.clientWidth - pad("paddingLeft", "paddingRight"));
    } else {
      overY = pane.scrollHeight - pane.clientHeight;
      overX = pane.scrollWidth - pane.clientWidth;
    }
    if (slack && grid && pane.clientHeight && mzHeldToItsTrack(pane, grid)) {
      const px = mzSlack(pane, wrap);
      if (px !== null) slack.push({ pane: name, px: Math.round(px) });
    }
    if (overY > tol) {
      issues.push({
        kind: "clipped",
        pane: name,
        detail: `content is ${Math.round(overY)}px taller than the pane`,
      });
    }
    if (overX > tol) {
      issues.push({
        kind: "clipped",
        pane: name,
        detail: `content is ${Math.round(overX)}px wider than the pane`,
      });
    }
    // The pane measuring clean is not the same as everything being visible.
    // An element that scrolls internally holds its overflow instead of
    // passing it up: `pre { overflow: auto }` is a flex item whose automatic
    // minimum size is zero, so it shrinks below its own content rather than
    // pushing the pane over, and the pane's scroll height still equals its
    // client height. Three of four code lines were invisible and this check
    // said the slide was fine. Nobody in an audience can scroll a slide, so
    // a scroll box is clipping, and it is measured where it happens.
    for (const el of pane.querySelectorAll("*")) {
      if (!(el instanceof HTMLElement) || !el.clientHeight) continue;
      const cs = getComputedStyle(el);
      const hidesY = cs.overflowY !== "visible";
      const hidesX = cs.overflowX !== "visible";
      if (!hidesY && !hidesX) continue;
      const hiddenY = hidesY ? el.scrollHeight - el.clientHeight : 0;
      const hiddenX = hidesX ? el.scrollWidth - el.clientWidth : 0;
      if (hiddenY <= tol && hiddenX <= tol) continue;
      const what = el.tagName.toLowerCase() + (el.className ? "." + String(el.className).split(/\s+/)[0] : "");
      const axis = hiddenY > tol ? `${Math.round(hiddenY)}px below` : `${Math.round(hiddenX)}px to the right`;
      issues.push({
        kind: "clipped",
        pane: name,
        detail: `<${what}> scrolls: ${axis} of its content is out of sight, and a slide cannot be scrolled`,
      });
      break;
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

  // Type sizes are written in `em`, which multiplies down a nesting: a list
  // inside a list shipped at 1.35 x 1.35, so the qualification under a point
  // was half again as large as the point. It reads as a styling choice
  // rather than a bug, which is why it survived a release - so measure it
  // rather than trusting the stylesheet.
  for (const el of sec.querySelectorAll("li li, li p, dd p, li dd")) {
    const parent = el.parentElement.closest("li, dd");
    if (!parent) continue;
    const inner = parseFloat(getComputedStyle(el).fontSize);
    const outer = parseFloat(getComputedStyle(parent).fontSize);
    if (inner > outer + 0.5) {
      const pane = el.closest(".pane");
      issues.push({
        kind: "nesting",
        pane: (pane?.className.match(/pane-([\w-]+)/) || [])[1] || "-",
        detail: `nested <${el.tagName.toLowerCase()}> is ${inner.toFixed(1)}px inside a ${outer.toFixed(1)}px parent`,
      });
      break;
    }
  }

  // An annotation whose anchor has been renamed is dropped just as quietly,
  // and costs more: the sentence still says "the circled bar".
  const missing = window.MZAnnot ? window.MZAnnot.missing(sec) : 0;
  if (missing) {
    issues.push({
      kind: "annotation",
      pane: "-",
      detail: `${missing} annotation(s) not drawn (unknown id?)`,
    });
  }

  // The resting-state rule: once a track has played, its elements hold the
  // slide's final state. One still holding its entrance state is invisible
  // to everybody - including the PDF, which never steps.
  const armed = window.MZAnim ? window.MZAnim.armed(sec, window.MZAnim.steps(sec)) : 0;
  if (armed) {
    issues.push({
      kind: "animation",
      pane: "-",
      detail: `${armed} element(s) left in their initial state after the last step`,
    });
  }

  // A shape's label is one SVG <text>, centred on its shape and never
  // wrapped, so a label longer than its box walks straight out of it: drawn,
  // wrong, and invisible to every measure above - nothing clips and nothing
  // scrolls in an SVG layer. Label and shape share a centre by construction
  // (`text-anchor: middle` at the shape's own centre), so overflow is a
  // difference of sizes, not of positions. A standalone `text` shape has no
  // box to overflow and always carries a style attribute; a label never does.
  for (const label of sec.querySelectorAll(
    "svg.mz-shapes text.mz-shape-label:not([style])"
  )) {
    const shape = label.previousElementSibling;
    if (!shape || (shape.tagName !== "rect" && shape.tagName !== "ellipse")) continue;
    let text, box;
    try {
      text = label.getBBox();
      box = shape.getBBox();
    } catch (e) {
      continue; // an unrendered subtree has no geometry to measure
    }
    if (!text.width) continue;
    const overX = text.width - box.width;
    const overY = text.height - box.height;
    if (overX <= tol && overY <= tol) continue;
    const words = (label.textContent || "").trim();
    const short = words.length > 32 ? words.slice(0, 32) + "…" : words;
    const axis =
      overX > tol ? `${Math.round(overX)}px wider` : `${Math.round(overY)}px taller`;
    issues.push({
      kind: "label",
      pane: "-",
      detail: `shape label "${short}" is ${axis} than its ${shape.tagName}`,
    });
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
}

/**
 * Walks every slide, collecting problems. Returns
 * `{ count, problems, notes }` — `notes` being what the run was measured
 * with, which a reader needs to know how far to trust the rest of it.
 *
 * `opts.minSlack` turns the tightest-pane note into a problem: a pane with
 * less than that much room left is reported, so a deck can be held to a margin
 * in CI rather than to "it fitted on the machine that ran this".
 */
async function mzRunCheck(opts) {
  const TOLERANCE = 2; // px of sub-pixel slack before calling it an overflow
  const minSlack = (opts && opts.minSlack) || 0;

  // Both can still be loading once the `load` event fires; measuring before
  // either settles reads the wrong layout - a CJK fallback font's line
  // height, or an image at its alt-text size.
  await mzWaitImagesLoaded();
  if (document.fonts && document.fonts.ready) await document.fonts.ready;

  const problems = [];
  const slacks = [];

  // `--debug-layout` is for screenshotting a broken deck, not for
  // publishing. Baked on, it tints every pane pink for the audience, and the
  // only way to notice is to look - so look, once, before measuring anything.
  if (document.documentElement.classList.contains("mz-debug")) {
    problems.push({
      slide: 1,
      kind: "debug",
      pane: "-",
      detail: "the layout debug overlay is baked into this build (--debug-layout)",
    });
  }

  const count = document.querySelectorAll("section.slide").length;
  for (let i = 0; i < count; i++) {
    // Measure the slide the audience ends on, not the one it starts as. An
    // element revealed on the third click can overflow its pane, and a
    // connector pointing at an annotation cannot be routed until that
    // annotation has been drawn - the state a reader without JavaScript, and
    // the PDF, both see.
    if (window.__mirzamGoto) window.__mirzamGoto(i);
    mzFinishAnimations();
    if (window.__mirzamConnectors) window.__mirzamConnectors();
    const sec = document.querySelector(`section.slide[data-index="${i}"]`);
    const active = document.querySelector("section.slide.active") || sec;
    const n = Math.max(
      window.MZAnim && active ? window.MZAnim.steps(active) : 0,
      window.MZAnnot && active ? window.MZAnnot.steps(active) : 0
    );
    for (let s = 0; s < n; s++) {
      dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));
      mzFinishAnimations();
    }
    // A step can move or reveal a connector's own endpoint (an annotation
    // mark placed on a later click), so the redraw runs again after stepping.
    if (window.__mirzamConnectors) window.__mirzamConnectors();
    if (sec) {
      const slack = [];
      for (const f of mzSlideIssues(sec, TOLERANCE, slack)) problems.push({ slide: i + 1, ...f });
      for (const s of slack) slacks.push({ slide: i + 1, ...s });
    }
  }

  const notes = [];
  const { present, missing } = mzFontsPresent(mzRequestedFamilies());
  if (missing.length) {
    const shown = missing.slice(0, 3).join(", ");
    const more = missing.length > 3 ? `, and ${missing.length - 3} more` : "";
    // A deck full of Japanese measured on a machine with no Japanese family
    // from its own stack is the case worth naming: a CJK substitution moves
    // text extent by far more than a tight pane has to give.
    const cjk = /[぀-ヿ㐀-鿿]/.test(document.body.textContent || "");
    notes.push(
      `fonts: measured with ${present.join(", ") || "none of the families this deck names"}` +
        `; not on this machine: ${shown}${more}` +
        (cjk && !present.length ? " - including every one that covers its CJK text" : "") +
        ". A reader who has them sees different line breaks"
    );
  }
  slacks.sort((a, b) => a.px - b.px);
  const tightest = slacks.find((s) => s.px >= 0);
  if (tightest) {
    notes.push(
      `tightest pane: slide ${tightest.slide} "${tightest.pane}", ${tightest.px}px of room left`
    );
  }
  if (minSlack) {
    for (const s of slacks.filter((s) => s.px >= 0 && s.px < minSlack)) {
      problems.push({
        slide: s.slide,
        kind: "slack",
        pane: s.pane,
        detail: `${s.px}px of room left, under the ${minSlack}px asked for`,
      });
    }
  }
  return { count, problems, notes };
}
