//! `mermaid` fences, rendered to inline SVG at build time.
//!
//! Two rules shape this module, both from [W23]:
//!
//! - **It follows the `chart` path, not the `shape` path.** Mermaid hands back
//!   an SVG carrying its own `viewBox`, so the diagram scales to fit the box it
//!   lands in exactly as a chart does. Nothing here takes part in the
//!   build-time pane arithmetic a `shape` block needs, so a margin moved in CSS
//!   alone cannot desynchronise a diagram.
//! - **The renderer arrives through a trait**, the way images arrive through
//!   [`crate::AssetSource`]. This crate must not touch the filesystem and must
//!   not spawn a process either — same reason, the WebAssembly build has
//!   neither. The CLI implements [`DiagramRenderer`] by running `mmdc`; the
//!   browser build implements nothing, and every `mermaid` fence stays an
//!   ordinary code block there.
//!
//! No renderer is a **warning**, never a silent fallback: the fence renders as
//! the code block a plain CommonMark parser would have made of it *and* the
//! build says so, as `build.mermaid`. That degradation is the one Mirzam
//! extension that reads better in a plain viewer than in Mirzam, because GitHub
//! renders a ```mermaid fence as a diagram itself.
//!
//! Mermaid emits its own palette. Baked in, a diagram would ignore the deck's
//! theme and stay light when the reader presses `D`, so every colour this
//! module recognises is rewritten to a `var(--mz-*)` reference — the move
//! [W20] made when it mapped a highlighter's token kinds onto classes.
//!
//! [W23]: ../../../docs/workstreams.md#w23--mermaid-diagrams-rendered-at-build-time
//! [W20]: ../../../docs/workstreams.md#w20--syntax-highlighting-at-build-time

use regex::Regex;
use std::sync::OnceLock;

/// Renders Mermaid source to a standalone SVG document.
///
/// The host answers, exactly as it does for [`crate::AssetSource`]: producing
/// an SVG means reading files and running a program, and a core crate may do
/// neither. `Err` carries a sentence for a person — it is printed as a build
/// warning beside the fence it belongs to.
pub trait DiagramRenderer {
    /// `source` is the fence body, verbatim.
    fn render(&self, source: &str) -> Result<String, String>;
}

/// Extracts ```mermaid fences, rendering each one through `renderer`.
///
/// A fence that produced a diagram is replaced by a placeholder comment
/// [`render_in`] later fills with the SVG, the way a `chart` block is. A fence
/// that did **not** — no renderer, or a renderer that failed — is left in the
/// Markdown exactly as written, so comrak makes the code block out of it that a
/// plain CommonMark parser would have, and `warnings` says why.
pub fn extract(
    md: &str,
    slide_index: usize,
    renderer: Option<&dyn DiagramRenderer>,
    warnings: &mut Vec<String>,
) -> (String, Vec<String>) {
    let mut out = String::with_capacity(md.len());
    let mut blocks: Vec<String> = Vec::new();
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "```mermaid" {
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.trim() == "```" {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            let id = format!("mz-mermaid-{}-{}", slide_index + 1, blocks.len() + 1);
            match render_one(&body, renderer, &id) {
                Ok(svg) => {
                    out.push_str(&format!("\n<!--mz-mermaid:{}-->\n", blocks.len()));
                    blocks.push(svg);
                }
                Err(why) => {
                    warnings.push(format!("slide {}: mermaid: {why}", slide_index + 1));
                    out.push_str("```mermaid\n");
                    out.push_str(&body);
                    out.push_str("```\n");
                }
            }
        } else if let Some(open) = mirzam_syntax::fence_len(trimmed).filter(|n| *n > 3) {
            // A longer fence quotes Mermaid syntax instead of using it — which
            // is how this file's own documentation shows a diagram block.
            out.push_str(line);
            out.push('\n');
            for inner in lines.by_ref() {
                out.push_str(inner);
                out.push('\n');
                let t = inner.trim();
                if t.chars().all(|c| c == '`') && t.len() >= open {
                    break;
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    (out, blocks)
}

/// The whole of one fence: ask the host, then make what it returns safe to
/// inline, unique on the page, and dressed in the deck's tokens.
fn render_one(
    source: &str,
    renderer: Option<&dyn DiagramRenderer>,
    id: &str,
) -> Result<String, String> {
    let Some(renderer) = renderer else {
        return Err(NO_RENDERER.to_string());
    };
    let svg = renderer.render(source)?;
    prepare(&svg, id)
}

/// What the build says when nothing can draw a diagram. One sentence, and then
/// the two ways to fix it — the `build.mermaid` record an agent reads carries
/// this same text.
pub const NO_RENDERER: &str = "no diagram renderer found, so the block is shown as code; \
     install mermaid-cli (`npm install -g @mermaid-js/mermaid-cli`) or point \
     MIRZAM_MMDC at an `mmdc` binary";

/// Replaces mermaid placeholders in `html` with the SVG rendered for them.
pub fn render_in(html: &str, blocks: &[String]) -> String {
    let mut out = html.to_string();
    for (i, svg) in blocks.iter().enumerate() {
        out = out.replace(
            &format!("<!--mz-mermaid:{i}-->"),
            &format!("<div class=\"mz-mermaid\">{svg}</div>"),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Making an external tool's SVG fit to inline
// ---------------------------------------------------------------------------

/// Sanitizes, re-identifies and re-colours one SVG document.
///
/// Everything here exists because the input came from a program this one does
/// not control, and it is about to be pasted into the page rather than loaded
/// into an `<img>`, where a sandbox would have done this work instead.
pub fn prepare(svg: &str, id: &str) -> Result<String, String> {
    let start = svg
        .find("<svg")
        .ok_or_else(|| "the renderer returned no SVG".to_string())?;
    let end = svg
        .rfind("</svg>")
        .map(|i| i + "</svg>".len())
        .ok_or_else(|| "the renderer returned an unterminated SVG".to_string())?;
    if end <= start {
        return Err("the renderer returned an unterminated SVG".to_string());
    }
    // Anything before `<svg` is an XML prolog or a doctype, neither of which
    // may appear in the middle of an HTML document.
    let svg = &svg[start..end];
    let svg = sanitize(svg);
    let svg = reidentify(&svg, id);
    let svg = reroot(&svg, id);
    Ok(rewrite_colors(&svg))
}

/// Removes everything a document from outside can carry that must not run.
///
/// `foreignObject` **stays**: Mermaid puts its labels there, and dropping the
/// element would delete the words off the diagram. Its contents go through the
/// same scrubbing as the rest of the document, which is what makes keeping it
/// safe rather than convenient.
fn sanitize(svg: &str) -> String {
    static SCRIPT: OnceLock<Regex> = OnceLock::new();
    static HANDLER: OnceLock<Regex> = OnceLock::new();
    static JS_URL: OnceLock<Regex> = OnceLock::new();
    let script = SCRIPT
        .get_or_init(|| Regex::new(r"(?is)<script\b.*?(?:</script\s*>|/>)").expect("static regex"));
    let handler = HANDLER.get_or_init(|| {
        Regex::new(r#"(?is)\son[a-z]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)"#).expect("static regex")
    });
    let js_url = JS_URL.get_or_init(|| {
        Regex::new(r#"(?is)\s(?:xlink:)?href\s*=\s*(?:"\s*javascript:[^"]*"|'\s*javascript:[^']*'|\s*javascript:[^\s>]+)"#)
            .expect("static regex")
    });
    let out = script.replace_all(svg, "");
    let out = handler.replace_all(&out, "");
    js_url.replace_all(&out, "").into_owned()
}

/// Gives the document an id of Mirzam's choosing, and rewrites every reference
/// to the old one.
///
/// Mermaid scopes the stylesheet it embeds to the root element's id and names
/// its arrowhead markers after it. Two diagrams in a deck therefore arrive
/// carrying the *same* id — `mmdc` writes `my-svg` unless told otherwise — and
/// the second one's markers and styles would answer to the first one's
/// selectors. Substituting the whole id string is deliberate: the marker ids
/// are the old id with a suffix glued on, so a word-boundary replacement would
/// miss exactly the references that matter.
fn reidentify(svg: &str, id: &str) -> String {
    static ROOT_ID: OnceLock<Regex> = OnceLock::new();
    let re = ROOT_ID
        .get_or_init(|| Regex::new(r#"(?is)^<svg\b[^>]*?\sid\s*=\s*"([^"]*)""#).expect("regex"));
    match re.captures(svg).map(|c| c[1].to_string()) {
        Some(old) if !old.is_empty() && old != id => svg.replace(&old, id),
        _ => svg.to_string(),
    }
}

/// Rewrites the root element so the diagram scales to the box it lands in.
///
/// `mmdc` writes a pixel `width`/`height` and a `style="max-width:…px"`, which
/// together pin the diagram at whatever size the renderer happened to choose.
/// Dropping all three and keeping the `viewBox` is what makes it behave like a
/// chart: the CSS decides the box, the `viewBox` decides the aspect.
fn reroot(svg: &str, id: &str) -> String {
    static ROOT: OnceLock<Regex> = OnceLock::new();
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let root = ROOT.get_or_init(|| Regex::new(r"(?is)^<svg\b([^>]*)>").expect("static regex"));
    let attr = ATTR.get_or_init(|| {
        Regex::new(r#"(?is)\s(width|height|style|id|class|preserveAspectRatio)\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)"#)
            .expect("static regex")
    });
    let Some(caps) = root.captures(svg) else {
        return svg.to_string();
    };
    let raw = caps[1].to_string();
    let width = attr_value(&raw, "width");
    let height = attr_value(&raw, "height");
    let mut kept = attr.replace_all(&raw, "").trim_end().to_string();
    // Only when the renderer gave no `viewBox`: without one there is no aspect
    // to preserve, and the pixel size is the only record of it there is.
    if !raw.to_ascii_lowercase().contains("viewbox") {
        if let (Some(w), Some(h)) = (
            width.as_deref().and_then(px),
            height.as_deref().and_then(px),
        ) {
            kept.push_str(&format!(" viewBox=\"0 0 {w} {h}\""));
        }
    }
    let opened = format!(
        "<svg{kept} id=\"{id}\" class=\"mz-mermaid-svg\" preserveAspectRatio=\"xMidYMid meet\">"
    );
    format!("{opened}{}", &svg[caps.get(0).expect("match").end()..])
}

/// One attribute's value out of a raw attribute string.
fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"(?is)\s{name}\s*=\s*"([^"]*)""#)).ok()?;
    re.captures(attrs).map(|c| c[1].to_string())
}

/// `"640"` or `"640px"` as a number, and nothing else — a percentage width
/// says nothing about the aspect, so it must not become a `viewBox`.
fn px(v: &str) -> Option<f64> {
    let v = v.trim().trim_end_matches("px").trim();
    v.parse::<f64>().ok().filter(|n| *n > 0.0)
}

// ---------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------

/// Where a colour was written, which decides what it is *for*.
///
/// A grey is a panel when it fills and a rule when it strokes, and the token
/// the deck has for each of those is a different one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Fill,
    Line,
}

/// The chromatic half of Mermaid's default theme, by the role each colour
/// plays in it rather than by where it happens to appear.
///
/// Only these are rewritten. A colour that is chromatic and *not* here is one
/// the author chose — a `classDef fill:#f9f`, or one of Mermaid's other
/// themes — and rewriting it would overrule a decision somebody made on
/// purpose. The greys, whites and blacks are not listed: they are recognised
/// by [`achromatic_token`], which is both shorter and right about shades this
/// table has never seen.
const MERMAID_PALETTE: &[(&str, &str)] = &[
    ("#ececff", "shape-fill"), // mainBkg: node and actor fill
    ("#9370db", "accent1"),    // border1: node border
    ("#ccccff", "border"),     // actorBorder
    ("#ffffde", "surface"),    // secondBkg: subgraph cluster
    ("#aaaa33", "accent2"),    // border2: cluster and note border
    ("#fff5ad", "surface"),    // noteBkgColor
    ("#ffcccc", "danger-bg"),  // errorBkgColor
    ("#ff0000", "danger-fg"),  // errorTextColor
];

/// Rewrites every colour the module recognises to a `var(--mz-*)` reference.
///
/// The original stays as the `var()` fallback, so a theme missing a token
/// renders the diagram Mermaid's way rather than not at all — the same shape
/// `base.css` uses when one token stands in for another.
fn rewrite_colors(svg: &str) -> String {
    static PRESENTATION: OnceLock<Regex> = OnceLock::new();
    static STYLE_ATTR: OnceLock<Regex> = OnceLock::new();
    static STYLE_BLOCK: OnceLock<Regex> = OnceLock::new();
    // Presentation attributes: `fill="#333"` on the element itself.
    let presentation = PRESENTATION.get_or_init(|| {
        Regex::new(
            r#"(?i)(^|[^\w-])(stop-color|flood-color|lighting-color|fill|stroke|color)(\s*=\s*")([^"]*)(")"#,
        )
        .expect("static regex")
    });
    // The two places CSS can live inside an SVG. Both are rewritten by the same
    // declaration pass, and neither is confused with the diagram's own text:
    // a node labelled `color: red` stays a node labelled `color: red`.
    let style_attr = STYLE_ATTR
        .get_or_init(|| Regex::new(r#"(?is)\sstyle\s*=\s*"([^"]*)""#).expect("static regex"));
    let style_block = STYLE_BLOCK
        .get_or_init(|| Regex::new(r"(?is)(<style\b[^>]*>)(.*?)(</style\s*>)").expect("regex"));

    let out = presentation.replace_all(svg, |c: &regex::Captures| {
        let prop = c[2].to_ascii_lowercase();
        match map_color(&c[4], role_of(&prop)) {
            Some(v) => format!("{}{}{}{v}{}", &c[1], &c[2], &c[3], &c[5]),
            None => c[0].to_string(),
        }
    });
    let out = style_attr.replace_all(&out, |c: &regex::Captures| {
        format!(" style=\"{}\"", rewrite_declarations(&c[1]))
    });
    style_block
        .replace_all(&out, |c: &regex::Captures| {
            format!("{}{}{}", &c[1], rewrite_declarations(&c[2]), &c[3])
        })
        .into_owned()
}

/// `fill: #333; stroke-width: 2` — the colour declarations only.
fn rewrite_declarations(css: &str) -> String {
    static DECL: OnceLock<Regex> = OnceLock::new();
    let decl = DECL.get_or_init(|| {
        Regex::new(
            r#"(?i)(^|[^\w-])(background-color|border-color|outline-color|text-decoration-color|caret-color|stop-color|flood-color|lighting-color|background|stroke|fill|color)(\s*:\s*)([^;}"'\n]+)"#,
        )
        .expect("static regex")
    });
    decl.replace_all(css, |c: &regex::Captures| {
        let prop = c[2].to_ascii_lowercase();
        match map_color(&c[4], role_of(&prop)) {
            Some(v) => format!("{}{}{}{v}", &c[1], &c[2], &c[3]),
            None => c[0].to_string(),
        }
    })
    .into_owned()
}

fn role_of(prop: &str) -> Role {
    match prop {
        "fill" | "background" | "background-color" | "stop-color" | "flood-color"
        | "lighting-color" => Role::Fill,
        _ => Role::Line,
    }
}

/// `var(--mz-token, <what mermaid wrote>)`, or `None` to leave it alone.
fn map_color(value: &str, role: Role) -> Option<String> {
    let raw = value.trim();
    if raw.is_empty() || raw.len() > 64 {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    // `none`, `transparent`, `currentColor` and `url(#gradient)` are not
    // colours to be swapped; they are decisions about whether to paint at all.
    if matches!(
        lower.as_str(),
        "none" | "transparent" | "inherit" | "currentcolor"
    ) || lower.starts_with("url(")
        || lower.starts_with("var(")
    {
        return None;
    }
    let rgb = parse_color(&lower)?;
    let token = MERMAID_PALETTE
        .iter()
        .find(|(hex, _)| *hex == hex_of(rgb))
        .map(|(_, t)| *t)
        .or_else(|| achromatic_token(rgb, role))?;
    Some(format!("var(--mz-{token}, {raw})"))
}

/// The token for a grey, a white or a black, by how light it is and what it is
/// being used for. A colour with any real hue in it is left to the palette
/// table above, which is what keeps an author's own `classDef` colour theirs.
fn achromatic_token(rgb: [u8; 3], role: Role) -> Option<&'static str> {
    let max = rgb.iter().copied().max()? as f64;
    let min = rgb.iter().copied().min()? as f64;
    // Chroma as a fraction of full scale: 8% of 255 is about two steps of a
    // hex digit, which is as much drift as an anti-aliased grey ever carries.
    if (max - min) / 255.0 > 0.08 {
        return None;
    }
    let lightness = (max + min) / 2.0 / 255.0;
    Some(match (role, lightness) {
        (_, l) if l >= 0.95 => "slide-bg",
        (Role::Fill, l) if l >= 0.75 => "surface",
        (Role::Line, l) if l >= 0.75 => "border",
        (_, l) if l >= 0.35 => "muted",
        _ => "fg",
    })
}

fn hex_of(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// The colour notations an SVG from outside actually contains: hex in three
/// lengths, `rgb()`/`rgba()`, `hsl()`/`hsla()`, and the handful of names a
/// generator reaches for. Anything else is left alone rather than guessed at.
fn parse_color(v: &str) -> Option<[u8; 3]> {
    if let Some(hex) = v.strip_prefix('#') {
        let d: Vec<u8> = hex
            .chars()
            .filter_map(|c| c.to_digit(16).map(|d| d as u8))
            .collect();
        if d.len() != hex.len() {
            return None;
        }
        return match d.len() {
            3 | 4 => Some([d[0] * 17, d[1] * 17, d[2] * 17]),
            6 | 8 => Some([d[0] * 16 + d[1], d[2] * 16 + d[3], d[4] * 16 + d[5]]),
            _ => None,
        };
    }
    if let Some(args) = fn_args(v, "rgb").or_else(|| fn_args(v, "rgba")) {
        let n: Vec<f64> = args.iter().take(3).filter_map(|a| channel(a)).collect();
        if n.len() == 3 {
            return Some([n[0] as u8, n[1] as u8, n[2] as u8]);
        }
        return None;
    }
    if let Some(args) = fn_args(v, "hsl").or_else(|| fn_args(v, "hsla")) {
        if args.len() >= 3 {
            let h = args[0].trim().trim_end_matches("deg").parse::<f64>().ok()?;
            let s = args[1].trim().trim_end_matches('%').parse::<f64>().ok()? / 100.0;
            let l = args[2].trim().trim_end_matches('%').parse::<f64>().ok()? / 100.0;
            return Some(hsl_to_rgb(h, s, l));
        }
        return None;
    }
    named_color(v)
}

/// The arguments of `name(...)`, split on commas and spaces, or `None` when
/// `v` is not a call to that function. A `/` alpha separator is dropped with
/// the alpha, which is all this module ever wanted from it.
fn fn_args(v: &str, name: &str) -> Option<Vec<String>> {
    let rest = v.strip_prefix(name)?.trim_start();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    Some(
        inner
            .split(['/', ',', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// One `rgb()` channel: `0-255`, or a percentage of it.
fn channel(a: &str) -> Option<f64> {
    let a = a.trim();
    match a.strip_suffix('%') {
        Some(p) => p.parse::<f64>().ok().map(|p| (p * 2.55).clamp(0.0, 255.0)),
        None => a.parse::<f64>().ok().map(|n| n.clamp(0.0, 255.0)),
    }
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

/// The named colours a diagram generator actually writes. Every one of them is
/// achromatic on purpose: a named hue would be left alone anyway, so listing it
/// would only be a longer table saying the same thing.
fn named_color(v: &str) -> Option<[u8; 3]> {
    Some(match v {
        "white" => [255, 255, 255],
        "snow" | "ivory" | "whitesmoke" => [245, 245, 245],
        "gainsboro" => [220, 220, 220],
        "lightgray" | "lightgrey" => [211, 211, 211],
        "silver" => [192, 192, 192],
        "darkgray" | "darkgrey" => [169, 169, 169],
        "gray" | "grey" => [128, 128, 128],
        "dimgray" | "dimgrey" => [105, 105, 105],
        "black" => [0, 0, 0],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A renderer that answers with a canned Mermaid-shaped SVG, so every step
    /// after "the host answered" is testable on a machine with no `mmdc` on it.
    struct Fake(&'static str);

    impl DiagramRenderer for Fake {
        fn render(&self, _: &str) -> Result<String, String> {
            Ok(self.0.to_string())
        }
    }

    struct Broken;

    impl DiagramRenderer for Broken {
        fn render(&self, _: &str) -> Result<String, String> {
            Err("mmdc exited 1: Parse error on line 2".to_string())
        }
    }

    /// Cut down from real `mmdc` output: the pixel size and inline `max-width`
    /// it pins, the id it scopes its stylesheet to, the marker named after that
    /// id, and one node in the default palette.
    const CANNED: &str = concat!(
        r##"<?xml version="1.0" encoding="UTF-8"?>"##,
        r##"<svg id="my-svg" width="640" height="220" viewBox="0 0 640 220" "##,
        r##"style="max-width: 640px;" xmlns="http://www.w3.org/2000/svg">"##,
        r##"<style>#my-svg .node rect{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}"##,
        r##"#my-svg .edgeLabel{background-color:#e8e8e8;color:#333;}</style>"##,
        r##"<marker id="my-svg_flowchart-pointEnd"><path fill="#333333"/></marker>"##,
        r##"<g class="node"><rect style="fill:#ECECFF"/>"##,
        r##"<text fill="#333" stroke="none">a</text></g>"##,
        r##"<path marker-end="url(#my-svg_flowchart-pointEnd)" stroke="#333333"/>"##,
        r##"</svg>"##,
    );

    fn extract_with(
        md: &str,
        r: Option<&dyn DiagramRenderer>,
    ) -> (String, Vec<String>, Vec<String>) {
        let mut warnings = Vec::new();
        let (out, blocks) = extract(md, 2, r, &mut warnings);
        (out, blocks, warnings)
    }

    #[test]
    fn a_fence_becomes_a_placeholder_the_svg_lands_in() {
        let fake = Fake(CANNED);
        let (md, blocks, warnings) = extract_with(
            "before\n\n```mermaid\ngraph TD;A-->B;\n```\n\nafter\n",
            Some(&fake),
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(blocks.len(), 1);
        assert!(md.contains("<!--mz-mermaid:0-->"), "{md}");
        assert!(!md.contains("```mermaid"), "{md}");
        assert!(md.contains("before") && md.contains("after"));
        let html = render_in("<p><!--mz-mermaid:0--></p>", &blocks);
        assert!(html.contains("<div class=\"mz-mermaid\">"), "{html}");
        assert!(html.contains("class=\"mz-mermaid-svg\""), "{html}");
    }

    /// The headline rule: no renderer is a warning *and* a code block, never
    /// one without the other.
    #[test]
    fn without_a_renderer_the_fence_stays_a_code_block_and_the_build_says_so() {
        let (md, blocks, warnings) = extract_with("```mermaid\ngraph TD;A-->B;\n```\n", None);
        assert!(blocks.is_empty());
        assert!(md.contains("```mermaid\ngraph TD;A-->B;\n```"), "{md}");
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].starts_with("slide 3: mermaid: "),
            "{warnings:?}"
        );
        assert!(warnings[0].contains("shown as code"), "{warnings:?}");
        assert!(warnings[0].contains("MIRZAM_MMDC"), "{warnings:?}");
    }

    #[test]
    fn a_renderer_that_fails_degrades_the_same_way_and_quotes_it() {
        let (md, blocks, warnings) = extract_with("```mermaid\nnonsense\n```\n", Some(&Broken));
        assert!(blocks.is_empty());
        assert!(md.contains("```mermaid\nnonsense\n```"), "{md}");
        assert_eq!(
            warnings,
            vec!["slide 3: mermaid: mmdc exited 1: Parse error on line 2"]
        );
    }

    #[test]
    fn a_longer_fence_quotes_mermaid_syntax_rather_than_drawing_it() {
        let fake = Fake(CANNED);
        let (md, blocks, _) = extract_with(
            "````markdown\n```mermaid\ngraph TD;A-->B;\n```\n````\n",
            Some(&fake),
        );
        assert!(blocks.is_empty(), "a quoted fence must not be rendered");
        assert!(md.contains("```mermaid"), "{md}");
    }

    #[test]
    fn the_prolog_goes_and_the_root_stops_pinning_its_own_size() {
        let out = prepare(CANNED, "mz-mermaid-1-1").expect("an SVG");
        assert!(out.starts_with("<svg"), "{out}");
        assert!(!out.contains("<?xml"), "{out}");
        assert!(!out.contains("width=\"640\""), "{out}");
        assert!(!out.contains("max-width"), "{out}");
        assert!(out.contains("viewBox=\"0 0 640 220\""), "{out}");
        assert!(
            out.contains("preserveAspectRatio=\"xMidYMid meet\""),
            "{out}"
        );
        assert!(
            out.contains("xmlns=\"http://www.w3.org/2000/svg\""),
            "{out}"
        );
    }

    /// Without this, two diagrams on one deck answer to each other's styles:
    /// `mmdc` writes the same root id every time it runs.
    #[test]
    fn the_root_id_and_everything_named_after_it_become_this_diagrams_own() {
        let out = prepare(CANNED, "mz-mermaid-4-2").expect("an SVG");
        assert!(!out.contains("my-svg"), "{out}");
        assert!(out.contains("id=\"mz-mermaid-4-2\""), "{out}");
        assert!(out.contains("#mz-mermaid-4-2 .node rect"), "{out}");
        assert!(
            out.contains("id=\"mz-mermaid-4-2_flowchart-pointEnd\""),
            "{out}"
        );
        assert!(
            out.contains("url(#mz-mermaid-4-2_flowchart-pointEnd)"),
            "the arrowhead reference has to move with the marker: {out}"
        );
    }

    #[test]
    fn no_viewbox_is_rebuilt_from_the_pixel_size_it_did_give() {
        let out = prepare(r#"<svg width="300" height="150"><g/></svg>"#, "d").expect("an SVG");
        assert!(out.contains("viewBox=\"0 0 300 150\""), "{out}");
    }

    #[test]
    fn a_percentage_width_is_no_aspect_ratio_and_invents_no_viewbox() {
        let out = prepare(r#"<svg width="100%" height="100%"><g/></svg>"#, "d").expect("an SVG");
        assert!(!out.contains("viewBox"), "{out}");
    }

    #[test]
    fn what_is_not_an_svg_is_reported_rather_than_pasted_into_the_page() {
        assert!(prepare("mmdc: command not found\n", "d").is_err());
        assert!(prepare("<svg><g/>", "d").is_err());
    }

    // -- sanitizing ---------------------------------------------------------

    #[test]
    fn scripts_handlers_and_javascript_urls_do_not_survive() {
        let hostile = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">"#,
            r#"<script>fetch('//evil')</script>"#,
            r#"<script type="module" src="x.js"/>"#,
            r##"<rect onclick="steal()" onmouseover='steal()' fill="#333"/>"##,
            r#"<a xlink:href="javascript:steal()"><text>click</text></a>"#,
            r#"<a href = 'javascript:steal()'><text>and here</text></a>"#,
            r#"</svg>"#,
        );
        let out = prepare(hostile, "d").expect("an SVG");
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("evil"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
        assert!(!out.contains("onmouseover"), "{out}");
        assert!(!out.to_lowercase().contains("javascript:"), "{out}");
        // The words on the diagram are not what was dangerous about it.
        assert!(out.contains("click") && out.contains("and here"), "{out}");
    }

    /// Mermaid's labels live in a `foreignObject`, so removing the element
    /// would remove the diagram's words. It stays, scrubbed.
    #[test]
    fn foreign_object_labels_survive_with_their_handlers_stripped() {
        let svg = concat!(
            r#"<svg viewBox="0 0 10 10"><foreignObject width="80" height="20">"#,
            r#"<div xmlns="http://www.w3.org/1999/xhtml" onload="steal()">Ingest</div>"#,
            r#"</foreignObject></svg>"#,
        );
        let out = prepare(svg, "d").expect("an SVG");
        assert!(out.contains("<foreignObject"), "{out}");
        assert!(out.contains("Ingest"), "{out}");
        assert!(!out.contains("onload"), "{out}");
    }

    // -- colours ------------------------------------------------------------

    #[test]
    fn mermaids_palette_becomes_the_decks_tokens_wherever_it_was_written() {
        let out = prepare(CANNED, "d").expect("an SVG");
        // In the embedded stylesheet.
        assert!(out.contains("fill:var(--mz-shape-fill, #ECECFF)"), "{out}");
        assert!(out.contains("stroke:var(--mz-accent1, #9370DB)"), "{out}");
        // In a `style=` attribute.
        assert!(
            out.contains(r#"style="fill:var(--mz-shape-fill, #ECECFF)""#),
            "{out}"
        );
        // And as a presentation attribute.
        assert!(out.contains(r#"fill="var(--mz-fg, #333333)""#), "{out}");
        assert!(out.contains(r#"stroke="var(--mz-fg, #333333)""#), "{out}");
        assert!(out.contains(r#"fill="var(--mz-fg, #333)""#), "{out}");
    }

    #[test]
    fn the_original_stays_as_the_fallback_so_a_missing_token_is_not_a_blank_diagram() {
        assert_eq!(
            map_color("#9370DB", Role::Line).as_deref(),
            Some("var(--mz-accent1, #9370DB)")
        );
    }

    /// A grey is a panel when it fills and a rule when it strokes.
    #[test]
    fn greys_take_the_token_for_what_they_are_being_used_as() {
        for (v, fill, line) in [
            ("#ffffff", "slide-bg", "slide-bg"),
            ("white", "slide-bg", "slide-bg"),
            ("#e8e8e8", "surface", "border"),
            ("#cccccc", "surface", "border"),
            ("#888", "muted", "muted"),
            ("grey", "muted", "muted"),
            ("#333", "fg", "fg"),
            ("black", "fg", "fg"),
        ] {
            assert!(
                map_color(v, Role::Fill)
                    .unwrap()
                    .contains(&format!("--mz-{fill}")),
                "{v} as a fill"
            );
            assert!(
                map_color(v, Role::Line)
                    .unwrap()
                    .contains(&format!("--mz-{line}")),
                "{v} as a line"
            );
        }
    }

    /// The other half of the promise: a colour the author chose is theirs.
    #[test]
    fn a_colour_nobody_recognises_is_left_exactly_as_written() {
        for v in ["#f9f", "#1e88e5", "rgb(30, 136, 229)", "hsl(207, 90%, 51%)"] {
            assert_eq!(map_color(v, Role::Fill), None, "{v}");
        }
        let svg = r##"<svg viewBox="0 0 10 10"><rect fill="#f9f" stroke="#1e88e5"/></svg>"##;
        let out = prepare(svg, "d").expect("an SVG");
        assert!(out.contains(r##"fill="#f9f""##), "{out}");
        assert!(out.contains(r##"stroke="#1e88e5""##), "{out}");
    }

    #[test]
    fn a_decision_not_to_paint_is_not_a_colour() {
        for v in [
            "none",
            "transparent",
            "currentColor",
            "url(#grad1)",
            "inherit",
        ] {
            assert_eq!(map_color(v, Role::Fill), None, "{v}");
        }
    }

    /// `stroke-width: 2` is not a colour declaration, and a `2` swapped for a
    /// token is a diagram with no lines in it.
    #[test]
    fn properties_that_only_start_like_a_colour_are_untouched() {
        let css = "stroke-width:2px;fill-opacity:0.5;stroke-dasharray:3 3;background-image:none";
        assert_eq!(rewrite_declarations(css), css);
    }

    /// The diagram's own words are not a stylesheet.
    #[test]
    fn text_that_reads_like_css_is_not_rewritten() {
        let svg = r#"<svg viewBox="0 0 10 10"><text>set color: #333333 here</text></svg>"#;
        let out = prepare(svg, "d").expect("an SVG");
        assert!(out.contains("set color: #333333 here"), "{out}");
    }

    #[test]
    fn every_token_it_can_emit_is_one_the_stylesheet_defines() {
        let css = crate::theme::theme_css_for(&["mirzam"]);
        for token in MERMAID_PALETTE
            .iter()
            .map(|(_, t)| *t)
            .chain(["slide-bg", "surface", "border", "muted", "fg"])
        {
            let name = format!("--mz-{token}:");
            assert!(css.contains(&name), "nothing defines `{name}`");
        }
    }

    #[test]
    fn colour_notations_all_arrive_at_the_same_grey() {
        for v in [
            "#808080",
            "rgb(128,128,128)",
            "rgba(128, 128, 128, 0.5)",
            "hsl(0, 0%, 50%)",
        ] {
            assert_eq!(parse_color(v), Some([128, 128, 128]), "{v}");
        }
        assert_eq!(parse_color("#fff"), Some([255, 255, 255]));
        assert_eq!(parse_color("#33333380"), Some([51, 51, 51]));
        assert_eq!(parse_color("#zzz"), None);
        assert_eq!(parse_color("chartreuse"), None);
    }
}
