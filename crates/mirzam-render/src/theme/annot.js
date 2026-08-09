// The annotation overlay: circles, boxes, arrows and labels drawn over an
// image, a chart or any pane. Inlined only into decks that annotate
// something - including the print page, so the PDF carries them too.
//
// Everything here is *additive*. An annotation never hides content, so a deck
// read without JavaScript is the deck minus its annotations, not a deck with
// something missing from the middle of it. That is why this file may run in
// print where the viewer may not.
//
// Coordinates are percentages of the target's *painted* box, and anchored
// items carry no coordinates at all - they take the live bounding box of the
// element they name. Both are resolved here rather than at build time,
// because neither is known until the browser has laid the slide out.
(() => {
  const NS = 'http://www.w3.org/2000/svg';
  const svgEl = (name, attrs) => {
    const el = document.createElementNS(NS, name);
    for (const k in attrs) el.setAttribute(k, attrs[k]);
    return el;
  };

  // The deck is displayed through a CSS `scale()`, so a client rectangle is
  // in screen pixels while everything written back - the SVG viewBox, a
  // label's `left` - is in the slide's own untransformed pixels. `k` is the
  // ratio between them, and every measurement below is divided by it.
  function metrics(sec) {
    const r = sec.getBoundingClientRect();
    return { left: r.left, top: r.top, k: r.width ? r.width / sec.offsetWidth : 1 };
  }

  const PICTURES = 'img,video,canvas,svg';

  // What a target actually paints. `target: fig` names a pane, but a pane
  // holding one picture is a way of saying "that picture": annotating the
  // pane's box would measure its padding and whatever whitespace the
  // alignment left, and put every mark somewhere the author did not point.
  function paintTarget(el) {
    if (el.matches(PICTURES)) return el;
    const inner = el.querySelectorAll(PICTURES);
    return inner.length === 1 ? inner[0] : el;
  }

  // A picture's natural size, and how it is fitted into its element box.
  // `<img>` and `<video>` carry both; an `<svg>` has a viewBox for the first
  // and `preserveAspectRatio` for the second, which is the same rule under a
  // different name.
  function fitting(el) {
    if (el.tagName === 'IMG') {
      return { w: el.naturalWidth, h: el.naturalHeight, fit: getComputedStyle(el).objectFit };
    }
    if (el.tagName === 'VIDEO') {
      return { w: el.videoWidth, h: el.videoHeight, fit: getComputedStyle(el).objectFit };
    }
    if (el.viewBox && el.viewBox.baseVal && el.viewBox.baseVal.width) {
      const par = (el.getAttribute('preserveAspectRatio') || 'xMidYMid meet').trim();
      const fit = par.startsWith('none') ? 'fill' : (par.endsWith('slice') ? 'cover' : 'contain');
      return { w: el.viewBox.baseVal.width, h: el.viewBox.baseVal.height, fit };
    }
    return null;
  }

  // The box the target actually paints, in the slide's coordinates. A picture
  // fitted with `contain` (or an SVG's `meet`) is smaller than its element and
  // centred in it; annotating the element box would be wrong by exactly the
  // letterboxing, which is why this is not just `getBoundingClientRect`.
  function paintedBox(el, m) {
    const r = el.getBoundingClientRect();
    let w = r.width / m.k, h = r.height / m.k;
    let dx = 0, dy = 0;
    const nat = fitting(el);
    if (nat && nat.w && nat.h && w && h && nat.fit !== 'fill') {
      const scale = nat.fit === 'cover' ? Math.max(w / nat.w, h / nat.h)
                  : nat.fit === 'none' ? 1
                  : nat.fit === 'scale-down' ? Math.min(1, Math.min(w / nat.w, h / nat.h))
                  : Math.min(w / nat.w, h / nat.h);   // contain, and the default
      // `cover` paints larger than the element and is clipped back to it, so
      // what the audience sees is still the element box.
      const pw = Math.min(nat.w * scale, w);
      const ph = Math.min(nat.h * scale, h);
      dx = (w - pw) / 2; dy = (h - ph) / 2;
      w = pw; h = ph;
    }
    return { x: (r.left - m.left) / m.k + dx, y: (r.top - m.top) / m.k + dy, w, h };
  }

  const rectIn = (el, m) => {
    const r = el.getBoundingClientRect();
    return {
      x: (r.left - m.left) / m.k, y: (r.top - m.top) / m.k,
      w: r.width / m.k, h: r.height / m.k,
    };
  };

  // Where an item sits, in slide pixels. Either a percentage of the target
  // box, or the live box of the element it anchors to.
  function place(item, key, box, sec, m) {
    const id = key === 'to' ? item.anchor2 : item.anchor;
    if (id) {
      const el = sec.querySelector('#' + CSS.escape(id));
      if (!el) return null;
      const r = rectIn(el, m);
      const pad = item.pad || 0;
      return { x: r.x - pad, y: r.y - pad, w: r.w + pad * 2, h: r.h + pad * 2 };
    }
    const x = key === 'to' ? item.x2 : item.x;
    const y = key === 'to' ? item.y2 : item.y;
    if (x == null || y == null) return null;
    const p = { x: box.x + (x / 100) * box.w, y: box.y + (y / 100) * box.h, w: 0, h: 0 };
    if (key !== 'to' && item.w != null) {
      p.w = (item.w / 100) * box.w;
      p.h = (item.h / 100) * box.h;
      // A coordinate pair names the shape's centre, the way `shape` does: it
      // is where you point, not a corner you have to compute.
      p.x -= p.w / 2; p.y -= p.h / 2;
    }
    return p;
  }

  const mid = (b) => ({ x: b.x + b.w / 2, y: b.y + b.h / 2 });

  // Trims an arrow so it stops at the edge of the box it points at rather
  // than in the middle of it, when it points at an anchored element.
  function edgeOf(box, from) {
    const c = mid(box);
    if (!box.w && !box.h) return c;
    const dx = c.x - from.x, dy = c.y - from.y;
    if (!dx && !dy) return c;
    const sx = dx ? (box.w / 2) / Math.abs(dx) : Infinity;
    const sy = dy ? (box.h / 2) / Math.abs(dy) : Infinity;
    const t = Math.min(sx, sy);
    return { x: c.x - dx * t, y: c.y - dy * t };
  }

  function draw(overlay, items, box, sec, m, step) {
    const svg = overlay.firstChild;
    svg.setAttribute('viewBox', `0 0 ${sec.offsetWidth} ${sec.offsetHeight}`);
    while (svg.firstChild) svg.removeChild(svg.firstChild);
    overlay.querySelectorAll('.mz-annot-label').forEach((n) => n.remove());

    for (const item of items) {
      if ((item.step || 0) > step) continue;
      const a = place(item, 'from', box, sec, m);
      if (!a) continue;
      const color = item.color || 'var(--mz-accent1)';
      const dash = item.dashed ? '7 6' : null;
      const common = { stroke: color, fill: 'none', 'stroke-width': 3,
                       'vector-effect': 'non-scaling-stroke' };
      if (dash) common['stroke-dasharray'] = dash;
      let labelAt = { x: a.x + a.w / 2, y: a.y };

      if (item.kind === 'rect') {
        svg.appendChild(svgEl('rect', { ...common, x: a.x, y: a.y, width: a.w, height: a.h, rx: 6 }));
      } else if (item.kind === 'circle') {
        const c = mid(a);
        svg.appendChild(svgEl('ellipse', { ...common, cx: c.x, cy: c.y, rx: a.w / 2, ry: a.h / 2 }));
      } else if (item.kind === 'arrow') {
        const bBox = place(item, 'to', box, sec, m);
        if (!bBox) continue;
        const from = mid(a);
        const to = edgeOf(bBox, from);
        svg.appendChild(svgEl('line', { ...common, x1: from.x, y1: from.y, x2: to.x, y2: to.y }));
        const ang = Math.atan2(to.y - from.y, to.x - from.x);
        const L = 13, S = 0.45;
        const pts = [
          `${to.x},${to.y}`,
          `${to.x - L * Math.cos(ang - S)},${to.y - L * Math.sin(ang - S)}`,
          `${to.x - L * Math.cos(ang + S)},${to.y - L * Math.sin(ang + S)}`,
        ].join(' ');
        svg.appendChild(svgEl('polygon', { points: pts, fill: color }));
        labelAt = { x: from.x, y: from.y };
      } else if (item.kind === 'text') {
        labelAt = { x: a.x, y: a.y };
      }

      if (item.label) {
        const tag = document.createElement('span');
        tag.className = 'mz-annot-label' + (item.kind === 'text' ? ' mz-annot-text' : '');
        tag.textContent = item.label;
        if (item.color) tag.style.color = item.color;
        tag.style.left = labelAt.x + 'px';
        tag.style.top = labelAt.y + 'px';
        // A shape's label sits above it; a bare text annotation sits where it
        // was placed, since there is nothing for it to get out of the way of.
        if (item.kind !== 'text') tag.classList.add('mz-annot-above');
        overlay.appendChild(tag);
      }
    }
  }

  function mount(script) {
    const sec = script.closest('section.slide');
    if (!sec) return null;
    let items;
    try { items = JSON.parse(script.textContent).items; } catch (e) { return null; }
    if (!items || !items.length) return null;
    const named = sec.querySelector(script.dataset.target);
    if (!named) return null;
    const target = paintTarget(named);

    const overlay = document.createElement('div');
    overlay.className = 'mz-annot-layer';
    overlay.appendChild(svgEl('svg', { preserveAspectRatio: 'none' }));
    sec.appendChild(overlay);
    return { sec, target, items, overlay };
  }

  const layers = [];

  // How far through the slide's clicks we are. `Infinity` until a viewer says
  // otherwise, so a page with no viewer — the PDF export above all — shows
  // every mark. An annotation waits for a click; it does not depend on one.
  const steps = new WeakMap();
  const stepOn = (sec) => (steps.has(sec) ? steps.get(sec) : Infinity);

  function refresh(only) {
    for (const l of layers) {
      if (only && l.sec !== only) continue;
      const m = metrics(l.sec);
      draw(l.overlay, l.items, paintedBox(l.target, m), l.sec, m, stepOn(l.sec));
    }
  }

  function init() {
    for (const script of document.querySelectorAll('script.mz-annot')) {
      const l = mount(script);
      if (l) layers.push(l);
    }
    if (!layers.length) return;
    refresh();
    // The overlay is measured from the laid-out page, so anything that changes
    // the layout has to re-measure it: a resize, a font arriving, an image
    // decoding, or a live-reload patch.
    addEventListener('resize', refresh);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(refresh);
    for (const l of layers) {
      if (l.target.tagName === 'IMG' && !l.target.complete) l.target.addEventListener('load', refresh);
      new ResizeObserver(refresh).observe(l.target);
    }
    window.__mirzamAnnot = refresh;
  }

  // What the viewer talks to. Present from the moment this file runs, because
  // the viewer asks for a slide's step count before the overlays are mounted.
  window.MZAnnot = {
    // The highest click any of this slide's annotations waits for, so the
    // viewer knows to keep stepping before it turns the page.
    steps(sec) {
      let n = 0;
      for (const tag of sec.querySelectorAll(':scope > script.mz-annot')) {
        try {
          for (const item of JSON.parse(tag.textContent).items || []) {
            n = Math.max(n, item.step || 0);
          }
        } catch (e) { /* a malformed block simply adds no steps */ }
      }
      return n;
    },

    show(sec, step) {
      if (steps.get(sec) === step) return;
      steps.set(sec, step);
      refresh(sec);
    },
  };

  if (document.readyState === 'loading') addEventListener('DOMContentLoaded', init);
  else init();
})();
