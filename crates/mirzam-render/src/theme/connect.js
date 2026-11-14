// The connector layer: the arrows `connect` blocks declare, routed from the
// live layout. Inlined only into decks that declare one - and, like annot.js,
// into the print, handout and shot pages as well, so the PDF and the PowerPoint
// file carry the arrows the deck exists to draw.
//
// An arrow is drawn over the slide and hides nothing, so a scriptless read is
// the deck minus its arrows, not a deck with something missing from the middle
// of it. That is what lets this run where the viewer may not.
//
// It stands alone: no active slide, no page counter, no animation runtime. A
// slide that is not laid out (the viewer keeps all but one `display: none`)
// is skipped, and drawn the next time it is shown.
(() => {
  const NS = 'http://www.w3.org/2000/svg';

  function draw(sec) {
    if (!sec) return;
    let svg = sec.querySelector('svg.mz-connect');
    if (!sec.dataset.connectors) { if (svg) svg.remove(); return; }
    let conns;
    try { conns = JSON.parse(sec.dataset.connectors); } catch { return; }
    // The deck is displayed through a CSS `scale()`: client rectangles are in
    // screen pixels, the viewBox is in the slide's own, and `sx`/`sy` is the
    // ratio between them.
    const secRect = sec.getBoundingClientRect();
    if (secRect.width === 0 || !sec.offsetWidth) return;
    const W = sec.offsetWidth, H = sec.offsetHeight;
    if (!svg) {
      svg = document.createElementNS(NS, 'svg');
      svg.setAttribute('class', 'mz-connect');
      svg.setAttribute('preserveAspectRatio', 'none');
      sec.appendChild(svg);
    }
    svg.setAttribute('viewBox', `0 0 ${W} ${H}`);
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

  // With a slide, route that slide's connectors; without one, every slide that
  // is laid out. The viewer asks for the slide it just showed, the annotation
  // overlay and the checker for "whatever is on screen", and a page with no
  // viewer has every slide on screen at once.
  const route = (sec) => {
    if (sec) { draw(sec); return; }
    for (const s of document.querySelectorAll('section.slide[data-connectors]')) draw(s);
  };
  window.__mirzamConnectors = route;

  // Routed from the laid-out page, so anything that changes the layout has to
  // route again: a resize and a font arriving. Run once now as well - on a page
  // with no viewer nothing else will ask.
  route();
  addEventListener('resize', () => route());
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(() => route());
})();
