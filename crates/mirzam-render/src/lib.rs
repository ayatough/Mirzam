//! Turns a list of `SlideSource` values plus `DeckMeta` into a single HTML
//! file with the viewer embedded.

mod assets;
mod charts;
mod inline;
mod theme;

pub use assets::{AssetSource, FsAssets};
pub use charts::render_charts_in;
pub use inline::{parse_attrs, preprocess, render_markdown};

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
}

/// Whether any section contains math, deciding if the math font is bundled.
pub fn sections_have_math(sections: &[String]) -> bool {
    sections.iter().any(|s| s.contains("<math"))
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
    format!(
        r#"<!doctype html>
<html lang="en">
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
<div id="deck" data-slide-w="{w}" data-slide-h="{h}">
{sections}</div>
<div id="hud"></div>
<div id="hint">← → navigate / N notes / F fullscreen</div>
<div id="notes-panel" hidden></div>
<script>{js}</script>
{live_js}</body>
</html>
"#,
        title = inline::html_escape(title),
        css = theme::DEFAULT_CSS,
        custom_css = opts.custom_css.as_deref().unwrap_or(""),
        js = theme::VIEWER_JS,
        sections = sections.concat(),
    )
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
    format!(
        r#"<!doctype html>
<html lang="en">
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
</body>
</html>
"#,
        title = inline::html_escape(title),
        css = theme::DEFAULT_CSS,
        print_css = theme::PRINT_CSS,
        custom_css = custom_css.unwrap_or(""),
        sections = sections.concat(),
    )
}

/// Renders a whole deck to a single HTML file.
/// `asset_dir` is the base directory for relative asset paths.
pub fn render_deck(meta: &DeckMeta, slides: &[SlideSource], asset_dir: &Path) -> RenderResult {
    let mut warnings = Vec::new();
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

    let body = match &grid {
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

    // Reserved blocks are parsed but only summarized, keeping decks forward compatible.
    let mut reserved_html = String::new();
    for (kind, src) in &slide.reserved {
        let phase = match kind {
            mirzam_syntax::BlockKind::Anim => "Phase 3",
        };
        reserved_html.push_str(&format!(
            "<details class=\"mz-reserved\"><summary>```{kind} block (planned for {phase})</summary><pre>{body}</pre></details>\n",
            kind = kind.as_str(),
            body = inline::html_escape(src.trim()),
        ));
    }

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
        "<section class=\"slide\" data-index=\"{index}\"{connect_attr}>\n{error_html}{body}{shapes_html}{reserved_html}{notes_html}</section>\n"
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
        panes_html.push_str(&format!(
            "<div class=\"pane pane-{name}{extra_cls}\" style=\"{style}\">{body}</div>\n"
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
    format!(
        "<div class=\"grid\" style='grid-template-columns:1fr;grid-template-rows:1fr;grid-template-areas:\"main\"'>\n<div class=\"pane pane-main\" style=\"grid-area:main\">{body}</div>\n</div>\n"
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
    fn unknown_pane_warns() {
        let slide = parse_slide("```pane\n+---+\n| a |\n+---+\n```\n\n::: pane zzz\nlost\n:::\n");
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.warnings.iter().any(|w| w.contains("zzz")));
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
