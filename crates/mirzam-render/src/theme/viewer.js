(() => {
  const html = document.documentElement;
  // ?mode=dark|light overrides frontmatter's baked mode for this view only,
  // applied before anything else so there is no flash of the wrong palette.
  const qMode = new URLSearchParams(location.search).get('mode');
  if (qMode === 'dark' || qMode === 'light') html.dataset.mode = qMode;

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
    else if (e.key === 'l' || e.key === 'L') {
      document.documentElement.classList.toggle('mz-debug');
    }
    else if (e.key === 'd' || e.key === 'D') {
      // No data-mode attribute yet means the OS preference is in effect;
      // read that to know which way "toggle" should go the first time.
      const isDark = html.dataset.mode
        ? html.dataset.mode === 'dark'
        : matchMedia('(prefers-color-scheme: dark)').matches;
      html.dataset.mode = isDark ? 'light' : 'dark';
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
