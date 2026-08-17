//! Preprocessing applied to the Markdown inside a pane:
//! - Attribute syntax `{#id .class k=v}` on headings, images and spans becomes raw HTML
//! - Math `$...$` / `$$...$$` is converted to MathML at build time; the deck's
//!   `math:` frontmatter chooses whether the source is LaTeX or Typst

use mirzam_core::MathDialect;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Parsed `#id .class k=v` attribute list.
#[derive(Debug, Default, Clone)]
pub struct Attrs {
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub kv: BTreeMap<String, String>,
}

pub fn parse_attrs(src: &str) -> Attrs {
    let mut a = Attrs::default();
    for token in src.split_whitespace() {
        if let Some(id) = token.strip_prefix('#') {
            a.id = Some(id.to_string());
        } else if let Some(cls) = token.strip_prefix('.') {
            a.classes.push(cls.to_string());
        } else if let Some((k, v)) = token.split_once('=') {
            a.kv.insert(k.to_string(), v.trim_matches('"').to_string());
        }
    }
    a
}

impl Attrs {
    fn html_id_class(&self) -> String {
        let mut s = String::new();
        if let Some(id) = &self.id {
            s.push_str(&format!(" id=\"{id}\""));
        }
        if !self.classes.is_empty() {
            s.push_str(&format!(" class=\"{}\"", self.classes.join(" ")));
        }
        s
    }
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("static regex"))
}

/// Tracks fence state across lines, honouring fence length so a longer fence can
/// quote shorter ones.
struct Fences(Option<usize>);

impl Fences {
    fn new() -> Self {
        Fences(None)
    }

    /// Feeds a line and reports whether it lies inside a fence (the fence lines
    /// themselves count as inside).
    fn inside(&mut self, line: &str) -> bool {
        let trimmed = line.trim_start();
        match self.0 {
            Some(open) => {
                if mirzam_syntax::fence_len(trimmed)
                    .is_some_and(|n| n >= open && trimmed.trim_end().chars().all(|c| c == '`'))
                {
                    self.0 = None;
                }
                true
            }
            None => match mirzam_syntax::fence_len(trimmed) {
                Some(n) => {
                    self.0 = Some(n);
                    true
                }
                None => false,
            },
        }
    }
}

/// Applies `f` only to lines outside code fences.
fn map_outside_fences(src: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(src.len());
    let mut fences = Fences::new();
    for line in src.lines() {
        if fences.inside(line) {
            out.push_str(line);
        } else {
            out.push_str(&f(line));
        }
        out.push('\n');
    }
    out
}

/// Applies `f` to each contiguous run of lines outside code fences.
fn map_fence_segments(src: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(src.len());
    let mut segment = String::new();
    let mut fences = Fences::new();
    for line in src.lines() {
        let was_open = fences.0.is_some();
        if fences.inside(line) {
            // Flush the pending segment when a fence opens.
            if !was_open {
                out.push_str(&f(&segment));
                segment.clear();
            }
            out.push_str(line);
            out.push('\n');
        } else {
            segment.push_str(line);
            segment.push('\n');
        }
    }
    out.push_str(&f(&segment));
    out
}

/// [`preprocess_math`] with the default LaTeX math front end.
pub fn preprocess(src: &str) -> String {
    preprocess_math(src, MathDialect::Latex)
}

/// Preprocesses Markdown into Markdown-with-raw-HTML.
/// Math runs first so TeX such as `\sqrt[3]{x}` is not mistaken for the
/// `[...]{...}` span attribute syntax.
pub fn preprocess_math(src: &str, math: MathDialect) -> String {
    // `$$...$$` can span lines, so block math is handled per fence-free segment.
    let src = map_fence_segments(src, |s| block_math(s, math));
    // As can `<picture>`, which is usually written across four.
    let src = map_fence_segments(&src, picture_modes);
    let src = map_outside_fences(&src, |l| inline_math(l, math));
    let src = map_outside_fences(&src, emphasis_guard);
    let src = map_outside_fences(&src, heading_attrs);
    let src = map_outside_fences(&src, image_attrs);
    map_outside_fences(&src, span_attrs)
}

/// A `<picture>` choosing art by `prefers-color-scheme` becomes one image per
/// mode, switched the way `bg-light=`/`bg-dark=` switches.
///
/// This is the standard way a README ships a logo that survives GitHub's dark
/// theme, and `--split h2` on a README is a headline feature — so the markup
/// arrives whether or not anyone writing a deck chose it. Left alone it is
/// wrong here in a way it never is on GitHub: `media` can only ask the
/// *machine*, while a deck's mode is `mode:`, `?mode=` or the reader pressing
/// `D`. Mirzam's own README, published as a deck, showed a pale wordmark on a
/// white slide for exactly that reason — the deck was light and the phone was
/// not.
///
/// Both images ship, as they already did; only which one is displayed changes.
/// A `<picture>` with no `prefers-color-scheme` source is left untouched, since
/// then it is doing something else (art direction by width, a format
/// fallback) that this would break.
fn picture_modes(segment: &str) -> String {
    static PICTURE: OnceLock<Regex> = OnceLock::new();
    static DARK_SRC: OnceLock<Regex> = OnceLock::new();
    static IMG: OnceLock<Regex> = OnceLock::new();
    let picture = re(&PICTURE, r"(?s)<picture\b[^>]*>(.*?)</picture>");
    // `srcset` may carry a candidate list; the plain URL is the first entry.
    let dark = re(
        &DARK_SRC,
        r#"(?s)<source\b[^>]*prefers-color-scheme:\s*dark[^>]*srcset\s*=\s*"([^"]+)""#,
    );
    let img = re(&IMG, r"(?s)<img\b[^>]*>");

    picture
        .replace_all(segment, |c: &regex::Captures| {
            let inner = &c[1];
            let (Some(d), Some(i)) = (dark.captures(inner), img.find(inner)) else {
                return c[0].to_string();
            };
            let dark_src = d[1]
                .split(',')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            let light_tag = i.as_str();
            let Some(dark_tag) = swap_src(light_tag, dark_src) else {
                return c[0].to_string();
            };
            format!(
                "{}{}",
                add_class(light_tag, "mz-only-light"),
                add_class(&dark_tag, "mz-only-dark")
            )
        })
        .into_owned()
}

/// The same `<img>` tag pointing somewhere else, so alt text, width and every
/// other attribute the author wrote survives into the second copy.
fn swap_src(tag: &str, src: &str) -> Option<String> {
    static SRC: OnceLock<Regex> = OnceLock::new();
    let r = re(&SRC, r#"\ssrc\s*=\s*"[^"]*""#);
    let m = r.find(tag)?;
    Some(format!(
        "{}{}{}",
        &tag[..m.start()],
        format_args!(" src=\"{src}\""),
        &tag[m.end()..]
    ))
}

/// Adds a class, joining one the author already wrote rather than replacing it.
fn add_class(tag: &str, class: &str) -> String {
    static CLASS: OnceLock<Regex> = OnceLock::new();
    let r = re(&CLASS, r#"\sclass\s*=\s*"([^"]*)""#);
    match r.captures(tag) {
        Some(c) => {
            let m = c.get(0).expect("whole match");
            format!(
                "{} class=\"{} {class}\"{}",
                &tag[..m.start()],
                &c[1],
                &tag[m.end()..]
            )
        }
        // `<img ...>` and `<img ... />` both end in `>`, and the slash is an
        // attribute-position character, so inserting before it is safe.
        None => {
            let cut = tag.rfind('>').unwrap_or(tag.len());
            let (head, tail) = tag.split_at(cut);
            format!("{head} class=\"{class}\"{tail}")
        }
    }
}

/// `## Text {attrs}` becomes `<h2 ...>Text</h2>` with inline Markdown rendered.
fn heading_attrs(line: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let r = re(&RE, r"^(#{1,6})\s+(.*?)\s*\{([^{}]*)\}\s*$");
    match r.captures(line) {
        Some(c) => {
            let level = c[1].len();
            let attrs = parse_attrs(&c[3]);
            let inner = render_inline(&c[2]);
            format!("<h{level}{}>{inner}</h{level}>", attrs.html_id_class())
        }
        None => line.to_string(),
    }
}

/// `![alt](src){attrs}` becomes an `<img>`, `<video>`, `<audio>` or an embed.
///
/// The attribute block is optional, because what a reference *is* follows from
/// its target, not from whether the author wrote any attributes: a bare
/// `![clip](talk.mp4)` used to slip past this and become a broken image.
/// A plain image with no attributes is handed back untouched for the Markdown
/// parser to render, which keeps `title` text and reference links working.
fn image_attrs(line: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let r = re(&RE, r#"!\[([^\]]*)\]\(([^()\s"]+)\)(?:\{([^{}]*)\})?"#);
    r.replace_all(line, |c: &regex::Captures| {
        let alt = html_escape(&c[1]);
        let src = &c[2];
        let braces = c.get(3);
        if braces.is_none() && !is_video(src) && !is_audio(src) && embed_url(src).is_none() {
            return c[0].to_string();
        }
        let attrs = parse_attrs(braces.map(|m| m.as_str()).unwrap_or(""));
        let mut style = String::new();
        match attrs.kv.get("fit").map(String::as_str) {
            Some("contain") => style.push_str("object-fit:contain;width:100%;height:100%;"),
            Some("cover") => style.push_str("object-fit:cover;width:100%;height:100%;"),
            _ => {}
        }
        if let Some(w) = attrs.kv.get("w") {
            style.push_str(&format!("width:{w};"));
        }
        if let Some(h) = attrs.kv.get("h") {
            style.push_str(&format!("height:{h};"));
        }
        if attrs.kv.get("align").map(String::as_str) == Some("center") {
            style.push_str("display:block;margin-inline:auto;");
        }
        let style_attr = if style.is_empty() {
            String::new()
        } else {
            format!(" style=\"{style}\"")
        };
        if is_video(src) {
            return video_html(src, &alt, &attrs, &style_attr);
        }
        if is_audio(src) {
            return audio_html(src, &alt, &attrs);
        }
        if let Some(embed) = embed_url(src) {
            return embed_html(&embed, src, &alt, &attrs);
        }
        format!(
            "<img src=\"{src}\" alt=\"{alt}\"{}{style_attr}>",
            attrs.html_id_class()
        )
    })
    .into_owned()
}

fn extension_of(src: &str) -> Option<String> {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    path.rsplit('.').next().map(str::to_ascii_lowercase)
}

/// Whether an image reference points at a video; GIFs stay images.
fn is_video(src: &str) -> bool {
    matches!(
        extension_of(src).as_deref(),
        Some("mp4" | "webm" | "ogv" | "mov" | "m4v")
    )
}

fn is_audio(src: &str) -> bool {
    matches!(
        extension_of(src).as_deref(),
        Some("mp3" | "m4a" | "wav" | "oga" | "ogg" | "flac" | "aac" | "opus")
    )
}

/// `![Interview](clip.mp3)` becomes an `<audio>`. The file is inlined like
/// any other asset, so a deck with a recording in it is still one file.
fn audio_html(src: &str, alt: &str, attrs: &Attrs) -> String {
    let autoplay = attrs.classes.iter().any(|c| c == "autoplay");
    let mut flags = String::from(" controls");
    if autoplay {
        flags.push_str(" autoplay");
    }
    if attrs.classes.iter().any(|c| c == "loop") {
        flags.push_str(" loop");
    }
    let mut carried = attrs.clone();
    carried
        .classes
        .retain(|c| !matches!(c.as_str(), "autoplay" | "loop" | "controls"));
    format!(
        "<div class=\"mz-audio\"{}><audio src=\"{src}\" title=\"{alt}\"{flags}></audio>\
         <span class=\"mz-audio-label\">{alt}</span></div>",
        carried.html_id_class()
    )
}

/// Where the two hosts' frames are served from. Named because the asset pass
/// has to recognise a URL this file wrote — see [`is_player_url`].
const YOUTUBE_PLAYER: &str = "https://www.youtube-nocookie.com/embed/";
const VIMEO_PLAYER: &str = "https://player.vimeo.com/video/";

/// Whether this is a player URL Mirzam generated for a hosted video.
///
/// The asset pass reports every reference it leaves on the network, because a
/// deck that quietly stops being one file is worth knowing about. A hosted
/// video is the documented exception (`docs/syntax.md`), so it must not be
/// reported as a surprise — and the prefixes come from the constants
/// [`embed_url`] builds with, so the two cannot drift apart.
pub(crate) fn is_player_url(src: &str) -> bool {
    src.starts_with(YOUTUBE_PLAYER) || src.starts_with(VIMEO_PLAYER)
}

/// The player URL for a video-host page, or `None` when this is not one.
///
/// Only the two hosts worth special-casing: anything else can be written as a
/// link, and an author who wants some other embed can still write the `iframe`
/// by hand.
fn embed_url(src: &str) -> Option<String> {
    let rest = src
        .strip_prefix("https://")
        .or_else(|| src.strip_prefix("http://"))?;
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let id_ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    };

    if let Some(q) = rest.strip_prefix("youtube.com/watch?") {
        let id = q
            .split('&')
            .find_map(|p| p.strip_prefix("v="))?
            .split('#')
            .next()?;
        return id_ok(id).then(|| format!("{YOUTUBE_PLAYER}{id}"));
    }
    if let Some(id) = rest.strip_prefix("youtu.be/") {
        let id = id.split(['?', '#']).next()?;
        return id_ok(id).then(|| format!("{YOUTUBE_PLAYER}{id}"));
    }
    if let Some(id) = rest.strip_prefix("vimeo.com/") {
        let id = id.split(['?', '#', '/']).next()?;
        return id
            .chars()
            .all(|c| c.is_ascii_digit())
            .then(|| format!("{VIMEO_PLAYER}{id}"));
    }
    None
}

/// A hosted video, as an `iframe` filling its pane.
///
/// This is the one thing in a deck that is *not* self-contained: the frame is
/// fetched when the slide is shown. The original page URL is carried on the
/// wrapper so the print path can offer it as a link instead — a PDF cannot
/// play anything.
fn embed_html(player: &str, page: &str, alt: &str, attrs: &Attrs) -> String {
    format!(
        "<div class=\"mz-embed\"{} data-href=\"{page}\" data-title=\"{alt}\">\
         <iframe src=\"{player}\" title=\"{alt}\" loading=\"lazy\" allowfullscreen \
         allow=\"accelerometer; clipboard-write; encrypted-media; picture-in-picture\"></iframe></div>",
        attrs.html_id_class()
    )
}

/// `![alt](demo.mp4){.autoplay .loop .controls poster=...}` becomes a `<video>`.
fn video_html(src: &str, alt: &str, attrs: &Attrs, style_attr: &str) -> String {
    // Boolean attributes accept either class syntax (`.autoplay`) or `key=true`.
    let flag = |name: &str| -> bool {
        attrs.classes.iter().any(|c| c == name)
            || matches!(attrs.kv.get(name).map(String::as_str), Some("" | "true"))
    };
    let mut flags = String::new();
    // Browsers block audible autoplay, so `autoplay` implies `muted`.
    let autoplay = flag("autoplay");
    for (name, on) in [
        ("autoplay", autoplay),
        ("muted", flag("muted") || autoplay),
        ("loop", flag("loop")),
        ("controls", flag("controls")),
        ("playsinline", true),
    ] {
        if on {
            flags.push(' ');
            flags.push_str(name);
        }
    }
    let poster = attrs
        .kv
        .get("poster")
        .map(|p| format!(" poster=\"{p}\""))
        .unwrap_or_default();
    // Drop the boolean-flag classes from the emitted class list.
    let mut carried = attrs.clone();
    carried
        .classes
        .retain(|c| !matches!(c.as_str(), "autoplay" | "muted" | "loop" | "controls"));
    format!(
        "<video src=\"{src}\" title=\"{alt}\"{}{poster}{flags}{style_attr}></video>",
        carried.html_id_class()
    )
}

/// `[text]{attrs}` becomes `<span ...>text</span>`; links `[t](u)` are left alone.
///
/// The closing `]` is found by matching brackets, not by refusing to allow one
/// inside: a footnote reference, a nested span and inline maths each bring
/// their own `[...]`, and every one of them used to turn the whole run into
/// literal `[…]{.small}` on the slide — a failure that looks exactly like
/// Markdown the author meant literally. The content stays Markdown and is
/// handed to the one parse of the slide rather than rendered here on its own,
/// which is what lets `[…[^a]]{.small}` find the footnote's definition.
fn span_attrs(line: &str) -> String {
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < cs.len() {
        // `![alt](src){attrs}` belongs to the image pass, and a `\[` is the
        // author asking for a literal bracket.
        let escaped = i > 0 && (cs[i - 1] == '!' || cs[i - 1] == '\\');
        match (cs[i], escaped) {
            ('[', false) => match span_at(&cs, i) {
                Some((inner, attrs, next)) => {
                    let a = parse_attrs(&attrs);
                    // Recurse, so `[a [b]{.accent} c]{.small}` nests.
                    out.push_str(&format!(
                        "<span{}>{}</span>",
                        a.html_id_class(),
                        span_attrs(&inner)
                    ));
                    i = next;
                }
                None => {
                    out.push('[');
                    i += 1;
                }
            },
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// The span opening at `open`, as `(content, attributes, index after it)`.
///
/// `None` when the brackets do not close on this line, when nothing is between
/// them, or when what follows the `]` is not a single `{...}` — a link's
/// `[text](url)` and a plain bracketed aside both land there and are left
/// alone.
fn span_at(cs: &[char], open: usize) -> Option<(String, String, usize)> {
    let mut depth = 0usize;
    let mut close = None;
    let mut i = open;
    while i < cs.len() {
        match cs[i] {
            // An escape covers whatever follows, bracket or not.
            '\\' => i += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let close = close?;
    if close == open + 1 || cs.get(close + 1) != Some(&'{') {
        return None;
    }
    let end = cs[close + 2..]
        .iter()
        .position(|c| *c == '}' || *c == '{')
        .map(|p| close + 2 + p)?;
    if cs[end] != '}' {
        return None;
    }
    Some((
        cs[open + 1..close].iter().collect(),
        cs[close + 2..end].iter().collect(),
        end + 1,
    ))
}

/// Keeps `**+ text**` bold, the way every other Markdown parser reads it.
///
/// `comrak` decides whether a `*`/`_` run can open emphasis by looking at the
/// character after it, and it scans past the extension delimiters `~`, `=`
/// and `+` first so that `**==marked==**` works. When one of those is on its
/// own and followed by a space, that scan lands on the space, the run counts
/// as followed by whitespace, and the emphasis silently does not open —
/// `| **+ wheel odometry** |` shows its asterisks to the audience. Escaping
/// the delimiter puts a character there that comrak does not skip. It can
/// only ever be a delimiter that is followed by a space, which cannot open
/// anything of its own, so nothing else changes meaning.
fn emphasis_guard(line: &str) -> String {
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '\\' && i + 1 < cs.len() {
            out.extend(&cs[i..i + 2]);
            i += 2;
            continue;
        }
        // Nothing inside a code span is a delimiter.
        if c == '`' {
            let open = run_of(&cs, i, '`');
            out.extend(&cs[i..i + open]);
            i += open;
            while i < cs.len() {
                if cs[i] == '`' {
                    let n = run_of(&cs, i, '`');
                    out.extend(&cs[i..i + n]);
                    i += n;
                    if n == open {
                        break;
                    }
                } else {
                    out.push(cs[i]);
                    i += 1;
                }
            }
            continue;
        }
        if c == '*' || c == '_' {
            let run = run_of(&cs, i, c);
            out.extend(&cs[i..i + run]);
            i += run;
            let mut j = i;
            while matches!(cs.get(j), Some('~' | '=' | '+')) {
                j += 1;
            }
            if j > i && cs.get(j).is_none_or(|c| c.is_whitespace()) {
                out.push('\\');
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// How many of `c` start at `from`.
fn run_of(cs: &[char], from: usize, c: char) -> usize {
    cs[from..].iter().take_while(|x| **x == c).count()
}

/// Converts `$$...$$` (which may span lines) to block MathML.
fn block_math(segment: &str, math: MathDialect) -> String {
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    let b = re(&BLOCK, r"\$\$([^$]+)\$\$");
    b.replace_all(segment, |c: &regex::Captures| {
        math_html(c[1].trim(), math_core::MathDisplay::Block, math)
    })
    .into_owned()
}

/// Converts `$...$` to inline MathML.
fn inline_math(line: &str, math: MathDialect) -> String {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    let i = re(&INLINE, r"\$([^$\n]+)\$");
    i.replace_all(line, |c: &regex::Captures| {
        math_html(c[1].trim(), math_core::MathDisplay::Inline, math)
    })
    .into_owned()
}

fn math_converter() -> &'static math_core::LatexToMathML {
    static CONV: OnceLock<math_core::LatexToMathML> = OnceLock::new();
    CONV.get_or_init(|| {
        math_core::LatexToMathML::new(math_core::MathCoreConfig::default())
            .expect("default math config")
    })
}

/// Math source to MathML. A Typst formula is lowered to LaTeX first; both
/// dialects then share one converter. On failure the source is shown in an
/// error span — the source as the author wrote it, whichever step failed.
fn math_html(src: &str, display: math_core::MathDisplay, math: MathDialect) -> String {
    let tex = match math {
        MathDialect::Latex => src.to_string(),
        MathDialect::Typst => match mirzam_tmath::to_latex(src) {
            Ok(tex) => tex,
            Err(e) => return math_error(src, &e.to_string(), display),
        },
    };
    match math_converter().convert_with_local_state(&tex, display) {
        Ok(r) => widen_accent_glyphs(r.mathml),
        Err(e) => math_error(src, &e.to_string(), display),
    }
}

/// Swaps the three accent characters `math-core` picks that no browser can
/// place, for the spacing characters that draw the same mark.
///
/// A combining mark has no advance width and its ink sits to the *left* of the
/// origin, because it is drawn over the character already typed. An `mover`
/// centres its accent on the base by advance width, so a zero-width mark lands
/// beside the letter rather than above it: `overline(z)` puts the bar off the
/// left shoulder of the `z`, and `arrow(v)` hangs the arrow outside the `v`.
/// The mark only lands right when the browser stretches it — which it does for
/// `\widehat`'s U+0302, and does not for a base too narrow to need stretching.
///
/// Every other accent already arrives as a spacing character (`\hat` U+02C6,
/// `\bar` U+00AF, `\dot` U+02D9), which is why only these three are wrong, and
/// why they are wrong the same way in display and inline math. The replacements
/// are the spacing twins of the same marks, and U+2192 is what `\overrightarrow`
/// is already given — so `arrow(v)` and `arrow(A B)`, one Typst function that
/// lowers to `\vec` or `\overrightarrow` by width, stop drawing two arrows.
fn widen_accent_glyphs(mathml: String) -> String {
    // Cheap reject: the marks are rare, the pages are not.
    if !mathml.contains(['\u{332}', '\u{20d7}']) {
        return mathml;
    }
    mathml
        // U+0332 is `\overline` above and `\underline` below — one character
        // for both, so which spacing twin it wants depends on the element.
        .replace(
            "<mo stretchy=\"true\">\u{332}</mo></mover>",
            "<mo stretchy=\"true\">\u{203e}</mo></mover>",
        )
        .replace(
            "<mo stretchy=\"true\">\u{332}</mo></munder>",
            "<mo stretchy=\"true\">\u{5f}</mo></munder>",
        )
        // `\vec`. Scoped to the `mo` it arrives in, so the same character
        // typed inside `\text{...}` reaches the slide as the author wrote it.
        .replace(
            "<mo stretchy=\"false\">\u{20d7}</mo>",
            "<mo stretchy=\"false\">\u{2192}</mo>",
        )
}

/// Renders one formula on its own — MathML, or the error span a deck would
/// show. For hosts that preview a formula outside any slide, such as the
/// browser editor's math panel.
pub fn render_math(src: &str, math: MathDialect, block: bool) -> String {
    let display = if block {
        math_core::MathDisplay::Block
    } else {
        math_core::MathDisplay::Inline
    };
    math_html(src.trim(), display, math)
}

fn math_error(shown: &str, error: &str, display: math_core::MathDisplay) -> String {
    let cls = match display {
        math_core::MathDisplay::Block => "math-error math-block",
        math_core::MathDisplay::Inline => "math-error",
    };
    // The span is raw HTML in a Markdown document, so its text is still read
    // as Markdown: a backslash before punctuation would be eaten as an escape
    // and the failing formula shown back to the author without it — `\/`
    // displayed as `/`, which is not the line they typed. A numeric reference
    // reaches the page as a backslash without ever looking like one.
    format!(
        "<span class=\"{cls}\" title=\"{}\">{}</span>",
        html_escape(error),
        html_escape(shown).replace('\\', "&#92;")
    )
}

/// Renders Markdown inline, stripping the wrapping `<p>`.
pub fn render_inline(md: &str) -> String {
    let html = render_markdown(md);
    let t = html.trim();
    let t = t.strip_prefix("<p>").unwrap_or(t);
    let t = t.strip_suffix("</p>").unwrap_or(t);
    t.to_string()
}

/// Markdown to HTML via comrak: raw HTML allowed, GFM extensions, CJK-friendly emphasis.
pub fn render_markdown(md: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    // The marks a slide actually reaches for, none of which CommonMark has.
    // Each degrades to its own punctuation in a plain reader rather than
    // vanishing, which is the trade the rest of the markup makes too.
    // ==marked==
    options.extension.highlight = true;
    // ++underlined++
    options.extension.insert = true;
    // A term and its definition, which is a list shape a slide wants often and
    // has had to fake with a two-column table.
    options.extension.description_lists = true;
    // `:tada:`. Writing the character directly still works and always did;
    // this is for the keyboards that make that hard.
    options.extension.shortcodes = true;
    // Handles emphasis adjacent to CJK text and punctuation correctly.
    options.extension.cjk_friendly_emphasis = true;
    // Citations. A slide is rendered on its own, so the notes collect at the
    // foot of the slide that cites them, which is where a reference belongs on
    // a slide rather than at the end of a document nobody scrolls.
    options.extension.footnotes = true;
    // A bare DOI or arXiv URL in a reference becomes a link without ceremony.
    options.extension.autolink = true;
    options.render.r#unsafe = true;
    // Syntax highlighting runs here rather than in a later pass over the HTML,
    // because comrak hands the adapter the *raw* fence contents: the tokenizer
    // sees the code the author wrote, and escaping happens once, on the way
    // out. A language the table in `code.rs` does not list writes exactly the
    // bytes comrak would have written on its own.
    let mut plugins = comrak::options::Plugins::default();
    plugins.render.codefence_syntax_highlighter = Some(&crate::code::Highlighter);
    comrak::markdown_to_html_with_plugins(md, &options, &plugins)
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_with_attrs() {
        let out = preprocess("## Title {.center #sec1}\n");
        assert!(out.contains("<h2 id=\"sec1\" class=\"center\">Title</h2>"));
    }

    #[test]
    fn span_with_attrs_not_link() {
        let out = preprocess("see [word]{#w .u} and [link](http://x)\n");
        assert!(out.contains("<span id=\"w\" class=\"u\">word</span>"));
        assert!(out.contains("[link](http://x)"));
    }

    #[test]
    fn a_span_holds_brackets_of_its_own() {
        // A footnote reference, a nested span and inline maths all put a `]`
        // inside the span. Each of them used to end the run early, so the
        // whole thing reached the slide as literal `[…]{.small}` — which is
        // indistinguishable from Markdown the author meant literally.
        let out = preprocess("[a footnote ref[^a]]{.small}\n");
        assert!(
            out.contains("<span class=\"small\">a footnote ref[^a]</span>"),
            "{out}"
        );

        let out = preprocess("[a [nested]{.accent} span]{.small}\n");
        assert!(
            out.contains(
                "<span class=\"small\">a <span class=\"accent\">nested</span> span</span>"
            ),
            "{out}"
        );

        let out = preprocess_math("[maths $x[i]$ inside]{.small}\n", MathDialect::Typst);
        assert!(out.contains("<span class=\"small\">maths <math"), "{out}");
        assert!(out.contains("inside</span>"), "{out}");
    }

    #[test]
    fn a_spans_content_is_read_with_the_rest_of_the_slide() {
        // Rendering the content on its own cannot resolve a footnote, whose
        // definition is elsewhere on the slide; leaving it as Markdown for
        // the one parse of the slide can.
        let html = render_markdown(&preprocess("[an aside[^a]]{.small}\n\n[^a]: The note.\n"));
        assert!(html.contains("footnote-ref"), "{html}");
        assert!(html.contains("The note."), "{html}");
    }

    #[test]
    fn a_bracket_that_is_not_a_span_is_left_alone() {
        // Unclosed, empty, or followed by something other than one `{...}`.
        for src in ["[a {b}\n", "[]{.small}\n", "[a](b){c\n"] {
            let out = preprocess(src);
            assert!(!out.contains("<span"), "{src}: {out}");
        }
    }

    #[test]
    fn emphasis_opens_before_a_lone_extension_delimiter() {
        // `+`, `=` and `~` are the extension delimiters comrak scans past
        // when deciding whether `**` opens emphasis. On its own and followed
        // by a space, that scan landed on the space and the bold silently did
        // not open: `| **+ wheel odometry** |` showed its asterisks.
        for (src, want) in [
            ("A **+ alpha** here.\n", "<strong>+ alpha</strong>"),
            ("A **= alpha** here.\n", "<strong>= alpha</strong>"),
            ("A **~ alpha** here.\n", "<strong>~ alpha</strong>"),
            ("A *+ alpha* here.\n", "<em>+ alpha</em>"),
            // Untouched: these already matched CommonMark.
            ("A **+alpha** here.\n", "<strong>+alpha</strong>"),
            ("A **- alpha** here.\n", "<strong>- alpha</strong>"),
            // The extensions themselves still work, inside emphasis or not.
            ("A **++ins++ b** here.\n", "<ins>ins</ins>"),
            ("A ==marked== here.\n", "<mark>marked</mark>"),
            ("A **==m== b** here.\n", "<mark>m</mark>"),
        ] {
            let html = render_markdown(&preprocess(src));
            assert!(html.contains(want), "{src}: {html}");
        }
        // A code span is literal text, so nothing in one is escaped.
        let html = render_markdown(&preprocess("Write `**+ a**` for it.\n"));
        assert!(html.contains("<code>**+ a**</code>"), "{html}");
    }

    #[test]
    fn audio_becomes_a_player_with_its_label() {
        let out = preprocess("![An interview](media/talk.mp3)\n");
        assert!(out.contains("<audio src=\"media/talk.mp3\""), "{out}");
        assert!(out.contains(" controls"));
        assert!(out.contains("An interview</span>"), "{out}");
    }

    #[test]
    fn media_is_recognised_without_an_attribute_block() {
        // The braces used to be required, so a bare reference to a video
        // silently became a broken image.
        let out = preprocess("![clip](media/talk.mp4)\n");
        assert!(out.contains("<video src=\"media/talk.mp4\""), "{out}");
    }

    #[test]
    fn a_plain_image_is_left_to_the_markdown_parser() {
        let src = "![alt](img/a.png)\n";
        assert_eq!(preprocess(src), src);
    }

    #[test]
    fn video_hosts_become_embeds() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
        ] {
            let out = preprocess(&format!("![Talk]({url})\n"));
            assert!(
                out.contains("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"),
                "{url}: {out}"
            );
            // The page URL is carried so the print path can link to it.
            assert!(out.contains(&format!("data-href=\"{url}\"")), "{out}");
        }
        let out = preprocess("![V](https://vimeo.com/76979871)\n");
        assert!(
            out.contains("https://player.vimeo.com/video/76979871"),
            "{out}"
        );
    }

    #[test]
    fn a_link_that_merely_mentions_a_host_is_not_an_embed() {
        assert_eq!(embed_url("https://example.com/youtube.com/watch?v=x"), None);
        assert_eq!(embed_url("https://www.youtube.com/watch?list=x"), None);
        assert_eq!(embed_url("https://vimeo.com/channels/staffpicks"), None);
        assert_eq!(embed_url("img/a.png"), None);
    }

    #[test]
    fn footnotes_render_as_slide_local_references() {
        let out = render_markdown("Claim[^a].\n\n[^a]: Someone, *A paper*, 2026.\n");
        assert!(out.contains("footnote-ref"), "{out}");
        assert!(out.contains("A paper"), "{out}");
    }

    #[test]
    fn video_from_image_syntax() {
        let out = preprocess("![demo](media/demo.mp4){.autoplay .loop .controls fit=contain}\n");
        assert!(out.contains("<video src=\"media/demo.mp4\""));
        assert!(out.contains(" autoplay"));
        // autoplay implies muted, per browser autoplay policy.
        assert!(out.contains(" muted"));
        assert!(out.contains(" loop"));
        assert!(out.contains(" controls"));
        assert!(out.contains("object-fit:contain"));
        // Flags must not leak into the class attribute.
        assert!(!out.contains("class=\"autoplay"));
    }

    #[test]
    fn gif_stays_an_image() {
        let out = preprocess("![motion](a.gif){w=60%}\n");
        assert!(out.contains("<img src=\"a.gif\""));
    }

    #[test]
    fn image_with_fit() {
        let out = preprocess("![alt](img/a.png){fit=contain}\n");
        assert!(out.contains("<img src=\"img/a.png\""));
        assert!(out.contains("object-fit:contain"));
    }

    #[test]
    fn math_rendered_to_mathml() {
        let out = preprocess("inline $E=mc^2$ and $$\\int_0^1 x dx$$\n");
        // One inline and one block MathML element.
        assert_eq!(out.matches("<math").count(), 2);
        assert!(out.contains("display=\"block\""));
        assert!(!out.contains("math-error"));
    }

    #[test]
    fn broken_math_falls_back() {
        let out = preprocess("$\\frac{1$\n");
        assert!(out.contains("math-error"));
        // The formula is shown back as it was typed. Every backslash leaves
        // as a numeric reference: the span is raw HTML inside a Markdown
        // document, so a backslash before punctuation would be read as an
        // escape and dropped — `$mat(a \/ b)$` reached the slide as `\/`
        // turned into a bare `/`, pointing the author at a line they never
        // wrote. The reader still sees the backslash.
        assert!(out.contains("&#92;frac{1"), "{out}");
        assert!(render_markdown(&out).contains("\\frac{1"), "{out}");
        let slash = render_markdown(&preprocess_math("$$a \\/ b$$\n", MathDialect::Typst));
        assert!(slash.contains("a \\/ b"), "{slash}");
    }

    #[test]
    fn typst_math_renders_to_mathml() {
        let out = preprocess_math(
            "inline $alpha/2$ and $$sum_(i=1)^n i$$\n",
            MathDialect::Typst,
        );
        assert_eq!(out.matches("<math").count(), 2, "{out}");
        assert!(out.contains("mfrac"), "{out}");
        assert!(out.contains('α'), "{out}");
        assert!(!out.contains("math-error"), "{out}");
    }

    #[test]
    fn accents_land_over_their_base_not_beside_it() {
        // A combining mark has no advance width, so an `mover` centres it on
        // the base and the ink lands off the letter's left shoulder. These
        // three are the only accents `math-core` hands over as combining
        // marks, and the same three are the only ones that ever looked wrong.
        for (src, want) in [
            ("overline(z)", '\u{203e}'),
            ("underline(w)", '\u{5f}'),
            ("arrow(v)", '\u{2192}'),
            ("arrow(A B)", '\u{2192}'),
        ] {
            for block in [false, true] {
                let out = render_math(src, MathDialect::Typst, block);
                assert!(out.contains(want), "{src} (block={block}): {out}");
            }
        }
        // Nothing zero-width is left anywhere in the formula. If `math-core`
        // ever changes the shape this rewrite matches on, this is what says
        // so — a silent miss would put the mark back beside the letter.
        for src in [
            "overline(z)",
            "underline(w)",
            "arrow(v)",
            "overline(A B)",
            "underline(A B)",
            "arrow(A B)",
            "overline(hat(x))",
        ] {
            let out = render_math(src, MathDialect::Typst, true);
            assert!(!out.contains(['\u{332}', '\u{20d7}']), "{src}: {out}");
        }
        // The dialects share the converter, so LaTeX decks get the same fix.
        assert!(render_math("\\vec{v}", MathDialect::Latex, false).contains('\u{2192}'));
    }

    #[test]
    fn the_accents_that_were_already_right_are_left_alone() {
        // `\hat` and `\bar` arrive as spacing characters, which browsers do
        // place correctly; `\widehat` arrives combining but is always
        // stretched, which gives it a width. Rewriting them would be a
        // regression, so they are pinned here.
        assert!(render_math("hat(x)", MathDialect::Typst, true).contains('\u{2c6}'));
        assert!(render_math("macron(x)", MathDialect::Typst, true).contains('\u{af}'));
        assert!(render_math("tilde(y)", MathDialect::Typst, true).contains('\u{2dc}'));
        assert!(render_math("hat(A B)", MathDialect::Typst, true).contains('\u{302}'));
    }

    #[test]
    fn broken_typst_math_shows_its_own_source() {
        let out = preprocess_math("$sqrt(x$\n", MathDialect::Typst);
        assert!(out.contains("math-error"), "{out}");
        // The Typst source, not the intermediate LaTeX.
        assert!(out.contains("sqrt(x"), "{out}");
        assert!(out.contains("typst math"), "{out}");
    }

    #[test]
    fn the_default_dialect_is_latex() {
        assert_eq!(
            preprocess("$E=mc^2$\n"),
            preprocess_math("$E=mc^2$\n", MathDialect::Latex)
        );
    }

    #[test]
    fn multiline_block_math() {
        let out = preprocess("$$\n\\int_0^1 x\\, dx\n= \\frac{1}{2}\n$$\n");
        assert!(out.contains("<math"));
        assert!(out.contains("display=\"block\""));
        assert!(!out.contains("$$"));
    }

    #[test]
    fn subsup_is_not_staircase() {
        // latex2mathml 0.2 mis-converted x_{84}^{7} into nested msub(msup(...)).
        let out = preprocess("$x_{84}^{7}$\n");
        assert!(out.contains("<msubsup>"));
    }

    #[test]
    fn tex_brackets_not_eaten_by_span_attrs() {
        // `[3]{x}` inside \sqrt[3]{x} must not match the span attribute syntax.
        let out = preprocess("$\\sqrt[3]{x}$\n");
        assert!(out.contains("<math"));
        assert!(!out.contains("<span>3</span>"));
    }

    #[test]
    fn cjk_emphasis_with_punctuation() {
        let out = render_markdown("図形は**ページ座標(%)**で宣言する\n"); // CJK sample
        assert!(out.contains("<strong>ページ座標(%)</strong>"), "{out}");
    }

    #[test]
    fn code_fences_untouched() {
        let src = "```\n## not a heading {#x}\n```\n";
        assert_eq!(preprocess(src), src);
    }

    /// The exact shape a README uses to survive GitHub's dark theme, which
    /// `--split h2` then turns into a slide.
    #[test]
    fn picture_becomes_one_image_per_mode() {
        let out = preprocess(
            "<picture>\n  \
             <source media=\"(prefers-color-scheme: dark)\" srcset=\"logo-dark.svg\">\n  \
             <img src=\"logo-light.svg\" alt=\"Mirzam\" width=\"340\">\n\
             </picture>\n",
        );
        assert!(
            !out.contains("<picture"),
            "the picture element should be gone"
        );
        // Both ship; the alt text and the width survive into both copies.
        assert!(out.contains(
            r#"<img src="logo-light.svg" alt="Mirzam" width="340" class="mz-only-light">"#
        ));
        assert!(out.contains(
            r#"<img src="logo-dark.svg" alt="Mirzam" width="340" class="mz-only-dark">"#
        ));
    }

    /// Art direction by width, or a `webp` fallback, is doing something this
    /// rewrite would break — so it is left alone.
    #[test]
    fn a_picture_without_a_mode_source_is_left_alone() {
        let src = "<picture>\n  \
                   <source media=\"(min-width: 800px)\" srcset=\"wide.png\">\n  \
                   <img src=\"narrow.png\" alt=\"x\">\n\
                   </picture>\n";
        assert_eq!(preprocess(src), src);
    }

    #[test]
    fn an_existing_class_is_joined_rather_than_replaced() {
        let out = preprocess(
            "<picture><source media=\"(prefers-color-scheme:dark)\" srcset=\"d.svg\">\
             <img class=\"hero\" src=\"l.svg\"></picture>\n",
        );
        assert!(out.contains(r#"class="hero mz-only-light""#), "got: {out}");
        assert!(out.contains(r#"class="hero mz-only-dark""#), "got: {out}");
    }
}
