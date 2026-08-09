(() => {
  const html = document.documentElement;
  // ?mode=dark|light overrides frontmatter's baked mode for this view only,
  // applied before anything else so there is no flash of the wrong palette.
  const query = new URLSearchParams(location.search);
  const qMode = query.get('mode');
  if (qMode === 'dark' || qMode === 'light') html.dataset.mode = qMode;
  // ?presenter=1 is the same file, opened a second time with a flag. Set here
  // rather than in presenter.js so the layout is right on the first paint.
  const PRESENTING = query.get('presenter') === '1';
  if (PRESENTING) html.dataset.presenter = '1';

  const deck = document.getElementById('deck');
  const hud = document.getElementById('hud');
  const notesPanel = document.getElementById('notes-panel');
  const W = +deck.dataset.slideW, H = +deck.dataset.slideH;
  // Live updates replace DOM nodes, so re-query the slide list each time.
  const slides = () => Array.from(document.querySelectorAll('section.slide'));
  let cur = Math.min(Math.max(parseInt((location.hash || '').slice(1)) || 1, 1), slides().length) - 1;

  // The animation runtime, present only in decks that animate something.
  // Everything below has to work without it.
  const anim = window.MZAnim || null;
  let transition = null;
  try { transition = JSON.parse(deck.dataset.transition || 'null'); } catch (e) {}
  // How far through the current slide's click steps we are.
  let step = 0;
  // The annotation overlay is loaded after this file, and is absent from decks
  // that annotate nothing, so it is looked up when needed rather than captured.
  const annot = () => window.MZAnnot || null;
  // A slide's steps are however many the animation and the annotations
  // between them ask for: either may be the last thing waiting for a click.
  const stepsOn = (sec) => {
    if (!sec) return 0;
    const a = anim ? anim.steps(sec) : 0;
    const n = annot();
    return Math.max(a, n ? n.steps(sec) : 0);
  };
  const showStep = (sec) => {
    const n = annot();
    if (n && sec) n.show(sec, step);
  };

  // Every slide's markup as it was before anything ran. The presenter window's
  // next-slide preview is built from this rather than from the live DOM: the
  // animation runtime writes inline styles onto elements to arm them, and a
  // preview built from an armed slide would show a slide with holes in it.
  let pristine = slides().map((s) => s.outerHTML);

  function fit() {
    // In the presenter window the deck lives inside a box, not the viewport.
    const host = deck.parentElement === document.body ? null : deck.parentElement;
    const box = host
      ? host.getBoundingClientRect()
      : { width: innerWidth, height: innerHeight };
    const s = Math.min(box.width / (W + 40), box.height / (H + 40));
    deck.style.width = W + 'px';
    deck.style.height = H + 'px';
    deck.style.transform = `translate(-50%, -50%) scale(${s})`;
  }

  // `play` is what separates a page turn from a repaint: a resize, a font
  // load and a live-reload patch all land here too, and none of them should
  // replay the slide's entrance.
  function show(i, opts) {
    const play = !opts || opts.play !== false;
    const ss = slides();
    const from = ss[cur];
    const idx = Math.min(Math.max(i, 0), ss.length - 1);
    // Navigating to the slide already showing - advancing past the end,
    // retreating before the start, End on the last page - is not an arrival,
    // and must not replay the slide's entrance. Only the initial paint plays
    // in place.
    if (play && from && idx === cur && !(opts && opts.first)) return;
    const backwards = idx < cur;
    const changed = from && idx !== cur;
    // Parts of a slide broken by `<!-- next -->` are one slide the author chose
    // to serve in instalments: every other pane holds the same elements in the
    // same places, so moving between them is a cut, not a page turn.
    const group = (s) => (s && s.dataset.cont) || null;
    const cut = changed && group(from) !== null && group(from) === group(ss[idx]);
    const turn = play && !cut;
    if (changed && turn) leave(from, backwards);
    else if (changed && anim) anim.settle(from);
    cur = idx;
    const sec = ss[cur];
    ss.forEach((s, j) => s.classList.toggle('active', j === cur));
    // Arriving from a later slide means arriving at a slide already fully
    // revealed; arriving forwards means starting from its first step.
    if (changed) step = backwards ? stepsOn(sec) : 0;
    // Shrink-to-fit measures boxes, and a slide only has boxes once it is the
    // one displayed - so it runs here, before anything else measures anything.
    if (window.__mirzamFit) window.__mirzamFit(sec);
    if (anim) anim.show(sec, step, transition, { play: turn, backwards: backwards && changed, arriving: changed });
    showStep(sec);
    updateHud(ss.length, sec);
    // replaceState throws inside srcdoc iframes (editor previews). Recording the
    // page position is optional, so a failure must not abort the rest of the update.
    try { history.replaceState(null, '', '#' + (cur + 1)); } catch (e) {}
    renderNotes();
    // Connectors resolve their endpoints once layout has settled.
    requestAnimationFrame(() => drawConnectors(sec));
  }

  // Everything that moves the deck ends here, which makes it the one place a
  // second window has to be told about.
  function updateHud(total, sec) {
    const n = stepsOn(sec);
    hud.textContent = `${cur + 1} / ${total}` + (n ? ` · ${step}/${n}` : '');
    notify();
  }
  const watchers = [];
  const notify = () => { for (const fn of watchers) fn(); };

  // The slide being left has to stay painted for as long as its exit takes,
  // so it animates out over the slide arriving rather than vanishing first.
  let leaveTimer = null;
  function leave(sec, backwards) {
    if (!anim) return;
    const ms = anim.leave(sec, transition, backwards);
    clearTimeout(leaveTimer);
    document.querySelectorAll('section.mz-leaving').forEach((s) => s.classList.remove('mz-leaving'));
    if (!ms) { anim.settle(sec); return; }
    sec.classList.add('mz-leaving');
    leaveTimer = setTimeout(() => {
      sec.classList.remove('mz-leaving');
      anim.settle(sec);
    }, ms + 30);
  }

  // Forward through the slide's click steps first, then on to the next slide.
  function advance() {
    const sec = slides()[cur];
    if (step < stepsOn(sec)) {
      step += 1;
      if (anim) anim.step(sec, step);
      showStep(sec);
      updateHud(slides().length, sec);
    } else {
      show(cur + 1);
    }
  }

  function retreat() {
    const sec = slides()[cur];
    if (step > 0) {
      if (anim) anim.unstep(sec, step);
      step -= 1;
      showStep(sec);
      updateHud(slides().length, sec);
    } else {
      show(cur - 1);
    }
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

  // ---- Table of contents ----
  // An entry is an ordinary link to the slide number, so it works with no
  // JavaScript at all; all the runtime does is notice the address changed.
  addEventListener('hashchange', () => {
    const n = parseInt((location.hash || '').slice(1));
    if (n >= 1 && n <= slides().length) show(n - 1);
  });

  // `current: true` asks the list to say where the talk has got to: the entry
  // marked is the last one at or before the slide on screen, which is the
  // section the presenter is inside rather than the heading they last passed.
  function markCurrent() {
    const lists = document.querySelectorAll('nav.mz-toc[data-current]');
    for (const nav of lists) {
      let here = null;
      for (const li of nav.querySelectorAll('li[data-slide]')) {
        li.classList.remove('mz-toc-here');
        if (+li.dataset.slide <= cur) here = li;
      }
      if (here) here.classList.add('mz-toc-here');
    }
  }
  watchers.push(markCurrent);

  function renderNotes() {
    const notes = slides()[cur]?.querySelector('aside.notes');
    notesPanel.innerHTML = '<h4>SPEAKER NOTES</h4>' +
      (notes && notes.innerHTML.trim() ? notes.innerHTML : '<em>(no notes for this slide)</em>');
  }

  // Re-route the current slide's connectors. The annotation overlay calls
  // this after it draws, because a connector may point at a mark that does
  // not exist until the overlay has laid it out.
  window.__mirzamConnectors = () => drawConnectors(slides()[cur]);

  // What a second window needs to follow this one and to drive it. Deliberately
  // small: the presenter window is not privileged, it is another viewer.
  window.MZDeck = {
    presenting: PRESENTING,
    state: () => ({ slide: cur, step, total: slides().length }),
    // How the deck is being *looked at*, as opposed to where it is. Dark mode
    // and the layout outline are properties of the deck, not of one window:
    // a presenter who turns the lights down means both screens.
    view: () => ({ mode: html.dataset.mode || '', debug: html.classList.contains('mz-debug') }),
    setView(v) {
      if (v.mode) html.dataset.mode = v.mode; else delete html.dataset.mode;
      html.classList.toggle('mz-debug', !!v.debug);
    },
    // The slide as it was authored, for a preview that must not inherit this
    // window's animation state.
    html: (i) => pristine[i] || '',
    onChange: (fn) => watchers.push(fn),
    refit: () => { fit(); show(cur, { play: false }); },
    advance,
    retreat,

    // Absolute rather than incremental: a window that opened late, or missed a
    // message, lands on the right slide anyway. A different slide still turns
    // the page properly - the audience is watching this one too.
    sync(i, n) {
      const ss = slides();
      if (i !== cur) show(Math.max(0, Math.min(i, ss.length - 1)));
      const sec = ss[cur];
      const want = Math.max(0, Math.min(n, stepsOn(sec)));
      while (step < want) { step += 1; if (anim) anim.step(sec, step); }
      while (step > want) { if (anim) anim.unstep(sec, step); step -= 1; }
      showStep(sec);
      updateHud(ss.length, sec);
    },
  };

  // Restore the current page after a live update. An edit must not replay the
  // slide's entrance: the presenter is looking at a step, not at slide one.
  window.__mirzamRefresh = () => {
    pristine = slides().map((s) => s.outerHTML);
    show(cur, { play: false });
  };
  // Let a host (editor extension) jump to a specific slide. Cursor sync fires
  // on every keystroke, so this follows the cursor without animating.
  window.__mirzamGoto = (i) => show(i, { play: false });

  // ---- The cheat sheet ----
  // Every key the viewer answers to, plus the ones only this deck knows: the
  // `effects` bindings are per-slide and nobody can guess them, which is the
  // whole reason this overlay exists.
  const keysPanel = document.getElementById('keys');
  // Set when a gesture has already decided what a touch meant, so the
  // synthetic click that follows it does not undo the gesture: a long press
  // opens the sheet, and the click it produces lands on the sheet it just
  // opened.
  let handled = false;
  const KEYS = [
    ['Navigate', [
      [['→', 'Space'], 'Next step, then next slide'],
      [['←'], 'Back a step, then back a slide'],
      [['Home', 'End'], 'First / last slide'],
    ]],
    ['Display', [
      [['N'], 'Speaker notes'],
      [['P'], 'Presenter window'],
      [['F'], 'Fullscreen'],
      [['D'], 'Dark / light'],
      [['L'], 'Outline the layout'],
    ]],
  ];
  // On a phone the keys above are all unreachable, so the sheet leads with the
  // gestures instead. `pointer: coarse` is the primary pointer, so a laptop
  // with a touchscreen still gets the keyboard first.
  const COARSE = matchMedia('(pointer: coarse)').matches;
  const TOUCH = ['Touch', [
    [['Swipe ←', 'Swipe →'], 'Next / previous'],
    [['Swipe ↑', 'Swipe ↓'], 'Show / hide notes'],
    [['Two-finger tap'], 'This sheet'],
    [['Tap left', 'Tap right'], 'Back / forward'],
    [['Long press'], 'Select text, as anywhere else'],
  ]];
  const esc = (s) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
  const row = (keys, what) =>
    `<dt>${keys.map((k) => `<kbd>${esc(k)}</kbd>`).join('')}</dt><dd>${esc(what)}</dd>`;

  // Effects live in the same per-slide JSON `effects.js` reads. Reading the tag
  // rather than asking that file keeps the sheet working in a deck that binds
  // no effects at all, where the file is not inlined.
  function effectRows(sec) {
    const tag = sec && sec.querySelector(':scope > script.mz-fx');
    if (!tag) return '';
    let list = [];
    try { list = JSON.parse(tag.textContent) || []; } catch (e) { return ''; }
    if (!list.length) return '';
    const items = list.map((b) => row([b.key], b.arg ? `${b.effect} — ${b.arg}` : b.effect));
    items.push(row(['Esc'], 'Clear whatever is flying'));
    return `<h4>On this slide</h4><dl>${items.join('')}</dl>`;
  }

  function renderKeys() {
    const groups = (COARSE ? [TOUCH, ...KEYS] : KEYS).map(([name, rows]) =>
      `<h4>${name}</h4><dl>${rows.map(([k, w]) => row(k, w)).join('')}</dl>`).join('');
    const fx = effectRows(slides()[cur]);
    keysPanel.innerHTML = '<div class="mz-keys-card">' +
      `<div class="mz-keys-cols"><div>${groups}</div>` +
      (fx ? `<div>${fx}</div>` : '') + '</div>' +
      '<p class="mz-keys-close"><kbd>/</kbd> or <kbd>Esc</kbd> to close</p></div>';
  }

  function toggleKeys(on) {
    const want = on === undefined ? keysPanel.hidden : on;
    if (want) renderKeys();
    keysPanel.hidden = !want;
  }
  keysPanel.addEventListener('click', () => {
    if (handled) { handled = false; return; }
    toggleKeys(false);
  });

  // ---- The control cluster ----
  // Quiet until someone reaches for it. `mz-awake` only drives an opacity, so
  // waking the chrome mid-sentence cannot reflow the slide.
  let sleepTimer = null;
  function wake() {
    html.classList.add('mz-awake');
    clearTimeout(sleepTimer);
    sleepTimer = setTimeout(() => html.classList.remove('mz-awake'), 2500);
  }
  addEventListener('pointermove', wake, { passive: true });
  document.getElementById('mz-prev').addEventListener('click', retreat);
  document.getElementById('mz-next').addEventListener('click', advance);
  document.getElementById('mz-help').addEventListener('click', () => toggleKeys());

  addEventListener('keydown', (e) => {
    // A modified key belongs to the browser: Cmd-R, Ctrl-F, Alt-Tab.
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === '/' || e.key === '?') { e.preventDefault(); toggleKeys(); return; }
    // Escape closes the sheet. It also clears effects, which `effects.js`
    // handles on its own listener, so this one only owns the overlay.
    if (e.key === 'Escape') { toggleKeys(false); return; }
    if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') { e.preventDefault(); advance(); }
    else if (e.key === 'ArrowLeft' || e.key === 'PageUp') { e.preventDefault(); retreat(); }
    else if (e.key === 'Home') show(0);
    else if (e.key === 'End') show(slides().length - 1);
    else if (e.key === 'n' || e.key === 'N') notesPanel.hidden = !notesPanel.hidden;
    // Absent in a deck built without the presenter script; the key then does
    // nothing rather than throwing and taking navigation down with it.
    else if (e.key === 'p' || e.key === 'P') { if (window.MZPresenter) window.MZPresenter.open(); }
    else if (e.key === 'f' || e.key === 'F') {
      document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen();
    }
    else if (e.key === 'l' || e.key === 'L') {
      html.classList.toggle('mz-debug');
      notify();
    }
    else if (e.key === 'd' || e.key === 'D') {
      // No data-mode attribute yet means the OS preference is in effect;
      // read that to know which way "toggle" should go the first time.
      const isDark = html.dataset.mode
        ? html.dataset.mode === 'dark'
        : matchMedia('(prefers-color-scheme: dark)').matches;
      html.dataset.mode = isDark ? 'light' : 'dark';
      notify();
    }
  });

  // ---- Touch: a phone has no keyboard ----
  // Swipe to turn the page, swipe up for notes, a two-finger tap for the cheat
  // sheet. The click zones stay, because that is what a presenter with a
  // clicker or a trackpad is using.
  //
  // There is deliberately no long-press binding. On a phone the long press is
  // how you select text, and taking it for the cheat sheet took the reader's
  // ability to copy a line off a slide with it. The two-finger tap and the `?`
  // button — which touch wakes like a pointer does — cover the same ground and
  // collide with nothing.
  const SWIPE = 45;      // px before a drag counts as a swipe rather than a tap
  let touch = null;
  const selecting = () => (getSelection()?.toString() || '').trim() !== '';

  addEventListener('touchstart', (e) => {
    wake();
    if (e.touches.length > 1) { touch = null; handled = true; toggleKeys(); return; }
    const t = e.touches[0];
    touch = { x: t.clientX, y: t.clientY, held: selecting() };
    handled = false;
  }, { passive: true });

  addEventListener('touchend', (e) => {
    if (!touch) return;
    const t = e.changedTouches[0];
    const dx = t.clientX - touch.x, dy = t.clientY - touch.y;
    const wasSelecting = touch.held;
    touch = null;
    if (Math.max(Math.abs(dx), Math.abs(dy)) < SWIPE) return;   // a tap, or a scroll
    // Dragging a selection handle travels exactly as far as a swipe does.
    // Whichever way it is read, a reader adjusting a selection is not asking
    // for the next slide.
    if (wasSelecting || selecting()) return;
    handled = true;
    if (Math.abs(dx) > Math.abs(dy)) dx < 0 ? advance() : retreat();
    else notesPanel.hidden = dy >= 0;
  }, { passive: true });

  // Click to advance, but never while the reader is selecting text or using a
  // control: a drag that ends on the deck is a selection, not a page turn.
  let downAt = null;
  deck.addEventListener('pointerdown', (e) => { downAt = { x: e.clientX, y: e.clientY }; });
  deck.addEventListener('click', (e) => {
    const moved = downAt
      ? Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) > 6
      : false;
    downAt = null;
    // A gesture already decided what this touch meant.
    if (handled) { handled = false; return; }
    if (moved) return;
    if ((getSelection()?.toString() || '').trim()) return;
    if (e.target.closest('a, video, details, summary, button, input')) return;
    const r = deck.getBoundingClientRect();
    (e.clientX - r.left) / r.width < 0.3 ? retreat() : advance();
  });

  // A repaint, not a page turn: keep the slide exactly where the presenter
  // left it, animations and all.
  addEventListener('resize', () => { fit(); show(cur, { play: false }); });
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(() => show(cur, { play: false }));
  fit();
  show(cur, { first: true });
})();
