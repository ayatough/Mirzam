//! The built-in theme's CSS and the viewer runtime JS.
//! Decks override this with frontmatter `css:`.

pub const DEFAULT_CSS: &str = r#"
:root {
  --mz-bg: #14161d;
  --mz-slide-bg: #ffffff;
  --mz-fg: #23272f;
  --mz-muted: #6d7590;
  --mz-accent1: #3056d3;
  --mz-accent2: #12b8a6;
  --mz-border: #e3e6ef;
}
* { box-sizing: border-box; }
html, body {
  margin: 0; height: 100%;
  background: var(--mz-bg);
  overflow: hidden;
  /* Name Japanese fonts explicitly: without them, some systems fall back to a
     non-Japanese CJK font and render kanji with the wrong glyph shapes.
     PDF export uses the fonts installed on the exporting machine. */
  font-family: 'Helvetica Neue', Arial, 'Hiragino Kaku Gothic ProN', 'Hiragino Sans',
    'Noto Sans CJK JP', 'Noto Sans JP', 'Yu Gothic Medium', 'Yu Gothic', Meiryo, sans-serif;
}
#deck {
  position: absolute; top: 50%; left: 50%;
  transform-origin: center center;
  background: var(--mz-slide-bg);
  border-radius: 6px;
  box-shadow: 0 12px 60px rgba(0,0,0,.5);
  overflow: hidden;
}
section.slide {
  position: absolute; inset: 0;
  display: none;
  color: var(--mz-fg);
}
section.slide.active { display: block; }
.grid {
  display: grid; width: 100%; height: 100%;
  gap: 20px; padding: 44px 60px;
}
.pane { min-width: 0; min-height: 0; overflow: hidden; padding: 2px 4px; }
/* A heading band that is drawn too short would silently clip its heading.
   Allow the text to stay legible instead, and let the author widen the band. */
.pane-head, .pane-title { overflow: visible; }
.pane > :first-child { margin-top: 0; }
.pane > :last-child { margin-bottom: 0; }

h1 { font-size: 2.6em; margin: .2em 0; letter-spacing: .01em; }
h2 {
  font-size: 1.85em; margin: 0 0 .4em; line-height: 1.25;
  padding-bottom: .25em;
  border-bottom: 3px solid var(--mz-accent1);
}
h3 { font-size: 1.3em; color: var(--mz-accent1); }
p, li { font-size: 1.35em; line-height: 1.65; }
li { margin: .25em 0; }
strong { color: var(--mz-accent1); }
a { color: var(--mz-accent1); }

.u { text-decoration: underline; text-decoration-color: var(--mz-accent2); text-decoration-thickness: 3px; text-underline-offset: 4px; }
.center { text-align: center; }
.right { text-align: right; }
.small { font-size: .8em; color: var(--mz-muted); }

/* Title slide */
section.slide:has(.title-slide) .grid {
  place-items: center; text-align: center;
  grid-template: 1fr / 1fr;
}
section.slide:has(.title-slide) .pane { overflow: visible; }
.title-slide { font-size: 3.4em; border: none; }
section.slide:has(.title-slide) p { color: var(--mz-muted); font-size: 1.5em; }

/* Math: converted to MathML at build time and drawn natively by the browser */
math { font-size: 1.15em; }
math[display="block"] { font-size: 1.35em; margin: .5em 0; }
/* Fallback when conversion fails: show the TeX source */
.math-error {
  font-family: 'SF Mono', Consolas, monospace; font-style: normal;
  background: #fff0f0; color: #b3261e; border-radius: 4px; padding: 0 .3em;
}
.math-block { display: block; text-align: center; margin: .6em 0; padding: .4em; }

/* Tables */
table { border-collapse: collapse; font-size: 1.15em; margin: .5em 0; }
th, td { border: 1px solid var(--mz-border); padding: .35em .8em; }
th { background: #f3f5fb; }

/* Code */
pre {
  background: #f6f8fa; border: 1px solid var(--mz-border); border-radius: 8px;
  padding: .8em 1em; font-size: 1.05em; overflow: auto;
}
code { font-family: 'SF Mono', Consolas, Menlo, monospace; }
p code, li code { background: #f3f5fb; border-radius: 4px; padding: .1em .35em; font-size: .9em; }
blockquote { border-left: 4px solid var(--mz-accent2); margin: .5em 0; padding: .1em 1em; color: var(--mz-muted); }

img, video { max-width: 100%; max-height: 100%; }
video { background: #000; border-radius: 6px; }

/* Video placeholder in PDF output, when no poster is given */
.mz-video-still {
  display: flex; align-items: center; justify-content: center; gap: .6em;
  min-height: 200px; height: 100%;
  background: #f3f5fb; border: 2px dashed var(--mz-border); border-radius: 8px;
  color: var(--mz-muted);
}
.mz-video-still span { font-size: 2em; color: var(--mz-accent1); }
.mz-video-still em { font-style: normal; font-size: 1.1em; }

/* Charts (build-time SVG from data) */
:root {
  --mz-chart3: #f2994a; --mz-chart4: #9b51e0; --mz-chart5: #eb5757; --mz-chart6: #2d9cdb;
}
.mz-chart-wrap { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; }
svg.mz-chart { width: 100%; height: 100%; max-height: 100%; }
.mz-chart-title { font-size: 20px; font-weight: 600; fill: var(--mz-fg); }
.mz-chart-grid { stroke: var(--mz-border); stroke-width: 1; }
.mz-chart-tick { font-size: 15px; fill: var(--mz-muted); }
.mz-chart-axis { font-size: 14px; fill: var(--mz-muted); }
.mz-chart-legend { font-size: 15px; fill: var(--mz-fg); }
.mz-chart-value { font-size: 14px; fill: var(--mz-muted); }
.mz-chart-slice { stroke: var(--mz-slide-bg); stroke-width: 2; }

/* Shape layer (build-time SVG) and connector layer (drawn at runtime) */
:root { --mz-shape-fill: #eef1fb; }
svg.mz-shapes, svg.mz-connect {
  position: absolute; inset: 0; width: 100%; height: 100%;
  pointer-events: none; overflow: visible;
}
.mz-shape-label { font-size: 22px; fill: var(--mz-fg); font-family: inherit; }
.mz-shape-label.small, text.small { font-size: 16px; fill: var(--mz-muted); }

/* Blocks reserved for a later phase */
.mz-reserved {
  position: absolute; right: 14px; bottom: 12px;
  font-size: 12px; color: var(--mz-muted); opacity: .75;
  max-width: 40%;
}
.mz-reserved summary { cursor: pointer; }
.mz-reserved pre { font-size: 11px; margin: 4px 0 0; padding: .4em .6em; }

/* Parse error display */
.mz-error {
  border: 2px solid #e5484d; background: #fff0f0; color: #b3261e;
  border-radius: 8px; padding: .6em 1em; font-size: 1em; margin-bottom: 1em;
}

aside.notes { display: none; }

/* HUD */
#hud {
  position: fixed; right: 18px; bottom: 12px;
  color: #9aa1b8; font-size: 14px; user-select: none;
}
#hint {
  position: fixed; left: 18px; bottom: 12px;
  color: #5c6378; font-size: 12px; user-select: none;
}
#notes-panel {
  position: fixed; left: 0; right: 0; bottom: 0;
  background: rgba(20, 22, 29, .95); color: #dde1ee;
  padding: 16px 24px 40px; font-size: 15px; line-height: 1.7;
  border-top: 2px solid var(--mz-accent2);
  max-height: 40%; overflow: auto;
}
#notes-panel h4 { margin: 0 0 6px; color: var(--mz-accent2); font-size: 12px; letter-spacing: .1em; }
#notes-panel[hidden] { display: none; }
"#;

pub const VIEWER_JS: &str = r#"
(() => {
  const deck = document.getElementById('deck');
  const hud = document.getElementById('hud');
  const notesPanel = document.getElementById('notes-panel');
  const W = +deck.dataset.slideW, H = +deck.dataset.slideH;
  // Live updates replace DOM nodes, so re-query the slide list each time.
  const slides = () => Array.from(document.querySelectorAll('section.slide'));
  let cur = Math.min(Math.max(parseInt((location.hash || '').slice(1)) || 1, 1), slides().length) - 1;

  function fit() {
    const s = Math.min(innerWidth / (W + 40), innerHeight / (H + 40));
    deck.style.width = W + 'px';
    deck.style.height = H + 'px';
    deck.style.transform = `translate(-50%, -50%) scale(${s})`;
  }

  function show(i) {
    const ss = slides();
    cur = Math.min(Math.max(i, 0), ss.length - 1);
    ss.forEach((s, j) => s.classList.toggle('active', j === cur));
    hud.textContent = `${cur + 1} / ${ss.length}`;
    // replaceState throws inside srcdoc iframes (editor previews). Recording the
    // page position is optional, so a failure must not abort the rest of the update.
    try { history.replaceState(null, '', '#' + (cur + 1)); } catch (e) {}
    renderNotes();
    // Connectors resolve their endpoints once layout has settled.
    requestAnimationFrame(() => drawConnectors(ss[cur]));
  }

  // ---- Connector drawing: this is what makes connectors follow the layout ----
  // The data-connectors declarations are re-routed from the live layout every time.
  const NS = 'http://www.w3.org/2000/svg';
  function drawConnectors(sec) {
    if (!sec) return;
    let svg = sec.querySelector('svg.mz-connect');
    if (!sec.dataset.connectors) { if (svg) svg.remove(); return; }
    let conns;
    try { conns = JSON.parse(sec.dataset.connectors); } catch { return; }
    if (!svg) {
      svg = document.createElementNS(NS, 'svg');
      svg.setAttribute('class', 'mz-connect');
      svg.setAttribute('viewBox', `0 0 ${W} ${H}`);
      svg.setAttribute('preserveAspectRatio', 'none');
      sec.appendChild(svg);
    }
    const secRect = sec.getBoundingClientRect();
    if (secRect.width === 0) return;
    const sx = W / secRect.width, sy = H / secRect.height;
    // Convert an element's rect into the slide's logical coordinate system.
    const box = (id) => {
      const el = sec.querySelector('#' + CSS.escape(id));
      if (!el) return null;
      // A span wrapped across lines reports a union rect covering both lines,
      // which is not where the text is. Use the last client rect instead.
      const rects = el.getClientRects();
      const r = rects.length ? rects[rects.length - 1] : el.getBoundingClientRect();
      const inline = getComputedStyle(el).display.startsWith('inline');
      return {
        x: (r.left - secRect.left) * sx, y: (r.top - secRect.top) * sy,
        w: r.width * sx, h: r.height * sy, inline,
      };
    };
    const edgePt = (b, e) => ({
      n: { x: b.x + b.w / 2, y: b.y },
      s: { x: b.x + b.w / 2, y: b.y + b.h },
      e: { x: b.x + b.w, y: b.y + b.h / 2 },
      w: { x: b.x, y: b.y + b.h / 2 },
      c: { x: b.x + b.w / 2, y: b.y + b.h / 2 },
    })[e];
    let out = '';
    for (const c of conns) {
      const a = box(c.from), b = box(c.to);
      if (!a || !b) continue;
      // Without an explicit edge, pick the natural one from relative position.
      const dx = (b.x + b.w / 2) - (a.x + a.w / 2);
      const dy = (b.y + b.h / 2) - (a.y + a.h / 2);
      const horiz = Math.abs(dx) > Math.abs(dy);
      // Inline anchors leave from the horizontal centre of their underline, on
      // whichever side faces the target. Leaving sideways would run the line
      // straight through the sentence it is anchored to.
      const ae = c.fromEdge || (a.inline ? (dy < 0 ? 'n' : 's')
                                         : (horiz ? (dx > 0 ? 'e' : 'w') : (dy > 0 ? 's' : 'n')));
      const be = c.toEdge || (horiz ? (dx > 0 ? 'w' : 'e') : (dy > 0 ? 'n' : 's'));
      const p = edgePt(a, ae), q = edgePt(b, be);
      // Step clear of the text before curving, so the line leaves cleanly.
      if (a.inline && !c.fromEdge) p.y += ae === 'n' ? -6 : 6;
      const color = c.color || 'var(--mz-accent1)';
      const dash = c.dashed ? ' stroke-dasharray="8 6"' : '';
      // Leave and arrive along the edge normals. A curve that ignores the exit
      // direction swings back across the text it is anchored to.
      const dir = { n: [0, -1], s: [0, 1], e: [1, 0], w: [-1, 0], c: [0, 0] };
      const [ax, ay] = dir[ae], [bx, by] = dir[be];
      const span = Math.hypot(q.x - p.x, q.y - p.y);
      const k = c.curve == null ? 0.45 : c.curve;
      const d = Math.max(40, span * k);
      const c1 = { x: p.x + ax * d, y: p.y + ay * d };
      const c2 = { x: q.x + bx * d, y: q.y + by * d };
      out += `<path d="M ${p.x} ${p.y} C ${c1.x} ${c1.y} ${c2.x} ${c2.y} ${q.x} ${q.y}" fill="none" stroke="${color}" stroke-width="2.5"${dash}/>`;
      const head = (tip, from) => {
        const ang = Math.atan2(tip.y - from.y, tip.x - from.x);
        const L = 12, S = 0.45;
        return `<polygon points="${tip.x},${tip.y} ${tip.x - L * Math.cos(ang - S)},${tip.y - L * Math.sin(ang - S)} ${tip.x - L * Math.cos(ang + S)},${tip.y - L * Math.sin(ang + S)}" fill="${color}"/>`;
      };
      if (c.arrow === 'end' || c.arrow === 'both') out += head(q, c2);
      if (c.arrow === 'both') out += head(p, c1);
    }
    svg.innerHTML = out;
  }

  function renderNotes() {
    const notes = slides()[cur]?.querySelector('aside.notes');
    notesPanel.innerHTML = '<h4>SPEAKER NOTES</h4>' +
      (notes && notes.innerHTML.trim() ? notes.innerHTML : '<em>(no notes for this slide)</em>');
  }

  // Restore the current page after a live update.
  window.__mirzamRefresh = () => show(cur);
  // Let a host (editor extension) jump to a specific slide.
  window.__mirzamGoto = (i) => show(i);

  addEventListener('keydown', (e) => {
    if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') { e.preventDefault(); show(cur + 1); }
    else if (e.key === 'ArrowLeft' || e.key === 'PageUp') { e.preventDefault(); show(cur - 1); }
    else if (e.key === 'Home') show(0);
    else if (e.key === 'End') show(slides.length - 1);
    else if (e.key === 'n' || e.key === 'N') notesPanel.hidden = !notesPanel.hidden;
    else if (e.key === 'f' || e.key === 'F') {
      document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen();
    }
  });

  // Click to advance, but never while the reader is selecting text or using a
  // control: a drag that ends on the deck is a selection, not a page turn.
  let downAt = null;
  deck.addEventListener('pointerdown', (e) => { downAt = { x: e.clientX, y: e.clientY }; });
  deck.addEventListener('click', (e) => {
    const moved = downAt
      ? Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) > 6
      : false;
    downAt = null;
    if (moved) return;
    if ((getSelection()?.toString() || '').trim()) return;
    if (e.target.closest('a, video, details, summary, button, input')) return;
    const r = deck.getBoundingClientRect();
    (e.clientX - r.left) / r.width < 0.3 ? show(cur - 1) : show(cur + 1);
  });

  addEventListener('resize', () => { fit(); show(cur); });
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(() => show(cur));
  fit();
  show(cur);
})();
"#;

/// `@font-face` CSS embedding STIX Two Math (OFL, see assets/STIX-LICENSE.txt)
/// as a data URI. Added only to pages containing math (~540KB), so decks
/// render at TeX quality even on machines without a math font installed.
pub fn math_font_css() -> &'static str {
    use base64::Engine as _;
    use std::sync::OnceLock;
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let woff2 = include_bytes!("../assets/stix-two-math.woff2");
        let b64 = base64::engine::general_purpose::STANDARD.encode(woff2);
        format!(
            "@font-face {{ font-family: 'STIX Two Math'; \
             src: url(data:font/woff2;base64,{b64}) format('woff2'); font-display: swap; }}\n\
             math {{ font-family: 'STIX Two Math', math; }}"
        )
    })
}

/// Print overrides applied after DEFAULT_CSS.
/// Slide dimensions and the `@page` size are appended by `assemble_print_page`.
pub const PRINT_CSS: &str = r#"
html, body { background: #fff; overflow: visible; height: auto; }
#deck {
  position: static; transform: none; width: auto; height: auto;
  box-shadow: none; border-radius: 0; background: transparent;
}
section.slide {
  position: relative; display: block; overflow: hidden;
  background: var(--mz-slide-bg);
  break-after: page; page-break-after: always;
}
.mz-reserved { display: none; }
"#;

/// Hot-reload client injected in `serve` mode.
/// Long-polls for changed `<section>` HTML and patches the DOM.
pub const LIVE_JS: &str = r#"
(async () => {
  let v = window.__MIRZAM_V__;
  while (true) {
    try {
      const res = await fetch('/events?v=' + v);
      const j = await res.json();
      if (j.v === v) continue;
      v = j.v;
      if (j.full) { location.reload(); return; }
      for (const [i, html] of j.changes) {
        const sec = document.querySelector(`section.slide[data-index="${i}"]`);
        if (sec) sec.outerHTML = html;
      }
      if (window.__mirzamRefresh) window.__mirzamRefresh();
    } catch (e) {
      await new Promise(r => setTimeout(r, 1000));
    }
  }
})();
"#;
