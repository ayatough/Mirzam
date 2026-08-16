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

  // The line boxes a phrase occupies, in slide pixels. A sentence that wraps
  // gives an element *two* rectangles, and `getBoundingClientRect` returns the
  // union of them - a box that also covers the end of one line and the start of
  // the next, which is not where the words are. Rows within half a pixel of
  // each other are merged, since a phrase carrying `<strong>` reports one
  // rectangle per run rather than per line.
  function lineRects(el, m) {
    const raw = Array.from(el.getClientRects()).filter((r) => r.width || r.height);
    const rects = raw.length ? raw : [el.getBoundingClientRect()];
    const rows = [];
    for (const r of rects) {
      const row = rows.find((o) => Math.abs(o.top - r.top) < 0.5 && Math.abs(o.bottom - r.bottom) < 0.5);
      if (row) {
        row.left = Math.min(row.left, r.left);
        row.right = Math.max(row.right, r.right);
      } else {
        rows.push({ top: r.top, bottom: r.bottom, left: r.left, right: r.right });
      }
    }
    return rows.map((r) => ({
      x: (r.left - m.left) / m.k, y: (r.top - m.top) / m.k,
      w: (r.right - r.left) / m.k, h: (r.bottom - r.top) / m.k,
    }));
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

  // Returns the number of items that were due by `step` but could not be
  // placed — an anchor that is not on the slide, a phrase with no line boxes.
  // A dropped mark is silent by design (a stale annotation must never take a
  // deck down), so something has to count them for `check-layout.mjs`.
  function draw(overlay, items, box, sec, m, step) {
    let missed = 0;
    const svg = overlay.firstChild;
    svg.setAttribute('viewBox', `0 0 ${sec.offsetWidth} ${sec.offsetHeight}`);
    while (svg.firstChild) svg.removeChild(svg.firstChild);
    overlay.querySelectorAll('.mz-annot-label').forEach((n) => n.remove());

    for (const item of items) {
      if ((item.step || 0) > step) continue;
      const a = place(item, 'from', box, sec, m);
      if (!a) { missed++; continue; }
      const color = item.color || 'var(--mz-accent1)';
      const dash = item.dashed ? '7 6' : null;
      const common = { stroke: color, fill: 'none', 'stroke-width': 3,
                       'vector-effect': 'non-scaling-stroke' };
      if (dash) common['stroke-dasharray'] = dash;
      // An id put on the drawn shape lets the rest of the deck point at it —
      // a `connect` arrow from a sentence to the circle, say. It is the only
      // way to name something that does not exist until the page is laid out.
      if (item.id) common.id = item.id;
      let labelAt = { x: a.x + a.w / 2, y: a.y };

      if (item.kind === 'highlight' || item.kind === 'underline' || item.kind === 'box') {
        const el = item.anchor && sec.querySelector('#' + CSS.escape(item.anchor));
        if (!el) { missed++; continue; }
        const pad = item.pad || 0;
        const rows = lineRects(el, m);
        if (!rows.length) { missed++; continue; }
        for (const r of rows) {
          if (item.kind === 'highlight') {
            // A wash rather than an outline: the words stay the thing being
            // read, and the colour only says which words.
            svg.appendChild(svgEl('rect', {
              x: r.x - 2, y: r.y - 1, width: r.w + 4, height: r.h + 2, rx: 3,
              fill: color, 'fill-opacity': 0.24, stroke: 'none',
              ...(item.id && rows.length === 1 ? { id: item.id } : {}),
            }));
          } else if (item.kind === 'underline') {
            const yy = r.y + r.h - 1;
            svg.appendChild(svgEl('line', {
              ...common, x1: r.x, y1: yy, x2: r.x + r.w, y2: yy, 'stroke-linecap': 'round',
            }));
          } else {
            svg.appendChild(svgEl('rect', {
              ...common, x: r.x - pad, y: r.y - pad,
              width: r.w + pad * 2, height: r.h + pad * 2, rx: 5,
            }));
          }
        }
        // A label belongs over the first line, not over the union of them.
        labelAt = { x: rows[0].x + rows[0].w / 2, y: rows[0].y - (item.pad || 0) };
        if (item.label) {
          const tag = document.createElement('span');
          tag.className = 'mz-annot-label mz-annot-above';
          tag.textContent = item.label;
          if (item.color) tag.style.color = item.color;
          tag.style.left = labelAt.x + 'px';
          tag.style.top = labelAt.y + 'px';
          overlay.appendChild(tag);
        }
        continue;
      }

      if (item.kind === 'rect') {
        svg.appendChild(svgEl('rect', { ...common, x: a.x, y: a.y, width: a.w, height: a.h, rx: 6 }));
      } else if (item.kind === 'circle') {
        const c = mid(a);
        svg.appendChild(svgEl('ellipse', { ...common, cx: c.x, cy: c.y, rx: a.w / 2, ry: a.h / 2 }));
      } else if (item.kind === 'arrow') {
        const bBox = place(item, 'to', box, sec, m);
        if (!bBox) { missed++; continue; }
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
    return missed;
  }

  function mount(script) {
    const sec = script.closest('section.slide');
    if (!sec) return null;
    let items;
    try { items = JSON.parse(script.textContent).items; } catch (e) { return null; }
    if (!items || !items.length) return null;
    // `:scope` is what the renderer writes for a block that names no target,
    // because every one of its items is anchored to an element it finds for
    // itself. The overlay then covers the slide, and `paintTarget` must not
    // narrow it to a picture that happens to be the only one there.
    const scoped = script.dataset.target === ':scope';
    const named = scoped ? sec : sec.querySelector(script.dataset.target);
    if (!named) return null;
    const target = scoped ? sec : paintTarget(named);

    const overlay = document.createElement('div');
    overlay.className = 'mz-annot-layer';
    overlay.appendChild(svgEl('svg', { preserveAspectRatio: 'none' }));
    sec.appendChild(overlay);
    return { script, sec, target, items, overlay };
  }

  const layers = [];

  // Anything that changes a layer's geometry has to re-measure it. Attached
  // per layer rather than once, because a layer mounted by a live-reload patch
  // needs the same wiring the first paint gave the others.
  function watch(l) {
    if (l.target.tagName === 'IMG' && !l.target.complete) l.target.addEventListener('load', refresh);
    new ResizeObserver(refresh).observe(l.target);
  }

  // A live-reload patch replaces a whole `<section>`, overlay and all, so the
  // layers held here can outlive the page they were measured against: the
  // marks would then be drawn into a detached overlay, and the slide the
  // author is looking at would lose every annotation on it until a full
  // reload. Reconciling at the top of every refresh is what makes one entry
  // point serve the first paint and every patch after it — a section that is
  // no longer in the document is dropped, and a block with no layer is
  // mounted, whether it is new or replaced.
  function sync() {
    for (let i = layers.length - 1; i >= 0; i--) {
      if (!layers[i].script.isConnected) layers.splice(i, 1);
    }
    for (const script of document.querySelectorAll('script.mz-annot')) {
      if (layers.some((l) => l.script === script)) continue;
      const l = mount(script);
      if (l) { layers.push(l); watch(l); }
    }
  }

  // How far through the slide's clicks we are. `Infinity` until a viewer says
  // otherwise, so a page with no viewer — the PDF export above all — shows
  // every mark. An annotation waits for a click; it does not depend on one.
  const steps = new WeakMap();
  const stepOn = (sec) => (steps.has(sec) ? steps.get(sec) : Infinity);

  function refresh(only) {
    sync();
    for (const l of layers) {
      if (only && l.sec !== only) continue;
      const m = metrics(l.sec);
      l.missed = draw(l.overlay, l.items, paintedBox(l.target, m), l.sec, m, stepOn(l.sec));
    }
    // A connector may point at a mark drawn here, and the marks are only laid
    // out now — so the connectors have to be re-routed after, not before.
    if (window.__mirzamConnectors) window.__mirzamConnectors();
  }

  function init() {
    // `refresh` mounts what it finds, so the first paint is the same code
    // path as every patch after it.
    refresh();
    if (!layers.length) return;
    // The overlay is measured from the laid-out page, so anything that changes
    // the layout has to re-measure it: a resize, a font arriving, an image
    // decoding, or a live-reload patch.
    addEventListener('resize', refresh);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(refresh);
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

    // How many of this slide's marks were due but could not be drawn.
    // `check-layout.mjs` gates on it, so a renamed id fails a build rather
    // than quietly removing the circle the sentence refers to.
    missing(sec) {
      let n = 0;
      for (const l of layers) if (l.sec === sec) n += l.missed || 0;
      return n;
    },
  };

  if (document.readyState === 'loading') addEventListener('DOMContentLoaded', init);
  else init();
})();
