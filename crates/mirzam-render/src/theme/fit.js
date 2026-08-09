// Shrink-to-fit for a pane that has more in it than it can show.
//
// A pane clips what does not fit, which is the right default: it keeps the
// layout you drew, and the layout checker reports the overflow before anyone
// presents it. But a deck written the night before is not run through a
// checker, and text that silently disappears is the worst way to find out.
// `{fit}` on a pane, or `fit: shrink` in frontmatter, trades the type size for
// keeping the words.
//
// This is inlined into the print page as well. It only ever makes content
// smaller than the box it is already overflowing, so a page that runs it shows
// strictly more than one that does not — the same reason the annotation
// overlay is allowed there.
(() => {
  const MIN = 0.55;          // below this the text is unreadable anyway
  const STEP = 0.04;
  // Either the pane opted in, or the deck did.
  const selector = () => {
    const deck = document.getElementById('deck');
    return deck && deck.dataset.fit === 'shrink' ? '.pane' : '.pane.mz-fit';
  };
  const panes = (root) => (root || document).querySelectorAll(selector());

  // How far past its box the content runs. Measured on a wrapper rather than
  // the pane, because a pane is the thing doing the clipping: its own
  // scrollHeight stops growing once the overflow is hidden in some browsers.
  function overflow(pane) {
    const inner = pane.__mzFit;
    const cs = getComputedStyle(pane);
    const padY = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const padX = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
    return {
      y: inner.scrollHeight - (pane.clientHeight - padY),
      x: inner.scrollWidth - (pane.clientWidth - padX),
    };
  }

  function wrap(pane) {
    if (pane.__mzFit) return;
    const inner = document.createElement('div');
    inner.className = 'mz-fit-inner';
    while (pane.firstChild) inner.appendChild(pane.firstChild);
    pane.appendChild(inner);
    pane.__mzFit = inner;
  }

  function fit(pane) {
    // A slide that is not on screen has no boxes: every pane on it measures
    // zero high, which reads as "overflowing by everything" and shrinks the
    // text to the floor. Only what is displayed can be measured, so the viewer
    // calls back here on every page turn.
    if (!pane.clientHeight) return;
    wrap(pane);
    const inner = pane.__mzFit;
    inner.style.fontSize = '';
    let scale = 1;
    // Straight search rather than bisection: the relationship between font
    // size and wrapped height is not monotone enough for bisection to be
    // safe, and a dozen reflows of one pane is nothing at page-turn time.
    while (scale > MIN) {
      const o = overflow(pane);
      if (o.y <= 1 && o.x <= 1) break;
      scale -= STEP;
      inner.style.fontSize = `${scale.toFixed(2)}em`;
    }
    pane.dataset.mzFit = scale.toFixed(2);
  }

  // `root` narrows the work to one slide, which is what a page turn needs.
  const all = (root) => panes(root).forEach(fit);

  function init() {
    if (!panes().length) return;
    // The viewer calls this on every page turn, because a slide can only be
    // measured while it is the one on screen. In print every slide is laid
    // out at once, so the sweep below is all it takes there.
    window.__mirzamFit = all;
    all();
    const again = () => all();
    addEventListener('resize', again);
    addEventListener('load', again);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(again);
  }

  if (document.readyState === 'loading') addEventListener('DOMContentLoaded', init);
  else init();
})();
