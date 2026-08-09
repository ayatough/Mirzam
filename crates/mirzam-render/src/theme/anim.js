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
      save(el);
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

  window.MZAnim = {
    reduced: REDUCED,

    steps(sec) {
      const tl = timeline(sec);
      return tl && tl.steps ? tl.steps : 0;
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
      const tl = timeline(sec);
      stop(sec);
      if (!tl) {
        if (play && !ownTrack(sec, 'enter')) turn(sec, spec, spec && spec.in, backwards);
        return;
      }
      for (const t of tl.tracks) {
        const played = t.batch === 'enter'
          || (t.batch.startsWith('click:') && +t.batch.slice(6) <= step);
        // Exit tracks rest visible too - they have not played yet - but they
        // never rest in an armed state, so they are not `played` here.
        if (played || t.batch === 'exit') finalTrack(sec, t, played);
        else armTrack(sec, t);
      }
      if (!play) return;
      if (!ownTrack(sec, 'enter')) turn(sec, spec, spec && spec.in, backwards);
      // Entering backwards lands on a slide the audience has already seen, so
      // its entrance has nothing left to reveal.
      if (!backwards) playBatch(sec, 'enter');
    },

    // Plays click step `n`. Returns the milliseconds it will take, which the
    // viewer does not wait for - it only needs to know something happened.
    step(sec, n) {
      stopFinished(sec);
      return playBatch(sec, 'click:' + n);
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
    },

    // Leaves a slide: its `exit` tracks, plus the deck's page turn. Returns
    // how long the slide must stay painted for that to be visible.
    leave(sec, spec, backwards) {
      let end = playBatch(sec, 'exit');
      if (!ownTrack(sec, 'exit')) end = Math.max(end, turn(sec, spec, spec && spec.out, backwards));
      return end;
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
