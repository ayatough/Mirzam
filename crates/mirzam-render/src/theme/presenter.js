// The presenter window, and the link that keeps two windows in step.
//
// `P` opens the same file a second time with `?presenter=1`. There is no
// second document and no server: a deck is one file, and the presenter view is
// that file rendered differently. Both windows run this script, and neither is
// privileged — driving from either moves both.
//
// What crosses the link is *state*, not commands: `{slide, step}`, absolute.
// A window that opened late, or missed a message, still lands in the right
// place, and a peer that closes strands nobody.
(() => {
  const deck = window.MZDeck;
  if (!deck) return;                       // no viewer: nothing to present
  const html = document.documentElement;
  const W = +document.getElementById('deck').dataset.slideW;
  const H = +document.getElementById('deck').dataset.slideH;

  // ---- The link ----
  // BroadcastChannel covers two tabs of a served deck. A pair of `file://`
  // windows have opaque origins and never meet on one, so the opener/child
  // window references carry the same messages. Sending on both and ignoring
  // our own id makes the duplicate harmless.
  const ME = Math.random().toString(36).slice(2);
  const CHANNEL = 'mirzam:' + location.pathname;
  let bc = null;
  try { bc = new BroadcastChannel(CHANNEL); } catch (e) {}
  let child = null;                        // the presenter window we opened
  let muted = false;                       // applying a peer's state
  let last = null;                         // what we last told the peers

  const peers = () => [child, window.opener].filter((w) => w && !w.closed && w !== window);

  function post(msg) {
    msg.from = ME;
    try { if (bc) bc.postMessage(msg); } catch (e) {}
    for (const w of peers()) { try { w.postMessage(msg, '*'); } catch (e) {} }
  }

  function tell(force) {
    if (muted) return;
    const s = deck.state();
    const key = s.slide + ':' + s.step;
    // Without this, two windows holding identical state would answer each
    // other forever: every message provokes an update, and every update a
    // message.
    if (!force && key === last) return;
    last = key;
    post({ slide: s.slide, step: s.step });
  }

  function receive(msg) {
    if (!msg || msg.from === ME) return;
    if (msg.hello) { last = null; tell(true); return; }
    if (typeof msg.slide !== 'number') return;
    muted = true;
    try {
      deck.sync(msg.slide, msg.step);
      last = msg.slide + ':' + msg.step;
    } finally { muted = false; }
  }

  if (bc) bc.onmessage = (e) => receive(e.data);
  addEventListener('message', (e) => receive(e.data));
  deck.onChange(() => { tell(false); if (deck.presenting) paint(); });

  window.MZPresenter = {
    open() {
      if (deck.presenting) { if (window.opener && !window.opener.closed) window.opener.focus(); return; }
      if (child && !child.closed) { child.focus(); return; }
      const u = new URL(location.href);
      u.searchParams.set('presenter', '1');
      child = open(u.href, 'mirzam-presenter', 'width=1280,height=820');
    },
  };

  // Say hello once, so whichever window opened second adopts the other's
  // place in the deck rather than starting from slide one.
  addEventListener('load', () => post({ hello: true }));

  if (!deck.presenting) return;

  // ---- The presenter layout ----
  document.title = 'Presenter · ' + document.title;
  const panel = document.createElement('div');
  panel.id = 'presenter';
  panel.innerHTML =
    '<div id="pv-top">' +
      '<div class="pv-box"><h5>Now<span id="pv-pos"></span></h5><div id="pv-now"></div></div>' +
      '<div id="pv-side">' +
        '<div class="pv-box"><h5>Next</h5><div id="pv-next"></div></div>' +
        '<div class="pv-box" id="pv-time">' +
          '<div id="pv-clock"></div>' +
          '<button id="pv-timer" type="button" title="Click to restart">0:00</button>' +
        '</div>' +
      '</div>' +
    '</div>' +
    '<div class="pv-box" id="pv-notes"><h5>Notes</h5><div id="pv-notes-body"></div></div>';
  document.body.appendChild(panel);

  const now = document.getElementById('pv-now');
  const pos = document.getElementById('pv-pos');
  const next = document.getElementById('pv-next');
  const notes = document.getElementById('pv-notes-body');
  const clockEl = document.getElementById('pv-clock');
  const timerEl = document.getElementById('pv-timer');
  // The one deck this window has, moved into its box. `fit()` measures the
  // box rather than the window as soon as it is no longer a child of <body>.
  now.appendChild(document.getElementById('deck'));

  // The next slide, rendered from the authored markup at the deck's own size
  // and scaled down to the box. Static on purpose: it is a preview, not a
  // second deck, so nothing here animates, annotates or draws connectors.
  const stage = document.createElement('div');
  stage.className = 'pv-stage';
  stage.style.width = W + 'px';
  stage.style.height = H + 'px';
  next.appendChild(stage);

  let shown = -1;
  function paint() {
    const s = deck.state();
    const i = s.slide + 1;
    // The page counter lives in the chrome, which this window hides.
    pos.textContent = `${s.slide + 1} / ${s.total}` + (s.step ? ` · ${s.step}` : '');
    if (i !== shown) {
      shown = i;
      const src = deck.html(i);
      stage.innerHTML = src
        ? src.replace('<section class="slide"', '<section class="slide active"')
        : '<div class="pv-end">End of deck</div>';
      // A cloned slide must never be mistaken for the real one by anything
      // that walks the document looking for slides.
      const sec = stage.querySelector('section.slide');
      if (sec) { sec.classList.remove('slide'); sec.classList.add('pv-slide'); }
    }
    const live = document.querySelector('section.slide.active aside.notes');
    notes.innerHTML = live && live.innerHTML.trim() ? live.innerHTML : '<em>(no notes)</em>';
  }

  function scaleNext() {
    const box = next.getBoundingClientRect();
    if (!box.width) return;
    stage.style.transform = `scale(${box.width / W})`;
    next.style.height = (box.width * H / W) + 'px';
  }

  // ---- Clock and timer ----
  let started = Date.now();
  timerEl.addEventListener('click', () => { started = Date.now(); tick(); });
  function tick() {
    const d = new Date();
    clockEl.textContent = d.toTimeString().slice(0, 5);
    const s = Math.floor((Date.now() - started) / 1000);
    const mm = Math.floor(s / 60), ss = s % 60;
    timerEl.textContent = `${mm}:${ss < 10 ? '0' : ''}${ss}`;
  }
  setInterval(tick, 1000);
  tick();

  function relayout() { scaleNext(); deck.refit(); }
  addEventListener('resize', relayout);
  requestAnimationFrame(() => { relayout(); paint(); });
})();
