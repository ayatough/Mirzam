//! Parser for the `anim` fenced block DSL: text in, the timeline IR ([C1] in
//! `docs/workstreams.md`) out. Pure and self-contained — no HTML, no DOM.
//! Text splitting for `target.split` and checking whether a target exists on
//! the rendered slide both happen in `mirzam-render`, which has the HTML to
//! check against; a target that cannot be located there is reported through
//! the render pass's warnings, not here.
//!
//! ```text
//! [enter]   .title       : chars fade-in 400ms stagger=30ms ease=out-cubic
//! [click 1] #latency-0-2 : grow-y 500ms
//! [after #latency-0-2 +200ms] .caption : fade-in 300ms
//! [exit]    slide        : iris-out 500ms
//! ```
//!
//! [C1]: ../../../docs/workstreams.md#c1-animation-timeline

use std::fmt::Write as _;

/// What starts a track playing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// Plays when the slide is entered (forwards or, per the runtime, already
    /// played when entering backwards).
    Enter,
    /// Plays on the Nth `click` advance within the slide. `N` starts at 1.
    Click(u32),
    /// Plays when the slide is left.
    Exit,
    /// Plays `offset_ms` after another track's target id, which may be
    /// negative to start slightly before it.
    After { id: String, offset_ms: i64 },
}

/// A text-splitting granularity, applied at build time so the runtime only
/// ever selects existing spans, never mutates the DOM to make them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    Chars,
    Words,
    Lines,
}

impl Split {
    pub fn as_str(&self) -> &'static str {
        match self {
            Split::Chars => "chars",
            Split::Words => "words",
            Split::Lines => "lines",
        }
    }
}

/// What a track animates. `sel` is a CSS selector; the literal target keyword
/// `slide` lowers to `:scope`, meaning the slide's own `<section>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub sel: String,
    pub split: Option<Split>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Left => "left",
            Direction::Right => "right",
            Direction::Up => "up",
            Direction::Down => "down",
        }
    }
}

/// A named CSS-ish curve, or a spring resolved to a sampled curve at
/// `to_json` time so the runtime never simulates physics.
#[derive(Debug, Clone, PartialEq)]
pub enum Ease {
    Named(String),
    Spring {
        mass: f64,
        stiffness: f64,
        damping: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub trigger: Trigger,
    pub target: Target,
    pub effect: String,
    /// Only meaningful (and only allowed) on a [`DIRECTIONAL`] effect.
    pub dir: Option<Direction>,
    pub dur_ms: u32,
    pub delay_ms: u32,
    pub stagger_ms: u32,
    pub ease: Ease,
}

#[derive(Debug, Default)]
pub struct AnimDoc {
    pub tracks: Vec<Track>,
    pub errors: Vec<String>,
}

/// The parts of the effect grammar a consumer may swap out: which effect
/// names exist, which of those travel (and so require `dir=`), and how a
/// duration token reads. Everything else — the line shape, `key=value`
/// attributes, splits, eases — is the grammar itself and stays fixed, so a
/// hand that knows one dialect is never retrained by another.
///
/// Mirzam's own vocabulary is `Dialect::default()`; a tool with a different
/// time base (beats, frames) supplies its own `duration`/`bare_duration`
/// and converts to milliseconds before handing tokens back.
pub struct Dialect<'a> {
    pub effects: &'a [&'a str],
    pub directional: &'a [&'a str],
    /// Parses a duration token (from `dur=`, `delay=`, `stagger=`, or a bare
    /// token accepted by `bare_duration`) into milliseconds.
    pub duration: &'a dyn Fn(&str) -> Result<u32, String>,
    /// Whether a bare token (no `key=`) should be read as a duration.
    pub bare_duration: &'a dyn Fn(&str) -> bool,
}

impl Default for Dialect<'static> {
    /// The slide grammar: the v1 effect set and `ms`-only durations.
    fn default() -> Self {
        Dialect {
            effects: EFFECTS,
            directional: DIRECTIONAL,
            duration: &parse_ms,
            bare_duration: &is_ms_token,
        }
    }
}

fn is_ms_token(tok: &str) -> bool {
    tok.ends_with("ms")
}

/// Effect set for v1. The directional ones additionally require `dir=`.
const EFFECTS: &[&str] = &[
    "fade-in",
    "fade-out",
    "slide-in",
    "slide-out",
    "wipe-in",
    "wipe-out",
    "zoom-in",
    "zoom-out",
    "blur-in",
    "grow-x",
    "grow-y",
    "pop",
    "draw",
    "iris-out",
];

/// Effects that travel, and so need `dir=` to say which way.
const DIRECTIONAL: &[&str] = &["slide-in", "slide-out", "wipe-in", "wipe-out"];

/// Named easing curves the DSL accepts, beyond `spring(...)`, each paired with
/// the CSS easing function it lowers to. The lowering happens here rather than
/// in the viewer so that a deck does not carry a curve table it mostly does not
/// use, and so the curves are covered by these tests instead of by eye.
///
/// The Penner curves are the approximations published at <https://easings.net>.
const NAMED_EASES: &[(&str, &str)] = &[
    ("linear", "linear"),
    ("ease", "ease"),
    ("ease-in", "ease-in"),
    ("ease-out", "ease-out"),
    ("ease-in-out", "ease-in-out"),
    ("in-quad", "cubic-bezier(.11,0,.5,0)"),
    ("out-quad", "cubic-bezier(.5,1,.89,1)"),
    ("in-out-quad", "cubic-bezier(.45,0,.55,1)"),
    ("in-cubic", "cubic-bezier(.32,0,.67,0)"),
    ("out-cubic", "cubic-bezier(.33,1,.68,1)"),
    ("in-out-cubic", "cubic-bezier(.65,0,.35,1)"),
    ("in-quart", "cubic-bezier(.5,0,.75,0)"),
    ("out-quart", "cubic-bezier(.25,1,.5,1)"),
    ("in-out-quart", "cubic-bezier(.76,0,.24,1)"),
    ("in-quint", "cubic-bezier(.64,0,.78,0)"),
    ("out-quint", "cubic-bezier(.22,1,.36,1)"),
    ("in-out-quint", "cubic-bezier(.83,0,.17,1)"),
    ("in-sine", "cubic-bezier(.12,0,.39,0)"),
    ("out-sine", "cubic-bezier(.61,1,.88,1)"),
    ("in-out-sine", "cubic-bezier(.37,0,.63,1)"),
    ("in-expo", "cubic-bezier(.7,0,.84,0)"),
    ("out-expo", "cubic-bezier(.16,1,.3,1)"),
    ("in-out-expo", "cubic-bezier(.87,0,.13,1)"),
    ("in-circ", "cubic-bezier(.55,0,1,.45)"),
    ("out-circ", "cubic-bezier(0,.55,.45,1)"),
    ("in-out-circ", "cubic-bezier(.85,0,.15,1)"),
    ("in-back", "cubic-bezier(.36,0,.66,-.56)"),
    ("out-back", "cubic-bezier(.34,1.56,.64,1)"),
    ("in-out-back", "cubic-bezier(.68,-.6,.32,1.6)"),
];

/// The CSS easing function a named curve lowers to.
fn css_ease(name: &str) -> Option<&'static str> {
    NAMED_EASES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, css)| *css)
}

/// Number of points sampled into the CSS `linear()` easing function produced
/// for a `spring(...)` ease. High enough to read as smooth, low enough that
/// it does not bloat a deck that uses many springs.
const SPRING_SAMPLES: usize = 24;

/// Parses an `anim` block's source. One line is one track; blank lines are
/// ignored. A line that fails to parse is collected as an error rather than
/// aborting the rest of the block, so one typo does not hide every other
/// track's diagnostic.
pub fn parse(src: &str) -> AnimDoc {
    let mut doc = AnimDoc::default();
    for (i, raw) in src.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(track) => doc.tracks.push(track),
            Err(e) => doc.errors.push(format!("anim line {}: {e}", i + 1)),
        }
    }
    doc
}

/// The number of `click` steps the slide needs, i.e. the highest `click N`
/// trigger used. The viewer needs this to know when `→` advances a step and
/// when it turns the page.
pub fn steps(doc: &AnimDoc) -> u32 {
    doc.tracks
        .iter()
        .filter_map(|t| match t.trigger {
            Trigger::Click(n) => Some(n),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// Serializes a parsed, validated document to the [C1] JSON blob. Callers are
/// expected to have already dropped any tracks that failed target validation
/// against the rendered HTML (`mirzam-anim` has no HTML to check against).
///
/// [C1]: ../../../docs/workstreams.md#c1-animation-timeline
pub fn to_json(doc: &AnimDoc) -> String {
    let tracks: Vec<serde_json::Value> = doc.tracks.iter().map(track_json).collect();
    serde_json::json!({
        "steps": steps(doc),
        "tracks": tracks,
    })
    .to_string()
}

/// A deck-wide slide transition, from frontmatter `transition:`.
///
/// A transition is not a fourth kind of thing: it is the pair of whole-slide
/// tracks a deck would otherwise repeat on every slide. A slide that declares
/// its own `[enter] slide` or `[exit] slide` track overrides the matching half,
/// which is why this carries the two halves separately.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub name: String,
    pub in_effect: String,
    pub out_effect: String,
    /// The direction the slide travels, for the sliding transitions.
    pub dir: Option<Direction>,
    pub dur_ms: u32,
    pub ease: Ease,
}

/// Transition names, in the deck's vocabulary rather than the track effect set:
/// an author picks how pages turn, not which two effects that lowers to.
const TRANSITIONS: &[&str] = &[
    "none",
    "fade",
    "slide-left",
    "slide-right",
    "slide-up",
    "slide-down",
    "wipe-left",
    "wipe-right",
    "wipe-up",
    "wipe-down",
    "zoom",
    "iris",
];

/// Parses frontmatter `transition:`, e.g. `slide-left 300ms ease=out-cubic`.
/// The duration is optional here, unlike in a track: a deck-wide default is
/// worth being able to write as one word.
pub fn parse_transition(src: &str) -> Result<Transition, String> {
    let mut tokens = src.split_whitespace();
    let name = tokens.next().ok_or("empty transition")?;
    if !TRANSITIONS.contains(&name) {
        return Err(format!(
            "unknown transition `{name}`; expected one of {}",
            TRANSITIONS.join(", ")
        ));
    }
    let mut dur_ms = 300u32;
    let mut ease = Ease::Named("out-cubic".to_string());
    for tok in tokens {
        if let Some((k, v)) = tok.split_once('=') {
            match k {
                "dur" => dur_ms = parse_ms(v)?,
                "ease" => ease = parse_ease(v)?,
                other => return Err(format!("unknown attribute `{other}=`")),
            }
        } else if tok.ends_with("ms") {
            dur_ms = parse_ms(tok)?;
        } else {
            return Err(format!("unexpected token `{tok}`"));
        }
    }
    let (in_effect, out_effect, dir) = match name {
        "none" => ("none", "none", None),
        "fade" => ("fade-in", "fade-out", None),
        "iris" => ("fade-in", "iris-out", None),
        "zoom" => ("zoom-in", "zoom-out", None),
        "slide-left" => ("slide-in", "slide-out", Some(Direction::Left)),
        "slide-right" => ("slide-in", "slide-out", Some(Direction::Right)),
        "slide-up" => ("slide-in", "slide-out", Some(Direction::Up)),
        "slide-down" => ("slide-in", "slide-out", Some(Direction::Down)),
        "wipe-left" => ("wipe-in", "wipe-out", Some(Direction::Left)),
        "wipe-right" => ("wipe-in", "wipe-out", Some(Direction::Right)),
        "wipe-up" => ("wipe-in", "wipe-out", Some(Direction::Up)),
        "wipe-down" => ("wipe-in", "wipe-out", Some(Direction::Down)),
        _ => unreachable!("checked against TRANSITIONS above"),
    };
    Ok(Transition {
        name: name.to_string(),
        in_effect: in_effect.to_string(),
        out_effect: out_effect.to_string(),
        dir,
        dur_ms,
        ease,
    })
}

/// A deck that turns its own pages, from frontmatter `autoplay:`.
///
/// Autoplay is pacing rather than animation, but it lives here because this
/// is where the timing grammar lives: the interval is written the way a
/// track's duration is, and the viewer consumes it from `#deck` the way it
/// consumes the transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autoplay {
    /// Milliseconds between advances. One advance is one click step while the
    /// slide has any left, then the next slide — the same thing `→` does, so
    /// an animated slide plays through at the same pace as pages turn.
    pub interval_ms: u32,
    /// Whether the deck wraps from the last slide back to the first — a kiosk
    /// loop or a screensaver, rather than a talk that ends.
    pub looped: bool,
}

/// Parses frontmatter `autoplay:`, e.g. `8s`, `8s loop`, `4000ms`.
/// A bare number is seconds: how long a slide stays up is something people
/// think of in seconds, unlike an animation's duration.
pub fn parse_autoplay(src: &str) -> Result<Autoplay, String> {
    let mut interval_ms: Option<u32> = None;
    let mut looped = false;
    for tok in src.split_whitespace() {
        if tok == "loop" {
            looped = true;
            continue;
        }
        let (num, scale) = if let Some(n) = tok.strip_suffix("ms") {
            (n, 1.0)
        } else if let Some(n) = tok.strip_suffix('s') {
            (n, 1000.0)
        } else {
            (tok, 1000.0)
        };
        let v: f64 = num.parse().map_err(|_| {
            format!("unexpected token `{tok}`; write `autoplay: 8s` or `autoplay: 8s loop`")
        })?;
        interval_ms = Some((v * scale).round() as u32);
    }
    let interval_ms =
        interval_ms.ok_or("no interval; write `autoplay: 8s` or `autoplay: 8s loop`")?;
    // `as u32` saturates a negative interval to zero, so this floor catches
    // nonsense as well as intervals shorter than any page turn could be.
    if interval_ms < 100 {
        return Err(format!(
            "`{src}` is shorter than any page turn; 100ms is the floor"
        ));
    }
    Ok(Autoplay {
        interval_ms,
        looped,
    })
}

/// Serializes autoplay for the `data-autoplay` attribute on `#deck`.
pub fn autoplay_json(a: &Autoplay) -> String {
    serde_json::json!({ "ms": a.interval_ms, "loop": a.looped }).to_string()
}

/// Serializes a transition for the `data-transition` attribute on `#deck`.
pub fn transition_json(t: &Transition) -> String {
    let mut v = serde_json::json!({
        "in": t.in_effect,
        "out": t.out_effect,
        "dur": t.dur_ms,
        "ease": ease_css(&t.ease, t.dur_ms),
    });
    if let Some(d) = t.dir {
        v["dir"] = serde_json::Value::String(d.as_str().into());
    }
    v.to_string()
}

fn track_json(t: &Track) -> serde_json::Value {
    let trigger = match &t.trigger {
        Trigger::Enter => serde_json::json!({"kind": "enter"}),
        Trigger::Click(n) => serde_json::json!({"kind": "click", "n": n}),
        Trigger::Exit => serde_json::json!({"kind": "exit"}),
        Trigger::After { id, offset_ms } => {
            serde_json::json!({"kind": "after", "id": id, "offset": offset_ms})
        }
    };
    let mut v = serde_json::json!({
        "trigger": trigger,
        "target": {
            "sel": t.target.sel,
            "split": t.target.split.as_ref().map(Split::as_str),
        },
        "effect": t.effect,
        "dur": t.dur_ms,
        "delay": t.delay_ms,
        "stagger": t.stagger_ms,
        "ease": ease_css(&t.ease, t.dur_ms),
    });
    if let Some(d) = t.dir {
        v["dir"] = serde_json::Value::String(d.as_str().into());
    }
    v
}

/// Lowers an ease to the CSS easing function the runtime plays — a named
/// curve to its `cubic-bezier(...)`, a spring to a sampled `linear(...)`.
pub fn ease_css(ease: &Ease, dur_ms: u32) -> String {
    match ease {
        // Already validated by `parse_ease`; an unknown name here would be a
        // bug, and `linear` is the honest thing to fall back to.
        Ease::Named(name) => css_ease(name).unwrap_or("linear").to_string(),
        Ease::Spring {
            mass,
            stiffness,
            damping,
        } => sample_spring(*mass, *stiffness, *damping, dur_ms),
    }
}

/// Samples a damped harmonic oscillator into a CSS `linear()` easing string
/// over `[0, dur_ms]`, so the runtime needs no physics: it just plays the
/// curve like any other easing function.
fn sample_spring(mass: f64, stiffness: f64, damping: f64, dur_ms: u32) -> String {
    let duration_s = (f64::from(dur_ms) / 1000.0).max(0.001);
    let omega0 = (stiffness / mass).sqrt();
    let zeta = damping / (2.0 * (stiffness * mass).sqrt());

    let y = |t: f64| -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        if zeta < 1.0 {
            let omega_d = omega0 * (1.0 - zeta * zeta).sqrt();
            1.0 - (-zeta * omega0 * t).exp()
                * ((omega_d * t).cos() + (zeta * omega0 / omega_d) * (omega_d * t).sin())
        } else if zeta == 1.0 {
            1.0 - (-omega0 * t).exp() * (1.0 + omega0 * t)
        } else {
            let omega_d = omega0 * (zeta * zeta - 1.0).sqrt();
            1.0 - (-zeta * omega0 * t).exp()
                * ((omega_d * t).cosh() + (zeta * omega0 / omega_d) * (omega_d * t).sinh())
        }
    };

    let mut out = String::from("linear(");
    for i in 0..SPRING_SAMPLES {
        let t = duration_s * (i as f64) / (SPRING_SAMPLES as f64 - 1.0);
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "{:.4}", y(t));
    }
    out.push(')');
    out
}

/// Splits a track line into its three fields — `[trigger]`, target, effect —
/// without interpreting the trigger, which is the half of the grammar a
/// consumer with its own time model replaces wholesale.
pub fn split_line(line: &str) -> Result<(&str, &str, &str), String> {
    let after_bracket = line
        .strip_prefix('[')
        .ok_or("expected a `[trigger]` at the start of the line")?;
    let (trigger_src, rest) = after_bracket
        .split_once(']')
        .ok_or("unterminated `[trigger]`")?;
    let rest = rest.trim();
    let (target_src, effect_src) = rest.split_once(':').ok_or("expected `target : effect`")?;
    let target_src = target_src.trim();
    let effect_src = effect_src.trim();
    if target_src.is_empty() {
        return Err("missing target before `:`".into());
    }
    if effect_src.is_empty() {
        return Err("missing effect after `:`".into());
    }
    Ok((trigger_src.trim(), target_src, effect_src))
}

fn parse_line(line: &str) -> Result<Track, String> {
    let (trigger_src, target_src, effect_src) = split_line(line)?;
    let trigger = parse_trigger(trigger_src)?;
    let mut target = parse_target(target_src)?;
    let parsed = parse_effect_in(&Dialect::default(), effect_src)?;
    if let Some(split) = parsed.split {
        target.split = Some(split);
    }

    Ok(Track {
        trigger,
        target,
        effect: parsed.effect,
        dir: parsed.dir,
        dur_ms: parsed.dur_ms,
        delay_ms: parsed.delay_ms,
        stagger_ms: parsed.stagger_ms,
        ease: parsed.ease,
    })
}

fn parse_trigger(s: &str) -> Result<Trigger, String> {
    if s == "enter" {
        return Ok(Trigger::Enter);
    }
    if s == "exit" {
        return Ok(Trigger::Exit);
    }
    if let Some(n) = s.strip_prefix("click") {
        let n = n.trim();
        let n: u32 = n
            .parse()
            .map_err(|_| format!("`click` needs a step number, e.g. `[click 1]`, got `[{s}]`"))?;
        if n == 0 {
            return Err("click steps start at 1, not 0".into());
        }
        return Ok(Trigger::Click(n));
    }
    if let Some(after) = s.strip_prefix("after") {
        let after = after.trim();
        let mut parts = after.split_whitespace();
        let id_tok = parts
            .next()
            .ok_or("`after` needs a target, e.g. `[after #id]`")?;
        let id = id_tok
            .strip_prefix('#')
            .ok_or_else(|| format!("`after` target must be `#id`, got `{id_tok}`"))?;
        if id.is_empty() {
            return Err("empty `#id` after `after`".into());
        }
        let offset_ms = match parts.next() {
            Some(tok) => parse_signed_ms(tok)?,
            None => 0,
        };
        if parts.next().is_some() {
            return Err(format!("unexpected extra text in `[after #{id} ...]`"));
        }
        return Ok(Trigger::After {
            id: id.to_string(),
            offset_ms,
        });
    }
    Err(format!(
        "unknown trigger `[{s}]`; expected enter, click N, exit or after #id"
    ))
}

fn parse_signed_ms(tok: &str) -> Result<i64, String> {
    let (sign, digits) = if let Some(d) = tok.strip_prefix('+') {
        (1i64, d)
    } else if let Some(d) = tok.strip_prefix('-') {
        (-1i64, d)
    } else {
        return Err(format!(
            "expected a signed offset like `+200ms`, got `{tok}`"
        ));
    };
    let ms = digits
        .strip_suffix("ms")
        .ok_or_else(|| format!("expected an offset in ms, got `{tok}`"))?;
    let ms: i64 = ms
        .parse()
        .map_err(|_| format!("not a number of milliseconds: `{tok}`"))?;
    Ok(sign * ms)
}

pub fn parse_target(s: &str) -> Result<Target, String> {
    if s.chars().any(char::is_whitespace) {
        return Err(format!(
            "target must be a single token with no spaces: `{s}`"
        ));
    }
    let sel = if s == "slide" {
        ":scope".to_string()
    } else {
        s.to_string()
    };
    Ok(Target { sel, split: None })
}

/// A parsed effect clause: everything after the `:` on a track line.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEffect {
    pub effect: String,
    pub dir: Option<Direction>,
    pub dur_ms: u32,
    pub delay_ms: u32,
    pub stagger_ms: u32,
    pub ease: Ease,
    pub split: Option<Split>,
}

/// Parses an effect clause in a consumer's [`Dialect`]. The `dir=` rules are
/// enforced here rather than in `parse_line` so they hold identically in
/// every dialect: a travelling effect needs a direction, a stationary one
/// refuses it.
pub fn parse_effect_in(dialect: &Dialect, s: &str) -> Result<ParsedEffect, String> {
    let mut tokens = s.split_whitespace();
    let mut first = tokens.next().ok_or("empty effect")?;
    let split = parse_split(first);
    if split.is_some() {
        first = tokens
            .next()
            .ok_or("missing effect name after the split keyword")?;
    }
    if !dialect.effects.contains(&first) {
        return Err(format!(
            "unknown effect `{first}`; expected one of {}",
            dialect.effects.join(", ")
        ));
    }
    let effect = first.to_string();

    let mut dur_ms = None;
    let mut delay_ms = 0u32;
    let mut stagger_ms = 0u32;
    let mut ease = Ease::Named("linear".to_string());
    let mut dir = None;

    for tok in tokens {
        if let Some((k, v)) = tok.split_once('=') {
            match k {
                "dur" => dur_ms = Some((dialect.duration)(v)?),
                "delay" => delay_ms = (dialect.duration)(v)?,
                "stagger" => stagger_ms = (dialect.duration)(v)?,
                "ease" => ease = parse_ease(v)?,
                "dir" => dir = Some(parse_dir(v)?),
                other => return Err(format!("unknown attribute `{other}=`")),
            }
        } else if dur_ms.is_none() && (dialect.bare_duration)(tok) {
            dur_ms = Some((dialect.duration)(tok)?);
        } else {
            return Err(format!("unexpected token `{tok}`"));
        }
    }

    let dur_ms = dur_ms.ok_or("missing duration, e.g. `400ms`")?;
    let directional = dialect.directional.contains(&effect.as_str());
    if directional && dir.is_none() {
        return Err(format!(
            "`{effect}` needs a direction: add `dir=left|right|up|down`"
        ));
    }
    if dir.is_some() && !directional {
        return Err(format!(
            "`dir=` only applies to {}, not `{effect}`",
            dialect.directional.join("/")
        ));
    }
    Ok(ParsedEffect {
        effect,
        dir,
        dur_ms,
        delay_ms,
        stagger_ms,
        ease,
        split,
    })
}

pub fn parse_split(tok: &str) -> Option<Split> {
    match tok {
        "chars" => Some(Split::Chars),
        "words" => Some(Split::Words),
        "lines" => Some(Split::Lines),
        _ => None,
    }
}

pub fn parse_ms(tok: &str) -> Result<u32, String> {
    tok.strip_suffix("ms")
        .and_then(|d| d.parse().ok())
        .ok_or_else(|| format!("expected a duration in ms, e.g. `400ms`, got `{tok}`"))
}

pub fn parse_dir(v: &str) -> Result<Direction, String> {
    match v {
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        other => Err(format!(
            "unknown direction `{other}`; expected left, right, up or down"
        )),
    }
}

pub fn parse_ease(v: &str) -> Result<Ease, String> {
    if let Some(inner) = v.strip_prefix("spring(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        let [m, k, c] = parts.as_slice() else {
            return Err(format!(
                "`spring(...)` needs 3 arguments: mass, stiffness, damping, got `{v}`"
            ));
        };
        let mass: f64 = m.parse().map_err(|_| format!("bad spring mass: `{m}`"))?;
        let stiffness: f64 = k
            .parse()
            .map_err(|_| format!("bad spring stiffness: `{k}`"))?;
        let damping: f64 = c
            .parse()
            .map_err(|_| format!("bad spring damping: `{c}`"))?;
        if mass <= 0.0 || stiffness <= 0.0 || damping < 0.0 {
            return Err(format!(
                "spring mass and stiffness must be positive, damping must not be negative: `{v}`"
            ));
        }
        return Ok(Ease::Spring {
            mass,
            stiffness,
            damping,
        });
    }
    if css_ease(v).is_some() {
        return Ok(Ease::Named(v.to_string()));
    }
    Err(format!("unknown ease `{v}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_without_a_trigger_bracket_errors() {
        let doc = parse(".title : chars fade-in 400ms stagger=30ms ease=out-cubic\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("[trigger]"));
    }

    #[test]
    fn parses_the_docs_example() {
        let src = "\
[enter]   .title       : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #latency-0-2 : grow-y 500ms
[after #latency-0-2 +200ms] .caption : fade-in 300ms
[exit]    slide        : iris-out 500ms
";
        let doc = parse(src);
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.tracks.len(), 4);

        let t0 = &doc.tracks[0];
        assert_eq!(t0.trigger, Trigger::Enter);
        assert_eq!(t0.target.sel, ".title");
        assert_eq!(t0.target.split, Some(Split::Chars));
        assert_eq!(t0.effect, "fade-in");
        assert_eq!(t0.dur_ms, 400);
        assert_eq!(t0.stagger_ms, 30);
        assert_eq!(t0.ease, Ease::Named("out-cubic".into()));

        let t1 = &doc.tracks[1];
        assert_eq!(t1.trigger, Trigger::Click(1));
        assert_eq!(t1.target.sel, "#latency-0-2");
        assert_eq!(t1.effect, "grow-y");
        assert_eq!(t1.dur_ms, 500);
        assert_eq!(t1.delay_ms, 0);

        let t2 = &doc.tracks[2];
        assert_eq!(
            t2.trigger,
            Trigger::After {
                id: "latency-0-2".into(),
                offset_ms: 200,
            }
        );
        assert_eq!(t2.target.sel, ".caption");

        let t3 = &doc.tracks[3];
        assert_eq!(t3.trigger, Trigger::Exit);
        assert_eq!(t3.target.sel, ":scope");
        assert_eq!(t3.effect, "iris-out");

        assert_eq!(steps(&doc), 1);
    }

    #[test]
    fn negative_after_offset() {
        let doc = parse("[after #a -50ms] .b : fade-in 100ms\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(
            doc.tracks[0].trigger,
            Trigger::After {
                id: "a".into(),
                offset_ms: -50,
            }
        );
    }

    #[test]
    fn after_without_offset_defaults_to_zero() {
        let doc = parse("[after #a] .b : fade-in 100ms\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(
            doc.tracks[0].trigger,
            Trigger::After {
                id: "a".into(),
                offset_ms: 0,
            }
        );
    }

    #[test]
    fn steps_is_the_highest_click_number_not_the_track_count() {
        let doc = parse(
            "[click 2] .a : fade-in 100ms\n[click 2] .b : fade-in 100ms\n[click 1] .c : fade-in 100ms\n",
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(steps(&doc), 2);
    }

    #[test]
    fn steps_is_zero_without_click_triggers() {
        let doc = parse("[enter] .a : fade-in 100ms\n");
        assert_eq!(steps(&doc), 0);
    }

    #[test]
    fn unknown_effect_is_reported_as_a_warning_line() {
        let doc = parse("[enter] .a : nonexistent 100ms\n");
        assert!(doc.tracks.is_empty());
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("anim line 1"));
        assert!(doc.errors[0].contains("unknown effect"));
    }

    #[test]
    fn missing_duration_errors() {
        let doc = parse("[enter] .a : fade-in\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("missing duration"));
    }

    #[test]
    fn slide_in_without_direction_errors() {
        let doc = parse("[enter] .a : slide-in 300ms\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("needs a direction"));
    }

    #[test]
    fn slide_in_with_direction_succeeds() {
        let doc = parse("[enter] .a : slide-in 300ms dir=left\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.tracks[0].dir, Some(Direction::Left));
    }

    #[test]
    fn direction_on_non_slide_effect_errors() {
        let doc = parse("[enter] .a : fade-in 300ms dir=left\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("only applies to slide-in"));
    }

    #[test]
    fn unknown_trigger_errors() {
        let doc = parse("[hover] .a : fade-in 100ms\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("unknown trigger"));
    }

    #[test]
    fn click_zero_errors() {
        let doc = parse("[click 0] .a : fade-in 100ms\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("start at 1"));
    }

    #[test]
    fn missing_colon_errors() {
        let doc = parse("[enter] .a fade-in 100ms\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("target : effect"));
    }

    #[test]
    fn one_bad_line_does_not_hide_the_rest() {
        let doc = parse("[nope] .a : fade-in 100ms\n[enter] .b : fade-in 100ms\n");
        assert_eq!(doc.tracks.len(), 1);
        assert_eq!(doc.errors.len(), 1);
    }

    #[test]
    fn blank_lines_are_skipped() {
        let doc = parse("\n[enter] .a : fade-in 100ms\n\n\n");
        assert_eq!(doc.tracks.len(), 1);
        assert!(doc.errors.is_empty());
    }

    #[test]
    fn to_json_has_the_c1_shape() {
        let doc = parse("[enter] .title : chars fade-in 400ms stagger=30ms ease=out-cubic\n");
        let json: serde_json::Value = serde_json::from_str(&to_json(&doc)).unwrap();
        assert_eq!(json["steps"], 0);
        let track = &json["tracks"][0];
        assert_eq!(track["trigger"]["kind"], "enter");
        assert_eq!(track["target"]["sel"], ".title");
        assert_eq!(track["target"]["split"], "chars");
        assert_eq!(track["effect"], "fade-in");
        assert_eq!(track["dur"], 400);
        assert_eq!(track["delay"], 0);
        assert_eq!(track["stagger"], 30);
        // Named curves lower to CSS here, so the viewer can pass the value
        // straight to the Web Animations API.
        assert_eq!(track["ease"], "cubic-bezier(.33,1,.68,1)");
    }

    #[test]
    fn to_json_includes_dir_only_for_slide_effects() {
        let doc = parse("[enter] .a : slide-in 300ms dir=left\n");
        let json: serde_json::Value = serde_json::from_str(&to_json(&doc)).unwrap();
        assert_eq!(json["tracks"][0]["dir"], "left");

        let doc2 = parse("[enter] .a : fade-in 300ms\n");
        let json2: serde_json::Value = serde_json::from_str(&to_json(&doc2)).unwrap();
        assert!(json2["tracks"][0].get("dir").is_none());
    }

    #[test]
    fn spring_ease_resolves_to_a_sampled_linear_curve() {
        let doc = parse("[enter] .a : fade-in 400ms ease=spring(1,180,12)\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let json: serde_json::Value = serde_json::from_str(&to_json(&doc)).unwrap();
        let ease = json["tracks"][0]["ease"].as_str().unwrap();
        assert!(ease.starts_with("linear("));
        assert!(ease.ends_with(')'));
        // SPRING_SAMPLES values means SPRING_SAMPLES - 1 commas.
        assert_eq!(ease.matches(',').count(), SPRING_SAMPLES - 1);
    }

    #[test]
    fn spring_starts_at_zero_and_approaches_one() {
        // Heavily damped: settles smoothly without overshoot, so the shape is
        // easy to assert on. The first sample is exactly the rest position.
        let doc = parse("[enter] .a : fade-in 1000ms ease=spring(1,120,40)\n");
        let json: serde_json::Value = serde_json::from_str(&to_json(&doc)).unwrap();
        let ease = json["tracks"][0]["ease"].as_str().unwrap();
        let nums: Vec<f64> = ease
            .trim_start_matches("linear(")
            .trim_end_matches(')')
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        assert_eq!(nums[0], 0.0);
        assert!((nums[nums.len() - 1] - 1.0).abs() < 0.2);
    }

    #[test]
    fn unknown_ease_errors() {
        let doc = parse("[enter] .a : fade-in 100ms ease=bounce\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("unknown ease"));
    }

    #[test]
    fn malformed_spring_errors() {
        let doc = parse("[enter] .a : fade-in 100ms ease=spring(1,2)\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("3 arguments"));
    }

    #[test]
    fn whole_slide_target_keyword() {
        let doc = parse("[exit] slide : iris-out 500ms\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.tracks[0].target.sel, ":scope");
    }

    #[test]
    fn target_with_a_space_errors() {
        let doc = parse("[enter] .a .b : fade-in 100ms\n");
        assert_eq!(doc.errors.len(), 1);
    }

    /// A consumer's dialect can rename the effect set and re-base durations
    /// (here: beats at 120 bpm) — and the shared rules still hold.
    #[test]
    fn a_dialect_supplies_its_own_effects_and_durations() {
        fn beats(t: &str) -> Result<u32, String> {
            if let Some(n) = t.strip_suffix('b') {
                let v: f64 = n.parse().map_err(|_| format!("bad beats `{t}`"))?;
                return Ok((v * 500.0).round() as u32); // 120 bpm
            }
            parse_ms(t)
        }
        fn bare(t: &str) -> bool {
            t.ends_with('b') || t.ends_with("ms")
        }
        let d = Dialect {
            effects: &["pop", "strobe-slide"],
            directional: &["strobe-slide"],
            duration: &beats,
            bare_duration: &bare,
        };
        let eff = parse_effect_in(&d, "chars pop 1b stagger=250ms ease=out-back").unwrap();
        assert_eq!(eff.effect, "pop");
        assert_eq!(eff.dur_ms, 500);
        assert_eq!(eff.stagger_ms, 250);
        assert_eq!(eff.split, Some(Split::Chars));

        // `dir=` rules are the grammar's, not the dialect's.
        let e = parse_effect_in(&d, "strobe-slide 1b").unwrap_err();
        assert!(e.contains("needs a direction"), "{e}");
        let e = parse_effect_in(&d, "pop 1b dir=left").unwrap_err();
        assert!(e.contains("only applies to strobe-slide"), "{e}");
    }

    /// The default dialect is exactly the slide grammar: `ms` only.
    #[test]
    fn the_default_dialect_rejects_beat_durations() {
        let e = parse_effect_in(&Dialect::default(), "fade-in 1b").unwrap_err();
        assert!(e.contains("unexpected token `1b`"), "{e}");
        let eff = parse_effect_in(&Dialect::default(), "fade-in 400ms").unwrap();
        assert_eq!(eff.dur_ms, 400);
    }

    /// `split_line` hands the trigger back verbatim for the caller to parse.
    #[test]
    fn split_line_does_not_interpret_the_trigger() {
        let (trig, target, effect) = split_line("[beat 3] #x : pop 1b").unwrap();
        assert_eq!(trig, "beat 3");
        assert_eq!(target, "#x");
        assert_eq!(effect, "pop 1b");
        assert!(split_line("no brackets").is_err());
        assert!(split_line("[enter] no colon").is_err());
    }

    #[test]
    fn transition_name_alone_is_enough() {
        let t = parse_transition("fade").unwrap();
        assert_eq!(t.in_effect, "fade-in");
        assert_eq!(t.out_effect, "fade-out");
        assert_eq!(t.dur_ms, 300);
        assert_eq!(t.dir, None);
    }

    #[test]
    fn sliding_transitions_carry_a_direction() {
        let t = parse_transition("slide-left 400ms").unwrap();
        assert_eq!(t.in_effect, "slide-in");
        assert_eq!(t.out_effect, "slide-out");
        assert_eq!(t.dir, Some(Direction::Left));
        assert_eq!(t.dur_ms, 400);
        let json: serde_json::Value = serde_json::from_str(&transition_json(&t)).unwrap();
        assert_eq!(json["dir"], "left");
        assert_eq!(json["dur"], 400);
    }

    #[test]
    fn every_named_ease_lowers_to_a_css_easing_function() {
        for (name, css) in NAMED_EASES {
            assert_eq!(css_ease(name), Some(*css));
            assert!(
                *css == "linear" || css.starts_with("ease") || css.starts_with("cubic-bezier("),
                "`{name}` lowers to `{css}`, which the Web Animations API will reject"
            );
        }
    }

    #[test]
    fn transition_takes_an_ease_and_rejects_junk() {
        let t = parse_transition("iris 500ms ease=out-back").unwrap();
        assert_eq!(t.ease, Ease::Named("out-back".into()));
        assert!(parse_transition("swipe-left").is_err());
        assert!(parse_transition("fade sideways").is_err());
        assert!(parse_transition("fade stagger=10ms").is_err());
    }

    #[test]
    fn a_spring_transition_is_sampled_like_a_track() {
        let t = parse_transition("fade 300ms ease=spring(1,180,20)").unwrap();
        let json: serde_json::Value = serde_json::from_str(&transition_json(&t)).unwrap();
        assert!(json["ease"].as_str().unwrap().starts_with("linear("));
    }

    /// The interval reads in seconds by default because that is the unit a
    /// dwell time is thought in; `ms` is still accepted for symmetry with
    /// every other duration in the file.
    #[test]
    fn autoplay_parses_an_interval_and_a_loop() {
        let a = parse_autoplay("8s").unwrap();
        assert_eq!(a.interval_ms, 8000);
        assert!(!a.looped);

        let a = parse_autoplay("1.5s loop").unwrap();
        assert_eq!(a.interval_ms, 1500);
        assert!(a.looped);

        // Order-insensitive, bare numbers are seconds, `ms` is milliseconds.
        assert_eq!(parse_autoplay("loop 4").unwrap().interval_ms, 4000);
        assert_eq!(parse_autoplay("750ms").unwrap().interval_ms, 750);

        let json: serde_json::Value =
            serde_json::from_str(&autoplay_json(&parse_autoplay("8s loop").unwrap())).unwrap();
        assert_eq!(json["ms"], 8000);
        assert_eq!(json["loop"], true);
    }

    #[test]
    fn autoplay_refuses_nonsense() {
        // `loop` alone has no pace to play at.
        assert!(parse_autoplay("loop").is_err());
        assert!(parse_autoplay("").is_err());
        assert!(parse_autoplay("fast").is_err());
        // Zero, negative and sub-100ms intervals would spin the deck.
        assert!(parse_autoplay("0s").is_err());
        assert!(parse_autoplay("-5s").is_err());
        assert!(parse_autoplay("50ms").is_err());
    }
}
