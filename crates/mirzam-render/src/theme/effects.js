// Presentation effects: flourishes the speaker fires with a key.
//
// These are not animations, and the split is deliberate. An animation belongs
// to the document — ordered, deterministic, exported. An effect belongs to the
// performance: it happens because someone pressed a key in front of an
// audience, it never reaches the PDF, and a talk where none of them fire is
// the same talk. So nothing here may change the document. Every effect draws
// into a throwaway layer above the slide and takes that layer with it when it
// finishes.
//
// House rules, which are what keep a flourish from wrecking a live talk:
//   - compositor properties only (transform, opacity, filter) — no effect may
//     cause a reflow while the speaker is mid-sentence
//   - an effect in flight is cancelled when the slide changes
//   - Escape clears everything, immediately
//   - `prefers-reduced-motion` drops the movement and keeps the flash short
(() => {
  const REDUCED = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const rand = (a, b) => a + Math.random() * (b - a);

  // Every effect draws here: one layer per firing, removed when it ends, so a
  // finished effect leaves no trace in the DOM for the next one to trip over.
  function layerOn(sec) {
    const el = document.createElement('div');
    el.className = 'mz-fx-layer';
    el.setAttribute('aria-hidden', 'true');
    sec.appendChild(el);
    return el;
  }

  const live = new Set();

  function run(sec, build, ms) {
    const layer = layerOn(sec);
    const entry = { layer, timer: 0 };
    live.add(entry);
    build(layer);
    entry.timer = setTimeout(() => { layer.remove(); live.delete(entry); }, ms + 60);
    return entry;
  }

  function clearAll() {
    for (const e of live) { clearTimeout(e.timer); e.layer.remove(); }
    live.clear();
    for (const s of document.querySelectorAll('.mz-fx-shake')) s.classList.remove('mz-fx-shake');
  }

  // A node that animates once and is thrown away with its layer. Keyframes are
  // passed straight to the Web Animations API rather than written as CSS, so an
  // effect never adds a rule the rest of the deck has to live with.
  function fly(layer, node, frames, opts) {
    layer.appendChild(node);
    node.animate(frames, { fill: 'both', easing: 'cubic-bezier(.22,.61,.36,1)', ...opts });
    return node;
  }

  const span = (text, style) => {
    const el = document.createElement('span');
    el.textContent = text;
    el.style.cssText = style;
    return el;
  };

  const EFFECTS = {
    // A single bright pulse over the whole slide: "look here, now".
    flash(sec) {
      const ms = REDUCED ? 160 : 420;
      run(sec, (layer) => {
        fly(layer, span('', 'position:absolute;inset:0;background:var(--mz-fx-flash)'),
          [{ opacity: 0 }, { opacity: .85, offset: .12 }, { opacity: 0 }], { duration: ms });
      }, ms);
    },

    // The slide itself shakes. This is the one effect that touches an existing
    // element, so it uses a class it removes again rather than an inline style
    // the slide might already be using for something else.
    shake(sec) {
      if (REDUCED) return EFFECTS.flash(sec);
      sec.classList.add('mz-fx-shake');
      setTimeout(() => sec.classList.remove('mz-fx-shake'), 520);
    },

    // Emoji thrown upward from the bottom edge, tumbling as they rise.
    burst(sec, arg) {
      const ms = 1600;
      const text = arg || '🎉';
      run(sec, (layer) => {
        for (let i = 0; i < 26; i++) {
          const x = rand(4, 96), size = rand(24, 54);
          const node = span(text, `position:absolute;left:${x}%;bottom:-8%;font-size:${size}px;will-change:transform`);
          const rise = rand(60, 108), drift = rand(-16, 16), spin = rand(-220, 220);
          fly(layer, node, REDUCED
            ? [{ opacity: 0 }, { opacity: 1 }]
            : [
              { transform: 'translate(0,0) rotate(0deg)', opacity: 0 },
              { opacity: 1, offset: .1 },
              { transform: `translate(${drift}vw,-${rise}vh) rotate(${spin}deg)`, opacity: 0 },
            ], { duration: ms, delay: i * 26 });
        }
      }, ms + 26 * 26);
    },

    // Confetti: the same idea in paper rather than emoji, for when a slide is
    // already carrying enough symbols.
    confetti(sec) {
      const ms = 1800;
      const colors = ['var(--mz-accent1)', 'var(--mz-accent2)', 'var(--mz-fg)'];
      run(sec, (layer) => {
        for (let i = 0; i < 60; i++) {
          const node = span('', `position:absolute;left:${rand(0, 100)}%;top:-6%;width:${rand(6, 12)}px;` +
            `height:${rand(8, 16)}px;background:${colors[i % 3]};border-radius:2px;will-change:transform`);
          fly(layer, node, REDUCED
            ? [{ opacity: 0 }, { opacity: 1 }]
            : [
              { transform: 'translateY(0) rotate(0deg)', opacity: 1 },
              { transform: `translate(${rand(-8, 8)}vw, 112vh) rotate(${rand(-540, 540)}deg)`, opacity: .9 },
            ], { duration: rand(ms * .6, ms), delay: i * 12, easing: 'cubic-bezier(.3,.1,.6,1)' });
        }
      }, ms + 60 * 12);
    },

    // 集中線: lines converging on the middle of the slide.
    lines(sec) {
      const ms = REDUCED ? 240 : 700;
      run(sec, (layer) => {
        for (let i = 0; i < 44; i++) {
          const ang = (i / 44) * 360 + rand(-3, 3);
          const len = rand(18, 40);
          const node = span('', 'position:absolute;left:50%;top:50%;height:3px;transform-origin:0 50%;' +
            `width:${len}%;background:linear-gradient(90deg,transparent,var(--mz-fx-line));will-change:transform`);
          fly(layer, node, [
            { transform: `rotate(${ang}deg) translateX(120%) scaleX(1.4)`, opacity: 0 },
            { opacity: .9, offset: .35 },
            { transform: `rotate(${ang}deg) translateX(38%) scaleX(.5)`, opacity: 0 },
          ], { duration: ms, delay: rand(0, 90) });
        }
      }, ms + 120);
    },

    // An explosion out of the centre.
    boom(sec) {
      const ms = REDUCED ? 220 : 780;
      run(sec, (layer) => {
        fly(layer, span('', 'position:absolute;left:50%;top:50%;width:22vmin;height:22vmin;margin:-11vmin 0 0 -11vmin;' +
          'border-radius:50%;border:6px solid var(--mz-fx-line);will-change:transform'),
          [{ transform: 'scale(.1)', opacity: 1 }, { transform: 'scale(3.4)', opacity: 0 }], { duration: ms });
        if (REDUCED) return;
        for (let i = 0; i < 34; i++) {
          const ang = rand(0, Math.PI * 2), dist = rand(24, 62);
          const node = span('', `position:absolute;left:50%;top:50%;width:${rand(6, 14)}px;height:${rand(6, 14)}px;` +
            'border-radius:50%;background:var(--mz-fx-spark);will-change:transform');
          fly(layer, node, [
            { transform: 'translate(-50%,-50%) scale(1)', opacity: 1 },
            {
              transform: `translate(calc(-50% + ${Math.cos(ang) * dist}vmin), calc(-50% + ${Math.sin(ang) * dist}vmin)) scale(.2)`,
              opacity: 0,
            },
          ], { duration: rand(ms * .6, ms) });
        }
      }, ms + 40);
    },

    // A Nico-Nico-style comment sweeping across the slide.
    danmaku(sec, arg) {
      const ms = REDUCED ? 2200 : 4200;
      run(sec, (layer) => {
        const node = span(arg || '', 'position:absolute;left:100%;top:' + rand(8, 72) + '%;' +
          'font-size:44px;font-weight:700;white-space:nowrap;color:var(--mz-fx-danmaku);' +
          'text-shadow:0 2px 6px rgba(0,0,0,.55);will-change:transform');
        fly(layer, node, [
          { transform: 'translateX(0)' },
          { transform: 'translateX(calc(-100vw - 100%))' },
        ], { duration: ms, easing: 'linear' });
      }, ms);
    },
  };

  // The bindings of the slide currently on screen. Read fresh each time: live
  // reload replaces sections, and the presenter may have moved on.
  function bindingsFor(sec) {
    const tag = sec && sec.querySelector(':scope > script.mz-fx');
    if (!tag) return [];
    try { return JSON.parse(tag.textContent) || []; } catch (e) { return []; }
  }

  const current = () => document.querySelector('section.slide.active') || document.querySelector('section.slide');

  addEventListener('keydown', (e) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'Escape') { clearAll(); return; }
    const sec = current();
    if (!sec) return;
    const hit = bindingsFor(sec).find((b) => b.key === e.key || b.key.toLowerCase() === e.key.toLowerCase());
    if (!hit) return;
    const fx = EFFECTS[hit.effect];
    if (!fx) return;
    e.preventDefault();
    fx(sec, hit.arg);
  });

  // A flourish belongs to the slide it was fired on. Leaving the slide takes
  // it with us rather than letting it play over whatever comes next. The
  // viewer marks the current slide with `active`, so watching class changes
  // catches every way a page can turn — key, click, hash, live reload.
  //
  // What it must *not* catch is this file's own writing: `shake` sets a class
  // on the slide, and cancelling on any class change made shake cancel itself.
  // So the trigger is the active section actually changing, not a mutation
  // having happened.
  const deck = document.getElementById('deck');
  if (deck) {
    let on = current();
    new MutationObserver(() => {
      const now = current();
      if (now === on) return;
      on = now;
      clearAll();
    }).observe(deck, { subtree: true, attributes: true, attributeFilter: ['class'] });
  }
})();
