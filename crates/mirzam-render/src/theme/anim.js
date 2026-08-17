// The animation runtime: plays the timelines `mirzam-anim` compiled into each
// slide, and the deck's page-turn transition. Inlined only into decks that
// have animation, so a deck without it pays nothing.
//
// The resting state rule: elements are laid out in their *final* state, and
// this file is the only thing that ever puts one in its initial state. No
// JavaScript, and the print/PDF path which ships no scripts at all, therefore
// both show the finished slide. Nothing here is required for a deck to read.
//
// The viewer owns which slide and which step is current; this owns what that
// means visually. The interface is `window.MZAnim`, and every entry point is
// safe to call on a slide with no timeline.
(() => {
  const REDUCED = matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Every property arming may write. Saved per element before the first write
  // and put back afterwards, so an inline style the renderer set (a pane's
  // grid-area, a blurred background's transform) is never lost.
  const PROPS = ['opacity', 'transform', 'transformOrigin', 'clipPath', 'filter',
                 'strokeDasharray', 'strokeDashoffset', 'fillOpacity'];
  const PAINTED = 'path,line,polyline,polygon,circle,ellipse,rect,text';

  // Live reload replaces whole <section> elements, so keying on the element
  // invalidates the cache for free.
  const timelines = new WeakMap();
  const playing = new WeakMap();

  function save(el) {
    if (el.__mzSaved) return;
    const s = {};
    for (const p of PROPS) s[p] = el.style[p];
    el.__mzSaved = s;
  }

  function unsave(el) {
    const s = el.__mzSaved;
    if (!s) return;
    for (const p of PROPS) el.style[p] = s[p];
    el.__mzSaved = null;
  }

  // `dir` is the direction the element travels. Entering from the opposite
  // side and leaving towards `dir` is what makes a page turn read as one
  // movement rather than two unrelated ones.
  function travel(dir, far) {
    // A whole slide travels its own width, clipped by the deck; a paragraph
    // travels far enough to read as movement and no further.
    const d = far ? '100%' : '40px';
    return {
      left:  ['translateX(' + d + ')', 'translateX(-' + d + ')'],
      right: ['translateX(-' + d + ')', 'translateX(' + d + ')'],
      up:    ['translateY(' + d + ')', 'translateY(-' + d + ')'],
      down:  ['translateY(-' + d + ')', 'translateY(' + d + ')'],
    }[dir] || ['none', 'none'];
  }

  // The collapsed clip for a wipe travelling in `dir`. Entering, the content
  // is hidden on the side the edge starts from; leaving, on the side it ends.
  function wipe(dir, entering) {
    const side = entering
      ? { right: 'right', left: 'left', down: 'bottom', up: 'top' }[dir]
      : { right: 'left', left: 'right', down: 'top', up: 'bottom' }[dir];
    // Keyed by the edge the clip eats in from, in `inset(top right bottom left)`.
    return {
      top:    'inset(100% 0 0 0)',
      bottom: 'inset(0 0 100% 0)',
      left:   'inset(0 0 0 100%)',
      right:  'inset(0 100% 0 0)',
    }[side] || 'inset(0 0 0 0)';
  }

  function keyframes(effect, dir, far) {
    const t = travel(dir, far);
    // A whole slide sliding is a page turn: it is opaque, and covers what it
    // replaces. A paragraph sliding is a reveal, and fades in as it arrives.
    const fade = far ? {} : { opacity: '0' };
    const solid = far ? {} : { opacity: '1' };
    switch (effect) {
      case 'fade-in':   return { from: { opacity: '0' }, to: { opacity: '1' } };
      case 'fade-out':  return { from: { opacity: '1' }, to: { opacity: '0' }, out: true };
      case 'slide-in':  return { from: { ...fade, transform: t[0] },
                                 to:   { ...solid, transform: 'none' } };
      case 'slide-out': return { from: { ...solid, transform: 'none' },
                                 to:   { ...fade, transform: t[1] }, out: true };
      // A wipe uncovers rather than moves: the content sits still while the
      // edge that reveals it travels in `dir`.
      case 'wipe-in':   return { from: { clipPath: wipe(dir, true) },
                                 to:   { clipPath: 'inset(0 0 0 0)' } };
      case 'wipe-out':  return { from: { clipPath: 'inset(0 0 0 0)' },
                                 to:   { clipPath: wipe(dir, false) }, out: true };
      case 'zoom-in':   return { from: { opacity: '0', transform: 'scale(.85)' },
                                 to:   { opacity: '1', transform: 'scale(1)' } };
      case 'zoom-out':  return { from: { opacity: '1', transform: 'scale(1)' },
                                 to:   { opacity: '0', transform: 'scale(1.15)' }, out: true };
      case 'blur-in':   return { from: { opacity: '0', filter: 'blur(12px)' },
                                 to:   { opacity: '1', filter: 'blur(0px)' } };
      case 'grow-x':    return { from: { transform: 'scaleX(0)', transformOrigin: 'left center' },
                                 to:   { transform: 'scaleX(1)', transformOrigin: 'left center' } };
      case 'grow-y':    return { from: { transform: 'scaleY(0)', transformOrigin: 'center bottom' },
                                 to:   { transform: 'scaleY(1)', transformOrigin: 'center bottom' } };
      case 'pop':       return { from: { opacity: '0', transform: 'scale(.7)' },
                                 to:   { opacity: '1', transform: 'scale(1)' } };
      case 'iris-out':  return { from: { clipPath: 'circle(150% at 50% 50%)' },
                                 to:   { clipPath: 'circle(0% at 50% 50%)' }, out: true };
      case 'draw':      return { draw: true };
      default:          return null;   // `none`, and anything a newer build emits
    }
  }

  // Reduced motion keeps the reveal and drops the movement: a step still
  // changes what is on screen, it just does not travel to get there.
  function still(kf) {
    if (!kf || kf.draw) return kf;
    return kf.out
      ? { from: { opacity: '1' }, to: { opacity: '0' }, out: true }
      : { from: { opacity: '0' }, to: { opacity: '1' } };
  }

  function elementsFor(sec, target) {
    const base = target.sel === ':scope'
      ? [sec]
      : Array.from(sec.querySelectorAll(target.sel));
    if (!target.split) return base;
    const items = [];
    for (const el of base) items.push(...el.querySelectorAll('.mz-split-item'));
    return items.length ? items : base;
  }

  // Everything SVG shows is painted by a stroke, a fill, or both. `draw`
  // treats them differently: strokes are drawn tip-first over the full beat,
  // and fills - an arrow's head, a box's wash, a label's glyphs - are inked
  // in over the last stretch, once the pen has reached them. Fading the whole
  // group instead would show the arrowhead before the line arrives at it.
  function drawParts(el) {
    const els = el.matches && el.matches(PAINTED) ? [el] : Array.from(el.querySelectorAll(PAINTED));
    const parts = [];
    for (const p of els) {
      const cs = getComputedStyle(p);
      const len = cs.stroke !== 'none' && p.getTotalLength ? p.getTotalLength() : 0;
      const fill = cs.fill !== 'none';
      if (len || fill) parts.push({ el: p, len, fill });
    }
    return parts;
  }
  const INK = 0.65;   // fills start inking at this fraction of the duration

  // Resolves the timeline once per section: which elements each track owns,
  // which batch it belongs to, and when within that batch it starts. `after`
  // tracks join the batch of the track they name, so they inherit its trigger
  // rather than needing one of their own.
  function timeline(sec) {
    if (timelines.has(sec)) return timelines.get(sec);
    let tl = null;
    const tag = sec.querySelector(':scope > script.mz-anim');
    if (tag) { try { tl = JSON.parse(tag.textContent); } catch (e) { tl = null; } }
    if (tl && tl.tracks) {
      for (const t of tl.tracks) {
        t.els = elementsFor(sec, t.target);
        const k = t.trigger.kind;
        t.batch = k === 'click' ? 'click:' + t.trigger.n : (k === 'after' ? null : k);
        t.start = t.batch === null ? null : (t.delay || 0);
      }
      const byId = new Map();
      for (const t of tl.tracks) byId.set(t.target.sel, t);
      const endOf = (t) => t.start + t.dur + (t.stagger || 0) * Math.max(0, t.els.length - 1);
      // A chain of `after` tracks resolves one link per pass; the bound stops
      // a cycle (`a` after `b` after `a`) from spinning.
      for (let pass = 0; pass < 8; pass++) {
        let moved = false;
        for (const t of tl.tracks) {
          if (t.batch !== null) continue;
          const ref = byId.get('#' + t.trigger.id);
          if (!ref || ref.batch === null) continue;
          t.batch = ref.batch;
          t.start = Math.max(0, endOf(ref) + (t.trigger.offset || 0));
          moved = true;
        }
        if (!moved) break;
      }
      // Anything still unresolved names a target nothing animates. Play it on
      // entry rather than never: a broken reference should not silently swallow
      // content.
      for (const t of tl.tracks) {
        if (t.batch === null) { t.batch = 'enter'; t.start = Math.max(0, t.trigger.offset || 0); }
      }
    }
    timelines.set(sec, tl);
    return tl;
  }

  const tracksIn = (tl, batch) => (tl && tl.tracks ? tl.tracks.filter((t) => t.batch === batch) : []);

  function stop(sec) {
    const set = playing.get(sec);
    if (!set) return;
    for (const a of set) { try { a.cancel(); } catch (e) {} }
    set.clear();
  }

  const busy = (sec) => {
    const set = playing.get(sec);
    if (!set) return false;
    for (const a of set) if (a.playState === 'running' || a.playState === 'pending') return true;
    return false;
  };

  function track(sec, a) {
    let set = playing.get(sec);
    if (!set) { set = new Set(); playing.set(sec, set); }
    set.add(a);
    return a;
  }

  // A whole slide travels further than a paragraph does: the same 40px that
  // reads as a nudge on a heading is invisible across 1280px of slide.
  const farFor = (sec, t) => t.els.length === 1 && t.els[0] === sec;

  function armTrack(sec, t) {
    const far = farFor(sec, t);
    const kf = REDUCED ? still(keyframes(t.effect, t.dir, far)) : keyframes(t.effect, t.dir, far);
    if (!kf || kf.out) return;            // exits rest in their visible state
    for (const el of t.els) {
      // A `draw` track arms the painted parts, never the element around them.
      // Saving the element too would leave a snapshot nothing ever restores,
      // and a later `save` on it would then be a no-op holding stale styles.
      if (kf.draw) {
        for (const p of drawParts(el)) {
          save(p.el);
          if (p.len) {
            p.el.style.strokeDasharray = p.len;
            p.el.style.strokeDashoffset = p.len;
          }
          if (p.fill) p.el.style.fillOpacity = '0';
        }
      } else {
        save(el);
        Object.assign(el.style, kf.from);
      }
    }
  }

  // Restores a track's elements to their resting state. A `played` out-effect
  // instead rests in its hidden *end* state: arriving from a later slide, an
  // element that faded out during the talk stays faded out.
  function finalTrack(sec, t, played) {
    for (const el of t.els) {
      unsave(el);
      // Only a `draw` track ever touches painted descendants. Scanning them
      // unconditionally would let a whole-slide track reach into every shape
      // and chart mark on the slide and undo another track's arming.
      if (t.effect === 'draw') for (const p of drawParts(el)) unsave(p.el);
    }
    const kf = played && keyframes(t.effect, t.dir, farFor(sec, t));
    if (kf && kf.out) {
      for (const el of t.els) {
        save(el);
        Object.assign(el.style, kf.to);
      }
    }
  }

  function playTrack(sec, t, isTurn) {
    const far = isTurn || farFor(sec, t);
    const kf = REDUCED ? still(keyframes(t.effect, t.dir, far)) : keyframes(t.effect, t.dir, far);
    if (!kf) return 0;
    const dur = REDUCED ? 1 : t.dur;
    const stagger = REDUCED ? 0 : (t.stagger || 0);
    let end = 0;
    t.els.forEach((el, i) => {
      const delay = t.start + stagger * i;
      end = Math.max(end, delay + dur);
      if (kf.draw) {
        for (const p of drawParts(el)) {
          // Strokes and fills of one part can animate together, so the part
          // is handed back to the stylesheet only when its last animation ends.
          let open = 0;
          const done = (a) => { a.cancel(); if (--open === 0) unsave(p.el); };
          save(p.el);
          if (p.len) {
            p.el.style.strokeDasharray = p.len;
            open += 1;
            const a = track(sec, p.el.animate(
              [{ strokeDashoffset: p.len }, { strokeDashoffset: 0 }],
              { duration: dur, delay, easing: t.ease, fill: 'both' }
            ));
            a.onfinish = () => done(a);
          }
          if (p.fill) {
            p.el.style.fillOpacity = '0';
            open += 1;
            const a = track(sec, p.el.animate(
              [{ fillOpacity: 0 }, { fillOpacity: 1 }],
              { duration: dur * (1 - INK), delay: delay + dur * INK, easing: 'ease-out', fill: 'both' }
            ));
            a.onfinish = () => done(a);
          }
        }
        return;
      }
      save(el);
      Object.assign(el.style, kf.from);
      const a = track(sec, el.animate([kf.from, kf.to], {
        duration: dur, delay, easing: t.ease, fill: 'both',
      }));
      // An entrance hands the element back to the stylesheet; an exit has to
      // hold its end state, because the slide it is on is about to be hidden.
      a.onfinish = () => { if (!kf.out) { a.cancel(); unsave(el); } };
    });
    return end;
  }

  function playBatch(sec, batch) {
    let end = 0;
    for (const t of tracksIn(timeline(sec), batch)) end = Math.max(end, playTrack(sec, t));
    return end;
  }

  // The deck-wide page turn, as one synthetic track on the section itself.
  // A slide that declares its own whole-slide track for the same half owns
  // that half instead: the author was more specific than the deck default.
  function ownTrack(sec, kind) {
    return tracksIn(timeline(sec), kind).some((t) => t.target.sel === ':scope');
  }

  function turn(sec, spec, effect, backwards) {
    if (!spec || !effect || effect === 'none') return 0;
    let dir = spec.dir;
    if (dir && backwards) dir = { left: 'right', right: 'left', up: 'down', down: 'up' }[dir];
    const t = { els: [sec], effect, dir, dur: spec.dur, start: 0, stagger: 0, ease: spec.ease };
    return playTrack(sec, t, true);
  }

  // ---- Focus: one of a set forward, the rest back, without turning the page ----
  //
  // Three panes on a slide, and the talk is about each of them in turn. A page
  // turn per pane loses the other two; showing all three at once has nothing
  // to look at. `focus` keeps all three on the slide and changes which one the
  // room is looking at: the named one comes forward, every other element named
  // by a `focus` track on the same slide goes back.
  //
  // It is a *state*, not a from/to pair, which is why it is not in the
  // `keyframes` table: what `focus` does to an element depends on what the
  // other tracks name and on which step the deck is on. Stepping back is
  // therefore just the same state recomputed for a smaller step, which is what
  // makes going back through a sequence of these look like going forward.
  //
  // The resting state is untouched: with no step applied — a deck read without
  // JavaScript, and the PDF — every pane sits where the grid put it, at full
  // size and full opacity.

  const focusing = new WeakMap();   // element -> the animation holding its state

  // How far forward and how far back, as theme tokens rather than constants,
  // so a deck can make it a whisper or a shove without touching this file.
  function focusDial(sec, name, fallback) {
    const v = parseFloat(getComputedStyle(sec).getPropertyValue(name));
    return Number.isFinite(v) ? v : fallback;
  }

  function focusTracks(sec) {
    return (timeline(sec)?.tracks || []).filter((t) => t.effect === 'focus');
  }

  // The state each member of the group should be in at `step`: the tracks due
  // by now name what is forward, and the most recent of them wins. Before the
  // first one nothing is forward, and the slide looks as it was laid out.
  function focusState(sec, step) {
    const tracks = focusTracks(sec);
    if (!tracks.length) return null;
    const stepOf = (t) => (t.trigger.kind === 'click' ? t.trigger.n : 0);
    let due = -1;
    for (const t of tracks) {
      const n = stepOf(t);
      if (n <= step && n > due) due = n;
    }
    const front = new Set();
    if (due >= 0) {
      for (const t of tracks) {
        if (stepOf(t) === due) for (const el of t.els) front.add(el);
      }
    }
    const all = [];
    for (const t of tracks) for (const el of t.els) if (!all.includes(el)) all.push(el);
    return { all, front, any: due >= 0, dur: tracks[0].dur, ease: tracks[0].ease };
  }

  function applyFocus(sec, step, play) {
    const s = focusState(sec, step);
    if (!s) return;
    const fwd = focusDial(sec, '--mz-focus-scale', 1.06);
    const back = focusDial(sec, '--mz-focus-back-scale', 0.92);
    const dim = focusDial(sec, '--mz-focus-back-opacity', 0.4);
    const dur = REDUCED || !play ? 1 : s.dur;
    for (const el of s.all) {
      // Nothing is forward yet, so nothing is back either: the slide has not
      // started, and it must look exactly as it was laid out.
      const to = !s.any
        ? { transform: 'none', opacity: '1' }
        : s.front.has(el)
          ? { transform: `scale(${fwd})`, opacity: '1' }
          : { transform: `scale(${back})`, opacity: String(dim) };
      // Where the element is *now*, read before the animation holding it there
      // is cancelled. Starting from that rather than from a written-down
      // previous state is what lets a step pressed during the last one carry
      // on from where it got to instead of jumping back.
      const cs = getComputedStyle(el);
      const from = { transform: cs.transform, opacity: cs.opacity };
      const prev = focusing.get(el);
      if (prev) { try { prev.cancel(); } catch (e) {} }
      // An animation rather than an inline style, so a repaint that re-arms
      // the slide's other tracks cannot overwrite it, and so `settle` has one
      // thing to cancel rather than styles to guess at.
      focusing.set(el, el.animate([from, to], { duration: dur, easing: s.ease, fill: 'both' }));
      el.classList.toggle('mz-focus', s.any && s.front.has(el));
      el.classList.toggle('mz-unfocus', s.any && !s.front.has(el));
    }
  }

  function clearFocus(sec) {
    const s = focusState(sec, 0);
    if (!s) return;
    for (const el of s.all) {
      const a = focusing.get(el);
      if (a) { try { a.cancel(); } catch (e) {} focusing.delete(el); }
      el.classList.remove('mz-focus', 'mz-unfocus');
    }
  }

  // ---- Carrying an element from one slide to the next ----
  //
  // A `[carry] #id : move` track names an element that is on this slide and on
  // the next one. Instead of the deck turning the page under it, the element
  // travels between the two boxes it occupies. Everything else on both slides
  // still turns: that is the whole effect, one thing holding still while the
  // page moves around it.
  //
  // Three facts make it work. The slides are separate subtrees, so what flies
  // is a *copy* lifted into a layer of its own that outlives both; a copy has
  // no ancestors to inherit from, so it carries its computed style with it.
  // The two originals are hidden for exactly as long as the copy is up. And
  // both boxes are measured before anything is animated - the departing slide
  // before its exit starts, the arriving one before its entrance does - so the
  // flight path is between two resting positions, never two moving ones.

  // Slide-logical coordinates. The deck is laid out at its logical size and
  // CSS-scaled to the window, so a client rect has to be divided by that scale
  // before it means the same thing a stylesheet would mean by it.
  function metrics(sec) {
    const r = sec.getBoundingClientRect();
    return {
      left: r.left,
      top: r.top,
      k: r.width && sec.offsetWidth ? r.width / sec.offsetWidth : 1,
    };
  }

  const boxIn = (r, m) => ({
    x: (r.left - m.left) / m.k,
    y: (r.top - m.top) / m.k,
    w: r.width / m.k,
    h: r.height / m.k,
  });

  const rectIn = (el, m) => boxIn(el.getBoundingClientRect(), m);

  // What the audience sees move is the ink, not the box around it. A heading's
  // box is as wide as the column it sits in, while the words in it are the
  // thing that was also on the previous slide - line them up by the box and a
  // chip becoming a heading stretches threefold on the way. So a carry is
  // aimed by the contents' own rectangle, and falls back to the element's when
  // there is no text to measure: an image, an SVG shape, an empty pane.
  function inkIn(el, m) {
    try {
      const r = document.createRange();
      r.selectNodeContents(el);
      const b = r.getBoundingClientRect();
      if (b.width && b.height) return boxIn(b, m);
    } catch (e) {}
    return rectIn(el, m);
  }

  // A lifted copy has no ancestors, so every selector that dressed the
  // original - `.slide .pane h2`, a theme's token cascade - stops matching.
  // Copying the computed style onto the copy is what keeps it looking like
  // the thing it was cut from. The cap is a guard, not a budget: something
  // the size of a whole pane is not what this feature is for, and cloning it
  // per frame would cost more than the effect is worth.
  const LIFT_CAP = 400;

  function lift(el) {
    const clone = el.cloneNode(true);
    let n = 0;
    const dress = (src, dst) => {
      if (++n > LIFT_CAP) return false;
      const cs = getComputedStyle(src);
      let css = '';
      for (let i = 0; i < cs.length; i++) {
        const p = cs[i];
        css += p + ':' + cs.getPropertyValue(p) + ';';
      }
      dst.style.cssText = css;
      const a = src.children, b = dst.children;
      for (let i = 0; i < a.length && i < b.length; i++) {
        if (!dress(a[i], b[i])) return false;
      }
      return true;
    };
    return dress(el, clone) ? clone : null;
  }

  // The fraction of the flight the copy holds before handing over to the real
  // element. Where the two look alike the swap is invisible; where they do not
  // - a chip becoming a heading - this is what stops it being a cut.
  const HANDOVER = 0.7;

  // At most one flight is ever in the air. A second page turn during one is a
  // presenter pressing on, and the answer is to land the first immediately
  // rather than to run two.
  let flight = null;

  function carryEnd() {
    if (!flight) return;
    clearTimeout(flight.timer);
    for (const p of flight.pairs) {
      p.src.style.visibility = p.hidden;
      if (p.fade) { try { p.fade.cancel(); } catch (e) {} }
    }
    flight.layer.remove();
    flight = null;
  }

  window.MZAnim = {
    reduced: REDUCED,

    // Measures what is about to leave, before its exit moves it. Returns the
    // plan `carryPlay` finishes, or null when this boundary carries nothing.
    carryStart(from, to, backwards) {
      carryEnd();
      // Reduced motion drops the travel, and a carry is nothing but travel:
      // both slides then show the element in its own place, which is what the
      // deck looks like with no runtime at all.
      if (REDUCED || !from || !to || from === to) return null;
      // The declaration lives on the earlier slide of the pair, so the same
      // line governs the boundary whichever way it is crossed.
      const tracks = tracksIn(timeline(backwards ? to : from), 'carry');
      if (!tracks.length) return null;
      const m = metrics(from);
      const pairs = [];
      for (const t of tracks) {
        const src = from.querySelector(t.target.sel);
        const dst = to.querySelector(t.target.sel);
        // An id on only one of the two slides is not a pair. The build warns
        // about it; here it simply means an ordinary page turn.
        if (!src || !dst) continue;
        pairs.push({ t, src, dst, box: rectIn(src, m), ink: inkIn(src, m) });
      }
      return pairs.length ? { to, pairs } : null;
    },

    // Measures where each element lands - still before the arriving slide's
    // entrance moves it - and puts the copies in the air. Returns how long
    // the flight takes.
    carryPlay(plan) {
      if (!plan) return 0;
      const host = plan.to.parentElement;
      if (!host) return 0;
      const layer = document.createElement('div');
      layer.className = 'mz-carry-layer';
      host.appendChild(layer);
      const m = metrics(plan.to);
      const pairs = [];
      let end = 0;
      for (const p of plan.pairs) {
        const ink = inkIn(p.dst, m);
        const clone = lift(p.src);
        if (!clone) continue;
        const s = clone.style;
        s.position = 'absolute';
        s.margin = '0';
        s.left = p.box.x + 'px';
        s.top = p.box.y + 'px';
        s.width = p.box.w + 'px';
        s.height = p.box.h + 'px';
        // The box being handed over is the border box, whatever the original
        // was sizing by.
        s.boxSizing = 'border-box';
        s.transform = 'none';
        s.transformOrigin = 'top left';
        s.pointerEvents = 'none';
        layer.appendChild(clone);

        // One scale for both axes: text that grows grows the way type does,
        // and nothing ever arrives stretched. Height is the axis that carries
        // it — a line of text is as tall as its size and only as wide as it
        // happens to be. What is left over at the end is what the handover is
        // for.
        const k = p.ink.h ? ink.h / p.ink.h : 1;
        // Scaling happens about the copy's top-left, so where the ink ends up
        // is the ink's own offset within the box, scaled. Aim that at the
        // destination's ink and the two sets of words line up.
        const dx = (ink.x + ink.w / 2) - (p.box.x + (p.ink.x + p.ink.w / 2 - p.box.x) * k);
        const dy = (ink.y + ink.h / 2) - (p.box.y + (p.ink.y + p.ink.h / 2 - p.box.y) * k);
        const dur = p.t.dur;
        clone.animate(
          [{ transform: 'none' }, { transform: `translate(${dx}px, ${dy}px) scale(${k})` }],
          { duration: dur, easing: p.t.ease, fill: 'both' }
        );
        clone.animate(
          [{ opacity: 1, offset: 0 }, { opacity: 1, offset: HANDOVER }, { opacity: 0, offset: 1 }],
          { duration: dur, easing: 'linear', fill: 'both' }
        );

        // The originals: the departing one is simply out of the way, and the
        // arriving one fades up under the copy as it lands. That is done with
        // an animation rather than an inline style so that arming the slide's
        // own tracks - which writes inline styles - cannot undo it.
        const hidden = p.src.style.visibility;
        p.src.style.visibility = 'hidden';
        const fade = p.dst.animate(
          [{ opacity: 0, offset: 0 }, { opacity: 0, offset: HANDOVER }, { opacity: 1, offset: 1 }],
          { duration: dur, easing: 'linear', fill: 'both' }
        );
        pairs.push({ src: p.src, hidden, fade });
        end = Math.max(end, dur);
      }
      if (!pairs.length) { layer.remove(); return 0; }
      // Landing is on a timer rather than on the last animation's `finish`:
      // the copy has to come down and the originals come back even if the
      // window was in a background tab while it flew.
      flight = { layer, pairs, timer: setTimeout(carryEnd, end + 60) };
      return end;
    },

    // Puts every carried element back where the stylesheet had it. The viewer
    // calls this when it gives up on a flight; `carryStart` calls it before
    // starting the next one.
    carryLand: carryEnd,

    steps(sec) {
      const tl = timeline(sec);
      return tl && tl.steps ? tl.steps : 0;
    },

    // Elements whose track has already played but which nobody can see —
    // still transparent, still drawn to zero length, still off to one side.
    // That is the failure the resting-state rule exists to prevent, so
    // `check-layout.mjs` gates on it.
    //
    // This asks the page what it looks like rather than reading this file's
    // own bookkeeping, so a flag left behind by mistake cannot make a visible
    // slide look broken, and a slide that really is blank cannot hide behind
    // tidy internal state.
    //
    // Out-effects are excluded: an element that fades out during the talk is
    // *meant* to be gone once it has.
    armed(sec, step) {
      const tl = timeline(sec);
      if (!tl) return 0;
      const box = sec.getBoundingClientRect();
      const gone = (el) => {
        // `chars`/`words` splitting animates spans inside the element the
        // author named, so an ancestor left transparent hides them without
        // changing their own computed opacity. `checkVisibility` walks up.
        if (el.checkVisibility && !el.checkVisibility({
          opacityProperty: true, visibilityProperty: true, contentVisibilityAuto: true,
        })) return true;
        const cs = getComputedStyle(el);
        if (+cs.opacity < 0.05 || cs.visibility === 'hidden') return true;
        const r = el.getBoundingClientRect();
        if (!r.width && !r.height) return false;      // laid out as nothing to begin with
        return r.right < box.left || r.left > box.right
            || r.bottom < box.top || r.top > box.bottom;
      };
      let n = 0;
      for (const t of tl.tracks) {
        const due = t.batch === 'enter'
          || (typeof t.batch === 'string' && t.batch.startsWith('click:') && +t.batch.slice(6) <= step);
        if (!due) continue;
        const kf = keyframes(t.effect, t.dir, farFor(sec, t));
        if (!kf || kf.out) continue;
        for (const el of t.els) {
          if (kf.draw) {
            // A stroke still offset by its own length has drawn nothing.
            for (const p of drawParts(el)) {
              const s = getComputedStyle(p.el);
              if (p.len && Math.abs(parseFloat(s.strokeDashoffset) || 0) > 1) { n++; break; }
              if (p.fill && +s.fillOpacity < 0.05) { n++; break; }
            }
          } else if (gone(el)) {
            n++;
          }
        }
      }
      return n;
    },

    // Shows the slide as it stands after `step` click steps: everything up to
    // there played, everything past it armed. Entering forwards is `step` 0
    // with `play`; a live-reload repaint is the same call without it.
    show(sec, step, spec, opts) {
      const play = opts && opts.play;
      const backwards = !!(opts && opts.backwards);
      const arriving = !!(opts && opts.arriving);
      // A repaint that lands mid-animation must not become a cancel. The font
      // loader fires one a few frames after load, which is exactly when a
      // slide's entrance is still running.
      //
      // Arriving at a slide is never that repaint, and must never be skipped:
      // a slide left mid-`exit` is holding a transform that has carried it off
      // the screen, and refusing to stage it there strands it there. Editor
      // cursor sync and live reload both land here while the previous page
      // turn is still running.
      if (!play && !arriving && busy(sec)) return;
      // A slide's own `[enter] slide` track replaces the deck's page turn on
      // the way forward. Backwards it does not: an entrance reversed is not a
      // meaningful animation, and without *something* covering the departing
      // slide the page appears to cut while the old one slides out beneath.
      const turnIn = () => {
        if (play && (backwards || !ownTrack(sec, 'enter'))) {
          turn(sec, spec, spec && spec.in, backwards);
        }
      };
      const tl = timeline(sec);
      stop(sec);
      if (!tl) {
        turnIn();
        return;
      }
      for (const t of tl.tracks) {
        // `focus` is a state over a whole group rather than one element's
        // from/to, so it is applied once below instead of armed per track.
        if (t.effect === 'focus') continue;
        const played = t.batch === 'enter'
          || (t.batch.startsWith('click:') && +t.batch.slice(6) <= step);
        // Exit tracks rest visible too - they have not played yet - but they
        // never rest in an armed state, so they are not `played` here.
        if (played || t.batch === 'exit') finalTrack(sec, t, played);
        else armTrack(sec, t);
      }
      // Staged, not played: arriving at a slide part-way through shows it as
      // it stands, and only a step pressed while watching animates.
      applyFocus(sec, step, false);
      if (!play) return;
      turnIn();
      // Entering backwards lands on a slide the audience has already seen, so
      // its entrance has nothing left to reveal.
      if (!backwards) playBatch(sec, 'enter');
    },

    // Plays click step `n`. Returns the milliseconds it will take, which the
    // viewer does not wait for - it only needs to know something happened.
    step(sec, n) {
      stopFinished(sec);
      const end = playBatch(sec, 'click:' + n);
      applyFocus(sec, n, true);
      return Math.max(end, focusTracks(sec).length ? focusState(sec, n).dur : 0);
    },

    // Steps back within a slide by re-arming that step. Snapping back rather
    // than playing in reverse is deliberate: going back is a correction, and
    // a correction should be instant.
    unstep(sec, n) {
      for (const t of tracksIn(timeline(sec), 'click:' + n)) {
        // An exit effect holds its end state in a live animation (it has no
        // resting state to hand back to), so stepping back must cancel it
        // and restore the element, not just re-arm.
        for (const el of t.els) {
          for (const a of el.getAnimations({ subtree: true })) a.cancel();
          unsave(el);
        }
        armTrack(sec, t);
      }
      // Focus is the exception to "going back snaps": it is not a reveal being
      // taken back but the room being pointed somewhere else, and pointing
      // somewhere else looks the same whichever direction the talk is going.
      applyFocus(sec, n - 1, true);
    },

    // Leaves a slide: its `exit` tracks, plus the deck's page turn. Returns
    // how long the slide must stay painted for that to be visible.
    leave(sec, spec, backwards) {
      let end = playBatch(sec, 'exit');
      if (!ownTrack(sec, 'exit')) end = Math.max(end, turn(sec, spec, spec && spec.out, backwards));
      return end;
    },

    // Called once a departed slide is off screen. An exit holds its end state
    // in a *live* animation - it has no resting state to hand back to - and
    // nothing cancelled it until the slide was next shown, so a walk through a
    // long deck left one held animation per page behind it. Off screen there
    // is nothing left to hold.
    settle(sec) {
      stop(sec);
      clearFocus(sec);
      const tl = timeline(sec);
      if (!tl) return;
      for (const t of tl.tracks) {
        for (const el of t.els) unsave(el);
      }
      unsave(sec);
    },
  };

  // Dropping finished animations keeps the per-section set from growing across
  // a long presentation without cancelling anything still on screen.
  function stopFinished(sec) {
    const set = playing.get(sec);
    if (!set) return;
    for (const a of set) if (a.playState === 'finished' || a.playState === 'idle') set.delete(a);
  }
})();
