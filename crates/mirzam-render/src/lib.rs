//! Turns a list of `SlideSource` values plus `DeckMeta` into a single HTML
//! file with the viewer embedded.

mod anim;
mod annot;
mod assets;
mod charts;
mod cite;
mod code;
mod connect;
mod effects;
mod inline;
mod json;
mod mermaid;
mod shapes;
mod source;
mod theme;
mod toc;

pub use assets::{AssetSource, FsAssets};
pub use charts::render_charts_in;
pub use cite::mark as mark_citations;
pub use cite::resolve_deck as resolve_citations;
pub use code::{highlight as highlight_code, TOKEN_CLASSES};
pub use inline::{parse_attrs, preprocess, preprocess_math, render_markdown, render_math};
pub use mermaid::DiagramRenderer;
pub use mirzam_cite::{Bibliography, CiteStyle};
pub use mirzam_core::MathDialect;
pub use source::DeckSource;
pub use theme::{
    contrast_ratio, file_theme_warnings, mode_warning, theme_warnings, FileTheme, THEME_NAMES,
};
pub use toc::resolve_deck;

use mirzam_core::DeckMeta;
use mirzam_layout::{parse_grid, Edges, GridSpec};
use mirzam_syntax::SlideSource;
use regex::Regex;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;

pub struct RenderResult {
    pub html: String,
    pub warnings: Vec<String>,
}

/// What a slide has to know about the deck around it.
///
/// A slide is rendered on its own — that is what lets `serve` re-use every
/// unchanged slide while one re-renders — so nothing about the deck is in
/// reach from inside `render_slide`. Everything that is a property of the
/// deck and changes a slide's HTML arrives here instead, explicitly.
#[derive(Debug, Clone)]
pub struct DeckContext {
    /// Which syntax `$...$` holds, from frontmatter `math:`.
    pub math: MathDialect,
    /// Logical slide size from frontmatter `aspect:`. The shape layer's
    /// viewBox and every pane rectangle are computed in this space.
    pub slide_size: (u32, u32),
    /// The grid's margin and gutter, from frontmatter `grid-pad-x/-y` and
    /// `grid-gap` (stylesheet defaults otherwise). Pane rectangles — the
    /// frames pane-anchored `shape` blocks draw in — are arithmetic on these.
    pub grid: mirzam_core::GridMetrics,
    /// Named layouts a slide can be drawn on. Either frontmatter `masters:`
    /// written inline, or the file it named — resolved by the caller, which
    /// is the half of this that needs a filesystem.
    pub masters: BTreeMap<String, String>,
    /// The master a slide takes when it neither draws a grid nor names one,
    /// from frontmatter `layout:`.
    pub layout: Option<String>,
    /// Footer text drawn on every slide, from frontmatter `footer:`.
    pub footer: Option<String>,
    /// Slide-number template, from frontmatter `slide-number:`.
    pub slide_number: Option<String>,
    /// How many slides the deck has, for `{total}`.
    pub total: usize,
    /// Whether the deck declared any references, from frontmatter
    /// `bibliography:`. Only the flag, not the entries: a slide records the
    /// key it cited and nothing else, so editing a `.bib` rewrites the
    /// reference list without invalidating one cached slide.
    ///
    /// It has to reach a slide at all because a deck with no bibliography must
    /// leave `[@name]` as the text somebody typed.
    pub citations: bool,
    /// The stylesheets the deck's `theme:` named, once the host has read them
    /// — resolved by the caller, like [`Self::masters`], because reading a
    /// file is the half of this that needs a filesystem.
    ///
    /// A slide needs them for one question only: whether a `theme=` naming one
    /// of their stems is a name this deck has. That is why the whole theme is
    /// here rather than a list of names — the answer also depends on whether
    /// the file scopes its tokens to its own stem, which only its text says.
    pub file_themes: Vec<FileTheme>,
    /// Set when the deck named a masters file that could not be read. Every
    /// name it would have defined is then missing, and the file is the one
    /// fact worth reporting: without this, deleting it says "cannot read the
    /// file" and "no master named `body`" — the same sentence twice.
    pub masters_unavailable: bool,
}

impl DeckContext {
    /// The context a deck's frontmatter describes. `total` is the number of
    /// slides *rendered*, which is what a slide number counts: a slide broken
    /// by `<!-- next -->` is several pages to the audience.
    ///
    /// A deck whose `masters:` names a file starts with none: reading it needs
    /// a `FileProvider`, so the caller loads it with
    /// [`mirzam_syntax::load_masters`] and assigns [`Self::masters`].
    pub fn new(meta: &DeckMeta, total: usize) -> Self {
        Self {
            // A bad dialect renders as LaTeX and is reported where the
            // frontmatter was parsed; there is no warning channel here.
            math: meta.math_dialect().unwrap_or_default(),
            slide_size: meta.slide_size(),
            // Bad pixel values keep the defaults and are reported where the
            // frontmatter was parsed, like `math:`.
            grid: meta.grid_metrics().0,
            masters: meta.inline_masters().cloned().unwrap_or_default(),
            layout: meta.layout.clone(),
            footer: meta.footer.clone(),
            slide_number: meta.slide_number.clone(),
            total,
            citations: !meta.bibliography.is_empty(),
            file_themes: Vec::new(),
            masters_unavailable: false,
        }
    }

    /// Problems with the deck's own settings, reported once for the deck
    /// rather than once per slide: a `layout:` naming a master that does not
    /// exist would otherwise repeat on every slide in the deck.
    ///
    /// A master whose art does not parse is *not* reported here. It reaches
    /// `parse_grid` on the slides that use it, where the existing error path
    /// already names it — reporting it twice would be worse than once.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        // The file that would have defined every name is already reported.
        // Naming them one at a time after that is one fact, repeated.
        if self.masters_unavailable {
            return out;
        }
        if let Some(name) = self.layout.as_deref().map(str::trim) {
            if name != "none" && !self.masters.contains_key(name) {
                out.push(format!(
                    "layout: no master named `{name}`{}; slides that do not draw \
                     their own grid render as a single pane",
                    self.known_masters()
                ));
            }
        }
        out
    }

    /// ` (known: a, b)`, to say what was probably meant.
    ///
    /// A deck that defines none says so instead, because that is a different
    /// mistake with a different fix: not a typo in a name, but frontmatter
    /// missing its `masters:`. It is the one a deck assembled from `![[…]]`
    /// section files runs into — a section can name a master, but only the
    /// root deck's frontmatter is read, so the shapes have to be declared
    /// there however many files the slides come from.
    fn known_masters(&self) -> String {
        if self.masters.is_empty() {
            return " (this deck defines none)".into();
        }
        format!(
            " (known: {})",
            self.masters.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }

    /// Whether the deck draws a footer or a slide number at all.
    fn has_chrome(&self) -> bool {
        self.footer.is_some() || self.slide_number.is_some()
    }

    /// A key for everything here that changes a slide's rendered HTML, so a
    /// build cache keyed on slide text alone does not survive an edit to the
    /// deck's frontmatter.
    ///
    /// `total` is included only when something actually prints it. Otherwise
    /// adding a slide would invalidate every other slide in the deck, which is
    /// exactly the incremental rebuild the cache exists to avoid.
    pub fn fingerprint(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.math.hash(&mut h);
        self.slide_size.hash(&mut h);
        for v in [self.grid.pad_x, self.grid.pad_y, self.grid.gap] {
            v.to_bits().hash(&mut h);
        }
        self.masters.hash(&mut h);
        self.layout.hash(&mut h);
        self.footer.hash(&mut h);
        self.slide_number.hash(&mut h);
        self.citations.hash(&mut h);
        // Only what a slide can read off a theme file: whether the name is
        // one this deck has, and whether it answers to that name. The rest of
        // the stylesheet changes the page, not the markup, and hashing it here
        // would re-render every slide over a colour.
        for theme in &self.file_themes {
            theme.name.hash(&mut h);
            theme.scopes_to_stem().hash(&mut h);
        }
        if self.prints_total() {
            self.total.hash(&mut h);
        }
        h.finish()
    }

    fn prints_total(&self) -> bool {
        [&self.footer, &self.slide_number]
            .into_iter()
            .flatten()
            .any(|t| t.contains("{total}"))
    }
}

impl Default for DeckContext {
    /// The context of a deck whose frontmatter says nothing.
    fn default() -> Self {
        Self::new(&DeckMeta::default(), 0)
    }
}

/// The deck's references, however its frontmatter declared them.
///
/// `read` is how the host reaches a named `.bib`: the CLI reads it from disk
/// and adds it to the watch set, the browser takes it from the table the
/// editor injected. The core cannot do either, and the wording of what went
/// wrong should not be written twice.
///
/// A bibliography that cannot be read is a warning, never a build failure. The
/// deck still renders; every `[@key]` on it stays as the author typed it, and
/// the warning says so rather than leaving the marks to be explained by
/// whoever notices them on the slide.
pub fn deck_bibliography(
    meta: &DeckMeta,
    read: impl FnOnce(&str) -> Result<String, String>,
) -> (Bibliography, Vec<String>) {
    let Some(rel) = meta.bibliography_file() else {
        let entries = meta
            .inline_bibliography()
            .map(|m| {
                m.iter()
                    .map(|(k, fields)| {
                        (
                            k.clone(),
                            mirzam_cite::Entry::from_fields(k, fields.clone()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        return (entries, Vec::new());
    };
    let src = match read(rel) {
        Ok(src) => src,
        Err(e) => {
            return (
                Bibliography::new(),
                vec![format!(
                    "bibliography: {e}; every `[@key]` is left as written"
                )],
            )
        }
    };
    let (entries, parse_warnings) = mirzam_cite::parse_bibtex(&src);
    let mut warnings: Vec<String> = parse_warnings
        .into_iter()
        .map(|w| format!("{rel}: {w}"))
        .collect();
    if entries.is_empty() && warnings.is_empty() {
        warnings.push(format!(
            "bibliography: {rel} defines no entries; an entry is `@type{{key, \
             field = {{value}}}}`"
        ));
    }
    (entries, warnings)
}

/// How this deck writes a citation, with a warning for a name that is not a
/// style. An unrecognised name numbers the references, which is the default a
/// deck that said nothing would have had.
pub fn citation_style(meta: &DeckMeta) -> (CiteStyle, Option<String>) {
    match meta.citation_style.as_deref() {
        None => (CiteStyle::default(), None),
        Some(src) => match CiteStyle::parse(src) {
            Ok(style) => (style, None),
            Err(e) => (
                CiteStyle::default(),
                Some(format!("{e}; citing by number instead")),
            ),
        },
    }
}

/// One rendered slide: a `<section>` element with assets already inlined.
pub struct RenderedSlide {
    pub html: String,
    pub warnings: Vec<String>,
    /// Local assets this slide referenced, for cache validation and watching.
    pub assets: Vec<std::path::PathBuf>,
}

/// What a slide render needs from whoever is hosting it: the two jobs a core
/// crate may not do for itself, because the WebAssembly build can do neither.
///
/// Reading a file and running a program are the same kind of favour asked in
/// two ways, so they travel together rather than as a growing list of
/// parameters through every function between here and the pane that needs
/// them. `diagrams` is `None` for a host with no diagram renderer — which is
/// every browser build, and any machine without `mmdc` — and that is the state
/// a `mermaid` fence degrades to a code block in.
#[derive(Clone, Copy)]
struct Host<'a> {
    assets: &'a dyn AssetSource,
    diagrams: Option<&'a dyn DiagramRenderer>,
}

/// Renders one slide to `<section>` HTML; this is the unit `serve` updates.
/// Assets are resolved from the filesystem. `ctx` carries the deck's own
/// settings, which a slide cannot see from its text — see [`DeckContext`].
pub fn render_slide_html(
    slide: &SlideSource,
    index: usize,
    asset_dir: &Path,
    diagrams: Option<&dyn DiagramRenderer>,
    ctx: &DeckContext,
) -> RenderedSlide {
    render_slide_html_with(slide, index, &assets::FsAssets(asset_dir), diagrams, ctx)
}

/// Variant with pluggable asset resolution; WASM hosts inject their own table.
///
/// `diagrams` is the same arrangement one step further out: the renderer for
/// `mermaid` blocks arrives from the host or does not arrive at all. Passing
/// `None` is not an error — it is what the browser build does, and every
/// `mermaid` fence then renders as the code block a plain CommonMark parser
/// would have made, with a warning saying so.
pub fn render_slide_html_with(
    slide: &SlideSource,
    index: usize,
    asset_source: &dyn AssetSource,
    diagrams: Option<&dyn DiagramRenderer>,
    ctx: &DeckContext,
) -> RenderedSlide {
    let mut warnings = Vec::new();
    let mut assets_used = Vec::new();
    let host = Host {
        assets: asset_source,
        diagrams,
    };
    // Charts are rendered first: they may pull in CSV data through the same
    // asset source, and their SVG output must not be scanned for asset URLs.
    let html = render_slide(slide, index, &mut warnings, host, &mut assets_used, ctx);
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
    /// The stylesheets frontmatter's `theme:` named, in cascade order, once
    /// the host has read them. They are inlined **after** `base.css`, which
    /// is what lets a theme somebody wrote override the type the shared
    /// stylesheet sets.
    pub file_themes: Vec<FileTheme>,
    /// Bakes the layout debug overlay on at load, instead of leaving it to the
    /// viewer's `L` key. For screenshotting a broken deck headlessly.
    pub debug_layout: bool,
    /// Inlines every built-in theme's tokens, not only the ones this deck
    /// currently uses. `serve` sets it: a slide that gains `<!-- theme: -->`
    /// mid-edit is patched into a page whose `<head>` was assembled before
    /// that theme existed in the deck, and the pane would come out in the
    /// deck's palette until the next full reload.
    pub all_themes: bool,
    /// The deck's own Markdown, for the viewer's `V` panel and the handover to
    /// the browser editor. Absent — the default — and the deck carries no
    /// source, has no `V` key, and is the same file it always was.
    pub source: Option<source::DeckSource>,
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

/// Whether anything asks to be shrunk to fit, deciding if `fit.js` is inlined
/// — into the print page too, since it only ever reveals content a clipped
/// pane would have swallowed.
fn deck_fit_attr(meta: &DeckMeta) -> &'static str {
    match meta.fit.as_deref().map(str::trim) {
        Some("shrink") => " data-fit=\"shrink\"",
        _ => "",
    }
}

fn deck_has_fit(meta: &DeckMeta, sections: &[String]) -> bool {
    !deck_fit_attr(meta).is_empty() || sections.iter().any(|s| s.contains("mz-fit"))
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

/// The `data-autoplay` payload for `#deck`, or an empty string when the deck
/// declares no autoplay or an unusable one. Same contract as
/// [`transition_attr`]: reporting the problem is the build pipeline's job.
/// Autoplay needs no runtime beyond the viewer itself — advancing is what the
/// viewer already does — so nothing extra is inlined for it.
fn autoplay_attr(meta: &DeckMeta) -> String {
    match meta.autoplay.as_deref().map(mirzam_anim::parse_autoplay) {
        Some(Ok(a)) => format!(
            " data-autoplay=\"{}\"",
            inline::html_escape(&mirzam_anim::autoplay_json(&a))
        ),
        _ => String::new(),
    }
}

/// Resolves frontmatter `theme:`/`mode:` to the attributes baked onto
/// `<html>`. Always valid, silently falling back to `mirzam`/no mode
/// attribute for a name that is not a built-in: a caller that wants to
/// report an unknown name calls [`theme_warning`]/[`mode_warning`] where
/// `meta` was parsed, since this function has no warning channel of its own.
///
/// `mirzam` is the fallback because there is no longer a `default` to be one:
/// the two names carried the same 66 token declarations, and the one that
/// survived is the one that says whose palette it is. An unset `mode:` stays
/// unset, and so keeps meaning `prefers-color-scheme`: the room a deck is
/// opened in is something the viewer can see and the renderer cannot.
///
/// A `theme:` naming only stylesheets of the deck's own falls back here too:
/// `base.css` reads tokens that have to come from somewhere, and a file theme
/// loaded over them may set as few of them as it likes.
fn theme_attrs(meta: &DeckMeta) -> (&'static str, String) {
    let name = theme::THEME_NAMES
        .iter()
        .find(|n| Some(**n) == meta.theme_name())
        .copied()
        .unwrap_or(theme::FALLBACK_THEME);
    let mode_attr = match theme::normalize_mode(meta.mode.as_deref()) {
        Some(m) => format!(" data-mode=\"{m}\""),
        None => String::new(),
    };
    (name, mode_attr)
}

/// The deck's own file theme, as a `data-theme` on `<body>` — set only when
/// `theme:` names **no built-in at all** and one of the stylesheets it names
/// scopes its tokens to its own stem.
///
/// Without this, `theme: themes/acme.css` would be a deck that loads a theme
/// nothing on the page answers to: the stem is a selector, and the only
/// elements carrying `data-theme` would be the panes that asked for one. A
/// deck that names its own theme and nothing else means that theme, so the
/// name goes on the element everything is inside.
///
/// `<body>` rather than `<html>`, because `<html>` is carrying the built-in
/// token set the shared stylesheet reads: a file theme may set three tokens or
/// sixty, and the ones it leaves alone still have to come from somewhere. Body
/// is inside it, so the deck's chrome takes the deck's palette too, and
/// `data-mode` stays above both, where the mode selectors expect it.
///
/// A deck that names a built-in as well — `theme: [mirzam, acme.css]` — has
/// said which palette it is in, and its own stylesheet is then loaded for the
/// slides and panes that name it. That is what lets one deck show a theme
/// without wearing it.
fn body_theme_attr(meta: &DeckMeta, file_themes: &[FileTheme]) -> String {
    if meta.theme_name().is_some() {
        return String::new();
    }
    match file_themes.iter().rev().find(|t| t.scopes_to_stem()) {
        Some(theme) => format!(" data-theme=\"{}\"", theme.name),
        None => String::new(),
    }
}

/// Every built-in theme the page has to carry tokens for: the deck's own,
/// then any a slide or a pane switched to.
///
/// Read back out of the rendered HTML, the way this file already decides
/// whether to inline the animation runtime or the math font — a slide is
/// rendered without knowing the deck it belongs to, so the assembled sections
/// are where the answer is. A deck that merely *shows* the attribute in a code
/// block costs a few hundred bytes of CSS nothing uses, which is the safe
/// direction to be wrong in: the other one is a pane rendered in a palette the
/// page never loaded.
fn themes_used(meta: &DeckMeta, sections: &[String], all: bool) -> Vec<&'static str> {
    let (deck, _) = theme_attrs(meta);
    let mut used = vec![deck];
    for name in theme::THEME_NAMES {
        let needle = format!("data-theme=\"{name}\"");
        if !used.contains(name) && (all || sections.iter().any(|s| s.contains(&needle))) {
            used.push(name);
        }
    }
    used
}

/// A key for everything [`assemble_page`] puts *around* the slides: the theme
/// tokens, the `<html>` attributes, the title, the deck's size, and which
/// runtimes are inlined.
///
/// A host that patches changed `<section>`s into a page it assembled earlier —
/// `serve`, the editor preview — cannot show any of that changing, because none
/// of it lives in a slide. Comparing this key is how such a host finds out it
/// has to assemble the page again: without it, a deck that only swaps `theme:`
/// produces no changed slide and the edit is dropped on the floor.
///
/// `live_version` is deliberately left out: it moves on every reload and says
/// nothing about how the page looks.
///
/// Whatever `assemble_page` reads out of `meta`, `sections` or `opts` is read
/// here too — in the same change, or the host stops being told.
pub fn page_fingerprint(meta: &DeckMeta, sections: &[String], opts: &PageOptions) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    meta.title.hash(&mut h);
    meta.slide_size().hash(&mut h);
    theme_attrs(meta).hash(&mut h);
    themes_used(meta, sections, opts.all_themes).hash(&mut h);
    transition_attr(meta).hash(&mut h);
    autoplay_attr(meta).hash(&mut h);
    deck_fit_attr(meta).hash(&mut h);
    sections_have_math(sections).hash(&mut h);
    deck_has_anim(meta, sections).hash(&mut h);
    deck_has_annot(sections).hash(&mut h);
    deck_has_fit(meta, sections).hash(&mut h);
    effects::deck_has_effects(sections).hash(&mut h);
    opts.file_themes.hash(&mut h);
    body_theme_attr(meta, &opts.file_themes).hash(&mut h);
    opts.debug_layout.hash(&mut h);
    opts.source.hash(&mut h);
    meta.grid_metrics_css().hash(&mut h);
    h.finish()
}

/// The deck's own stylesheets, in cascade order, as the body of one `<style>`
/// element — the slot after `base.css` that a file theme has to occupy to be
/// able to override it.
///
/// One element rather than one each: source order inside a stylesheet is the
/// same cascade as source order between them, and a deck is one file, so the
/// only thing several elements would add is several places to look. Each is
/// labelled with the path it came from, because the page *is* the artefact
/// somebody debugs.
///
/// A theme that scopes its tokens to its own stem is a scope like a built-in
/// one, and gets the same reset block first — see [`theme::scope_defaults`].
/// Without it a pane wearing somebody's theme would still inherit the deck's
/// type and marks for everything that theme happens not to set, which is the
/// one form of the bug the author of the file cannot fix by editing a built-in.
/// It goes *before* their declarations, so their values still win; a file that
/// does not scope to its stem gets none, because nothing on the page carries
/// the name and the block would answer to no element.
fn file_themes_css(themes: &[FileTheme]) -> String {
    themes
        .iter()
        .map(|t| {
            let defaults = match t.scopes_to_stem() {
                true => theme::scope_defaults(&t.name),
                false => String::new(),
            };
            format!("/* {} */\n{defaults}{}", t.path, t.css)
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    let fit_js = if deck_has_fit(meta, sections) {
        format!("<script>{}</script>\n", theme::FIT_JS)
    } else {
        String::new()
    };
    // The source payload and the control that opens it travel together: a
    // button that opens an empty panel is worse than no button.
    let source_script = opts.source.as_ref().map(|s| s.script()).unwrap_or_default();
    let source_button = if source_script.is_empty() {
        String::new()
    } else {
        "\n<button id=\"mz-source-btn\" type=\"button\" aria-label=\"The Markdown behind this slide\">&lt;/&gt;</button>".to_string()
    };
    let transition = transition_attr(meta);
    let autoplay = autoplay_attr(meta);
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
<style>{theme_files_css}</style>
<style>{grid_css}</style>
</head>
<body{body_theme}>
<div id="deck" data-slide-w="{w}" data-slide-h="{h}"{transition}{autoplay}{fit}>
{sections}</div>
<div id="chrome">
<div id="hud"></div>
<div id="controls">
<button id="mz-prev" type="button" aria-label="Previous">‹</button>
<button id="mz-next" type="button" aria-label="Next">›</button>
<button id="mz-ov-btn" type="button" aria-label="All slides">⊞</button>
<button id="mz-mode" type="button" aria-label="Switch colour mode"></button>{source_button}
<button id="mz-help" type="button" aria-label="Keyboard shortcuts">?</button>
</div>
</div>
<div id="keys" hidden></div>
<div id="notes-panel" hidden></div>
<div id="source-panel" hidden></div>
<div id="overview" hidden></div>
{source_script}{fit_js}{anim_js}{annot_js}<script>{js}</script>
<script>{presenter_js}</script>
{effects_js}{live_js}</body>
</html>
"#,
        title = inline::html_escape(title),
        css = theme::theme_css_for(&themes_used(meta, sections, opts.all_themes)),
        theme_files_css = file_themes_css(&opts.file_themes),
        body_theme = body_theme_attr(meta, &opts.file_themes),
        // Last, so declared grid metrics beat a theme's or stylesheet's own
        // custom-property overrides: the core computed pane rectangles from
        // these numbers, and the browser has to lay the grid out from them too.
        grid_css = meta.grid_metrics_css(),
        js = theme::VIEWER_JS,
        presenter_js = theme::PRESENTER_JS,
        fit = deck_fit_attr(meta),
        sections = sections.concat(),
    )
}

/// Tags a rendered slide as one part of a continuation group.
///
/// `<!-- next -->` breaks one pane of a slide across several slides that are
/// otherwise identical. The viewer reads this attribute as *cut, do not
/// animate*: the panes that did not break are the same elements in the same
/// places, and turning the page between them is exactly the flicker the
/// feature exists to avoid.
///
/// Applied after rendering rather than during it, because which group a slide
/// belongs to is a property of the deck around it, not of the slide.
pub fn mark_continuation(section: &str, group: usize) -> String {
    section.replacen(
        "<section class=\"slide\"",
        &format!("<section class=\"slide\" data-cont=\"{group}\""),
        1,
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

/// Replaces the things a page cannot play with something a page can show.
///
/// A `<video>` becomes its poster, or a placeholder; a hosted embed becomes a
/// placeholder carrying the link it came from; a widget and an `<audio>` keep
/// their labels and lose the controls paper cannot offer. PDF output is static, and silently printing an
/// empty box would be worse than saying what is missing.
fn videos_to_stills(html: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"<video\b([^>]*)></video>"#).expect("static regex"));
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR.get_or_init(|| Regex::new(r#"(\w[\w-]*)="([^"]*)""#).expect("static regex"));
    let out = re.replace_all(html, |c: &regex::Captures| {
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
    .into_owned();

    // A hosted video cannot be printed either, and unlike a local file it has
    // somewhere to send the reader: the page it came from.
    static EMBED: OnceLock<Regex> = OnceLock::new();
    let embed = EMBED.get_or_init(|| {
        Regex::new(
            r#"<div class="mz-embed"[^>]*data-href="([^"]*)" data-title="([^"]*)"[^>]*>.*?</div>"#,
        )
        .expect("static regex")
    });
    let out = embed
        .replace_all(&out, |c: &regex::Captures| {
            format!(
                "<div class=\"mz-video-still\"><span>▶</span><em>{}</em>\
                 <a href=\"{}\">{}</a></div>",
                &c[2], &c[1], &c[1]
            )
        })
        .into_owned();

    // A widget cannot be printed at all: it is a document that only exists
    // while something is running it. Its label is all paper can carry, and an
    // empty box would say less than the dashed one saying so.
    static WIDGET: OnceLock<Regex> = OnceLock::new();
    let widget = WIDGET.get_or_init(|| {
        Regex::new(r#"<div class="mz-embed mz-html[^"]*"[^>]*data-title="([^"]*)"[^>]*>.*?</div>"#)
            .expect("static regex")
    });
    let out = widget
        .replace_all(&out, |c: &regex::Captures| {
            format!(
                "<div class=\"mz-video-still\"><span>⧉</span><em>{}</em></div>",
                &c[1]
            )
        })
        .into_owned();

    // A recording keeps its label and loses the transport it cannot offer.
    static AUDIO: OnceLock<Regex> = OnceLock::new();
    let audio =
        AUDIO.get_or_init(|| Regex::new(r#"<audio\b[^>]*></audio>"#).expect("static regex"));
    audio
        .replace_all(&out, "<div class=\"mz-video-still\"><span>♪</span></div>")
        .into_owned()
}

/// Print page for PDF export: fixed-size slides stacked one per page.
pub fn assemble_print_page(
    meta: &DeckMeta,
    sections: &[String],
    file_themes: &[FileTheme],
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
    let fit_js = if deck_has_fit(meta, sections) {
        format!("<script>{}</script>\n", theme::FIT_JS)
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
<style>{theme_files_css}</style>
<style>{grid_css}</style>
</head>
<body{body_theme}>
<div id="deck"{fit}>
{sections}</div>
{fit_js}{annot_js}</body>
</html>
"#,
        title = inline::html_escape(title),
        css = theme::theme_css_for(&themes_used(meta, sections, false)),
        print_css = theme::PRINT_CSS,
        fit = deck_fit_attr(meta),
        theme_files_css = file_themes_css(file_themes),
        body_theme = body_theme_attr(meta, file_themes),
        // Same placement and reason as `assemble_page`: the PDF's grid must
        // come out of the numbers pane rectangles were computed from.
        grid_css = meta.grid_metrics_css(),
        sections = sections.concat(),
    )
}

/// Renders a whole deck to a single HTML file.
/// `asset_dir` is the base directory for relative asset paths.
///
/// No diagram renderer: this is the one-call convenience, and finding one is
/// the host's job. `mirzam build` assembles a deck slide by slide through
/// [`render_slide_html`] instead, and passes the renderer it found there.
pub fn render_deck(meta: &DeckMeta, slides: &[SlideSource], asset_dir: &Path) -> RenderResult {
    let mut warnings = Vec::new();
    warnings.extend(theme_warnings(meta));
    warnings.extend(mode_warning(meta.mode.as_deref()));
    if let Err(w) = meta.math_dialect() {
        warnings.push(w);
    }
    warnings.extend(meta.grid_metrics().1);
    let ctx = DeckContext::new(meta, slides.len());
    warnings.extend(ctx.warnings());
    let mut sections = Vec::with_capacity(slides.len());
    for (i, slide) in slides.iter().enumerate() {
        let rendered = render_slide_html(slide, i, asset_dir, None, &ctx);
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
    host: Host<'_>,
    chart_files: &mut Vec<std::path::PathBuf>,
    ctx: &DeckContext,
) -> String {
    let mut errors: Vec<String> = Vec::new();

    // A palette this slide, or one pane of it, asks for. Checked here rather
    // than where the attribute is written, so every unknown name is reported
    // once, from the one place that knows which slide it is on; rendering then
    // silently keeps what it inherits, exactly as the deck's own `theme:` does.
    warnings.extend(theme::scope_warnings(
        &format!("slide {}", index + 1),
        slide.theme.as_deref(),
        slide.mode.as_deref(),
        &ctx.file_themes,
    ));
    for pb in &slide.panes {
        let attrs = parse_attrs(&pb.attrs);
        warnings.extend(theme::scope_warnings(
            &format!("slide {}, pane `{}`", index + 1, pb.name),
            attrs.kv.get("theme").map(String::as_str),
            attrs.kv.get("mode").map(String::as_str),
            &ctx.file_themes,
        ));
    }
    let slide_theme = theme::scope_attrs(
        slide.theme.as_deref(),
        slide.mode.as_deref(),
        &ctx.file_themes,
    );

    // Resolve the layout: the slide's own drawing, else the master it names,
    // else the deck's default master, else a single pane.
    let grid: Option<GridSpec> = match resolve_layout(slide, index, ctx, warnings) {
        Some((src, from)) => match parse_grid(src) {
            Ok(g) => Some(g),
            Err(e) => {
                errors.push(match from {
                    Some(name) => format!("slide {}: master `{name}`: {e}", index + 1),
                    None => format!("slide {}: {e}", index + 1),
                });
                None
            }
        },
        None => None,
    };

    // Pane-anchored shape blocks surface here: each pane's sources with the
    // frame — the pane's rectangle — its percentages map into.
    let mut pane_shapes: Vec<(mirzam_shape::Frame, String)> = Vec::new();
    let mut body = match &grid {
        Some(g) => render_grid_slide(
            g,
            slide,
            index,
            &mut errors,
            warnings,
            host,
            chart_files,
            ctx,
            &mut pane_shapes,
        ),
        None => render_single_pane_slide(
            slide,
            index,
            &mut errors,
            warnings,
            host,
            chart_files,
            ctx,
            &mut pane_shapes,
        ),
    };

    // A `[^key]` with no definition on this slide is left as literal text by
    // comrak rather than becoming a link - say so, once per key.
    warn_unresolved_footnotes(index, &body, warnings);
    warn_unrendered_spans(index, &body, warnings);
    warn_wide_braces(index, &body, warnings);

    // shape blocks become one static SVG layer, scaling with the slide.
    // Top-level blocks draw in page coordinates; blocks written inside a
    // `::: pane` draw in that pane's rectangle. Ids resolve across the whole
    // layer, so an arrow in one block may end on a shape another one drew.
    let mut shapes_html = String::new();
    let mut shape_docs: Vec<(mirzam_shape::ShapeDoc, mirzam_shape::Frame)> = Vec::new();
    if !slide.shapes.is_empty() {
        let src = slide.shapes.join("\n");
        let (w, h) = ctx.slide_size;
        shape_docs.push((
            mirzam_shape::parse_shapes(&src),
            mirzam_shape::Frame::page(w, h),
        ));
    }
    for (frame, src) in &pane_shapes {
        shape_docs.push((mirzam_shape::parse_shapes(src), *frame));
    }
    if !shape_docs.is_empty() {
        let blocks: Vec<(&mirzam_shape::ShapeDoc, mirzam_shape::Frame)> =
            shape_docs.iter().map(|(d, f)| (d, *f)).collect();
        let (svg, shape_errors) =
            mirzam_shape::render_layer(&blocks, ctx.slide_size.0, ctx.slide_size.1);
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
        // An endpoint that matches nothing draws no arrow; only the
        // browser-side checker used to notice. Same warning rule as anim/
        // annotate - the connector is still emitted exactly as written.
        // Marks an `annotate` block on this slide will draw (an id it names
        // with `id=`) resolve too, even though nothing in the static HTML
        // shows them yet - the viewer draws the overlay only once it lays
        // the slide out.
        let annot_ids: Vec<String> = slide
            .annots
            .iter()
            .flat_map(|src| mirzam_annot::parse(src).items)
            .filter_map(|item| item.id)
            .collect();
        connect::validate(
            index,
            &doc,
            &format!("{body}{shapes_html}"),
            &annot_ids,
            warnings,
        );
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

    let chrome_html = chrome_html(slide, index, ctx, warnings);

    format!(
        "<section class=\"slide\" data-index=\"{index}\"{slide_theme}{connect_attr}>\n{error_html}{body}{chrome_html}{shapes_html}{anim_html}{annot_html}{effects_html}{notes_html}</section>\n"
    )
}

/// The ASCII grid this slide is laid out on, with the name of the master it
/// came from when it came from one.
///
/// A slide that draws its own `pane` block keeps it, always: a master is what
/// a slide falls back to, never something that overrides what the author drew
/// in front of them. Below that, `<!-- layout: -->` beats the deck's `layout:`,
/// the same inwards-wins order a theme resolves in.
///
/// An unknown name on a slide is a warning, and the slide keeps what it would
/// have had without the name — its deck's default. That is the rule an unknown
/// `theme=` already follows: a deck never fails to build over a name, and the
/// element keeps what it inherited.
fn resolve_layout<'a>(
    slide: &'a SlideSource,
    index: usize,
    ctx: &'a DeckContext,
    warnings: &mut Vec<String>,
) -> Option<(&'a str, Option<&'a str>)> {
    if let Some(own) = &slide.layout {
        return Some((own.as_str(), None));
    }
    if let Some(named) = slide.layout_name.as_deref().map(str::trim) {
        // `<!-- layout: none -->`: a slide opting out of a deck-wide master,
        // which is the only way a title slide gets the whole surface back.
        if named == "none" {
            return None;
        }
        match ctx.masters.get_key_value(named) {
            Some((name, art)) => return Some((art.as_str(), Some(name.as_str()))),
            // Silent when the masters file itself could not be read: that is
            // reported once for the deck, and repeating it per slide would
            // bury every other warning in a deck of forty.
            None if ctx.masters_unavailable => {}
            None => warnings.push(format!(
                "slide {}: no master named `{named}`{}",
                index + 1,
                ctx.known_masters()
            )),
        }
    }
    // The deck's default. An unknown name here was reported once by
    // `DeckContext::warnings`, so this only has to survive it.
    let name = ctx.layout.as_deref().map(str::trim)?;
    let (name, art) = ctx.masters.get_key_value(name)?;
    Some((art.as_str(), Some(name.as_str())))
}

/// The footer and slide number drawn along the bottom of every slide.
///
/// Both spans are emitted whenever the deck asks for either, so the number
/// stays against the right edge on a deck that declares no footer.
fn chrome_html(
    slide: &SlideSource,
    index: usize,
    ctx: &DeckContext,
    warnings: &mut Vec<String>,
) -> String {
    if !ctx.has_chrome() {
        return String::new();
    }
    match slide.chrome.as_deref().map(str::trim) {
        None => {}
        Some("none") => return String::new(),
        Some(other) => warnings.push(format!(
            "slide {}: unknown chrome value `{other}`; `none` is the only one, \
             and the slide keeps the deck's footer",
            index + 1
        )),
    }
    let fill = |t: &Option<String>| match t {
        Some(t) => inline::html_escape(
            &t.replace("{n}", &(index + 1).to_string())
                .replace("{total}", &ctx.total.to_string()),
        ),
        None => String::new(),
    };
    format!(
        "<div class=\"mz-slide-chrome\"><span class=\"mz-footer\">{}</span>\
         <span class=\"mz-slide-number\">{}</span></div>\n",
        fill(&ctx.footer),
        fill(&ctx.slide_number),
    )
}

/// Warns about every `[^key]` left as literal text after rendering: `comrak`
/// only turns a reference into a link when its `[^key]:` definition is in the
/// same source it rendered, so a definition elsewhere - another slide, or (in
/// a grid layout) another pane - leaves the bracket text sitting on the page.
/// Skips `<pre>`/`<code>`, which is how a deck shows the syntax itself as an
/// example without it being mistaken for a real, broken reference.
fn warn_unresolved_footnotes(index: usize, body: &str, warnings: &mut Vec<String>) {
    let text = strip_code_regions(body);
    let mut seen = std::collections::BTreeSet::new();
    for cap in footnote_ref_regex().captures_iter(&text) {
        let key = &cap[1];
        if seen.insert(key.to_string()) {
            warnings.push(format!(
                "slide {}: footnote reference `[^{key}]` has no definition on this slide",
                index + 1
            ));
        }
    }
}

/// `underbrace`/`overbrace` stop growing at about ten em.
///
/// The brace is a stretchy operator, assembled by the browser out of the
/// font's brace pieces — and Blink stops extending that assembly at roughly
/// ten em, then draws a brace *shorter* than the base and flush left. The
/// formula still renders, and it renders wrong: the last characters of the
/// base have nothing under them. Nothing downstream can see it either, since
/// the operator's layout box does not grow with the glyph it paints, so the
/// layout check measures a brace that fits while looking at one that does not.
///
/// So it is estimated here, from the base's own content, and deliberately
/// under-eager: a warning that cried wolf on ordinary formulas would be worse
/// than none. The remedy is to move the words into the label, where they have
/// no brace to outgrow.
fn warn_wide_braces(index: usize, body: &str, warnings: &mut Vec<String>) {
    /// Where Blink stops stretching, in em, read off a ladder of bases
    /// rendered at increasing widths: the painted brace is the same length at
    /// twelve, fourteen and eighteen characters of base.
    const CAP_EM: f32 = 8.0;
    let mut seen = std::collections::BTreeSet::new();
    for cap in brace_regex().captures_iter(body) {
        let base = strip_tags(&cap[2]);
        let em: f32 = base
            .chars()
            .filter(|c| !c.is_whitespace())
            // A CJK character is one em square; Latin and digits average
            // rather less, italic in a maths font.
            .map(|c| if (c as u32) > 0x2E80 { 1.0 } else { 0.6 })
            .sum();
        if em > CAP_EM && seen.insert(base.clone()) {
            warnings.push(format!(
                "slide {}: the brace over `{}` will stop short of it - a browser \
                 stops stretching one at about {CAP_EM:.0}em, and this base is wider. \
                 Put the words in the label instead of the base",
                index + 1,
                base.trim()
            ));
        }
    }
}

fn brace_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?s)<m(?:under|over) accent(?:under)?="true">(<mrow>)?(.*?)(</mrow>)?<mo>[⏟⏞]</mo>"#,
        )
        .expect("static regex")
    })
}

/// The text of an HTML fragment, tags removed.
fn strip_tags(html: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]*>").expect("static regex"))
        .replace_all(html, "")
        .into_owned()
}

/// An attribute span that did not become a span reaches the slide as literal
/// `[text]{.small}` — which is exactly what intentionally-plain Markdown looks
/// like, so nothing about the rendered page says whether it was meant. The one
/// way left to write one that does not work is splitting it over a line break,
/// and the layout checker cannot see it either: the box is the right size, it
/// just has punctuation in it.
fn warn_unrendered_spans(index: usize, body: &str, warnings: &mut Vec<String>) {
    let text = strip_code_regions(body);
    let mut seen = std::collections::BTreeSet::new();
    for cap in unrendered_span_regex().captures_iter(&text) {
        let whole = cap[0].split_whitespace().collect::<Vec<_>>().join(" ");
        if seen.insert(whole.clone()) {
            warnings.push(format!(
                "slide {}: `{whole}` is still on the slide as text - an \
                 attribute span has to be on one line",
                index + 1
            ));
        }
    }
}

fn unrendered_span_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Only a `#id`/`.class` block counts: `[a](b){c}` and prose in brackets
    // beside a set of braces are not somebody's span.
    RE.get_or_init(|| {
        Regex::new(r"(?s)\[[^\[\]]{1,200}\]\{\s*[.#][^{}<>]{0,100}\}").expect("static regex")
    })
}

fn strip_code_regions(html: &str) -> String {
    let no_pre = pre_regex().replace_all(html, "");
    code_regex().replace_all(&no_pre, "").into_owned()
}

fn pre_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<pre\b[^>]*>.*?</pre>").expect("static regex"))
}

fn code_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<code\b[^>]*>.*?</code>").expect("static regex"))
}

fn footnote_ref_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\^([A-Za-z0-9_-]+)\]").expect("static regex"))
}

// Out-channels (errors, warnings, chart files, pane shapes) are parameters by
// design, like the syntax crate's slide walker.
//
// `errors` and `warnings` are two channels rather than one because they end
// differently: an error is also drawn on the slide as a red `⚠` box, which is
// right for a `chart` block whose YAML does not parse and wrong for a
// `mermaid` fence on a machine with no renderer — that deck is correct and
// still has to be presentable.
#[allow(clippy::too_many_arguments)]
fn render_grid_slide(
    grid: &GridSpec,
    slide: &SlideSource,
    index: usize,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    host: Host<'_>,
    chart_files: &mut Vec<std::path::PathBuf>,
    ctx: &DeckContext,
    pane_shapes: &mut Vec<(mirzam_shape::Frame, String)>,
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
        let mut extra_cls = attrs
            .classes
            .iter()
            .map(|c| format!(" {c}"))
            .collect::<String>();
        // `fit=shrink` on the pane: keep the words and give up the type size,
        // rather than clipping. `fit: shrink` in frontmatter says the same for
        // every pane, and rides on `#deck` since a slide is rendered without
        // knowing the deck it belongs to.
        if attrs.kv.get("fit").map(String::as_str) == Some("shrink") {
            extra_cls.push_str(" mz-fit");
        }
        extra_cls.push_str(&bleed_edge_classes(&attrs, || grid.edges(name)));
        let content = toc::extract(&content, errors);
        let content = cite::extract(&content, errors);
        let content = if ctx.citations {
            cite::mark(&content)
        } else {
            content
        };
        let (content, chart_blocks) = charts::extract(&content);
        // A `mermaid` fence takes the chart path, not the shape path: the SVG
        // the host hands back carries its own `viewBox` and scales to the box
        // it lands in, so nothing about it belongs in the pane arithmetic
        // below. A fence the host could not draw is left in the Markdown and
        // becomes an ordinary code block a few lines further down.
        let (content, mermaid_blocks) = mermaid::extract(&content, index, host.diagrams, warnings);
        // A shape fence in a pane draws in the pane's coordinate space. The
        // pane's rectangle is computed here — the name is from the grid, so
        // the rect always resolves — and a `.bleed` pane's frame runs out to
        // the slide edges its box actually reaches, like its background does.
        let (content, shape_srcs) = shapes::extract(&content);
        if !shape_srcs.is_empty() {
            let (w, h) = (ctx.slide_size.0 as f64, ctx.slide_size.1 as f64);
            let m = ctx.grid;
            if let Some(mut r) = grid.pane_rect(name, w, h, m.pad_x, m.pad_y, m.gap) {
                if attrs.classes.iter().any(|c| c == "bleed") {
                    let e = grid.edges(name);
                    if e.left {
                        r.w += r.x;
                        r.x = 0.0;
                    }
                    if e.top {
                        r.h += r.y;
                        r.y = 0.0;
                    }
                    if e.right {
                        r.w = w - r.x;
                    }
                    if e.bottom {
                        r.h = h - r.y;
                    }
                }
                pane_shapes.push((
                    mirzam_shape::Frame {
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: r.h,
                    },
                    shape_srcs.join("\n"),
                ));
            }
        }
        let mut body = render_markdown(&preprocess_math(&content, ctx.math));
        if !chart_blocks.is_empty() {
            let (with_charts, files) =
                charts::render_charts_in(&body, &chart_blocks, index, host.assets, errors);
            body = with_charts;
            chart_files.extend(files);
        }
        if !mermaid_blocks.is_empty() {
            body = mermaid::render_in(&body, &mermaid_blocks);
        }
        let body = columns_wrap(body, &attrs, index, name, errors);
        let bg = background_layers(&attrs, errors);
        // `theme=`/`mode=` on the pane: the palette is set on this element, so
        // everything inside it reads the other theme's tokens. Unknown names
        // are dropped here and reported in `render_slide`.
        let pane_theme = theme::scope_attrs(
            attrs.kv.get("theme").map(String::as_str),
            attrs.kv.get("mode").map(String::as_str),
            &ctx.file_themes,
        );
        panes_html.push_str(&format!(
            "<div class=\"pane pane-{name}{extra_cls}{}\" data-pane=\"{name}\"{pane_theme} style=\"{style}\">{}{body}{}</div>\n",
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

/// Wraps a pane's content in a multi-column container when the pane asks for
/// one with `columns=N`.
///
/// The browser balances the content across the columns, so a list of short
/// items fills the pane's width instead of running down its left edge with the
/// right half empty. The columns are a wrapper *inside* the pane rather than a
/// style on it because `valign=` turns the pane itself into a flex container,
/// and a flex container ignores `column-count`.
fn columns_wrap(
    body: String,
    attrs: &inline::Attrs,
    index: usize,
    pane: &str,
    errors: &mut Vec<String>,
) -> String {
    let Some(v) = attrs.kv.get("columns") else {
        return body;
    };
    match v.parse::<u32>() {
        // One column is what a pane already is; saying so is not an error.
        Ok(1) => body,
        Ok(n @ 2..=6) => {
            format!("<div class=\"mz-columns\" style=\"column-count:{n}\">{body}</div>")
        }
        _ => {
            errors.push(format!(
                "slide {}: pane `{pane}`: columns={v} is not a column count (1-6)",
                index + 1
            ));
            body
        }
    }
}

/// The `mz-bleed-*` classes a `.bleed` pane needs, one per slide edge it
/// reaches.
///
/// `.bleed` used to be a statement about the whole slide: the grid dropped its
/// padding, which took the margin away from every *other* pane too, so a
/// photograph down one half left the words on the other half against the slide
/// edge. Naming the edges lets the pane run out on its own — the grid keeps its
/// margin, and the pane beside the photograph keeps it too.
///
/// The edges are computed lazily because most panes are not bleeding and the
/// grid walk is wasted on them.
fn bleed_edge_classes(attrs: &inline::Attrs, edges: impl FnOnce() -> Edges) -> String {
    if !attrs.classes.iter().any(|c| c == "bleed") {
        return String::new();
    }
    let e = edges();
    let mut cls = String::new();
    for (touches, name) in [(e.top, "t"), (e.right, "r"), (e.bottom, "b"), (e.left, "l")] {
        if touches {
            cls.push_str(&format!(" mz-bleed-{name}"));
        }
    }
    cls
}

/// The markup a pane needs for a background image: the image itself, an optional
/// scrim, and a wrapper that keeps content above both.
struct Background {
    pane_class: String,
    layers: String,
    close: String,
}

/// The image(s) a pane asks for: one for both modes, or one per mode.
///
/// `bg-light=` and `bg-dark=` each override `bg=` for their own mode, so
/// `bg=` alone still means "this picture, whatever the reader's mode", and
/// naming only one of the pair keeps `bg=` as the other mode's image.
fn background_sources(attrs: &inline::Attrs) -> Option<(String, String)> {
    let base = attrs.kv.get("bg");
    let light = attrs.kv.get("bg-light").or(base)?;
    let dark = attrs.kv.get("bg-dark").or(base)?;
    Some((light.clone(), dark.clone()))
}

/// Builds the background layers for `bg=` and its treatments.
///
/// The image is a real `<img>` rather than a CSS `background-image` so it goes
/// through the same asset inlining as any other image, keeping a deck a single
/// self-contained file. A `<picture>` would be the obvious way to offer a
/// second image for dark mode, but its `media` can only ask the operating
/// system - it cannot see the deck's own `data-mode`, so the picture would
/// stay behind when the reader pressed `D`. Two `<img>`s and a CSS rule follow
/// the deck instead.
fn background_layers(attrs: &inline::Attrs, errors: &mut Vec<String>) -> Background {
    let Some((light_src, dark_src)) = background_sources(attrs) else {
        // Half a pair is not a background. The other mode would show a bare
        // pane - and, since a background flips the text colour for a photo,
        // one with unreadable text on it. Say so rather than render that.
        for key in ["bg-light", "bg-dark"] {
            if attrs.kv.contains_key(key) {
                errors.push(format!(
                    "`{key}` needs `bg=` or the other mode's image alongside it; \
                     the pane is rendered without a background"
                ));
            }
        }
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

    // One image when both modes want the same one: a deck that never asked for
    // the feature carries exactly the markup - and exactly the bytes - it did
    // before, rather than the same photo inlined twice.
    let images = if light_src == dark_src {
        format!(
            "<img class=\"mz-bg\" src=\"{src}\" alt=\"\" style=\"{img_style}\">",
            src = escape_attr(&light_src)
        )
    } else {
        format!(
            "<img class=\"mz-bg mz-bg-light\" src=\"{light}\" alt=\"\" style=\"{img_style}\">\
             <img class=\"mz-bg mz-bg-dark\" src=\"{dark}\" alt=\"\" style=\"{img_style}\">",
            light = escape_attr(&light_src),
            dark = escape_attr(&dark_src),
        )
    };

    Background {
        pane_class: format!(" has-bg{text}"),
        layers: format!("{images}{overlays}<div class=\"mz-bg-content\">"),
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

#[allow(clippy::too_many_arguments)]
fn render_single_pane_slide(
    slide: &SlideSource,
    index: usize,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    host: Host<'_>,
    chart_files: &mut Vec<std::path::PathBuf>,
    ctx: &DeckContext,
    pane_shapes: &mut Vec<(mirzam_shape::Frame, String)>,
) -> String {
    let mut content = slide.loose.clone();
    // Without a layout, `::: pane` blocks are simply concatenated.
    for pb in &slide.panes {
        content.push('\n');
        content.push_str(&pb.body);
    }
    let content = toc::extract(&content, errors);
    let content = cite::extract(&content, errors);
    let content = if ctx.citations {
        cite::mark(&content)
    } else {
        content
    };
    let (content, chart_blocks) = charts::extract(&content);
    let (content, mermaid_blocks) = mermaid::extract(&content, index, host.diagrams, warnings);
    let (content, shape_srcs) = shapes::extract(&content);
    let mut body = render_markdown(&preprocess_math(&content, ctx.math));
    if !chart_blocks.is_empty() {
        let (with_charts, files) =
            charts::render_charts_in(&body, &chart_blocks, index, host.assets, errors);
        body = with_charts;
        chart_files.extend(files);
    }
    if !mermaid_blocks.is_empty() {
        body = mermaid::render_in(&body, &mermaid_blocks);
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
        .find(|a| background_sources(a).is_some())
        .or(all.first())
        .cloned()
        .unwrap_or_default();
    let mut extra_cls = attrs
        .classes
        .iter()
        .map(|c| format!(" {c}"))
        .collect::<String>();
    // The one pane covers the slide, so a bleed here reaches all four edges.
    extra_cls.push_str(&bleed_edge_classes(&attrs, Edges::all));
    // Like the background and the palette: the first block that asks for
    // columns speaks for the whole slide.
    let body = match all.iter().find(|a| a.kv.contains_key("columns")) {
        Some(a) => columns_wrap(body, a, index, "main", errors),
        None => body,
    };
    // A pane shape's frame is the content box — or the whole slide when the
    // pane bleeds, since a bleed on a one-pane slide reaches every edge.
    if !shape_srcs.is_empty() {
        let (w, h) = (ctx.slide_size.0 as f64, ctx.slide_size.1 as f64);
        let m = ctx.grid;
        let frame = if attrs.classes.iter().any(|c| c == "bleed") {
            mirzam_shape::Frame::page(ctx.slide_size.0, ctx.slide_size.1)
        } else {
            mirzam_shape::Frame {
                x: m.pad_x,
                y: m.pad_y,
                w: w - 2.0 * m.pad_x,
                h: h - 2.0 * m.pad_y,
            }
        };
        pane_shapes.push((frame, shape_srcs.join("\n")));
    }
    let bg = background_layers(&attrs, errors);
    // A slide with no layout has one pane, so a `theme=` on any of its blocks
    // is a statement about the whole slide; the first one that names a palette
    // wins, the way the first background does.
    let pane_theme = all
        .iter()
        .find_map(|a| {
            let attrs = theme::scope_attrs(
                a.kv.get("theme").map(String::as_str),
                a.kv.get("mode").map(String::as_str),
                &ctx.file_themes,
            );
            (!attrs.is_empty()).then_some(attrs)
        })
        .unwrap_or_default();
    format!(
        "<div class=\"grid\" style='grid-template-columns:1fr;grid-template-rows:1fr;grid-template-areas:\"main\"'>\n<div class=\"pane pane-main{extra_cls}{}\" data-pane=\"main\"{pane_theme} style=\"grid-area:main\">{}{body}{}</div>\n</div>\n",
        bg.pane_class, bg.layers, bg.close
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirzam_syntax::parse_slide;

    /// A diagram renderer that never runs anything, so the whole slide path
    /// through a `mermaid` fence is exercised on a machine with no `mmdc`.
    /// It records nothing and answers the same SVG every time; what is under
    /// test here is the *injection*, not Mermaid.
    struct FakeDiagrams;

    impl DiagramRenderer for FakeDiagrams {
        fn render(&self, source: &str) -> Result<String, String> {
            assert!(source.contains("flowchart"), "the fence body arrives whole");
            Ok(concat!(
                r#"<svg id="my-svg" width="400" height="120" viewBox="0 0 400 120""#,
                r#" xmlns="http://www.w3.org/2000/svg">"#,
                r##"<rect fill="#ECECFF" stroke="#9370DB"/>"##,
                r##"<text fill="#333">Ingest</text></svg>"##,
            )
            .to_string())
        }
    }

    const FENCE: &str = "```mermaid\nflowchart LR\n  a --> b\n```\n";

    /// The trait is the whole seam: give the renderer and the fence becomes a
    /// diagram, in a pane on a grid like anything else in a pane.
    #[test]
    fn an_injected_renderer_turns_a_mermaid_fence_into_inline_svg() {
        let slide = parse_slide(&format!(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n::: pane a\n{FENCE}:::\n\n\
             ::: pane b\nwords\n:::\n"
        ));
        let out = render_slide_html(
            &slide,
            0,
            Path::new("."),
            Some(&FakeDiagrams),
            &DeckContext::default(),
        );
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(
            out.html.contains("<div class=\"mz-mermaid\">"),
            "{}",
            out.html
        );
        assert!(
            out.html.contains("class=\"mz-mermaid-svg\""),
            "{}",
            out.html
        );
        // Sized by the pane, coloured by the deck, and named for this slide.
        assert!(!out.html.contains("width=\"400\""), "{}", out.html);
        assert!(out.html.contains("var(--mz-shape-fill,"), "{}", out.html);
        assert!(out.html.contains("id=\"mz-mermaid-1-1\""), "{}", out.html);
        // And it is a diagram, not a listing.
        assert!(!out.html.contains("language-mermaid"), "{}", out.html);
    }

    /// The same fence with no renderer: the code block a plain CommonMark
    /// parser would have made, and a warning saying that is what happened.
    #[test]
    fn without_a_renderer_a_mermaid_fence_is_a_code_block_and_a_warning() {
        let slide = parse_slide(FENCE);
        let out = render_slide_html(&slide, 0, Path::new("."), None, &DeckContext::default());
        assert!(
            out.html.contains("<code class=\"language-mermaid\">"),
            "{}",
            out.html
        );
        assert!(out.html.contains("flowchart LR"), "{}", out.html);
        // A warning, never an error: no red box goes on a deck that is correct.
        assert!(!out.html.contains("mz-error"), "{}", out.html);
        assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
        assert!(
            out.warnings[0].starts_with("slide 1: mermaid: "),
            "{:?}",
            out.warnings
        );
    }

    /// Two diagrams on one slide must not share a stylesheet scope, and
    /// `mmdc` gives every run the same root id.
    #[test]
    fn two_diagrams_on_a_slide_get_ids_of_their_own() {
        let slide = parse_slide(&format!("{FENCE}\ntext between\n\n{FENCE}"));
        let out = render_slide_html(
            &slide,
            3,
            Path::new("."),
            Some(&FakeDiagrams),
            &DeckContext::default(),
        );
        assert!(out.html.contains("id=\"mz-mermaid-4-1\""), "{}", out.html);
        assert!(out.html.contains("id=\"mz-mermaid-4-2\""), "{}", out.html);
        assert!(!out.html.contains("my-svg"), "{}", out.html);
    }

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

    /// `columns=N` wraps the pane's content in a multi-column container; a
    /// value that is not a sensible count keeps the content and reports it.
    #[test]
    fn a_pane_can_balance_its_content_across_columns() {
        let slide = parse_slide(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n\
             ::: pane a {columns=2}\n- one\n- two\n:::\n\n::: pane b\nplain\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out
            .html
            .contains("<div class=\"mz-columns\" style=\"column-count:2\">"));
        // The pane that did not ask keeps its content unwrapped.
        assert!(out
            .html
            .contains("data-pane=\"b\" style=\"grid-area:b\"><p>plain</p>"));

        // A slide with no layout: the first block that asks speaks for it.
        let slide = parse_slide("::: pane main {columns=3}\n- one\n- two\n- three\n:::\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out
            .html
            .contains("<div class=\"mz-columns\" style=\"column-count:3\">"));

        // Not a count: the content survives, the mistake is reported.
        let slide = parse_slide(
            "```pane\n+---+\n| a |\n+---+\n```\n\n::: pane a {columns=twelve}\nwords\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.html.contains("<div class=\"mz-columns\""));
        assert!(out.html.contains("words"));
        assert!(out
            .warnings
            .iter()
            .any(|w| w.contains("columns=twelve is not a column count")));
    }

    /// A pane in a theme of its own: the attribute lands on that pane and
    /// nowhere else, and the page grows the tokens it now needs. Without the
    /// second half the pane would ask for a palette the page never loaded and
    /// come out in the deck's.
    #[test]
    fn a_pane_can_be_rendered_in_another_theme() {
        let slide = parse_slide(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n\
             ::: pane a {theme=wuwei mode=dark}\nquiet\n:::\n\n::: pane b\nloud\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out
            .html
            .contains("data-pane=\"a\" data-theme=\"wuwei\" data-mode=\"dark\""));
        assert!(out.html.contains("data-pane=\"b\" style="));
        assert!(out.html.contains(":where([data-theme=\"wuwei\"])"));
        assert!(out.html.contains(":where([data-theme=\"mirzam\"])"));
        assert!(!out.html.contains(":where([data-theme=\"nord\"])"));
    }

    /// The same at slide scope, written as an HTML comment so a plain
    /// CommonMark reader shows nothing at all.
    #[test]
    fn a_slide_can_be_rendered_in_another_theme() {
        let slide = parse_slide("<!-- theme: nord -->\n\n# Cold\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out
            .html
            .contains("<section class=\"slide\" data-index=\"0\" data-theme=\"nord\">"));
        assert!(out.html.contains(":where([data-theme=\"nord\"])"));
        // The deck around it is untouched.
        assert!(out
            .html
            .contains("<html lang=\"en\" data-theme=\"mirzam\">"));
    }

    /// A typo names no palette, so the pane keeps the one it inherits — and
    /// the author is told, with the slide and the pane in the message.
    #[test]
    fn an_unknown_scoped_theme_warns_and_changes_nothing() {
        let slide = parse_slide(
            "```pane\n+---+\n| a |\n+---+\n```\n\n::: pane a {theme=nope}\nhello\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert_eq!(out.warnings.len(), 1);
        assert!(out.warnings[0].contains("slide 1, pane `a`"));
        assert!(out.warnings[0].contains("nope"));
        assert!(!out.html.contains("data-theme=\"nope\""));
        assert!(out.html.contains("data-pane=\"a\" style="));
    }

    /// `serve` patches one slide into a page whose `<head>` is older than the
    /// edit, so the preview carries every palette rather than only today's.
    #[test]
    fn all_themes_inlines_every_palette() {
        let opts = PageOptions {
            all_themes: true,
            ..Default::default()
        };
        let html = assemble_page(&DeckMeta::default(), &[], &opts);
        for name in THEME_NAMES {
            assert!(html.contains(&format!(":where([data-theme=\"{name}\"])")));
        }
    }

    /// Every page-level setting a host can edit has to move the fingerprint,
    /// or the host patches slides into a page that still carries the old one.
    /// Checked against the assembled page rather than against a list of fields,
    /// so a setting added to `assemble_page` and forgotten here fails loudly.
    #[test]
    fn page_fingerprint_moves_with_every_page_level_setting() {
        let sections = vec!["<section class=\"slide\" data-index=\"0\">x</section>".to_string()];
        let base = DeckMeta::default();
        let opts = PageOptions::default();
        let key = page_fingerprint(&base, &sections, &opts);
        let page = assemble_page(&base, &sections, &opts);

        let variants: Vec<(&str, DeckMeta)> = vec![
            (
                "title",
                DeckMeta {
                    title: Some("T".into()),
                    ..Default::default()
                },
            ),
            (
                "theme",
                DeckMeta {
                    theme: mirzam_core::ThemeSpec::One("nord".into()),
                    ..Default::default()
                },
            ),
            (
                "mode",
                DeckMeta {
                    mode: Some("dark".into()),
                    ..Default::default()
                },
            ),
            (
                "aspect",
                DeckMeta {
                    aspect: Some("4:3".into()),
                    ..Default::default()
                },
            ),
            (
                "transition",
                DeckMeta {
                    transition: Some("fade".into()),
                    ..Default::default()
                },
            ),
            (
                "fit",
                DeckMeta {
                    fit: Some("shrink".into()),
                    ..Default::default()
                },
            ),
        ];
        for (what, meta) in variants {
            assert_ne!(
                assemble_page(&meta, &sections, &opts),
                page,
                "{what}: the test deck does not actually change the page"
            );
            assert_ne!(
                page_fingerprint(&meta, &sections, &opts),
                key,
                "{what} changes the page but not the fingerprint, so a host \
                 patching slides would keep showing the old one"
            );
        }

        // The same for what a slide, rather than the frontmatter, brings to the
        // page: a palette the `<head>` has to grow tokens for.
        let themed = vec!["<section class=\"slide\" data-theme=\"nord\">x</section>".to_string()];
        assert_ne!(page_fingerprint(&base, &themed, &opts), key);

        // And for the options the host itself supplies.
        let css = PageOptions {
            file_themes: vec![FileTheme::new(
                "acme.css",
                ":root { --mz-accent1: red }".to_string(),
            )],
            ..Default::default()
        };
        assert_ne!(page_fingerprint(&base, &sections, &css), key);
    }

    /// The counterpart: editing a slide is not a page change. A fingerprint
    /// that moved with the slides would make every keystroke a full reload.
    #[test]
    fn page_fingerprint_ignores_ordinary_slide_edits() {
        let meta = DeckMeta::default();
        let opts = PageOptions::default();
        let before = vec!["<section class=\"slide\">hello</section>".to_string()];
        let after = vec!["<section class=\"slide\">hello, world</section>".to_string()];
        assert_eq!(
            page_fingerprint(&meta, &before, &opts),
            page_fingerprint(&meta, &after, &opts)
        );
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
        assert!(html.contains("<html lang=\"en\" data-theme=\"mirzam\" class=\"mz-debug\">"));
    }

    #[test]
    fn debug_layout_off_by_default() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        assert!(html.contains("<html lang=\"en\" data-theme=\"mirzam\">"));
        assert!(!html.contains("class=\"mz-debug\""));
    }

    #[test]
    fn named_theme_is_baked_onto_html() {
        let meta = DeckMeta {
            theme: mirzam_core::ThemeSpec::One("nord".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(html.contains("data-theme=\"nord\""));
        assert!(html.contains("--mz-bg: #2e3440"));
    }

    #[test]
    fn unknown_theme_falls_back_to_the_default_palette_silently_in_assemble_page() {
        let meta = DeckMeta {
            theme: mirzam_core::ThemeSpec::One("does-not-exist".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(html.contains("data-theme=\"mirzam\""));
    }

    /// A deck built without `--embed-source` is the file it always was: no
    /// payload, and no control offering to open one.
    #[test]
    fn a_page_carries_no_source_unless_it_was_asked_for() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        // The names also appear in the viewer script, which every deck
        // carries; what a deck without source must not have is the tag and
        // the button themselves.
        assert!(!html.contains("<script type=\"application/json\" id=\"mz-source\""));
        assert!(!html.contains("<button id=\"mz-source-btn\""), "{html}");
    }

    #[test]
    fn embedded_source_arrives_with_a_control_that_opens_it() {
        let opts = PageOptions {
            source: Some(DeckSource {
                doc: "# One\n".into(),
                starts: vec![0],
                section_slides: vec![0],
                editor_url: Some("../../try/".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let html = assemble_page(&DeckMeta::default(), &[], &opts);
        assert!(html.contains("<script type=\"application/json\" id=\"mz-source\""));
        assert!(html.contains(r##""doc":"# One\n""##), "{html}");
        assert!(html.contains("<button id=\"mz-source-btn\""), "{html}");
    }

    /// The files the deck read by name go with a handed-over slide, so
    /// `theme:` and `bibliography:` resolve in the editor as they did here.
    #[test]
    fn embedded_source_carries_the_files_the_deck_read() {
        let opts = PageOptions {
            source: Some(DeckSource {
                doc: "# One\n".into(),
                starts: vec![0],
                section_slides: vec![0],
                files: vec![("themes/acme.css".into(), ":root { --mz-bg: #fff }".into())],
                ..Default::default()
            }),
            ..Default::default()
        };
        let html = assemble_page(&DeckMeta::default(), &[], &opts);
        assert!(
            html.contains(r#""themes/acme.css":":root { --mz-bg: #fff }""#),
            "{html}"
        );
    }

    /// Two decks that differ only in the source they carry are two different
    /// pages, and `serve` has to rebuild rather than patch slides into the old
    /// one.
    #[test]
    fn the_page_fingerprint_notices_embedded_source() {
        let meta = DeckMeta::default();
        let bare = PageOptions::default();
        let with = PageOptions {
            source: Some(DeckSource {
                doc: "# One\n".into(),
                starts: vec![0],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_ne!(
            page_fingerprint(&meta, &[], &bare),
            page_fingerprint(&meta, &[], &with)
        );
    }

    #[test]
    fn unknown_theme_is_reported_through_render_deck_warnings() {
        let meta = DeckMeta {
            theme: mirzam_core::ThemeSpec::One("does-not-exist".into()),
            ..Default::default()
        };
        let out = render_deck(&meta, &[], Path::new("."));
        assert!(out.warnings.iter().any(|w| w.contains("does-not-exist")));
    }

    /// A deck that names only a theme of its own wears it: the stem goes on
    /// `<body>`, inside the `<html>` that carries the built-in tokens the
    /// shared stylesheet reads. A deck that also names a built-in has said
    /// which palette it is in, and only its panes can ask for the other.
    #[test]
    fn a_deck_naming_only_its_own_theme_wears_it() {
        let acme = FileTheme::new(
            "themes/acme.css",
            "[data-theme=\"acme\"] { --mz-accent1: #6557d9; }".to_string(),
        );
        let opts = |themes: Vec<FileTheme>| PageOptions {
            file_themes: themes,
            ..Default::default()
        };
        let meta = mirzam_core::DeckMeta {
            theme: mirzam_core::ThemeSpec::One("themes/acme.css".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &opts(vec![acme.clone()]));
        assert!(html.contains("<html lang=\"en\" data-theme=\"mirzam\""));
        assert!(html.contains("<body data-theme=\"acme\">"), "{html}");

        // Named beside a built-in, it is loaded but not worn.
        let with_builtin = mirzam_core::DeckMeta {
            theme: mirzam_core::ThemeSpec::Many(vec!["nord".into(), "themes/acme.css".into()]),
            ..Default::default()
        };
        let html = assemble_page(&with_builtin, &[], &opts(vec![acme]));
        assert!(html.contains("<html lang=\"en\" data-theme=\"nord\""));
        assert!(html.contains("<body>"), "{html}");

        // And a stylesheet written at `:root` needs no attribute: it already
        // sets its tokens on the document.
        let loose = FileTheme::new("acme.css", ":root { --mz-accent1: #6557d9; }".to_string());
        let html = assemble_page(&meta, &[], &opts(vec![loose]));
        assert!(html.contains("<body>"), "{html}");

        // The print page wears it too. A PDF has no viewer to re-theme it
        // afterwards, so a deck exported without its own theme is a deck
        // nobody can fix from the file they were sent.
        let acme = FileTheme::new(
            "themes/acme.css",
            "[data-theme=\"acme\"] { --mz-accent1: #6557d9; }".to_string(),
        );
        let print = assemble_print_page(&meta, &[], std::slice::from_ref(&acme));
        assert!(print.contains("<body data-theme=\"acme\">"), "{print}");
        assert!(print.contains("--mz-accent1: #6557d9"), "{print}");
    }

    /// A theme somebody wrote is a scope like a built-in, so it opens with the
    /// same reset — and it is the one case the author of a built-in cannot fix
    /// for them. Their own declarations come after it and still win.
    #[test]
    fn a_file_theme_scoped_to_its_stem_starts_from_the_same_defaults() {
        let acme = FileTheme::new(
            "themes/acme.css",
            "[data-theme=\"acme\"] { --mz-h3-color: #6557d9; }".to_string(),
        );
        let meta = mirzam_core::DeckMeta::default();
        let html = assemble_page(
            &meta,
            &[],
            &PageOptions {
                file_themes: vec![acme],
                ..Default::default()
            },
        );
        let reset = html
            .find(":where([data-theme=\"acme\"]) {")
            .expect("acme's reset block");
        let own = html
            .find("[data-theme=\"acme\"] { --mz-h3-color: #6557d9; }")
            .expect("acme's own block");
        assert!(
            reset < own,
            "the reset must not overwrite the theme's values"
        );

        // A file that sets its tokens on `:root` registers a name nothing on
        // the page carries, so a block for that name would answer to no
        // element — and the diagnostic already tells the author to scope it.
        let loose = FileTheme::new("themes/loose.css", ":root { --mz-h3-color: #6557d9; }");
        let html = assemble_page(
            &meta,
            &[],
            &PageOptions {
                file_themes: vec![loose],
                ..Default::default()
            },
        );
        assert!(!html.contains(":where([data-theme=\"loose\"])"), "{html}");
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
            theme: mirzam_core::ThemeSpec::One("solarized".into()),
            mode: Some("dark".into()),
            ..Default::default()
        };
        let html = assemble_print_page(&meta, &[], &[]);
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

    /// Autoplay rides on the viewer itself — advancing is what the viewer
    /// already does — so the attribute appears without pulling in the
    /// animation runtime, and an unusable value leaves a deck driven by hand.
    #[test]
    fn autoplay_is_an_attribute_and_nothing_more() {
        let meta = DeckMeta {
            autoplay: Some("8s loop".into()),
            ..Default::default()
        };
        let html = assemble_page(&meta, &[], &PageOptions::default());
        assert!(html.contains(r#"data-autoplay="{&quot;loop&quot;:true,&quot;ms&quot;:8000}""#));
        assert!(!html.contains("window.MZAnim = {"));

        let bad = DeckMeta {
            autoplay: Some("whenever".into()),
            ..Default::default()
        };
        let html = assemble_page(&bad, &[], &PageOptions::default());
        assert!(!html.contains("data-autoplay"));
        assert!(
            !assemble_page(&DeckMeta::default(), &[], &PageOptions::default())
                .contains("data-autoplay")
        );
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
        let html = assemble_print_page(&meta, &sections, &[]);
        assert!(!html.contains("window.MZAnim = {"));
        assert!(!html.contains("data-transition"));
        // A printed page has no second window and no key to press.
        assert!(!html.contains("window.MZPresenter"));
    }

    /// The presenter window is the same file opened with a flag, so the viewer
    /// always carries the script that reads that flag.
    #[test]
    fn the_viewer_carries_the_presenter_window() {
        let html = assemble_page(&DeckMeta::default(), &[], &PageOptions::default());
        assert!(html.contains("window.MZPresenter"));
        assert!(html.contains("presenter=1") || html.contains("'presenter'"));
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
        assert!(!assemble_print_page(&DeckMeta::default(), &[], &[]).contains(ANNOT_MARKER));
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
        let html = assemble_print_page(&DeckMeta::default(), &annotated_section(), &[]);
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
        let html = assemble_print_page(&DeckMeta::default(), &effects_section(), &[]);
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

    /// A shape block inside a `::: pane` used to be a warning and a literal
    /// code block; it is now the pane-anchored form — percentages of the
    /// pane's rectangle, drawn into the slide's one shape layer.
    #[test]
    fn a_shape_block_inside_a_pane_draws_in_the_panes_frame() {
        let slide = parse_slide(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n\
             ::: pane b\n```shape\nrect #r at(50%, 50%) size(50%, 20%)\n```\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(
            !out.warnings.iter().any(|w| w.contains("shape")),
            "{:?}",
            out.warnings
        );
        // Not a code block any more - a rect in the SVG layer.
        assert!(!out.html.contains("language-shape"));
        assert!(out.html.contains("<g id=\"r\">"));
        // Pane `b` is the right half: cols 1fr/1fr, pad 60, gap 20 →
        // b spans x 650..1220, so 50% of it centres at 935; 50% wide = 285.
        assert!(out.html.contains("x=\"792.5\""), "{}", out.html);
        assert!(out.html.contains("width=\"285.0\""), "{}", out.html);
    }

    /// The same drawing at slide top level maps into the page, as ever.
    #[test]
    fn a_shape_block_at_slide_top_level_draws_in_page_coordinates() {
        let slide = parse_slide("```shape\nrect #r at(10%, 10%) size(20%, 20%)\n```\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.warnings.iter().any(|w| w.contains("shape")));
        assert!(out.html.contains("width=\"256.0\""), "{}", out.html);
    }

    /// An arrow in a page block may end on a shape a pane block drew: ids
    /// resolve across the whole layer, whatever frame each block used.
    #[test]
    fn a_page_arrow_reaches_a_pane_anchored_shape() {
        let slide = parse_slide(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n\
             ::: pane b\n```shape\nrect #target at(50%, 50%) size(50%, 20%)\n```\n:::\n\n\
             ```shape\narrow from(10%, 50%) to(#target.w)\n```\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(
            !out.warnings.iter().any(|w| w.contains("no element")),
            "{:?}",
            out.warnings
        );
        // West edge of #target: centre 935 minus half of 285 = 792.5.
        assert!(out.html.contains("x2=\"792.5\""), "{}", out.html);
    }

    /// Declared grid metrics move the frame: the same pane, a wider margin.
    #[test]
    fn grid_metrics_from_frontmatter_move_a_pane_frame() {
        let slide = parse_slide(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n\
             ::: pane a\n```shape\nrect #r at(0%, 0%) size(10%, 10%)\n```\n:::\n",
        );
        let meta = mirzam_core::parse_meta("grid-pad-x: 100px\n").unwrap();
        let out = render_deck(&meta, &[slide], Path::new("."));
        // Pane `a` starts at the horizontal margin: x = 100, so a rect centred
        // there starts at 100 - half its width (10% of the 530px pane = 53).
        assert!(out.html.contains("x=\"73.5\""), "{}", out.html);
        // The declared value reaches the page CSS too, so the browser lays the
        // grid out from the same number.
        assert!(out.html.contains("--mz-grid-pad-x:100px"), "{}", out.html);
    }

    #[test]
    fn a_shape_fence_quoted_inside_a_longer_fence_stays_markdown() {
        // A four-backtick fence quotes the block as an example, the same rule
        // the top-level parser applies - so this must not be mistaken for the
        // real thing.
        let slide = parse_slide(
            "::: pane main\n````markdown\n```shape\nrect #r at(0,0) size(1,1)\n```\n````\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.warnings.iter().any(|w| w.contains("shape")));
        // The example is a `markdown` code block with the fence as its text,
        // not an SVG layer.
        assert!(out.html.contains("language-markdown"), "{}", out.html);
        assert!(!out.html.contains("<g id=\"r\">"), "{}", out.html);
    }

    /// The brace is a stretchy operator the browser stops extending, and its
    /// layout box does not grow with the glyph it paints — so nothing
    /// downstream can catch this, and the slide shows a base with its last
    /// characters hanging over nothing.
    #[test]
    fn a_brace_wider_than_the_browser_will_draw_warns() {
        let meta = DeckMeta {
            math: Some("typst".into()),
            ..DeckMeta::default()
        };
        let long = parse_slide("$$underbrace(m m m m m m m m m m m m m m, \"label\")$$\n");
        let out = render_deck(&meta, &[long], Path::new("."));
        assert!(
            out.warnings.iter().any(|w| w.contains("stop short")),
            "{:?}",
            out.warnings
        );
        // A base the browser can draw, and the same words moved into the
        // label where they belong, both stay quiet.
        for src in [
            "$$underbrace(a + b, \"total\")$$\n",
            "$$underbrace(P, \"a rather long label indeed, many characters\")$$\n",
        ] {
            let out = render_deck(&meta, &[parse_slide(src)], Path::new("."));
            assert!(
                !out.warnings.iter().any(|w| w.contains("stop short")),
                "{src}: {:?}",
                out.warnings
            );
        }
    }

    #[test]
    fn an_undefined_footnote_reference_warns() {
        let slide = parse_slide("A claim[^missing].\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("[^missing]") && w.contains("slide 1")),
            "{:?}",
            out.warnings
        );
        // Degradation is unchanged: the bracket text is still exactly what
        // a plain Markdown reader (and this reader) shows.
        assert!(out.html.contains("[^missing]"));
    }

    #[test]
    fn a_footnote_defined_on_the_same_slide_does_not_warn() {
        let slide = parse_slide("A claim[^a].\n\n[^a]: The source.\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.warnings.iter().any(|w| w.contains("footnote")));
        assert!(out.html.contains("footnote-ref"));
    }

    #[test]
    fn a_footnote_reference_quoted_in_a_code_block_does_not_warn() {
        // Showing the syntax itself as an example must not be mistaken for a
        // real, broken reference.
        let slide = parse_slide("```markdown\nSee[^x].\n```\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(!out.warnings.iter().any(|w| w.contains("footnote")));
    }

    #[test]
    fn repeating_the_same_missing_key_warns_once() {
        let slide = parse_slide("First[^a] and again[^a].\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert_eq!(
            out.warnings.iter().filter(|w| w.contains("[^a]")).count(),
            1,
            "{:?}",
            out.warnings
        );
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
    fn per_mode_backgrounds_ship_both_images() {
        let slide = parse_slide(
            "```pane\n+-----+\n|     |\n| hero|\n|     |\n+-----+\n```\n\n::: pane hero {bg-light=day.jpg bg-dark=night.jpg}\nTitle\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(out.html.contains("class=\"mz-bg mz-bg-light\""));
        assert!(out.html.contains("class=\"mz-bg mz-bg-dark\""));
        assert!(out.html.contains("has-bg"));
    }

    /// Naming one mode leaves `bg=` as the other's, rather than dropping it.
    /// Checked on the sources rather than the HTML, where inlining has already
    /// replaced both paths with data URIs.
    #[test]
    fn a_named_mode_falls_back_to_bg_for_the_other() {
        let sources = |attrs: &str| background_sources(&parse_attrs(attrs));
        assert_eq!(
            sources("bg=day.jpg bg-dark=night.jpg"),
            Some(("day.jpg".into(), "night.jpg".into()))
        );
        assert_eq!(
            sources("bg=day.jpg bg-light=dawn.jpg"),
            Some(("dawn.jpg".into(), "day.jpg".into()))
        );
        assert_eq!(
            sources("bg-light=dawn.jpg bg-dark=night.jpg"),
            Some(("dawn.jpg".into(), "night.jpg".into()))
        );
        // Half a pair is not a background: with nothing to show in the other
        // mode, the pane would go blank there.
        assert_eq!(sources("bg-dark=night.jpg"), None);
        assert_eq!(sources("align=center"), None);
    }

    /// One image for both modes stays one `<img>`: naming the same file twice
    /// would inline the same photo twice and double that slide's weight.
    #[test]
    fn one_image_for_both_modes_is_not_duplicated() {
        let slide = parse_slide(
            "```pane\n+-----+\n|     |\n| hero|\n|     |\n+-----+\n```\n\n::: pane hero {bg=p.jpg}\nTitle\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        // Counted on the markup: the stylesheet in the same page names the
        // per-mode classes whether or not any slide uses them.
        assert_eq!(out.html.matches("class=\"mz-bg\"").count(), 1);
        assert!(!out.html.contains("class=\"mz-bg mz-bg-light\""));
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

    /// A printed recording keeps everything a page can carry — the sleeve and
    /// the title — and loses only the transport, which is the one part of it a
    /// page cannot offer.
    #[test]
    fn print_keeps_a_recordings_sleeve_and_label() {
        let card = "<div class=\"mz-audio mz-audio-card\">\
                    <img class=\"mz-audio-art\" src=\"art.png\" alt=\"\">\
                    <span class=\"mz-audio-body\">\
                    <span class=\"mz-audio-label\">Interview</span>\
                    <audio src=\"talk.mp3\" title=\"Interview\" controls></audio>\
                    </span></div>";
        let out = videos_to_stills(card);
        assert!(
            out.contains("<img class=\"mz-audio-art\" src=\"art.png\""),
            "{out}"
        );
        assert!(out.contains("Interview"), "{out}");
        assert!(!out.contains("<audio"), "{out}");
    }

    /// A widget is the one thing on a slide that cannot be printed at all: it
    /// is a document that only exists while something is running it. Paper
    /// gets its label, in the same box a clip that cannot play gets.
    #[test]
    fn print_replaces_a_widget_with_its_label() {
        let widget = "<div class=\"mz-embed mz-html\" data-title=\"Damped oscillation\">\
                      <iframe src=\"data:text/html;charset=utf-8;base64,PGI+\"></iframe>\
                      <button type=\"button\" class=\"mz-expand\">⛶</button></div>";
        let out = videos_to_stills(widget);
        assert!(out.contains("Damped oscillation"), "{out}");
        assert!(!out.contains("<iframe"), "{out}");
        assert!(!out.contains("<button"), "{out}");
    }

    #[test]
    fn slide_without_layout_is_single_pane() {
        let slide = parse_slide("# Hello {.title-slide}\n\nworld\n");
        let meta = DeckMeta::default();
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.html.contains("title-slide"));
        assert!(out.html.contains("world"));
    }

    /// A deck whose frontmatter defines masters, for the layout tests below.
    fn deck_with_masters() -> DeckMeta {
        deck_defining("two-up", "+--------+--------+\n| head            |\n+--------+--------+\n| main   | fig    |\n+--------+--------+\n")
    }

    /// A photograph down one half runs out on the three edges it reaches, and
    /// the pane beside it is left exactly as it was drawn — the whole point of
    /// naming the edges rather than stripping the grid's margin.
    #[test]
    fn a_bleeding_pane_names_only_the_edges_it_reaches() {
        let slide = parse_slide(
            "```pane\n+-------+-------+\n| photo | body  |\n+-------+-------+\n```\n\n\
             ::: pane photo {.bleed}\n:::\n\n::: pane body\nwords\n:::\n",
        );
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        let photo = out
            .html
            .lines()
            .find(|l| l.contains("pane-photo"))
            .expect("photo pane");
        assert!(photo.contains("mz-bleed-t"), "{photo}");
        assert!(photo.contains("mz-bleed-b"), "{photo}");
        assert!(photo.contains("mz-bleed-l"), "{photo}");
        assert!(!photo.contains("mz-bleed-r"), "{photo}");
        let body = out
            .html
            .lines()
            .find(|l| l.contains("pane-body"))
            .expect("body pane");
        assert!(!body.contains("mz-bleed"), "{body}");
    }

    /// A slide with no grid has one pane, and a bleed there covers everything.
    #[test]
    fn a_bleeding_slide_without_a_grid_reaches_every_edge() {
        let slide = parse_slide("::: pane hero {.bleed}\n# Title\n:::\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        for edge in ["mz-bleed-t", "mz-bleed-r", "mz-bleed-b", "mz-bleed-l"] {
            assert!(out.html.contains(edge), "{edge} missing: {}", out.html);
        }
    }

    /// A deck with one master written inline.
    fn deck_defining(name: &str, art: &str) -> DeckMeta {
        DeckMeta {
            masters: mirzam_core::Masters::Inline(
                [(name.to_string(), art.to_string())].into_iter().collect(),
            ),
            ..DeckMeta::default()
        }
    }

    #[test]
    fn a_slide_is_drawn_on_the_master_it_names() {
        let slide = parse_slide("<!-- layout: two-up -->\n\n::: pane fig\nright\n:::\n");
        let out = render_deck(&deck_with_masters(), &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(
            out.html.contains(r#""head main" "head fig""#) || out.html.contains("grid-area:fig")
        );
        assert!(out.html.contains("pane-fig"));
        assert!(out.html.contains("right"));
    }

    #[test]
    fn a_deck_wide_master_applies_to_every_slide_that_names_none() {
        let mut meta = deck_with_masters();
        meta.layout = Some("two-up".into());
        let slide = parse_slide("::: pane fig\nright\n:::\n");
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out.html.contains("pane-fig"));
    }

    /// A master is what a slide falls back to, never something that overrides
    /// the grid the author drew in front of them.
    #[test]
    fn a_slides_own_pane_block_beats_every_master() {
        let mut meta = deck_with_masters();
        meta.layout = Some("two-up".into());
        let slide = parse_slide(
            "<!-- layout: two-up -->\n\n```pane\n+-----+\n| solo |\n+-----+\n```\n\n::: pane solo\nmine\n:::\n",
        );
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.html.contains("pane-solo"));
        assert!(!out.html.contains("pane-fig"));
    }

    #[test]
    fn layout_none_opts_one_slide_out_of_the_deck_wide_master() {
        let mut meta = deck_with_masters();
        meta.layout = Some("two-up".into());
        let slide = parse_slide("<!-- layout: none -->\n\n# Title {.title-slide}\n");
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        // The single-pane fallback: one `main` area and nothing else.
        assert!(out.html.contains("grid-template-areas:\"main\""));
    }

    #[test]
    fn an_unknown_master_on_a_slide_warns_and_keeps_what_it_inherited() {
        let mut meta = deck_with_masters();
        meta.layout = Some("two-up".into());
        let slide = parse_slide("<!-- layout: three-up -->\n\n::: pane fig\nright\n:::\n");
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("slide 1") && w.contains("three-up") && w.contains("two-up")),
            "{:?}",
            out.warnings
        );
        // Inherited means the deck's own master, so the slide still lays out.
        assert!(out.html.contains("pane-fig"));
    }

    /// A deck assembled from `![[…]]` section files reads only the root's
    /// frontmatter, so a section naming a master the root never declared is
    /// the mistake this workflow makes. "No master named `two-up`" alone
    /// sends the author looking for a typo; the fix is a line of frontmatter
    /// in a different file.
    #[test]
    fn a_deck_that_defines_no_masters_at_all_says_so() {
        let slide = parse_slide("<!-- layout: two-up -->\n\ntext\n");
        let out = render_deck(&DeckMeta::default(), &[slide], Path::new("."));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("two-up") && w.contains("this deck defines none")),
            "{:?}",
            out.warnings
        );
    }

    /// Once for the deck, not once per slide: the same complaint on every
    /// slide of a long deck buries every other warning.
    #[test]
    fn an_unknown_deck_wide_master_is_reported_once() {
        let mut meta = deck_with_masters();
        meta.layout = Some("nope".into());
        let slides = [parse_slide("a\n"), parse_slide("b\n"), parse_slide("c\n")];
        let out = render_deck(&meta, &slides, Path::new("."));
        assert_eq!(
            out.warnings.iter().filter(|w| w.contains("nope")).count(),
            1,
            "{:?}",
            out.warnings
        );
    }

    #[test]
    fn a_master_whose_art_does_not_parse_names_itself_in_the_error() {
        let meta = deck_defining("broken", "| no borders |\n");
        let slide = parse_slide("<!-- layout: broken -->\n\ntext\n");
        let out = render_deck(&meta, &[slide], Path::new("."));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("master `broken`") && w.contains("slide 1")),
            "{:?}",
            out.warnings
        );
    }

    #[test]
    fn footer_and_slide_number_are_drawn_on_every_slide() {
        let meta = DeckMeta {
            footer: Some("Internal".into()),
            slide_number: Some("{n} / {total}".into()),
            ..DeckMeta::default()
        };
        let slides = [parse_slide("a\n"), parse_slide("b\n")];
        let out = render_deck(&meta, &slides, Path::new("."));
        assert!(out
            .html
            .contains("<span class=\"mz-footer\">Internal</span>"));
        assert!(out.html.contains("1 / 2"));
        assert!(out.html.contains("2 / 2"));
    }

    #[test]
    fn a_slide_can_drop_the_deck_chrome() {
        let meta = DeckMeta {
            slide_number: Some("{n}".into()),
            ..DeckMeta::default()
        };
        let slides = [
            parse_slide("<!-- chrome: none -->\n\n# Title\n"),
            parse_slide("b\n"),
        ];
        let out = render_deck(&meta, &slides, Path::new("."));
        // Counted on the markup, not the stylesheet, which names the class too.
        assert_eq!(
            out.html.matches("<div class=\"mz-slide-chrome\">").count(),
            1
        );
    }

    /// A page number that only exists on screen is the one place it is least
    /// needed. `print.css` hides the *viewer's* `#chrome` cluster, whose name
    /// is one letter from this element's — so this pins that the export keeps
    /// the deck's own.
    #[test]
    fn the_print_page_keeps_the_deck_chrome() {
        let meta = DeckMeta {
            slide_number: Some("{n}".into()),
            ..DeckMeta::default()
        };
        let section = render_slide_html(
            &parse_slide("a\n"),
            0,
            Path::new("."),
            None,
            &DeckContext::new(&meta, 1),
        );
        let page = assemble_print_page(&meta, &[section.html], &[]);
        assert!(page.contains("<div class=\"mz-slide-chrome\">"));
        assert!(!page.contains(".mz-slide-chrome { display: none"));
    }

    /// A deck that asks for neither carries neither: the element is not in the
    /// markup at all, so nothing changes for a deck built before it existed.
    #[test]
    fn a_deck_without_chrome_settings_emits_none() {
        let out = render_deck(&DeckMeta::default(), &[parse_slide("a\n")], Path::new("."));
        assert!(!out.html.contains("<div class=\"mz-slide-chrome\">"));
    }

    /// Footer text is escaped, not parsed: it is one line of chrome, and an
    /// author writing `Q3 <draft>` must not lose half of it to a tag.
    #[test]
    fn footer_text_is_escaped() {
        let meta = DeckMeta {
            footer: Some("Q3 <draft> & later".into()),
            ..DeckMeta::default()
        };
        let out = render_deck(&meta, &[parse_slide("a\n")], Path::new("."));
        assert!(out.html.contains("Q3 &lt;draft&gt; &amp; later"));
    }

    /// Adding a slide must not invalidate every other slide's cache entry, so
    /// the total only counts as a setting when something actually prints it.
    #[test]
    fn the_slide_total_only_changes_the_fingerprint_when_it_is_printed() {
        let mut meta = DeckMeta {
            footer: Some("Internal".into()),
            ..DeckMeta::default()
        };
        assert_eq!(
            DeckContext::new(&meta, 9).fingerprint(),
            DeckContext::new(&meta, 10).fingerprint()
        );
        meta.slide_number = Some("{n} / {total}".into());
        assert_ne!(
            DeckContext::new(&meta, 9).fingerprint(),
            DeckContext::new(&meta, 10).fingerprint()
        );
    }
}
