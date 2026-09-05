// The scene extractor for `mirzam export pptx`, run inside a shot page (one
// slide, laid out at its own pixel size) by the CLI over DevTools.
//
// The browser has done the layout; this reads it back as a scene that
// `mirzam-pptx` writes as DrawingML: every run of text with the font, size,
// weight and colour the browser resolved and the box it was laid out in;
// every surface with a fill or an edge as a shape; every table as a table;
// and, for what has no PowerPoint equivalent — SVG, MathML, a WebP or SVG
// picture, a gradient — a request to photograph exactly that element, with
// everything else on the slide hidden while the picture is taken.
//
// Three entry points, on `window`:
//   mzScene(opts)   -> the scene as a JSON string
//   mzShow(id, mode) -> make raster `id` the only visible thing, for a shot
//   mzHide()         -> put the page back
//
// The unit of text is the *group*: consecutive blocks that stack vertically
// inside one container — a heading, a paragraph, a list — become one text
// box with one paragraph per block, spaced by the gaps the browser left
// between them. That is how a PowerPoint deck is built (a body placeholder
// holds a heading and its bullets), so an edit reflows the way the reader
// expects. A block that stacks nothing — a chart, a table, a row of cards —
// ends the group and is written on its own.
//
// Geometry is exact on the way out: every box is where the browser drew it.
// What cannot be exact is the wrapping once PowerPoint substitutes a font,
// so the paragraphs carry their own left and right margins rather than
// trusting the box to be the right width by luck.
(() => {
  const RASTER_TAGS = new Set([
    'svg', 'math', 'img', 'video', 'canvas', 'iframe', 'object', 'embed',
    'audio', 'picture', 'select', 'button', 'textarea', 'meter', 'progress',
  ]);
  const SKIP_TAGS = new Set([
    'script', 'style', 'template', 'noscript', 'link', 'meta', 'head', 'title',
  ]);
  const SIDES = ['Left', 'Right', 'Top', 'Bottom'];

  let slide, origin, slideRect;
  let nodes, rasters, nextRaster;
  const styles = new Map();
  const capable = new Map();

  const cs = (el) => {
    let v = styles.get(el);
    if (!v) { v = getComputedStyle(el); styles.set(el, v); }
    return v;
  };
  const px = (v) => parseFloat(v) || 0;
  const rnd = (v) => Math.round(v * 100) / 100;
  const R = (r) => ({ x: rnd(r.x), y: rnd(r.y), w: rnd(r.w), h: rnd(r.h) });

  function rectOf(el) {
    const r = el.getBoundingClientRect();
    return { x: r.left - origin.x, y: r.top - origin.y, w: r.width, h: r.height };
  }
  function contentBox(el, st) {
    const r = rectOf(el);
    const l = px(st.borderLeftWidth) + px(st.paddingLeft);
    const t = px(st.borderTopWidth) + px(st.paddingTop);
    const rr = px(st.borderRightWidth) + px(st.paddingRight);
    const b = px(st.borderBottomWidth) + px(st.paddingBottom);
    return { x: r.x + l, y: r.y + t, w: Math.max(0, r.w - l - rr), h: Math.max(0, r.h - t - b) };
  }
  function paddingBox(el, st) {
    const r = rectOf(el);
    const l = px(st.borderLeftWidth), t = px(st.borderTopWidth);
    const rr = px(st.borderRightWidth), b = px(st.borderBottomWidth);
    return { x: r.x + l, y: r.y + t, w: Math.max(0, r.w - l - rr), h: Math.max(0, r.h - t - b) };
  }
  function intersect(a, b) {
    if (!a) return b;
    if (!b) return a;
    const x = Math.max(a.x, b.x), y = Math.max(a.y, b.y);
    const r = Math.min(a.x + a.w, b.x + b.w), bt = Math.min(a.y + a.h, b.y + b.h);
    return { x, y, w: r - x, h: bt - y };
  }
  const empty = (r) => !r || r.w <= 0.5 || r.h < 0;

  // Colours come back from the browser in whatever notation the stylesheet
  // used — `rgb()`, `rgba()`, `color(srgb …)`, `oklch()` — and a canvas
  // resolves every one of them to bytes.
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 1;
  const ctx2d = canvas.getContext('2d', { willReadFrequently: true });
  const colors = new Map();
  function color(s) {
    if (!s || s === 'transparent' || s === 'none') return null;
    if (colors.has(s)) return colors.get(s);
    ctx2d.clearRect(0, 0, 1, 1);
    ctx2d.fillStyle = s;
    ctx2d.fillRect(0, 0, 1, 1);
    const d = ctx2d.getImageData(0, 0, 1, 1).data;
    const out = d[3] === 0 ? null : {
      hex: [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, '0')).join(''),
      alpha: rnd(d[3] / 255),
    };
    colors.set(s, out);
    return out;
  }
  function faded(c, op) {
    if (!c || op >= 0.999) return c;
    return { hex: c.hex, alpha: rnd(c.alpha * op) };
  }
  const fadedGradient = (g, op) => (g ? { angle: g.angle, stops: g.stops.map((s) => ({ pos: s.pos, color: faded(s.color, op) })) } : null);
  function firstFamily(list) {
    const first = (list || '').split(',')[0].trim().replace(/^["']|["']$/g, '');
    return first || 'Arial';
  }

  // `linear-gradient(90deg, rgb(…), rgb(…) 60%)` as an angle and stops.
  // Anything richer — a corner, a pixel position, a radial, a second layer
  // — returns null and is photographed instead.
  function gradientOf(s) {
    if (!s || s === 'none') return null;
    const m = s.match(/^linear-gradient\((.*)\)$/);
    if (!m || s.indexOf('gradient(', 16) >= 0) return null;
    const parts = [];
    let depth = 0, cur = '';
    for (const ch of m[1]) {
      if (ch === '(') depth++;
      if (ch === ')') depth--;
      if (ch === ',' && depth === 0) { parts.push(cur.trim()); cur = ''; } else cur += ch;
    }
    parts.push(cur.trim());
    let angle = 180;
    const first = parts[0];
    let am = first.match(/^(-?[\d.]+)deg$/);
    if (am) { angle = px(am[1]); parts.shift(); }
    else if ((am = first.match(/^to (top|right|bottom|left)$/))) {
      angle = { top: 0, right: 90, bottom: 180, left: 270 }[am[1]];
      parts.shift();
    } else if (first.startsWith('to ')) return null;
    const stops = [];
    for (const part of parts) {
      const sm = part.match(/^(.*?)(?:\s+(-?[\d.]+)%)?$/);
      if (!sm) return null;
      if (/\d(px|em|rem)$/.test(part)) return null;
      const c = color(sm[1]) || { hex: '000000', alpha: 0 };
      stops.push({ color: c, pos: sm[2] !== undefined ? px(sm[2]) / 100 : null });
    }
    if (stops.length < 2) return null;
    stops.forEach((st, i) => { if (st.pos === null) st.pos = i / (stops.length - 1); });
    return { angle, stops };
  }

  function hidden(el, st) {
    return st.display === 'none' || st.visibility === 'hidden' || px(st.opacity) === 0;
  }
  const tagOf = (el) => el.tagName.toLowerCase();
  const isInline = (st) => st.display.startsWith('inline') || st.display === 'contents' || st.display === 'math';
  // An inline element holding block boxes — a `<code>` whose lines are
  // block spans — lays out as blocks, and is treated as one here.
  const hasBlockChild = (el) => !RASTER_TAGS.has(tagOf(el)) && [...el.children].some((c) => !isInline(cs(c)) && cs(c).display !== 'none');
  const isInlineNode = (n) => n.nodeType === 3 || (isInline(cs(n)) && !hasBlockChild(n));
  const stacks = (el, st) =>
    st.display === 'block' || st.display === 'list-item' || st.display === 'flow-root' ||
    (st.display === 'flex' && st.flexDirection.startsWith('column')) ||
    (isInline(st) && hasBlockChild(el));
  const positioned = (st) => st.position === 'absolute' || st.position === 'fixed' || st.float !== 'none';

  // The children of an element as the layout sees them: `display: contents`
  // wrappers dissolve into their parents.
  function kids(el) {
    const out = [];
    for (const n of el.childNodes) {
      if (n.nodeType === 1) {
        if (SKIP_TAGS.has(tagOf(n))) continue;
        const st = cs(n);
        if (st.display === 'contents') { out.push(...kids(n)); continue; }
        if (hidden(n, st)) continue;
        out.push(n);
      } else if (n.nodeType === 3) {
        out.push(n);
      }
    }
    return out;
  }

  // Can this element be written as paragraphs of one text box? It has to
  // stack its children top to bottom, and everything in it has to be text.
  function textCapable(el) {
    if (capable.has(el)) return capable.get(el);
    let ok = true;
    const tag = tagOf(el);
    const st = cs(el);
    if (RASTER_TAGS.has(tag) || tag === 'table' || tag === 'hr' || tag === 'input') ok = false;
    else if (positioned(st) || !stacks(el, st) || st.columnCount !== 'auto') ok = false;
    else if (st.transform !== 'none') ok = false;
    else {
      for (const n of kids(el)) {
        if (n.nodeType !== 1) continue;
        const cst = cs(n);
        if (isInlineNode(n)) {
          // A formula or an icon in the line is photographed and held
          // open with spaces; a table or a frame has no place in a line.
          if (n.querySelector('table,iframe,video')) { ok = false; break; }
          if (cst.display === 'inline-block' || cst.display === 'inline-flex' || cst.display === 'inline-grid') {
            // A block in a line: written as runs, so it must hold only text.
            if (n.querySelector('*:not(span):not(a):not(em):not(strong):not(b):not(i):not(code):not(sup):not(sub):not(u):not(s):not(del):not(mark):not(small):not(kbd):not(abbr):not(br)')) { ok = false; break; }
          }
        } else if (!textCapable(n)) { ok = false; break; }
      }
    }
    capable.set(el, ok);
    return ok;
  }

  // -- own paint ------------------------------------------------------------

  function radiusOf(el, st) {
    const v = st.borderTopLeftRadius || '0px';
    const r = rectOf(el);
    if (v.endsWith('%')) return Math.min(r.w, r.h) * px(v) / 100;
    return px(v);
  }
  function shadowOf(st) {
    // "rgba(0, 0, 0, 0.5) 0px 12px 60px 0px" — the first shadow only.
    const s = st.boxShadow;
    if (!s || s === 'none') return null;
    const m = s.match(/^(rgba?\([^)]*\)|color\([^)]*\)|#[0-9a-f]+|\w+)\s+(-?[\d.]+)px\s+(-?[\d.]+)px(?:\s+(-?[\d.]+)px)?/i);
    if (!m) return null;
    const c = color(m[1]);
    if (!c) return null;
    return { color: c, dx: px(m[2]), dy: px(m[3]), blur: px(m[4] || 0) };
  }
  function paintOf(el, st) {
    const fill = color(st.backgroundColor);
    const image = st.backgroundImage && st.backgroundImage !== 'none';
    const widths = SIDES.map((s) =>
      st['border' + s + 'Style'] === 'none' || st['border' + s + 'Style'] === 'hidden' ? 0 : px(st['border' + s + 'Width']));
    const colorsOf = SIDES.map((s) => color(st['border' + s + 'Color']));
    const drawn = widths.map((w, i) => w > 0 && colorsOf[i] ? w : 0);
    const uniform = drawn.every((w) => w === drawn[0]) &&
      colorsOf.every((c) => !drawn[0] || (c && c.hex === colorsOf[0].hex && c.alpha === colorsOf[0].alpha));
    const gradient = image ? gradientOf(st.backgroundImage) : null;
    const dash = { dashed: 'dash', dotted: 'sysDot' }[st.borderTopStyle] || null;
    return {
      fill, image: image && !gradient, gradient, radius: radiusOf(el, st),
      line: uniform && drawn[0] > 0 ? { width: drawn[0], color: colorsOf[0], dash } : null,
      sides: uniform ? null : drawn.map((w, i) => (w > 0 ? { width: w, color: colorsOf[i] } : null)),
      shadow: shadowOf(st),
      any: !!fill || image || drawn.some((w) => w > 0),
    };
  }

  // Writes an element's own surface: a filled, edged shape — or, when the
  // surface is a gradient or a picture, a photograph of the element alone.
  function emitPaint(el, st, paint, ctx) {
    emitPseudo(el, st, ctx);
    if (!paint.any) return;
    const r = rectOf(el);
    if (empty(intersect(r, ctx.clip))) return;
    if (paint.image) {
      emitRaster(el, 'self', ctx, 'png');
      return;
    }
    if (paint.fill || paint.line || paint.gradient) {
      nodes.push({
        k: 'shape', name: tagOf(el), rect: R(r),
        fill: faded(paint.fill, ctx.op), gradient: fadedGradient(paint.gradient, ctx.op),
        line: paint.line ? { width: paint.line.width, color: faded(paint.line.color, ctx.op), dash: paint.line.dash } : null,
        radius: paint.radius,
      });
    }
    if (paint.sides) {
      // A border on some sides only — a quote's bar, a rule under a
      // footnote block — is drawn as one thin bar per side.
      const [l, rr, t, b] = paint.sides;
      const bar = (rect, line) => nodes.push({ k: 'shape', name: 'border', rect: R(rect), fill: faded(line.color, ctx.op), line: null, radius: 0 });
      if (l) bar({ x: r.x, y: r.y, w: l.width, h: r.h }, l);
      if (rr) bar({ x: r.x + r.w - rr.width, y: r.y, w: rr.width, h: r.h }, rr);
      if (t) bar({ x: r.x, y: r.y, w: r.w, h: t.width }, t);
      if (b) bar({ x: r.x, y: r.y + r.h - b.width, w: r.w, h: b.width }, b);
    }
  }

  // A `::before` or `::after` drawn as a box of its own — the rule under a
  // heading — becomes a shape at the place the layout gives such a box: the
  // top or the bottom of the element's content, following its margins.
  function emitPseudo(el, st, ctx) {
    for (const which of ['::before', '::after']) {
      const ps = getComputedStyle(el, which);
      if (ps.content === 'none' || ps.content === 'normal') continue;
      if (ps.display !== 'block' && ps.display !== 'inline-block' && ps.display !== 'flex') continue;
      const w = px(ps.width), h = px(ps.height);
      if (w <= 0 || h <= 0 || ps.position === 'absolute') continue;
      const fill = color(ps.backgroundColor);
      const gradient = gradientOf(ps.backgroundImage);
      const leader = h <= 2 && /^repeating-linear-gradient\(/.test(ps.backgroundImage) ? color((ps.backgroundImage.match(/(rgba?\([^)]*\)|color\([^)]*\))/) || [])[1]) : null;
      if (!fill && !gradient && !leader) continue;
      const cb = contentBox(el, st);
      let x;
      if (ps.marginLeft === 'auto' && ps.marginRight === 'auto') x = cb.x + (cb.w - w) / 2;
      else if (ps.marginLeft === 'auto') x = cb.x + cb.w - w - px(ps.marginRight);
      else x = cb.x + px(ps.marginLeft);
      const y = which === '::after' ? cb.y + cb.h - px(ps.marginBottom) - h : cb.y + px(ps.marginTop);
      const rect = intersect({ x, y, w, h }, ctx.clip);
      if (empty(rect)) continue;
      if (leader) {
        // A dotted leader: a line, dotted, rather than a picture of dots.
        nodes.push({ k: 'shape', name: 'leader', kind: 'line', rect: R({ x: rect.x, y: rect.y + rect.h / 2, w: rect.w, h: 0 }), line: { width: rect.h, color: faded(leader, ctx.op), dash: 'sysDot' } });
        continue;
      }
      nodes.push({ k: 'shape', name: 'rule', rect: R(rect), fill: faded(fill, ctx.op), gradient: fadedGradient(gradient, ctx.op), line: null, radius: px(ps.borderTopLeftRadius) });
    }
  }

  // Generated text on an inline element: a quoted `content`, with `\A`
  // standing for a line break.
  function pseudoText(el, which) {
    const c = getComputedStyle(el, which).content;
    if (!c || c === 'none' || c === 'normal') return null;
    const m = c.match(/^"((?:[^"\\]|\\.)*)"$/);
    if (!m) return null;
    return m[1].replace(/\\A\s?/gi, '\n').replace(/\\(.)/g, '$1');
  }

  // -- rasters ----------------------------------------------------------------

  function emitRaster(el, mode, ctx, kind) {
    const r = rectOf(el);
    const vis = intersect(intersect(r, ctx.clip), slideRect);
    if (empty(vis)) return null;
    const id = nextRaster++;
    el.setAttribute('data-mz-r', String(id));
    rasters.push({ id, rect: R(vis), kind, mode });
    const st = cs(el);
    const whole = Math.abs(vis.w - r.w) < 0.5 && Math.abs(vis.h - r.h) < 0.5;
    nodes.push({ k: 'picture', name: tagOf(el), rect: R(vis), image: id, radius: whole ? radiusOf(el, st) : 0, alpha: 1 });
    return vis;
  }

  // An inline image in a format PowerPoint opens goes in as the bytes the
  // page holds, cropped the way `object-fit` and the pane's clip cropped it;
  // anything else is photographed.
  function emitImage(img, ctx) {
    const st = cs(img);
    const src = img.currentSrc || img.src || '';
    // A filter (a dimmed or blurred photo) is part of what was drawn, so
    // the element is photographed rather than embedded untouched.
    const direct = /^data:image\/(png|jpeg|jpg|gif);base64,/i.test(src) && img.naturalWidth > 0 && st.transform === 'none' && st.filter === 'none';
    if (!direct) {
      const photo = /^data:image\/(jpeg|jpg|webp)/i.test(src) && radiusOf(img, st) === 0;
      emitRaster(img, 'tree', ctx, photo ? 'jpeg' : 'png');
      return;
    }
    const box = contentBox(img, st);
    const nw = img.naturalWidth, nh = img.naturalHeight;
    let scale;
    switch (st.objectFit) {
      case 'contain': scale = Math.min(box.w / nw, box.h / nh); break;
      case 'cover': scale = Math.max(box.w / nw, box.h / nh); break;
      case 'none': scale = 1; break;
      case 'scale-down': scale = Math.min(1, Math.min(box.w / nw, box.h / nh)); break;
      default: scale = null;
    }
    let drawn;
    if (scale === null) drawn = box;
    else {
      const dw = nw * scale, dh = nh * scale;
      const pos = (st.objectPosition || '50% 50%').split(/\s+/);
      const at = (v, room) => (v.endsWith('%') ? room * px(v) / 100 : px(v));
      drawn = { x: box.x + at(pos[0] || '50%', box.w - dw), y: box.y + at(pos[1] || '50%', box.h - dh), w: dw, h: dh };
    }
    const vis = intersect(intersect(intersect(drawn, box), ctx.clip), slideRect);
    if (empty(vis) || drawn.w <= 0 || drawn.h <= 0) return;
    const id = nextRaster++;
    rasters.push({ id, kind: 'data', data: src, mode: 'tree' });
    const crop = [
      (vis.x - drawn.x) / drawn.w, (vis.y - drawn.y) / drawn.h,
      (drawn.x + drawn.w - vis.x - vis.w) / drawn.w, (drawn.y + drawn.h - vis.y - vis.h) / drawn.h,
    ].map((f) => Math.max(0, Math.min(1, rnd(f))));
    const own = rectOf(img);
    const whole = Math.abs(vis.w - own.w) < 0.5 && Math.abs(vis.h - own.h) < 0.5;
    nodes.push({ k: 'picture', name: 'img', rect: R(vis), image: id, crop, radius: whole ? radiusOf(img, st) : 0, alpha: rnd(ctx.op * px(st.opacity)) });
  }

  // -- text -------------------------------------------------------------------

  const collapse = (t) => t.replace(/[ \t\r\n\f]+/g, ' ');

  function runStyle(el, ctx) {
    const st = cs(el);
    const va = st.verticalAlign;
    const tag = tagOf(el);
    let baseline = 0;
    if (tag === 'sup' || va === 'super' || (va.endsWith('px') && px(va) > 0) || va === 'text-top') baseline = 30000;
    else if (tag === 'sub' || va === 'sub' || (va.endsWith('px') && px(va) < 0)) baseline = -25000;
    if (ctx.baseline) baseline = ctx.baseline;
    return {
      font: firstFamily(st.fontFamily),
      size: rnd(px(st.fontSize)),
      bold: px(st.fontWeight) >= 600 || st.fontWeight === 'bold' || st.fontWeight === 'bolder',
      italic: st.fontStyle !== 'normal',
      color: faded(color(st.color), ctx.op),
      underline: ctx.underline || st.textDecorationLine.includes('underline'),
      strike: ctx.strike || st.textDecorationLine.includes('line-through'),
      highlight: ctx.highlight || null,
      baseline,
      caps: st.textTransform === 'uppercase',
      spacing: st.letterSpacing === 'normal' ? 0 : rnd(px(st.letterSpacing)),
      href: ctx.href || null,
    };
  }
  const sameStyle = (a, b) =>
    a.font === b.font && a.size === b.size && a.bold === b.bold && a.italic === b.italic &&
    JSON.stringify(a.color) === JSON.stringify(b.color) && a.underline === b.underline && a.strike === b.strike &&
    JSON.stringify(a.highlight) === JSON.stringify(b.highlight) && a.baseline === b.baseline && a.caps === b.caps &&
    a.spacing === b.spacing && a.href === b.href;

  function pushText(runs, text, style, pre) {
    if (!text) return;
    const last = runs[runs.length - 1];
    if (last && last.t !== undefined && sameStyle(last, style) && !!last.pre === !!pre) { last.t += text; return; }
    runs.push({ t: text, ...style, ...(pre ? { pre: true } : {}) });
  }

  // Walks inline content, appending runs. A newline in preformatted text
  // becomes a `{nl}` marker, split into paragraphs afterwards.
  function walkInline(node, ctx, runs) {
    if (node.nodeType === 3) {
      const parent = node.parentElement;
      if (!parent) return;
      const st = cs(parent);
      const pre = st.whiteSpace.startsWith('pre') || st.whiteSpace === 'break-spaces';
      const style = runStyle(parent, ctx);
      let text = node.data;
      if (st.textTransform === 'capitalize') text = text.replace(/\b\w/g, (c) => c.toUpperCase());
      if (st.textTransform === 'lowercase') text = text.toLowerCase();
      if (!pre) { pushText(runs, collapse(text), style); return; }
      const tab = ' '.repeat(parseInt(st.tabSize, 10) || 8);
      const lines = text.replace(/\t/g, tab).split('\n');
      lines.forEach((line, i) => {
        if (i > 0) runs.push({ nl: true, size: style.size, font: style.font });
        pushText(runs, line, style, true);
      });
      return;
    }
    if (node.nodeType !== 1) return;
    const tag = tagOf(node);
    if (SKIP_TAGS.has(tag)) return;
    const st = cs(node);
    if (hidden(node, st)) return;
    if (tag === 'br') { runs.push({ br: true }); return; }
    if (tag === 'input') return;
    if (tag === 'math' && st.display === 'math' && node.parentElement) {
      const words = [];
      if (mathRuns(node, runStyle(node.parentElement, ctx), words, 0)) {
        for (const w of words) { const { t, ...style } = w; pushText(runs, t, style); }
        return;
      }
    }
    if (RASTER_TAGS.has(tag)) {
      // A formula or an icon in the line: photographed and laid over the
      // box, with spaces holding its width open in the line.
      const vis = tag === 'img' ? (emitImage(node, ctx), rectOf(node)) : emitRaster(node, 'tree', ctx, 'png');
      if (vis && node.parentElement) {
        const style = runStyle(node.parentElement, ctx);
        ctx2d.font = `${style.italic ? 'italic ' : ''}${style.bold ? 'bold ' : ''}${style.size}px "${style.font}"`;
        const space = ctx2d.measureText(' ').width || style.size / 4;
        pushText(runs, ' '.repeat(Math.max(1, Math.round(vis.w / space))), style);
      }
      return;
    }
    const inner = { ...ctx };
    const generated = (which) => {
      const t = pseudoText(node, which);
      if (t === null) return;
      const style = runStyle(node, inner);
      t.split('\n').forEach((piece, i) => {
        if (i > 0) runs.push({ br: true });
        pushText(runs, piece, style);
      });
    };
    if (tag === 'a' && node.getAttribute('href')) inner.href = node.getAttribute('href');
    if (st.textDecorationLine.includes('underline')) inner.underline = true;
    if (st.textDecorationLine.includes('line-through')) inner.strike = true;
    if (tag === 'sup') inner.baseline = 30000;
    if (tag === 'sub') inner.baseline = -25000;
    const bg = color(st.backgroundColor);
    if (bg) inner.highlight = faded(bg, ctx.op);
    inner.op = ctx.op * px(st.opacity);
    generated('::before');
    for (const child of node.childNodes) walkInline(child, inner, runs);
    generated('::after');
  }

  // A formula simple enough to be words: identifiers, numbers, operators,
  // and sub- or superscripts on them become runs, so `T₁` and `χ = g²/Δ`
  // stay in the line whatever font opens the file. A fraction, a root or a
  // table returns null and is photographed instead.
  const SPACED_OPS = new Set(['=', '+', '−', '-', '×', '·', '→', '←', '↔', '≈', '≠', '≤', '≥', '<', '>', '∝', '∼', '≡', '⇒', '⇔', '±', '∓', '∈', '∉', '⊂', '⊆', '∪', '∩']);
  function mathRuns(el, base, out, baseline) {
    const tag = tagOf(el);
    const push = (t, extra) => { if (t) out.push({ ...base, ...extra, baseline: baseline || 0, t }); };
    switch (tag) {
      case 'math': case 'mrow': case 'mstyle': case 'mpadded': case 'semantics':
        for (const c of el.children) {
          if (tagOf(c) === 'annotation' || tagOf(c) === 'annotation-xml') continue;
          if (!mathRuns(c, base, out, baseline)) return false;
        }
        return true;
      case 'mi': {
        const t = el.textContent;
        const upright = el.getAttribute('mathvariant') === 'normal' || [...t].length > 1;
        push(t, { italic: !upright });
        return true;
      }
      case 'mn': push(el.textContent); return true;
      case 'mtext': push(el.textContent); return true;
      case 'mspace': push(' '); return true;
      case 'mo': {
        const t = el.textContent.trim();
        const tight = el.getAttribute('lspace') === '0' || el.getAttribute('rspace') === '0';
        push(SPACED_OPS.has(t) && !tight ? ` ${t} ` : t);
        return true;
      }
      case 'msub': case 'msup': case 'msubsup': {
        const kids = [...el.children];
        if (kids.length < 2 || baseline) return false;
        if (!mathRuns(kids[0], base, out, 0)) return false;
        if (tag === 'msub' || tag === 'msubsup') { if (!mathRuns(kids[1], base, out, -25000)) return false; }
        if (tag === 'msup') { if (!mathRuns(kids[1], base, out, 30000)) return false; }
        if (tag === 'msubsup') { if (!mathRuns(kids[2], base, out, 30000)) return false; }
        return true;
      }
      default: return false;
    }
  }

  // Trims the spaces the browser would have collapsed at line edges.
  function tidy(runs) {
    const out = [];
    let atStart = true;
    for (const r of runs) {
      if (r.t === undefined) { // break or newline
        const last = out[out.length - 1];
        if (last && last.t !== undefined) last.t = last.t.replace(/ +$/, '');
        out.push(r);
        atStart = true;
        continue;
      }
      let t = r.t;
      if (!r.pre) {
        if (atStart) t = t.replace(/^ +/, '');
        else {
          const last = out[out.length - 1];
          if (last && last.t !== undefined && !last.pre && last.t.endsWith(' ')) t = t.replace(/^ +/, '');
        }
      }
      if (!t) continue;
      const { pre, ...rest } = r;
      out.push({ ...rest, t, ...(pre ? { pre } : {}) });
      atStart = false;
    }
    const last = out[out.length - 1];
    if (last && last.t !== undefined && !last.pre) last.t = last.t.replace(/ +$/, '');
    return out.filter((r) => r.t === undefined || r.t.length > 0).map((r) => { const rest = { ...r }; delete rest.pre; return rest; });
  }

  function alignOf(st) {
    // `align="right"` on a cell computes to the `-webkit-` form.
    switch (st.textAlign.replace(/^-webkit-/, '')) {
      case 'center': return 'center';
      case 'right': case 'end': return 'right';
      case 'justify': return 'justify';
      default: return 'left';
    }
  }
  function lineHeightOf(st) {
    return st.lineHeight === 'normal' ? rnd(px(st.fontSize) * 1.2) : rnd(px(st.lineHeight));
  }
  // The height of a font's content area — ascent plus descent — at a size.
  // The browser centres that area in the line box; PowerPoint and Impress
  // set exact line spacing with the text at the bottom of the line, so a
  // paragraph is moved up by half the leading to land where the browser
  // put it.
  //
  // Measured with the block's whole font stack, as the line box is: the
  // first *installed* family sets the strut, and it may be the CJK face at
  // the end of the list rather than the Latin face at the front.
  const naturals = new Map();
  function naturalHeight(stack, size) {
    const key = `${size}|${stack}`;
    if (naturals.has(key)) return naturals.get(key);
    ctx2d.font = `${size}px ${stack}`;
    const m = ctx2d.measureText('Hg');
    const h = (m.fontBoundingBoxAscent + m.fontBoundingBoxDescent) || size * 1.2;
    naturals.set(key, h);
    return h;
  }
  // The lines a run of inline nodes was laid out on: one box per line,
  // from the fragments the Range reports, merged where they share a line.
  function measureLines(nodesList) {
    const range = document.createRange();
    range.setStartBefore(nodesList[0]);
    range.setEndAfter(nodesList[nodesList.length - 1]);
    const rects = [...range.getClientRects()]
      .filter((r) => r.width > 0 && r.height > 0)
      .map((r) => ({ top: r.top - origin.y, bottom: r.bottom - origin.y, left: r.left - origin.x, right: r.right - origin.x }))
      .sort((a, b) => a.top - b.top || a.left - b.left);
    const lines = [];
    for (const r of rects) {
      const cur = lines[lines.length - 1];
      if (cur && r.top < cur.bottom - 1 && r.bottom > cur.top + 1) {
        cur.top = Math.min(cur.top, r.top); cur.bottom = Math.max(cur.bottom, r.bottom);
        cur.left = Math.min(cur.left, r.left); cur.right = Math.max(cur.right, r.right);
      } else lines.push({ ...r });
    }
    return lines;
  }

  // One paragraph entry: what the writer needs plus where the text sits,
  // so a group can turn positions into margins and spacing.
  function entry(para, top, bottom, left, right, extra) {
    return { para, top, bottom, left, right, ...extra };
  }

  // Splits an inline segment's runs into paragraphs at `{nl}` markers and
  // lays them out as lines under `top`.
  // `top`/`bottom` are either the measured text (its content area, from a
  // Range) or the block's own box; both come out as the line boxes
  // PowerPoint will stack, moved by the leading as `naturalHeight` explains.
  function segmentEntries(runs, block, st, ctx, top, bottom, opts) {
    let lh = lineHeightOf(st);
    let natural = naturalHeight(st.fontFamily, px(st.fontSize));
    const normal = st.lineHeight === 'normal';
    let single = false, slack = 0;
    const box = contentBox(block, st);
    if (opts.lines) {
      const L = opts.lines;
      const count = L.length;
      if (normal) {
        // `normal` is whatever the tallest font on the line asks for, so
        // it is read off the layout: the pitch between lines, or the one
        // line's own height. Blank lines of preformatted text leave no
        // fragment, so the pitch is taken over the logical lines that do.
        const logical = [];
        let li = 0;
        runs.forEach((r) => { if (r.nl) li++; else if (r.t && r.t.trim()) logical[li] = true; });
        const filled = logical.map((v, i) => (v ? i : -1)).filter((i) => i >= 0);
        const span = filled.length === count && count > 1 ? filled[count - 1] - filled[0] : count - 1;
        lh = span > 0 ? (L[count - 1].top - L[0].top) / span : L[0].bottom - L[0].top;
        natural = lh;
        if (filled.length && filled[0] > 0) top = L[0].top - filled[0] * lh; else top = L[0].top;
      } else top = L[0].top - (lh - natural);
      bottom = top + count * lh;
      single = count === 1;
      slack = box.w - (L[0].right - L[0].left);
    } else {
      if (normal) { lh = bottom - top; natural = lh; }
      top -= (lh - natural) / 2;
      bottom -= (lh - natural) / 2;
    }
    const base = { align: alignOf(st), line_height: rnd(lh), nowrap: st.whiteSpace === 'nowrap' || st.whiteSpace === 'pre' };
    const lines = [[]];
    for (const r of runs) {
      if (r.nl) lines.push([]);
      else lines[lines.length - 1].push(r);
    }
    const out = [];
    const many = lines.length > 1;
    lines.forEach((line, i) => {
      const clean = tidy(line);
      if (!clean.length && !many && !opts.keepEmpty) return;
      const t = many ? top + i * lh : top;
      const b = many ? t + lh : bottom;
      const size = clean.find((r) => r.t !== undefined)?.size || (runs[0] && runs[0].size) || px(st.fontSize);
      const para = { ...base, runs: clean.length ? clean : [{ t: '', size: rnd(size), font: firstFamily(st.fontFamily) }] };
      if (i === 0 && opts.bullet) { para.bullet = opts.bullet; }
      out.push(entry(para, t, b, box.x, box.x + box.w, { bulletLeft: i === 0 ? opts.bulletLeft : undefined, level: opts.level || 0, single: single || many, slack: many ? box.w : slack }));
    });
    return out;
  }

  // The paragraphs of a text-capable block, in order, with any surface of
  // its own written beneath them.
  function paragraphsOf(el, entries, ctx, opts) {
    const tag = tagOf(el);
    const st = cs(el);
    if (tag === 'ul' || tag === 'ol') { listParagraphs(el, entries, ctx, opts.level || 0); return; }
    if (!opts.skipPaint) emitPaint(el, st, paintOf(el, st), ctx);
    // A link laid out as a block — a contents entry — carries its target
    // down to its words the same as one in a line would.
    const inner = { ...ctx, op: ctx.op * px(st.opacity) };
    if (tag === 'a' && el.getAttribute('href')) inner.href = el.getAttribute('href');
    let segment = [];
    let first = true;
    // A numbered code line: the counter its `::before` draws, padded to the
    // column the stylesheet reserves, in non-breaking spaces so the paragraph
    // keeps them.
    let lineNumber = null;
    if (el.classList.contains('mz-cl') && el.parentElement && el.parentElement.classList.contains('mz-code-nums')) {
      const ps = getComputedStyle(el, '::before');
      const n = [...el.parentElement.children].filter((c) => c.classList.contains('mz-cl')).indexOf(el) + 1;
      ctx2d.font = `${px(ps.fontSize)}px "${firstFamily(ps.fontFamily)}"`;
      const ch = ctx2d.measureText('0').width || px(ps.fontSize) * 0.6;
      const width = Math.max(String(n).length, Math.round(px(ps.width) / ch));
      const gap = Math.max(1, Math.round(px(ps.marginRight) / ch));
      lineNumber = { t: String(n).padStart(width, '\u00a0') + '\u00a0'.repeat(gap), ...runStyle(el, inner), color: faded(color(ps.color), inner.op) };
    }
    const flush = () => {
      if (!segment.length) { return; }
      const runs = [];
      if (lineNumber) { runs.push({ ...lineNumber, pre: true }); lineNumber = null; }
      for (const n of segment) walkInline(n, inner, runs);
      const hasText = runs.some((r) => r.t !== undefined && r.t.trim().length) || runs.some((r) => r.nl);
      if (hasText) {
        const lines = measureLines(segment);
        const own = rectOf(el);
        entries.push(...segmentEntries(runs, el, st, inner, own.y, own.y + own.h, { ...opts, bullet: first ? opts.bullet : null, bulletLeft: first ? opts.bulletLeft : undefined, keepEmpty: false, lines: lines.length ? lines : null }));
        first = false;
      }
      segment = [];
    };
    for (const n of kids(el)) {
      if (isInlineNode(n)) { segment.push(n); continue; }
      flush();
      paragraphsOf(n, entries, inner, { ...opts, bullet: first ? opts.bullet : null, bulletLeft: first ? opts.bulletLeft : undefined, skipPaint: false });
      first = false;
    }
    flush();
    if (first && opts.keepEmpty !== false) {
      // Nothing in it, but it takes room: a blank line in a code block.
      const own = rectOf(el);
      if (own.h > 0) {
        entries.push(...segmentEntries(lineNumber ? [{ ...lineNumber, pre: true }] : [], el, st, inner, own.y, own.y + own.h, { ...opts, keepEmpty: true }));
      }
    }
  }

  const SCHEMES = {
    decimal: 'arabicPeriod', 'decimal-leading-zero': 'arabicPeriod',
    'lower-alpha': 'alphaLcPeriod', 'lower-latin': 'alphaLcPeriod',
    'upper-alpha': 'alphaUcPeriod', 'upper-latin': 'alphaUcPeriod',
    'lower-roman': 'romanLcPeriod', 'upper-roman': 'romanUcPeriod',
  };
  const BULLETS = { disc: '•', circle: '◦', square: '▪' };

  function listParagraphs(list, entries, ctx, level) {
    let number = parseInt(list.getAttribute('start') || '1', 10) || 1;
    let firstItem = true;
    for (const li of list.children) {
      if (tagOf(li) !== 'li') continue;
      const st = cs(li);
      if (hidden(li, st)) continue;
      const box = contentBox(li, st);
      const font = `${px(st.fontSize)}px "${firstFamily(st.fontFamily)}"`;
      const markerColor = faded(color(getComputedStyle(li, '::marker').color) || color(st.color), ctx.op);
      let bullet = null, gap = 0;
      const check = li.querySelector(':scope > input[type="checkbox"]');
      if (check) {
        bullet = { kind: 'char', text: check.checked ? '☑' : '☐', color: markerColor };
        gap = px(st.fontSize) * 1.3;
      } else if (st.listStyleType !== 'none') {
        const own = st.listStyleType;
        ctx2d.font = font;
        if (SCHEMES[own]) {
          bullet = { kind: 'auto', scheme: SCHEMES[own], start: firstItem ? number : 1, color: markerColor };
          gap = ctx2d.measureText(`${number}. `).width;
        } else {
          const text = BULLETS[own] || own.replace(/^["']|["']$/g, '') || '•';
          bullet = { kind: 'char', text, color: markerColor };
          gap = ctx2d.measureText(text + ' ').width;
        }
      }
      paragraphsOf(li, entries, ctx, { level, bullet, bulletLeft: bullet ? box.x - gap : undefined, keepEmpty: false });
      number += 1;
      firstItem = false;
    }
  }

  // Turns a group's entries into one text box. `container`, when given, is
  // the element whose box the text box takes — the pane or card the text
  // fills — so PowerPoint's anchoring follows the browser's alignment.
  function flushGroup(entries, container, paint, ctx) {
    if (!entries.length) return;
    // Paragraphs stack downward and a gap is never negative, so a loose
    // paragraph whose leading reaches up into a tight one above it cannot
    // follow it in the same box. Such a run is split into boxes that
    // overlap, each where its own lines go; the surface, if any, is drawn
    // once underneath.
    const chunks = [[entries[0]]];
    for (let i = 1; i < entries.length; i++) {
      const e = entries[i], prev = entries[i - 1];
      if (e.top < prev.bottom - 0.5) chunks.push([e]); else chunks[chunks.length - 1].push(e);
    }
    if (chunks.length > 1) {
      if (container && paint && (paint.fill || paint.line || paint.gradient)) {
        nodes.push({
          k: 'shape', name: tagOf(container), rect: R(rectOf(container)),
          fill: faded(paint.fill, ctx.op), gradient: fadedGradient(paint.gradient, ctx.op),
          line: paint.line ? { width: paint.line.width, color: faded(paint.line.color, ctx.op) } : null,
          radius: paint.radius,
        });
      }
      for (const chunk of chunks) textBox(chunk, null, null, ctx);
      return;
    }
    textBox(entries, container, paint, ctx);
  }

  function textBox(entries, container, paint, ctx) {
    let rect, insets, anchor = 'top';
    const first = entries[0], last = entries[entries.length - 1];
    const firstTop = first.top, lastBottom = last.bottom;
    let left = Infinity, right = -Infinity;
    for (const e of entries) {
      left = Math.min(left, e.bulletLeft !== undefined ? e.bulletLeft : e.left);
      right = Math.max(right, e.right);
    }
    if (container) {
      const st = cs(container);
      rect = rectOf(container);
      insets = [Math.max(0, left - rect.x), firstTop - rect.y, Math.max(0, rect.x + rect.w - right), rect.y + rect.h - lastBottom];
      if (st.display === 'flex' && st.flexDirection.startsWith('column')) {
        const j = st.justifyContent;
        if (j === 'center' || j === 'space-around' || j === 'space-evenly') anchor = 'middle';
        else if (j === 'flex-end' || j === 'end') anchor = 'bottom';
      } else if (st.display === 'flex' || st.display === 'grid') {
        const a = st.alignItems;
        if (a === 'center') anchor = 'middle';
        else if (a === 'flex-end' || a === 'end') anchor = 'bottom';
      }
    } else {
      rect = { x: left, y: firstTop, w: right - left, h: lastBottom - firstTop };
      insets = [0, 0, 0, 0];
    }
    // A first line whose leading reaches above its container's edge: give
    // the box the room rather than clipping the inset.
    if (insets[1] < 0) { rect = { ...rect, y: rect.y + insets[1], h: rect.h - insets[1] }; insets[1] = 0; }
    if (insets[3] < 0) { rect = { ...rect, h: rect.h - insets[3] }; insets[3] = 0; }
    // Room for a substituted font: a box exactly as wide as the browser's
    // lines wraps one word early in any face a little wider. A bare box
    // grows into the margin it has no fill to betray; a filled one gives
    // up some of its padding instead. Centred text grows on both sides.
    const centred = entries.every((e) => e.para.align === 'center');
    const grow = Math.max(4, 0.03 * rect.w);
    if (!container) {
      rect = { x: centred ? rect.x - grow : rect.x, y: rect.y, w: rect.w + (centred ? 2 * grow : grow), h: rect.h };
    } else {
      insets[2] = Math.max(0, insets[2] - grow);
      if (centred) insets[0] = Math.max(0, insets[0] - grow);
    }
    const textLeft = rect.x + insets[0], textRight = rect.x + rect.w - insets[2];
    const paragraphs = [];
    let prevBottom = null;
    for (const e of entries) {
      const p = { ...e.para };
      p.margin_left = rnd(Math.max(0, e.left - textLeft));
      p.margin_right = rnd(Math.max(0, textRight - e.right));
      p.indent = e.bulletLeft !== undefined ? rnd(e.bulletLeft - e.left) : 0;
      p.level = e.level || 0;
      p.space_before = prevBottom === null ? 0 : rnd(Math.max(0, e.top - prevBottom));
      prevBottom = e.bottom;
      delete p.nowrap;
      paragraphs.push(p);
    }
    // Wrapping is left on unless every line is a single one with room to
    // spare: a label or a code line stays one line in a wider font, where
    // a sentence that just fit should wrap rather than run off the slide.
    const roomy = entries.every((e) => e.para.nowrap || (e.single && (e.slack <= 2 || e.slack >= 0.15 * (e.right - e.left))));
    const wrap = !roomy;
    nodes.push({
      k: 'shape', name: container ? tagOf(container) : 'text', rect: R(rect),
      fill: paint ? faded(paint.fill, ctx.op) : null,
      gradient: paint ? fadedGradient(paint.gradient, ctx.op) : null,
      line: paint && paint.line ? { width: paint.line.width, color: faded(paint.line.color, ctx.op) } : null,
      radius: paint ? paint.radius : 0,
      text: { anchor, insets: insets.map(rnd), wrap, paragraphs },
    });
  }

  // -- tables -----------------------------------------------------------------

  function edges(values) {
    const sorted = [...values].sort((a, b) => a - b);
    const out = [];
    for (const v of sorted) if (!out.length || v - out[out.length - 1] > 1.5) out.push(v);
    return out;
  }
  const slot = (list, v) => {
    let best = 0;
    for (let i = 0; i < list.length; i++) if (Math.abs(list[i] - v) < Math.abs(list[best] - v)) best = i;
    return best;
  };

  function emitTable(table, ctx) {
    const rows = [...table.querySelectorAll(':scope > thead > tr, :scope > tbody > tr, :scope > tfoot > tr, :scope > tr')];
    const cells = [];
    for (const tr of rows) for (const td of tr.children) {
      const t = tagOf(td);
      if (t !== 'td' && t !== 'th') continue;
      cells.push({ el: td, r: rectOf(td) });
    }
    if (!cells.length) return;
    const xs = edges(cells.flatMap((c) => [c.r.x, c.r.x + c.r.w]));
    const ys = edges(cells.flatMap((c) => [c.r.y, c.r.y + c.r.h]));
    const ncol = xs.length - 1, nrow = ys.length - 1;
    if (ncol < 1 || nrow < 1) return;
    const grid = Array.from({ length: nrow }, () => Array.from({ length: ncol }, () => null));
    // Pictures inside cells are written after the table, over it.
    const outer = nodes;
    const deferred = [];
    for (const c of cells) {
      const st = cs(c.el);
      const c0 = slot(xs, c.r.x), c1 = slot(xs, c.r.x + c.r.w);
      const r0 = slot(ys, c.r.y), r1 = slot(ys, c.r.y + c.r.h);
      const entries = [];
      nodes = deferred;
      paragraphsOf(c.el, entries, ctx, { skipPaint: true, keepEmpty: false });
      nodes = outer;
      const paragraphs = entries.map((e, i) => {
        const p = { ...e.para };
        p.margin_left = 0; p.margin_right = 0; p.level = e.level || 0;
        p.indent = e.bulletLeft !== undefined ? rnd(e.bulletLeft - e.left) : 0;
        p.space_before = i === 0 ? 0 : rnd(Math.max(0, e.top - entries[i - 1].bottom));
        delete p.nowrap;
        return p;
      });
      const marT = entries.length ? Math.max(0, entries[0].top - c.r.y) : px(st.paddingTop);
      const borders = SIDES.map((s) => {
        const w = st['border' + s + 'Style'] === 'none' ? 0 : px(st['border' + s + 'Width']);
        const col = color(st['border' + s + 'Color']);
        return w > 0 && col ? { width: w, color: faded(col, ctx.op) } : null;
      });
      const va = st.verticalAlign;
      const cell = {
        text: { paragraphs }, fill: faded(color(st.backgroundColor), ctx.op), borders,
        // Narrower side margins than the browser's: room for a wider
        // font before a cell wraps a number onto two lines.
        insets: [px(st.paddingLeft) * 0.7, marT, px(st.paddingRight) * 0.7, px(st.paddingBottom)].map(rnd),
        anchor: va === 'middle' ? 'middle' : va === 'bottom' ? 'bottom' : 'top',
        col_span: Math.max(1, c1 - c0), row_span: Math.max(1, r1 - r0),
      };
      for (let r = r0; r < r1; r++) for (let cc = c0; cc < c1; cc++) {
        if (r === r0 && cc === c0) grid[r][cc] = cell;
        else grid[r][cc] = { merged_h: cc > c0, merged_v: r > r0, text: null };
      }
    }
    const tr = rectOf(table);
    nodes.push({
      k: 'table', name: 'table', rect: R(tr),
      cols: xs.slice(1).map((x, i) => rnd(x - xs[i])),
      rows: ys.slice(1).map((y, i) => ({ height: rnd(y - ys[i]), cells: grid[i].map((c) => c || { text: null }) })),
    });
    nodes.push(...deferred);
  }

  // -- the walk ---------------------------------------------------------------

  function visit(el, ctx) {
    const tag = tagOf(el);
    if (SKIP_TAGS.has(tag)) return;
    const st = cs(el);
    if (hidden(el, st)) return;
    if (st.display === 'contents') { for (const n of kids(el)) if (n.nodeType === 1) visit(n, ctx); return; }
    const inner = { ...ctx, op: ctx.op * px(st.opacity) };
    if (tag === 'a' && el.getAttribute('href')) inner.href = el.getAttribute('href');
    if (tag === 'img') { emitImage(el, inner); return; }
    if (RASTER_TAGS.has(tag)) { emitRaster(el, 'tree', inner, 'png'); return; }
    if (tag === 'table') { emitPaint(el, st, paintOf(el, st), inner); emitTable(el, inner); return; }
    const paint = paintOf(el, st);
    if (tag === 'hr' || tag === 'input') { emitPaint(el, st, paint, inner); return; }
    if (st.overflowX !== 'visible' || st.overflowY !== 'visible') inner.clip = intersect(ctx.clip, paddingBox(el, st));
    if (textCapable(el)) {
      const at = nodes.length;
      const entries = [];
      paragraphsOf(el, entries, inner, { skipPaint: true, keepEmpty: false });
      if (entries.length) {
        if (nodes.length > at || paint.image) {
          // Something inside painted — a code block's paper, a rule — so
          // the surface goes underneath it and the words in a box of
          // their own on top.
          const kidsEnd = nodes.length;
          emitPaint(el, st, paint, inner);
          const own = nodes.splice(kidsEnd);
          nodes.splice(at, 0, ...own);
          flushGroup(entries, el, null, inner);
        } else {
          flushGroup(entries, el, paint, inner);
          emitPseudo(el, st, inner);
        }
        return;
      }
      emitPaint(el, st, paint, inner);
      return;
    }
    emitPaint(el, st, paint, inner);
    const vertical = stacks(el, st);
    let group = [];
    let segment = [];
    const flushSegment = () => {
      if (!segment.length) return;
      const runs = [];
      for (const n of segment) walkInline(n, inner, runs);
      if (runs.some((r) => r.t !== undefined && r.t.trim().length)) {
        const lines = measureLines(segment);
        if (lines.length) group.push(...segmentEntries(runs, el, st, inner, 0, 0, { lines }));
      }
      segment = [];
    };
    const flush = () => { flushSegment(); flushGroup(group, null, null, inner); group = []; };
    for (const n of kids(el)) {
      if (isInlineNode(n)) { segment.push(n); continue; }
      const cst = cs(n);
      if (vertical && !positioned(cst) && textCapable(n)) {
        flushSegment();
        paragraphsOf(n, group, inner, { keepEmpty: false });
        continue;
      }
      flush();
      visit(n, inner);
    }
    flush();
  }

  window.mzScene = function (opts) {
    opts = opts || {};
    slide = document.querySelector('section.slide');
    if (!slide) throw new Error('no slide on the page');
    const sr = slide.getBoundingClientRect();
    origin = { x: sr.left, y: sr.top };
    slideRect = { x: 0, y: 0, w: sr.width, h: sr.height };
    nodes = []; rasters = []; nextRaster = 1;
    styles.clear(); capable.clear();
    for (const el of document.querySelectorAll('[data-mz-r]')) el.removeAttribute('data-mz-r');
    const sst = cs(slide);
    let background = color(sst.backgroundColor);
    if (opts.pictures) {
      // The slide as it is, background and all: photographed as a page,
      // not as an element with the rest of the page hidden.
      emitRaster(slide, 'page', { op: 1, clip: null }, 'png');
      background = null;
    } else {
      const ctx = { op: 1, clip: slideRect };
      for (const n of kids(slide)) if (n.nodeType === 1) visit(n, ctx);
    }
    return JSON.stringify({ origin: { x: rnd(origin.x), y: rnd(origin.y) }, background, nodes, rasters });
  };

  const PASS_CSS = `
.mz-raster-pass * { visibility: hidden !important; }
.mz-raster-pass [data-mz-show="tree"], .mz-raster-pass [data-mz-show="tree"] * { visibility: visible !important; }
.mz-raster-pass [data-mz-show="self"] { visibility: visible !important; color: transparent !important; }
html.mz-raster-pass, .mz-raster-pass body, .mz-raster-pass #deck, .mz-raster-pass section.slide { background: transparent !important; box-shadow: none !important; }
`;
  let passStyle = null;
  window.mzShow = function (id, mode) {
    if (!passStyle) {
      passStyle = document.createElement('style');
      passStyle.textContent = PASS_CSS;
      document.head.appendChild(passStyle);
    }
    document.documentElement.classList.add('mz-raster-pass');
    for (const el of document.querySelectorAll('[data-mz-show]')) el.removeAttribute('data-mz-show');
    const el = document.querySelector(`[data-mz-r="${id}"]`);
    if (!el) throw new Error(`raster ${id} is gone`);
    el.setAttribute('data-mz-show', mode === 'self' ? 'self' : 'tree');
    return true;
  };
  window.mzHide = function () {
    document.documentElement.classList.remove('mz-raster-pass');
    for (const el of document.querySelectorAll('[data-mz-show]')) el.removeAttribute('data-mz-show');
    return true;
  };
})();
