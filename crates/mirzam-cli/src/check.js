// Layout-check algorithm, run inside a built deck's own page. This is the one
// place the check lives: `scripts/check-layout.mjs` (Playwright, for CI) and
// `mirzam check` (a raw headless Chromium process, driven from Rust) both
// load this exact file rather than keeping their own copy, so the two tools
// can never drift into checking different things.
//
// Nothing here talks to Node, Playwright, or Rust - it only touches the DOM,
// through the same globals the viewer runtime exposes (`window.__mirzamGoto`,
// `window.__mirzamConnectors`, `window.MZAnim`, `window.MZAnnot`). The one
// entry point is `mzRunCheck()`, an async function returning
// `{ count, problems }`.
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

/** Collects layout problems for one already-shown slide. */
function mzSlideIssues(sec, tol) {
  const issues = [];
  const panes = [...sec.querySelectorAll(".pane")];

  for (const pane of panes) {
    const name = (pane.className.match(/pane-([\w-]+)/) || [])[1] || "?";
    // Content taller or wider than the pane is clipped by overflow:hidden,
    // which is exactly the "the heading disappeared" failure.
    //
    // A background pane deliberately overflows: the photo is scaled up so
    // its blurred edges stay off screen. Measure the content wrapper against
    // the pane's content box instead, so the decoration is ignored but
    // clipped text is still caught.
    const wrap = pane.querySelector(":scope > .mz-bg-content");
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

/** Walks every slide, collecting problems. Returns `{ count, problems }`. */
async function mzRunCheck() {
  const TOLERANCE = 2; // px of sub-pixel slack before calling it an overflow

  // Both can still be loading once the `load` event fires; measuring before
  // either settles reads the wrong layout - a CJK fallback font's line
  // height, or an image at its alt-text size.
  await mzWaitImagesLoaded();
  if (document.fonts && document.fonts.ready) await document.fonts.ready;

  const problems = [];

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
      for (const f of mzSlideIssues(sec, TOLERANCE)) problems.push({ slide: i + 1, ...f });
    }
  }
  return { count, problems };
}
