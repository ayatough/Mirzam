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
        const pad = item.pad || 0;
        let rows;
        if (item.anchor) {
          const el = sec.querySelector('#' + CSS.escape(item.anchor));
          if (!el) { missed++; continue; }
          rows = lineRects(el, m);
        } else {
          // Words in a picture, placed by coordinates: `a` is already the
          // box, and it is one line - a passage cut out of a paper is marked
          // one item per line, the way `import pdf --quote` writes it.
          rows = [{ x: a.x, y: a.y, w: a.w, h: a.h }];
        }
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
    return { sec, target, items, overlay };
  }

  const layers = [];

  // ---- Cards: a picture the slide does not show, opened from a chip ----
  //
  // A block whose `target:` is an image file draws no overlay. The renderer
  // wrote a chip (`a.mz-chip`) after each phrase the block anchors to and one
  // hidden `aside.mz-card` per block, its marks already positioned in percent
  // of the picture. All that is left to do at run time is open the card
  // beside the chip, show the marks that belong to that chip, and close it.
  //
  // Hover opens, and leaving closes, with a short grace so a hand crossing a
  // chip on its way somewhere else does not flash a card. A click pins the
  // card - the presenter wants it to stay while they talk - and Escape, a
  // click anywhere else, or the next click step lets it go. On a touch screen
  // there is no hover, so the tap is the pin.
  const OPEN_AFTER = 110, CLOSE_AFTER = 180;
  const opened = new WeakMap();   // section -> { chip, pinned }
  let timer = null;

  const chipsIn = (sec) => Array.from(sec.querySelectorAll('a.mz-chip'));
  const cardOf = (chip) => {
    const sec = chip.closest('section.slide');
    return sec && chip.dataset.card ? sec.querySelector('#' + CSS.escape(chip.dataset.card)) : null;
  };
  const phraseOf = (chip) => {
    const sec = chip.closest('section.slide');
    return sec && chip.dataset.for ? sec.querySelector('#' + CSS.escape(chip.dataset.for)) : null;
  };

  // Where the card goes, in slide pixels: to the right of the chip when it
  // fits, else to the left of the phrase's first line, else under the chip.
  // Never past the slide's edge in either direction.
  function placeCard(chip, card) {
    const sec = chip.closest('section.slide');
    const m = metrics(sec);
    const c = rectIn(chip, m);
    const W = sec.offsetWidth, H = sec.offsetHeight;
    const margin = 24, gap = 18;
    const cw = card.offsetWidth, ch = card.offsetHeight;
    let left, top = c.y - 10;
    if (c.x + c.w + gap + cw <= W - margin) {
      left = c.x + c.w + gap;
    } else {
      const phrase = phraseOf(chip);
      const p = phrase ? lineRects(phrase, m) : [];
      const start = p.length ? Math.min(c.x, ...p.map((r) => r.x)) : c.x;
      if (start - gap - cw >= margin) {
        left = start - gap - cw;
      } else {
        left = Math.min(W - margin - cw, c.x + c.w - cw);
        top = c.y + c.h + gap;
      }
    }
    left = Math.max(margin, Math.min(left, W - margin - cw));
    top = Math.max(margin, Math.min(top, H - margin - ch));
    card.style.left = left + 'px';
    card.style.top = top + 'px';
  }

  function openCard(chip, pin) {
    const sec = chip.closest('section.slide');
    if (!sec) return;
    const card = cardOf(chip);
    if (!card) return;
    const state = opened.get(sec);
    if (state && state.chip !== chip) closeCard(sec, true);
    // The marks, the quote and the source line that belong to this chip.
    const group = chip.dataset.group || '';
    for (const el of card.querySelectorAll('[data-group]')) {
      el.hidden = el.dataset.group !== '' && el.dataset.group !== group;
    }
    const color = chip.style.getPropertyValue('--mz-chip');
    card.style.setProperty('--mz-chip', color);
    card.hidden = false;
    placeCard(chip, card);
    chip.classList.add('mz-chip-open');
    const phrase = phraseOf(chip);
    if (phrase) {
      phrase.classList.add('mz-chip-lit');
      phrase.style.setProperty('--mz-chip', color);
    }
    opened.set(sec, { chip, pinned: pin || (state && state.chip === chip && state.pinned) || false });
    // The picture may not have decoded when the card was measured; a card
    // placed for a picture of no height would be placed again when it has one.
    const img = card.querySelector('img');
    if (img && !img.complete) {
      img.addEventListener('load', () => { if (!card.hidden) placeCard(chip, card); }, { once: true });
    }
  }

  function closeCard(sec, force) {
    const state = opened.get(sec);
    if (!state || (state.pinned && !force)) return;
    const card = cardOf(state.chip);
    if (card) card.hidden = true;
    state.chip.classList.remove('mz-chip-open');
    const phrase = phraseOf(state.chip);
    if (phrase) phrase.classList.remove('mz-chip-lit');
    opened.delete(sec);
  }

  const later = (fn, ms) => { clearTimeout(timer); timer = setTimeout(fn, ms); };

  function wireCards() {
    if (!document.querySelector('a.mz-chip')) return;
    document.addEventListener('mouseover', (e) => {
      const chip = e.target.closest && e.target.closest('a.mz-chip');
      if (chip) { later(() => openCard(chip, false), OPEN_AFTER); return; }
      // Over an open card: reading it, not leaving it.
      if (e.target.closest && e.target.closest('aside.mz-card')) clearTimeout(timer);
    });
    document.addEventListener('mouseout', (e) => {
      const from = e.target.closest && (e.target.closest('a.mz-chip') || e.target.closest('aside.mz-card'));
      if (!from) return;
      const to = e.relatedTarget && e.relatedTarget.closest
        && (e.relatedTarget.closest('a.mz-chip') || e.relatedTarget.closest('aside.mz-card'));
      if (to) return;
      const sec = from.closest('section.slide');
      later(() => { if (sec) closeCard(sec, false); }, CLOSE_AFTER);
    });
    document.addEventListener('click', (e) => {
      const chip = e.target.closest && e.target.closest('a.mz-chip');
      if (chip) {
        e.preventDefault();
        e.stopPropagation();
        clearTimeout(timer);
        const sec = chip.closest('section.slide');
        const state = opened.get(sec);
        if (state && state.chip === chip && state.pinned) closeCard(sec, true);
        else openCard(chip, true);
        return;
      }
      if (e.target.closest && e.target.closest('aside.mz-card')) { e.stopPropagation(); return; }
      const sec = e.target.closest && e.target.closest('section.slide');
      if (sec && opened.get(sec)) { closeCard(sec, true); e.stopPropagation(); }
    }, true);
    document.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') return;
      for (const sec of document.querySelectorAll('section.slide')) {
        if (opened.get(sec)) { closeCard(sec, true); e.stopPropagation(); }
      }
    }, true);
    // Keyboard: a chip is a link, so it takes focus; focusing it is hovering.
    document.addEventListener('focusin', (e) => {
      const chip = e.target.closest && e.target.closest('a.mz-chip');
      if (chip) openCard(chip, false);
    });
    document.addEventListener('focusout', (e) => {
      const chip = e.target.closest && e.target.closest('a.mz-chip');
      if (chip) later(() => closeCard(chip.closest('section.slide'), false), CLOSE_AFTER);
    });
    addEventListener('resize', () => {
      for (const sec of document.querySelectorAll('section.slide')) {
        const state = opened.get(sec);
        if (state) placeCard(state.chip, cardOf(state.chip));
      }
    });
  }

  // The Sources appendix of an export: each card's picture is as wide as its
  // column, and a picture too tall for its row is narrowed until it fits. The
  // box stays the picture's own either way, which is what its marks - percent
  // of that box - depend on. A cut-out is an SVG with a viewBox and no size,
  // so only its aspect ratio, known once it has loaded, can say how wide a
  // picture of a given height is.
  function fitSources() {
    for (const pic of document.querySelectorAll('figure.mz-source .mz-card-pic')) {
      const img = pic.querySelector('img');
      const fig = pic.closest('figure.mz-source');
      if (!img || !fig) continue;
      const fit = () => {
        if (!img.naturalWidth || !img.naturalHeight) return;
        const sec = fig.closest('.mz-sources');
        const maxH = parseFloat(sec && getComputedStyle(sec).getPropertyValue('--mz-sources-pic')) || 168;
        const width = Math.min(fig.clientWidth, maxH * img.naturalWidth / img.naturalHeight);
        if (width > 0) pic.style.width = Math.floor(width) + 'px';
      };
      if (img.complete) fit(); else img.addEventListener('load', fit, { once: true });
    }
  }

  // How far through the slide's clicks we are. `Infinity` until a viewer says
  // otherwise, so a page with no viewer — the PDF export above all — shows
  // every mark. An annotation waits for a click; it does not depend on one.
  const steps = new WeakMap();
  const stepOn = (sec) => (steps.has(sec) ? steps.get(sec) : Infinity);

  function refresh(only) {
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
    wireCards();
    fitSources();
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
      // A chip waits for its click like any other mark.
      for (const chip of chipsIn(sec)) n = Math.max(n, Number(chip.dataset.step) || 0);
      return n;
    },

    show(sec, step) {
      if (steps.get(sec) === step) return;
      steps.set(sec, step);
      // A click is the presenter moving on: whatever card was open goes, and
      // the chips due by now are there for the next hover.
      closeCard(sec, true);
      for (const chip of chipsIn(sec)) chip.hidden = (Number(chip.dataset.step) || 0) > step;
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
