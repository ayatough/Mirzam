// The connector layer: a curve between two anchors on a slide, routed from the
// laid-out page. Inlined only into decks that connect something - including
// the print, handout and shot pages, so the PDF and the PowerPoint file carry
// the arrows too.
//
// This is the same reasoning `annot.js` is built on, and the same promise. A
// connector is drawn *over* the slide and hides nothing, so a deck read
// without JavaScript is the deck minus its arrows, not a deck with something
// missing from the middle of it. That is why this file may run in print where
// the viewer may not - and why it may never reach for the viewer, the active
// slide or the animation runtime.
//
// The endpoints are element ids and the route is computed from live bounding
// boxes, neither of which exists until the browser has laid the slide out. So
// the geometry is resolved here rather than at build time, which is also what
// makes an arrow follow a chart's last data point instead of a screenshot of
// where it used to be.
(() => {
  const NS = 'http://www.w3.org/2000/svg';

  // Every slide that declares connectors, whichever page shape it is on: the
  // viewer shows one at a time, the print page lays them all out at once.
  const targets = () =>
    Array.from(document.querySelectorAll('section.slide[data-connectors]'));

  // Route one slide's connectors, replacing whatever was drawn before.
  //
  // The slide is displayed through a CSS `scale()` - by the viewer to fit the
  // screen, by the handout to fit the page - so a client rectangle is in
  // screen pixels while the SVG's viewBox is in the slide's own untransformed
  // pixels. `sx`/`sy` are the ratio between them, taken from the section's own
  // offset size: the same measurement `annot.js` makes, and the reason neither
  // file needs `#deck`'s declared dimensions.
  function draw(sec) {
    if (!sec) return;
    let svg = sec.querySelector('svg.mz-connect');
    if (!sec.dataset.connectors) { if (svg) svg.remove(); return; }
    let conns;
    try { conns = JSON.parse(sec.dataset.connectors); } catch (e) { return; }
    const secRect = sec.getBoundingClientRect();
    // A slide with no box is a slide the viewer is not showing. Measured now,
    // before an empty layer is appended to it: the print page draws every
    // slide, so a hidden one must cost nothing rather than an unused node.
    if (secRect.width === 0 || secRect.height === 0) return;
    const W = sec.offsetWidth, H = sec.offsetHeight;
    if (!W || !H) return;
    if (!svg) {
      svg = document.createElementNS(NS, 'svg');
      svg.setAttribute('class', 'mz-connect');
      svg.setAttribute('viewBox', `0 0 ${W} ${H}`);
      svg.setAttribute('preserveAspectRatio', 'none');
      sec.appendChild(svg);
    }
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

  const drawAll = () => { for (const sec of targets()) draw(sec); };

  // What the viewer and the annotation overlay talk to. Present from the
  // moment this file runs: the viewer routes the slide it is showing, the
  // overlay asks for a redraw once its marks exist, and the layout checker
  // drives it directly rather than waiting for a frame it cannot see.
  window.MZConnect = { draw, drawAll };

  // The hook `annot.js` and `check.js` already call, kept under the name they
  // call it by. With no argument it routes every slide that has connectors,
  // which on the viewer's page costs nothing for the slides it is not showing:
  // a hidden slide has no box, and `draw` measures before it touches anything.
  window.__mirzamConnectors = (sec) => (sec ? draw(sec) : drawAll());

  // Routing reads the laid-out page, so it runs once layout has settled rather
  // than on a listener: a resize only rescales the slide, and the route is
  // already in the slide's own coordinates. A face arriving late is the one
  // thing that genuinely moves an anchor, so the fonts are waited for.
  function init() {
    if (!targets().length) return;
    requestAnimationFrame(drawAll);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(drawAll);
  }

  if (document.readyState === 'loading') addEventListener('DOMContentLoaded', init);
  else init();
})();
