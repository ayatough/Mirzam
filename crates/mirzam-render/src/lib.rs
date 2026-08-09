//! Turns a list of `SlideSource` values plus `DeckMeta` into a single HTML
//! file with the viewer embedded.

mod anim;
mod annot;
mod assets;
mod charts;
mod effects;
mod inline;
mod theme;

pub use assets::{AssetSource, FsAssets};
pub use charts::render_charts_in;
pub use inline::{parse_attrs, preprocess, render_markdown};
pub use theme::{mode_warning, theme_warning, THEME_NAMES};

use mirzam_core::DeckMeta;
use mirzam_layout::{parse_grid, GridSpec};
use mirzam_syntax::SlideSource;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct RenderResult {
    pub html: String,
    pub warnings: Vec<String>,
}

/// One rendered slide: a `<section>` element with assets already inlined.
pub struct RenderedSlide {
    pub html: String,
    pub warnings: Vec<String>,
    /// Local assets this slide referenced, for cache validation and watching.
    pub assets: Vec<std::path::PathBuf>,
}

/// Renders one slide to `<section>` HTML; this is the unit `serve` updates.
/// Assets are resolved from the filesystem.
pub fn render_slide_html(slide: &SlideSource, index: usize, asset_dir: &Path) -> RenderedSlide {
    render_slide_html_with(slide, index, &assets::FsAssets(asset_dir))
}

/// Variant with pluggable asset resolution; WASM hosts inject their own table.
pub fn render_slide_html_with(
    slide: &SlideSource,
    index: usize,
    asset_source: &dyn AssetSource,
) -> RenderedSlide {
    let mut warnings = Vec::new();
    let mut assets_used = Vec::new();
    // Charts are rendered first: they may pull in CSV data through the same
    // asset source, and their SVG output must not be scanned for asset URLs.
    let html = render_slide(slide, index, &mut warnings, asset_source, &mut assets_used);
    let html = assets::embed_assets(&html, asset_source, &mut warnings, &mut assets_used);
    RenderedSlide {
        html,
        warnings,
        assets: assets_used,
    }
}

/// Options for assembling the page.
#[derive(Default)]
pub struct PageOptions {
    /// When set, injects the hot-reload client used by `serve`.
    pub live_version: Option<u64>,
    /// Contents of the stylesheet named by frontmatter `css:`.
    pub custom_css: Option<String>,
    /// Bakes the layout debug overlay on at load, instead of leaving it to the
    /// viewer's `L` key. For screenshotting a broken deck headlessly.
    pub debug_layout: bool,
}

/// Whether any section contains math, deciding if the math font is bundled.
pub fn sections_have_math(sections: &[String]) -> bool {
    sections.iter().any(|s| s.contains("<math"))
}

/// Whether the deck animates anything, deciding if the animation runtime is
/// inlined. A deck-wide `transition:` counts: it is animation the author asked
/// for without writing an `anim` block.
fn deck_has_anim(meta: &DeckMeta, sections: &[String]) -> bool {
    meta.transition.is_some() || sections.iter().any(|s| s.contains("class=\"mz-anim\""))
}

/// Whether the deck annotates anything, deciding if the annotation overlay is
/// inlined — into the print page as well as the viewer.
fn deck_has_annot(sections: &[String]) -> bool {
    sections.iter().any(|s| s.contains("class=\"mz-annot\""))
}

/// The `data-transition` payload for `#deck`, or an empty string when the deck
/// declares no transition or an unusable one. Reporting the problem is the
/// build pipeline's job; rendering must not fail over it.
fn transition_attr(meta: &DeckMeta) -> String {
    match meta
        .transition
        .as_deref()
        .map(mirzam_anim::parse_transition)
    {
        Some(Ok(t)) => format!(
            " data-transition=\"{}\"",
            inline::html_escape(&mirzam_anim::transition_json(&t))
        ),
        _ => String::new(),
    }
}

/// Resolves frontmatter `theme:`/`mode:` to the attributes baked onto
/// `<html>`. Always valid, silently falling back to `default`/no mode
/// attribute for a name that is not a built-in: a caller that wants to
/// report an unknown name calls [`theme_warning`]/[`mode_warning`] where
/// `meta` was parsed, since this function has no warning channel of its own.
fn theme_attrs(meta: &DeckMeta) -> (&'static str, String) {
    let name = theme::THEME_NAMES
        .iter()
        .find(|n| Some(**n) == meta.theme.as_deref())
        .copied()
        .unwrap_or("default");
    let mode_attr = match theme::normalize_mode(meta.mode.as_deref()) {
        Some(m) => format!(" data-mode=\"{m}\""),
        None => String::new(),
    };
    (name, mode_attr)
}

/// Assembles rendered sections into a complete HTML page with the viewer.
pub fn assemble_page(meta: &DeckMeta, sections: &[String], opts: &PageOptions) -> String {
    let (w, h) = meta.slide_size();
    let title = meta.title.as_deref().unwrap_or("Mirzam Deck");
    let math_css = if sections_have_math(sections) {
        theme::math_font_css()
    } else {
        ""
    };
    let live_js = match opts.live_version {
        Some(v) => format!(
            "<script>window.__MIRZAM_V__={v};{}</script>",
            theme::LIVE_JS
        ),
        None => String::new(),
    };
    let html_class = if opts.debug_layout {
        " class=\"mz-debug\""
    } else {
        ""
    };
    let anim_js = if deck_has_anim(meta, sections) {
        format!("<script>{}</script>\n", theme::ANIM_JS)
    } else {
        String::new()
    };
    let annot_js = if deck_has_annot(sections) {
        format!("<script>{}</script>\n", theme::ANNOT_JS)
    } else {
        String::new()
    };
    let effects_js = if effects::deck_has_effects(sections) {
        format!("<script>{}</script>\n", theme::EFFECTS_JS)
    } else {
        String::new()
    };
    let transition = transition_attr(meta);
    let (theme_name, mode_attr) = theme_attrs(meta);
    format!(
        r#"<!doctype html>
<html lang="en" data-theme="{theme_name}"{mode_attr}{html_class}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="mirzam 0.0.1">
<title>{title}</title>
<style>{css}</style>
<style>{math_css}</style>
<style>{custom_css}</style>
</head>
<body>
<div id="deck" data-slide-w="{w}" data-slide-h="{h}"{transition}>
{sections}</div>
<div id="hud"></div>
<div id="hint">← → navigate / N notes / F fullscreen / L layout / D mode</div>
<div id="notes-panel" hidden></div>
{anim_js}{annot_js}<script>{js}</script>
{effects_js}{live_js}</body>
</html>
"#,
        title = inline::html_escape(title),
        css = theme::theme_css(theme_name),
        custom_css = opts.custom_css.as_deref().unwrap_or(""),
        js = theme::VIEWER_JS,
        sections = sections.concat(),
    )
}

/// Rewrites document-relative hyperlinks so they still resolve when the deck
/// is published somewhere other than beside its source.
///
/// Images and video are inlined by [`assets::embed_assets`], so what is left
/// relative is links to other documents — and `docs/layout.md` next to
/// `README.md` is not `docs/layout.md` next to `decks/readme/index.html`.
/// `base` is the URL the *source file's directory* maps to.
///
/// Absolute URLs, protocol-relative URLs, root-relative paths and bare
/// fragments are left exactly as they are.
pub fn rewrite_relative_links(html: &str, base: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"href="([^"]*)""#).expect("static regex"));
    re.replace_all(html, |c: &regex::Captures| match resolve_url(base, &c[1]) {
        Some(url) => format!("href=\"{}\"", inline::html_escape(&url)),
        None => c[0].to_string(),
    })
    .into_owned()
}

/// `None` when `rel` is not a document-relative reference and must be left
/// alone.
fn resolve_url(base: &str, rel: &str) -> Option<String> {
    if rel.is_empty() || rel.starts_with('#') || rel.starts_with('/') {
        return None;
    }
    // A scheme (`https:`, `mailto:`, ...) before any slash means it is absolute.
    if let Some(colon) = rel.find(':') {
        if !rel[..colon].contains('/') {
            return None;
        }
    }

    // Keep the query and fragment out of path resolution, then put them back.
    let (path, suffix) = match rel.find(['#', '?']) {
        Some(i) => (&rel[..i], &rel[i..]),
        None => (rel, ""),
    };

    let (origin, base_path) = split_origin(base);
    let mut segs: Vec<&str> = base_path.split('/').filter(|s| !s.is_empty()).collect();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            other => segs.push(other),
        }
    }
    Some(format!("{origin}/{}{suffix}", segs.join("/")))
}

/// Splits `https://host/a/b/` into `("https://host", "/a/b/")`.
fn split_origin(base: &str) -> (&str, &str) {
    match base.find("://") {
        Some(i) => match base[i + 3..].find('/') {
            Some(j) => (&base[..i + 3 + j], &base[i + 3 + j..]),
            None => (base, "/"),
        },
        None => ("", base),
    }
}

/// Replaces `<video>` with a still image for print.
/// Uses the poster when one is given, otherwise a placeholder with a play
/// icon, since PDF output is static.
fn videos_to_stills(html: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"<video\b([^>]*)></video>"#).expect("static regex"));
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR.get_or_init(|| Regex::new(r#"(\w[\w-]*)="([^"]*)""#).expect("static regex"));
    re.replace_all(html, |c: &regex::Captures| {
        let mut attrs: std::collections::BTreeMap<&str, &str> = Default::default();
        for a in attr_re.captures_iter(&c[1]) {
            let (k, v) = (a.get(1).unwrap().as_str(), a.get(2).unwrap().as_str());
            attrs.insert(k, v);
        }
        let style = attrs.get("style").copied().unwrap_or("");
        let title = attrs.get("title").copied().unwrap_or("");
        match attrs.get("poster") {
            Some(poster) => format!("<img src=\"{poster}\" alt=\"{title}\" style=\"{style}\">"),
            None => format!(
                "<div class=\"mz-video-still\" style=\"{style}\"><span>▶</span><em>{title}</em></div>"
            ),
        }
    })
    .into_owned()
}

/// Print page for PDF export: fixed-size slides stacked one per page.
pub fn assemble_print_page(
    meta: &DeckMeta,
    sections: &[String],
    custom_css: Option<&str>,
) -> String {
    let (w, h) = meta.slide_size();
    let title = meta.title.as_deref().unwrap_or("Mirzam Deck");
    let math_css = if sections_have_math(sections) {
        theme::math_font_css()
    } else {
        ""
    };
    let sections: Vec<String> = sections.iter().map(|s| videos_to_stills(s)).collect();
    let sections = &sections;
    // The one script the print page carries. An annotation is drawn *over*
    // the deck and hides nothing, so running it cannot break the guarantee
    // that a scriptless read shows every slide in full — and without it the
    // PDF would lose the marks the annotated slide exists to make.
    let annot_js = if deck_has_annot(sections) {
        format!("<script>{}</script>\n", theme::ANNOT_JS)
    } else {
        String::new()
    };
    let (theme_name, mode_attr) = theme_attrs(meta);
    format!(
        r#"<!doctype html>
<html lang="en" data-theme="{theme_name}"{mode_attr}>
<head>
<meta charset="utf-8">
<meta name="generator" content="mirzam 0.0.1">
<title>{title}</title>
<style>{css}</style>
<style>{math_css}</style>
<style>{print_css}
@page {{ size: {w}px {h}px; margin: 0; }}
section.slide {{ width: {w}px; height: {h}px; }}
</style>
<style>{custom_css}</style>
</head>
<body>
<div id="deck">
{sections}</div>
{annot_js}</body>
</html>
"#,
        title = inline::html_escape(title),
        css = theme::theme_css(theme_name),
        print_css = theme::PRINT_CSS,
        custom_css = custom_css.unwrap_or(""),
        sections = sections.concat(),
    )
}

/// Renders a whole deck to a single HTML file.
/// `asset_dir` is the base directory for relative asset paths.
pub fn render_deck(meta: &DeckMeta, slides: &[SlideSource], asset_dir: &Path) -> RenderResult {
    let mut warnings = Vec::new();
    warnings.extend(theme_warning(meta.theme.as_deref()));
    warnings.extend(mode_warning(meta.mode.as_deref()));
    let mut sections = Vec::with_capacity(slides.len());
    for (i, slide) in slides.iter().enumerate() {
        let rendered = render_slide_html(slide, i, asset_dir);
        warnings.extend(rendered.warnings);
        sections.push(rendered.html);
    }
    RenderResult {
        html: assemble_page(meta, &sections, &PageOptions::default()),
        warnings,
    }
}

fn render_slide(
    slide: &SlideSource,
    index: usize,
    warnings: &mut Vec<String>,
    asset_source: &dyn AssetSource,
    chart_files: &mut Vec<std::path::PathBuf>,
) -> String {
    let mut errors: Vec<String> = Vec::new();

    // Resolve the layout.
    let grid: Option<GridSpec> = match &slide.layout {
        Some(src) => match parse_grid(src) {
            Ok(g) => Some(g),
            Err(e) => {
                errors.push(format!("slide {}: {e}", index + 1));
                None
            }
        },
        None => None,
    };

    let mut body = match &grid {
        Some(g) => render_grid_slide(g, slide, index, &mut errors, asset_source, chart_files),
        None => render_single_pane_slide(slide, index, &mut errors, asset_source, chart_files),
    };

    // shape blocks become a static SVG layer in page coordinates, scaling with the slide.
    let mut shapes_html = String::new();
    if !slide.shapes.is_empty() {
        let src = slide.shapes.join("\n");
        let doc = mirzam_shape::parse_shapes(&src);
        let (svg, shape_errors) = mirzam_shape::render_svg(&doc, 1280, 720);
        for e in shape_errors {
            errors.push(format!("slide {}: {e}", index + 1));
        }
        shapes_html = svg;
    }

    // connect blocks are embedded as JSON; the runtime resolves endpoints after
    // layout so connectors follow resizes and live updates.
    let mut connect_attr = String::new();
    if !slide.connects.is_empty() {
        let src = slide.connects.join("\n");
        let doc = mirzam_connect::parse_connectors(&src);
        for e in &doc.errors {
            errors.push(format!("slide {}: {e}", index + 1));
        }
        if !doc.connectors.is_empty() {
            connect_attr = format!(
                " data-connectors=\"{}\"",
                inline::html_escape(&mirzam_connect::to_json(&doc))
            );
        }
    }

    // anim blocks compile to the C1 timeline JSON. A line that points at
    // nothing is a warning, not a build failure: the slide renders unanimated.
    let anim_html = anim::extract(index, &slide.reserved, &mut body, &shapes_html, warnings);

    // effects bind a key to a flourish. Nothing about the slide changes; the
    // block only records what the presenter may fire while it is on screen.
    let effects_html = effects::extract(index, &slide.effects, warnings);

    // annotate blocks compile to the C2 model, drawn over the target once the
    // browser has laid the slide out. Same warning rule as anim.
    let annot_html = annot::extract(
        index,
        &slide.annots,
        &format!("{body}{shapes_html}"),
        warnings,
    );

    let error_html: String = errors
        .iter()
        .map(|e| {
            warnings.push(e.clone());
            format!("<div class=\"mz-error\">⚠ {}</div>", inline::html_escape(e))
        })
        .collect();

    let notes_html = if slide.notes.is_empty() {
        String::new()
    } else {
        format!(
            "<aside class=\"notes\">{}</aside>\n",
            slide
                .notes
                .iter()
                .map(|n| render_markdown(n))
                .collect::<String>()
        )
    };

    format!(
        "<section class=\"slide\" data-index=\"{index}\"{connect_attr}>\n{error_html}{body}{shapes_html}{anim_html}{annot_html}{effects_html}{notes_html}</section>\n"
    )
}

fn render_grid_slide(
    grid: &GridSpec,
    slide: &SlideSource,
    index: usize,
    errors: &mut Vec<String>,
    asset_source: &dyn AssetSource,
    chart_files: &mut Vec<std::path::PathBuf>,
) -> String {
    let names = grid.pane_names();
    let mut panes_html = String::new();

    // Unassigned content goes to `main` when it exists, else the first pane.
    let default_pane = if names.iter().any(|n| n == "main") {
        "main".to_string()
    } else {
        names.first().cloned().unwrap_or_else(|| "main".into())
    };

    for name in &names {
        let mut content = String::new();
        let mut attrs_src = "";
        if *name == default_pane && !slide.loose.trim().is_empty() {
            content.push_str(&slide.loose);
            content.push('\n');
        }
        for pb in &slide.panes {
            if pb.name == *name {
                if attrs_src.is_empty() {
                    attrs_src = &pb.attrs;
                }
                content.push_str(&pb.body);
                content.push('\n');
            }
        }
        // Pane attributes: align (text-align), valign (vertical placement), extra classes.
        let attrs = parse_attrs(attrs_src);
        let mut style = format!("grid-area:{name}");
        if let Some(a) = attrs.kv.get("align") {
            if matches!(a.as_str(), "center" | "right" | "left") {
                style.push_str(&format!(";text-align:{a}"));
            }
        }
        match attrs.kv.get("valign").map(String::as_str) {
            Some("middle") => {
                style.push_str(";display:flex;flex-direction:column;justify-content:center")
            }
            Some("bottom") => {
                style.push_str(";display:flex;flex-direction:column;justify-content:flex-end")
            }
            _ => {}
        }
        let extra_cls = attrs
            .classes
            .iter()
            .map(|c| format!(" {c}"))
            .collect::<String>();
        let (content, chart_blocks) = charts::extract(&content);
        let mut body = render_markdown(&preprocess(&content));
        if !chart_blocks.is_empty() {
            let (with_charts, files) =
                charts::render_charts_in(&body, &chart_blocks, index, asset_source, errors);
            body = with_charts;
            chart_files.extend(files);
        }
        let bg = background_layers(&attrs);
        panes_html.push_str(&format!(
            "<div class=\"pane pane-{name}{extra_cls}{}\" data-pane=\"{name}\" style=\"{style}\">{}{body}{}</div>\n",
            bg.pane_class, bg.layers, bg.close
        ));
    }

    // Warn about content assigned to a pane the layout does not define.
    for pb in &slide.panes {
        if !names.contains(&pb.name) {
            errors.push(format!(
                "slide {}: pane `{}` is not in the layout",
                index + 1,
                pb.name
            ));
        }
    }

    // grid-template-areas contains double quotes, so the style attribute uses single quotes.
    format!(
        "<div class=\"grid\" style='grid-template-columns:{cols};grid-template-rows:{rows};grid-template-areas:{areas}'>\n{panes_html}</div>\n",
        cols = grid.css_columns(),
        rows = grid.css_rows(),
        areas = grid.css_areas(),
    )
}

/// The markup a pane needs for a background image: the image itself, an optional
/// scrim, and a wrapper that keeps content above both.
struct Background {
    pane_class: String,
    layers: String,
    close: String,
}

/// Builds the background layers for `bg=` and its treatments.
///
/// The image is a real `<img>` rather than a CSS `background-image` so it goes
/// through the same asset inlining as any other image, keeping a deck a single
/// self-contained file.
fn background_layers(attrs: &inline::Attrs) -> Background {
    let Some(src) = attrs.kv.get("bg") else {
        return Background {
            pane_class: String::new(),
            layers: String::new(),
            close: String::new(),
        };
    };

    let fit = match attrs.kv.get("bg-fit").map(String::as_str) {
        Some("contain") => "contain",
        _ => "cover",
    };
    let pos = attrs
        .kv
        .get("bg-pos")
        .map(|p| sanitize_css(p))
        .unwrap_or_else(|| "center".into());
    let blur = attrs
        .kv
        .get("blur")
        .and_then(|b| b.trim_end_matches("px").parse::<f32>().ok())
        .filter(|b| *b > 0.0);
    // `dim` darkens the whole image; `scrim` darkens one edge so text placed
    // there stays legible while the rest of the photo shows through.
    let dim = attrs
        .kv
        .get("dim")
        .and_then(|d| d.parse::<f32>().ok())
        .map(|d| d.clamp(0.0, 1.0));
    let scrim = attrs.kv.get("scrim").map(String::as_str);

    let mut img_style = format!("object-fit:{fit};object-position:{pos};");
    if let Some(b) = blur {
        // Scale up slightly so the blurred edges do not show the pane behind.
        img_style.push_str(&format!(
            "filter:blur({b}px);transform:scale({:.3});",
            1.0 + b / 60.0
        ));
    }

    let mut overlays = String::new();
    if let Some(d) = dim.filter(|d| *d > 0.0) {
        overlays.push_str(&format!(
            "<div class=\"mz-scrim\" style=\"background:rgba(0,0,0,{d})\"></div>"
        ));
    }
    if let Some(edge) = scrim {
        let strength = dim.unwrap_or(0.75);
        let direction = match edge {
            "top" => "to bottom",
            "left" => "to right",
            "right" => "to left",
            _ => "to top",
        };
        overlays.push_str(&format!(
            "<div class=\"mz-scrim\" style=\"background:linear-gradient({direction}, rgba(0,0,0,{strength}) 0%, rgba(0,0,0,{:.2}) 45%, rgba(0,0,0,0) 100%)\"></div>",
            strength * 0.35
        ));
    }

    // Light text is the sensible default over a darkened photo.
    let text = match attrs.kv.get("text").map(String::as_str) {
        Some("dark") => " bg-text-dark",
        Some("light") => " bg-text-light",
        _ if dim.is_some_and(|d| d > 0.0) || scrim.is_some() => " bg-text-light",
        _ => "",
    };

    Background {
        pane_class: format!(" has-bg{text}"),
        layers: format!(
            "<img class=\"mz-bg\" src=\"{src}\" alt=\"\" style=\"{img_style}\">{overlays}<div class=\"mz-bg-content\">",
            src = escape_attr(src)
        ),
        close: "</div>".into(),
    }
}

/// Keeps a user-supplied value from escaping the style attribute.
fn sanitize_css(v: &str) -> String {
    v.chars()
        .filter(|c| c.is_alphanumeric() || " %.-".contains(*c))
        .collect()
}

/// Keeps a user-supplied value from escaping a double-quoted attribute.
/// `embed_assets` matches `src="…"`, so the quoting has to hold.
fn escape_attr(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_single_pane_slide(
    slide: &SlideSource,
    index: usize,
    errors: &mut Vec<String>,
    asset_source: &dyn AssetSource,
    chart_files: &mut Vec<std::path::PathBuf>,
) -> String {
    let mut content = slide.loose.clone();
    // Without a layout, `::: pane` blocks are simply concatenated.
    for pb in &slide.panes {
        content.push('\n');
        content.push_str(&pb.body);
    }
    let (content, chart_blocks) = charts::extract(&content);
    let mut body = render_markdown(&preprocess(&content));
    if !chart_blocks.is_empty() {
        let (with_charts, files) =
            charts::render_charts_in(&body, &chart_blocks, index, asset_source, errors);
        body = with_charts;
        chart_files.extend(files);
    }
    // Without a layout there is a single pane, so the first `::: pane` block that
    // asks for a background dresses the whole slide.
    let all: Vec<inline::Attrs> = slide
        .panes
        .iter()
        .map(|pb| parse_attrs(&pb.attrs))
        .collect();
    let attrs = all
        .iter()
        .find(|a| a.kv.contains_key("bg"))
        .or(all.first())
        .cloned()
        .unwrap_or_default();
    let extra_cls = attrs
        .classes
        .iter()
        .map(|c| format!(" {c}"))
        .collect::<String>();
    let bg = background_layers(&attrs);
    format!(
        "<div class=\"grid\" style='grid-template-columns:1fr;grid-template-rows:1fr;grid-template-areas:\"main\"'>\n<div class=\"pane pane-main{extra_cls}{}\" data-pane=\"main\" style=\"grid-area:main\">{}{body}{}</div>\n</div>\n",
        bg.pane_class, bg.layers, bg.close
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirzam_syntax::parse_slide;

    #[test]
    fn renders_grid_slide() {
        let slide = parse_slide(
            "## T\n\n```pane\n+---+---+\n| main  |\n+---+---+\n| a | b |\n+---+---+\n```\n\n::: pane a\nhello\n:::\n",
        );
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out
            .html
            .contains("grid-template-areas:\"main main\" \"a b\""));
        assert!(out.html.contains("pane-a"));
        assert!(out.html.contains("hello"));
    }

    #[test]
    fn panes_carry_a_data_pane_attribute() {
        let slide = parse_slide(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n::: pane a\nhello\n:::\n",
        );
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.html.contains("data-pane=\"a\""));
        assert!(out.html.contains("data-pane=\"b\""));
    }

    #[test]
    fn single_pane_slide_carries_a_data_pane_attribute() {
        let slide = parse_slide("# Hello\n\nworld\n");
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.html.contains("data-pane=\"main\""));
    }

    #[test]
    fn debug_layout_option_bakes_on_the_overlay_class() {
        let opts = PageOptions {
            debug_layout: true,
            ..Default::default()
        };
        let html = assemble_page(&DeckMeta::default(), &[], &opts);
        assert!(html.contains("<html lang=\"en\" data-theme=\"default\" class=\"mz-debug\">"));
    }

    #[test]
    fn debug_layout_off_by_default() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        assert!(html.contains("<html lang=\"en\" data-theme=\"default\">"));
        assert!(!html.contains("class=\"mz-debug\""));
    }

    #[test]
    fn named_theme_is_baked_onto_html() {
        let meta = DeckMeta {
            theme: Some("nord".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(html.contains("data-theme=\"nord\""));
        assert!(html.contains("--mz-bg: #2e3440"));
    }

    #[test]
    fn unknown_theme_falls_back_to_default_silently_in_assemble_page() {
        let meta = DeckMeta {
            theme: Some("does-not-exist".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(html.contains("data-theme=\"default\""));
    }

    #[test]
    fn unknown_theme_is_reported_through_render_deck_warnings() {
        let meta = DeckMeta {
            theme: Some("does-not-exist".into()),
            ..Default::default()
        };
        let out = render_deck(&meta, &[], Path::new("."));
        assert!(out.warnings.iter().any(|w| w.contains("does-not-exist")));
    }

    #[test]
    fn explicit_mode_is_baked_onto_html_and_unset_mode_is_not() {
        let dark = DeckMeta {
            mode: Some("dark".into()),
            ..Default::default()
        };
        let html = assemble_page(&dark, &[], &PageOptions::default());
        let html_tag = html.lines().nth(1).unwrap();
        assert!(html_tag.contains("data-mode=\"dark\""));

        let unset = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        let unset_html_tag = unset.lines().nth(1).unwrap();
        assert!(!unset_html_tag.contains("data-mode"));
    }

    #[test]
    fn print_page_also_carries_theme_and_mode() {
        let meta = DeckMeta {
            theme: Some("solarized".into()),
            mode: Some("dark".into()),
            ..Default::default()
        };
        let html = assemble_print_page(&meta, &[], None);
        assert!(html.contains("data-theme=\"solarized\""));
        assert!(html.contains("data-mode=\"dark\""));
    }

    #[test]
    fn relative_links_resolve_against_the_publish_base() {
        let base = "https://github.com/ayatough/Mirzam/blob/main/";
        let html = r#"<a href="docs/layout.md">a</a><a href="../docs/syntax.md#anim">b</a>"#;
        let out = rewrite_relative_links(html, base);
        assert!(out.contains("https://github.com/ayatough/Mirzam/blob/main/docs/layout.md"));
        // `..` climbs out of the base directory, as it would on disk.
        assert!(out.contains("https://github.com/ayatough/Mirzam/blob/docs/syntax.md#anim"));
    }

    #[test]
    fn absolute_and_in_page_links_are_left_alone() {
        let base = "https://example.com/a/";
        for href in [
            "https://example.org/x",
            "mailto:someone@example.com",
            "//cdn.example.com/x.js",
            "/root/path",
            "#section",
        ] {
            let html = format!("<a href=\"{href}\">x</a>");
            assert_eq!(rewrite_relative_links(&html, base), html, "{href}");
        }
    }

    #[test]
    fn a_deck_without_animation_carries_no_runtime() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        assert!(!html.contains("window.MZAnim = {"));
        assert!(!html.contains("data-transition"));
    }

    #[test]
    fn an_anim_block_pulls_in_the_runtime() {
        let sections = vec![
            "<section class=\"slide\"><script type=\"application/json\" \
                             class=\"mz-anim\">{}</script></section>"
                .to_string(),
        ];
        let html = assemble_page(&DeckMeta::default(), &sections, &PageOptions::default());
        assert!(html.contains("window.MZAnim = {"));
    }

    #[test]
    fn a_transition_pulls_in_the_runtime_on_its_own() {
        let meta = DeckMeta {
            transition: Some("slide-left 400ms".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(html.contains("window.MZAnim = {"));
        assert!(html.contains(r#"data-transition="{&quot;dir&quot;:&quot;left&quot;"#));
    }

    #[test]
    fn an_unusable_transition_leaves_plain_cuts() {
        let meta = DeckMeta {
            transition: Some("swipe-sideways".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(!html.contains("data-transition"));
    }

    #[test]
    fn print_pages_never_ship_the_runtime() {
        let meta = DeckMeta {
            transition: Some("fade".into()),
            ..Default::default()
        };
        let sections = vec![
            "<section class=\"slide\"><script type=\"application/json\" \
                             class=\"mz-anim\">{}</script></section>"
                .to_string(),
        ];
        let html = assemble_print_page(&meta, &sections, None);
        assert!(!html.contains("window.MZAnim = {"));
        assert!(!html.contains("data-transition"));
    }

    fn annotated_section() -> Vec<String> {
        vec!["<section class=\"slide\"><div data-pane=\"fig\"></div>\
             <script type=\"application/json\" class=\"mz-annot\" \
             data-target=\"[data-pane=&quot;fig&quot;]\">{\"items\":[]}</script></section>"
            .to_string()]
    }

    /// `mz-annot-layer` appears in the stylesheet whether or not the overlay
    /// ships, so the marker for "the script is here" has to be something only
    /// the script defines.
    const ANNOT_MARKER: &str = "window.__mirzamAnnot";

    #[test]
    fn a_deck_without_annotations_carries_no_overlay() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        assert!(!html.contains(ANNOT_MARKER));
        assert!(!assemble_print_page(&DeckMeta::default(), &[], None).contains(ANNOT_MARKER));
    }

    #[test]
    fn an_annotate_block_pulls_in_the_overlay() {
        let html = assemble_page(
            &DeckMeta::default(),
            &annotated_section(),
            &PageOptions::default(),
        );
        assert!(html.contains(ANNOT_MARKER));
    }

    /// The one script the print page carries, and deliberately so: an
    /// annotation is drawn *over* the slide and hides nothing, so the PDF
    /// would otherwise lose the marks the slide exists to make.
    #[test]
    fn print_pages_do_ship_the_annotation_overlay() {
        let html = assemble_print_page(&DeckMeta::default(), &annotated_section(), None);
        assert!(html.contains(ANNOT_MARKER));
        assert!(!html.contains("window.MZAnim = {"));
    }

    fn effects_section() -> Vec<String> {
        vec![
            "<section class=\"slide\"><script type=\"application/json\" \
             class=\"mz-fx\">[{\"key\":\"1\",\"effect\":\"flash\"}]</script></section>"
                .to_string(),
        ]
    }

    /// `.mz-fx-layer` is styled in the shared stylesheet whether or not the
    /// runtime ships, so the marker has to be something only the script has.
    const FX_MARKER: &str = "function bindingsFor";

    #[test]
    fn a_deck_without_effects_carries_none_of_the_runtime() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        assert!(!html.contains(FX_MARKER));
    }

    #[test]
    fn an_effects_block_pulls_in_the_runtime() {
        let html = assemble_page(
            &DeckMeta::default(),
            &effects_section(),
            &PageOptions::default(),
        );
        assert!(html.contains(FX_MARKER));
    }

    /// An effect belongs to the performance, not the document: the export must
    /// never carry one, however the deck was written.
    #[test]
    fn print_pages_never_ship_effects() {
        let html = assemble_print_page(&DeckMeta::default(), &effects_section(), None);
        assert!(!html.contains(FX_MARKER));
    }

    #[test]
    fn effects_block_binds_a_key() {
        let slide = parse_slide("Body\n\n```effects\n1 : flash\ne : burst 🎉\n```\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out.html.contains("class=\"mz-fx\""));
        assert!(out
            .html
            .contains(r#"{"key":"e","effect":"burst","arg":"🎉"}"#));
    }

    #[test]
    fn effects_binding_a_viewer_key_warns_and_drops() {
        let slide = parse_slide("Body\n\n```effects\nf : flash\n```\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.html.contains("class=\"mz-fx\""));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("taken by the viewer")),
            "{:?}",
            out.warnings
        );
    }

    #[test]
    fn annotate_block_emits_the_c2_model() {
        let slide = parse_slide(
            "```pane\n+---+\n| fig |\n+---+\n```\n\n::: pane fig\ntext\n:::\n\n\
             ```annotate\ntarget: fig\ncircle 40,30 20x20 : label=\"here\"\n```\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out.html.contains("class=\"mz-annot\""));
        assert!(out
            .html
            .contains("data-target=\"[data-pane=&quot;fig&quot;]\""));
        assert!(out.html.contains("\"kind\":\"circle\""));
    }

    #[test]
    fn annotate_pointing_at_nothing_warns_and_drops() {
        let slide = parse_slide(
            "```pane\n+---+\n| fig |\n+---+\n```\n\n::: pane fig\ntext\n:::\n\n\
             ```annotate\ntarget: ghost\ncircle 40,30 20x20\n```\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.html.contains("class=\"mz-annot\""));
        assert!(
            out.warnings.iter().any(|w| w.contains("matches nothing")),
            "{:?}",
            out.warnings
        );
    }

    #[test]
    fn unknown_pane_warns() {
        let slide = parse_slide("```pane\n+---+\n| a |\n+---+\n```\n\n::: pane zzz\nlost\n:::\n");
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.warnings.iter().any(|w| w.contains("zzz")));
    }

    #[test]
    fn pane_background_adds_image_and_scrim() {
        let slide = parse_slide(
            "```pane\n+-----+\n|     |\n| hero|\n|     |\n+-----+\n```\n\n::: pane hero {bg=img/p.jpg dim=0.5 blur=4 scrim=bottom}\nTitle\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.html.contains("class=\"mz-bg\""));
        assert!(out.html.contains("object-fit:cover"));
        assert!(out.html.contains("filter:blur(4px)"));
        assert!(out.html.contains("rgba(0,0,0,0.5)"));
        assert!(out.html.contains("linear-gradient(to top"));
        assert!(out.html.contains("has-bg bg-text-light"));
        assert!(out.html.contains("mz-bg-content"));
    }

    #[test]
    fn pane_without_background_is_unchanged() {
        let slide = parse_slide("```pane\n+-----+\n| a   |\n+-----+\n```\n\n::: pane a\nx\n:::\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.html.contains("class=\"mz-bg\""));
        assert!(out.html.contains("class=\"pane pane-a\""));
    }

    #[test]
    fn background_position_is_sanitized() {
        let slide = parse_slide(
            "```pane\n+-----+\n| a   |\n+-----+\n```\n\n::: pane a {bg=p.jpg bg-pos=\"top;color:red\"}\nx\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.html.contains("color:red"));
    }

    #[test]
    fn print_replaces_video_with_poster() {
        let html = "<video src=\"a.mp4\" title=\"demo\" poster=\"p.png\" autoplay style=\"width:100%\"></video>";
        let out = videos_to_stills(html);
        assert!(out.contains("<img src=\"p.png\""));
        assert!(out.contains("width:100%"));
        assert!(!out.contains("<video"));
    }

    #[test]
    fn print_video_without_poster_gets_placeholder() {
        let out = videos_to_stills("<video src=\"a.mp4\" title=\"Demo clip\"></video>");
        assert!(out.contains("mz-video-still"));
        assert!(out.contains("Demo clip"));
    }

    #[test]
    fn slide_without_layout_is_single_pane() {
        let slide = parse_slide("# Hello {.title-slide}\n\nworld\n");
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.html.contains("title-slide"));
        assert!(out.html.contains("world"));
    }
}
