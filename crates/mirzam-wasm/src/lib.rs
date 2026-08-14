//! WebAssembly bindings for the Mirzam core.
//!
//! Exposes the *same* parsing, layout and rendering implementation the CLI uses
//! to browsers, VS Code webviews and Obsidian.
//! Since those hosts have no filesystem, transcluded files and assets are
//! supplied by the host as JSON tables.
//!
//! ```js
//! import init, { Renderer } from './mirzam_wasm.js';
//! await init();
//! const r = new Renderer();
//! r.set_files(JSON.stringify({ 'sections/a.md': '## Included\n' }));
//! r.set_assets(JSON.stringify({ 'img/x.svg': 'data:image/svg+xml;base64,...' }));
//! const html = r.render_page(source);                  // whole page
//! const changed = JSON.parse(r.render_changed(source)); // changed slides only
//! const slide = r.slide_at_offset(source, cursor, file); // where the cursor is
//! ```
//!
//! `render_changed` answers with `structural` when patching the changed slides
//! into the page is not enough — a host that ignores it shows a `theme:` edit
//! doing nothing at all.

// The structural math editor's back end: built, field-tested in the browser
// editor, and withdrawn — typing the Typst dialect beat editing it by touch.
// The logic lives on tested (the edit layer in `mirzam-tmath`, the JSON layer
// here under `cfg(test)`), but shipping WASM carries none of it unless the
// `math-editor` feature turns it back on.
#[cfg(any(test, feature = "math-editor"))]
mod math;

use mirzam_render::AssetSource;
use mirzam_syntax::FileProvider;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;

/// Resolves includes from the host-provided file table.
struct MapFiles<'a>(&'a BTreeMap<String, String>);

impl FileProvider for MapFiles<'_> {
    fn read(&self, path: &Path) -> Result<String, String> {
        let key = normalize(path);
        self.0
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("`{key}` was not provided by the host"))
    }
}

/// Host-provided asset table mapping paths to data URIs or URLs.
struct MapAssets<'a>(&'a BTreeMap<String, String>);

impl AssetSource for MapAssets<'_> {
    fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
        let key = normalize(Path::new(rel));
        let result = self
            .0
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("`{key}` was not provided by the host"));
        (result, None)
    }
}

/// Normalizes paths like `./a/../b.md` so they match the table keys.
fn normalize(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
        }
    }
    parts.join("/")
}

/// Result of rendering a whole page.
#[wasm_bindgen(getter_with_clone)]
pub struct RenderOutput {
    pub html: String,
    /// Warnings, as a JSON array.
    pub warnings: String,
    pub slide_count: usize,
}

/// Renderer with incremental support.
/// Keeps the previous slide output so it can return only what changed.
#[wasm_bindgen]
pub struct Renderer {
    files: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
    /// Slide HTML from the previous render, used to compute diffs.
    previous: RefCell<Vec<String>>,
    /// The page the previous render was assembled for. A host patches slides
    /// into a page it built earlier, so a `theme:` it cannot see change is a
    /// theme it never applies; this is what tells it to build the page again.
    previous_page: RefCell<Option<u64>>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Renderer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Renderer {
        Renderer {
            files: BTreeMap::new(),
            assets: BTreeMap::new(),
            previous: RefCell::new(Vec::new()),
            previous_page: RefCell::new(None),
        }
    }

    /// Sets the transclusion table (JSON: `{path: content}`).
    pub fn set_files(&mut self, json: &str) -> Result<(), JsError> {
        self.files = parse_table(json)?;
        Ok(())
    }

    /// Sets the media table (JSON: `{path: dataUriOrUrl}`).
    pub fn set_assets(&mut self, json: &str) -> Result<(), JsError> {
        self.assets = parse_table(json)?;
        Ok(())
    }

    /// Resets the diff baseline; the next call reports every slide as changed.
    pub fn reset(&self) {
        self.previous.borrow_mut().clear();
        *self.previous_page.borrow_mut() = None;
    }

    /// Renders a complete HTML page with the viewer.
    pub fn render_page(&self, source: &str) -> RenderOutput {
        let built = self.build(source);
        let opts = built.page_options();
        RenderOutput {
            html: mirzam_render::assemble_page(&built.meta, &built.sections, &opts),
            warnings: serde_json::to_string(&built.warnings).unwrap_or_else(|_| "[]".into()),
            slide_count: built.sections.len(),
        }
    }

    /// Renders a single slide as `<section>` HTML, for partial preview updates.
    pub fn render_slide(&self, source: &str, index: usize) -> Option<String> {
        self.build(source).sections.get(index).cloned()
    }

    /// Returns only the slides that changed since the last render.
    /// JSON: `{"count": n, "changes": [[index, html], ...], "structural": bool}`
    ///
    /// `structural` means the changed slides are not enough: the slide count
    /// moved, or something the page carries around them did — a theme, the
    /// aspect ratio, the title, a palette a slide newly asks for. Both cases
    /// need the page assembled again, and a host that only patched the changes
    /// would show an edit that visibly did nothing.
    pub fn render_changed(&self, source: &str) -> String {
        let built = self.build(source);
        let mut prev = self.previous.borrow_mut();
        let mut prev_page = self.previous_page.borrow_mut();
        let structural =
            prev.len() != built.sections.len() || *prev_page != Some(built.page_fingerprint);
        *prev_page = Some(built.page_fingerprint);
        let changes: Vec<(usize, &String)> = built
            .sections
            .iter()
            .enumerate()
            .filter(|(i, html)| prev.get(*i) != Some(html))
            .collect();
        let json = serde_json::json!({
            "count": built.sections.len(),
            "structural": structural,
            "changes": changes,
            "warnings": built.warnings,
        })
        .to_string();
        *prev = built.sections;
        json
    }

    /// Which slide (0-based) the byte at `offset` belongs to — the answer an
    /// editor needs to follow the cursor. `file` names which of the deck's
    /// files the offset is in, by the same key `set_files` uses; the deck's own
    /// source is `null` or the empty string.
    ///
    /// Counting `---` in the file being edited is only right for a deck that
    /// is one file. A deck split across files transcludes whole sections with
    /// `![[…]]`, and every slide inside one of them is a slide the root never
    /// wrote a rule for: counted that way, the preview lands earlier and
    /// earlier the further down the deck the cursor goes. So the count happens
    /// where the deck is actually assembled, on the expanded document, and the
    /// source map carries the cursor across — from a section file as readily
    /// as from the deck, since the map knows every file it read.
    ///
    /// `offset` is a byte offset into the whole file, frontmatter included; a
    /// cursor there, in a file this deck does not read, or past the end,
    /// resolves to the first slide.
    pub fn slide_at_offset(&self, source: &str, offset: usize, file: Option<String>) -> usize {
        let (fm, body) = mirzam_syntax::split_frontmatter(source);
        let meta = fm
            .and_then(|y| mirzam_core::parse_meta(y).ok())
            .unwrap_or_default();
        let root = Path::new("");
        let expanded = mirzam_syntax::expand_includes_mapped(
            body,
            source.len() - body.len(),
            root,
            root,
            &MapFiles(&self.files),
            &mut Default::default(),
        );
        // Split before variables are substituted: a rule is a rule either way,
        // and skipping the pass keeps every offset the map speaks for intact.
        let slides = mirzam_syntax::split_slides_spanned(&expanded.text, meta.split_level());
        // The map holds each file under the path the include was written with,
        // so `./sections/a.md` and `sections/a.md` are the same file said two
        // ways and both have to find it.
        let want = normalize(Path::new(file.as_deref().unwrap_or("")));
        let Some(in_file) = expanded
            .map
            .files()
            .iter()
            .find(|p| normalize(p) == want)
            .cloned()
        else {
            return 0;
        };
        let Some(at) = expanded.map.locate(&in_file, offset) else {
            return 0;
        };
        slides.iter().rposition(|s| s.start <= at).unwrap_or(0)
    }

    /// Parses only, returning a summary of the deck structure for outlines and diagnostics.
    /// JSON: `{"slides": [{"index", "panes", "hasShapes", "hasConnectors", "notes"}], "warnings": []}`
    pub fn outline(&self, source: &str) -> String {
        let (fm, body) = mirzam_syntax::split_frontmatter(source);
        let meta = fm
            .and_then(|y| mirzam_core::parse_meta(y).ok())
            .unwrap_or_default();
        let body = mirzam_syntax::expand_includes(body, Path::new(""), &MapFiles(&self.files));
        let vars = meta.var_table();
        let body = substitute_outside_fences(&body, &vars);
        let slides: Vec<serde_json::Value> =
            mirzam_syntax::split_slides_at(&body, meta.split_level())
                .iter()
                .enumerate()
                .map(|(i, src)| {
                    let s = mirzam_syntax::parse_slide(src);
                    serde_json::json!({
                        "index": i,
                        "panes": s.panes.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                        "hasShapes": !s.shapes.is_empty(),
                        "hasConnectors": !s.connects.is_empty(),
                        "notes": s.notes,
                    })
                })
                .collect();
        serde_json::json!({ "title": meta.title, "slides": slides }).to_string()
    }
}

struct Built {
    meta: mirzam_core::DeckMeta,
    sections: Vec<String>,
    warnings: Vec<String>,
    /// The stylesheets named by `theme:`, when the host supplied them.
    file_themes: Vec<mirzam_render::FileTheme>,
    /// Everything the assembled page carries around the slides.
    page_fingerprint: u64,
}

impl Built {
    fn page_options(&self) -> mirzam_render::PageOptions {
        page_options(self.file_themes.clone())
    }
}

/// The page settings this renderer has: the deck's own stylesheets, and nothing
/// a browser host can ask for. Assembling the page and fingerprinting it read
/// the same value, or an edit to a stylesheet is one the host never hears about.
fn page_options(file_themes: Vec<mirzam_render::FileTheme>) -> mirzam_render::PageOptions {
    mirzam_render::PageOptions {
        file_themes,
        ..Default::default()
    }
}

impl Renderer {
    fn build(&self, source: &str) -> Built {
        let (fm, body) = mirzam_syntax::split_frontmatter(source);
        let mut warnings = Vec::new();
        let meta = match fm {
            Some(yaml) => match mirzam_core::parse_meta(yaml) {
                Ok(m) => m,
                Err(e) => {
                    warnings.push(e);
                    mirzam_core::DeckMeta::default()
                }
            },
            None => mirzam_core::DeckMeta::default(),
        };
        warnings.extend(mirzam_render::theme_warnings(&meta));
        warnings.extend(mirzam_render::mode_warning(meta.mode.as_deref()));
        if let Err(w) = meta.math_dialect() {
            warnings.push(w);
        }
        warnings.extend(meta.grid_metrics().1);
        // There is no current directory in WASM, so the base is an empty path.
        let expanded = mirzam_syntax::expand_includes_mapped(
            body,
            0,
            Path::new(""),
            Path::new(""),
            &MapFiles(&self.files),
            &mut Default::default(),
        );
        // Reported here as well as in the CLI: a section drawn on the deck's
        // shapes rather than its own looks the same in both, and a preview
        // that stayed quiet about it would be the place nobody found out.
        warnings.extend(mirzam_core::transclusion_warnings(
            &meta,
            Path::new(""),
            &expanded.frontmatter,
        ));
        let vars = meta.var_table();
        let body = substitute_outside_fences(&expanded.text, &vars);

        let assets = MapAssets(&self.assets);
        let slide_srcs = mirzam_syntax::split_slides_at(&body, meta.split_level());
        // The deck settings a slide cannot see from its own text: the math
        // dialect, the masters it can be drawn on, the footer it carries.
        let mut ctx = mirzam_render::DeckContext::new(&meta, slide_srcs.len());
        // A masters file comes out of the host's table, the same place a
        // transcluded `![[…]]` does — so the preview draws the deck on the
        // same shapes the CLI does instead of falling back to single panes.
        if let Some(rel) = meta.masters_file() {
            match mirzam_syntax::load_masters(rel, Path::new(""), &MapFiles(&self.files)) {
                Ok((masters, master_warnings)) => {
                    ctx.masters = masters;
                    warnings.extend(master_warnings);
                }
                Err(w) => {
                    ctx.masters_unavailable = true;
                    warnings.push(w);
                }
            }
        }
        // The references `[@key]` can name come out of the host's file table,
        // the same place a masters file and a transcluded `![[…]]` do — so the
        // preview cites, lists and links exactly as the CLI does.
        let (bib, bib_warnings) = mirzam_render::deck_bibliography(&meta, |rel| {
            MapFiles(&self.files).read(Path::new(rel))
        });
        warnings.extend(bib_warnings);
        // And so do the stylesheets named by `theme:`. They carry the deck's
        // own type, colour and any class its slides use, so a preview that
        // ignored them — as this one did — is not the deck slightly off, it is
        // a different deck, and every difference from the CLI looks like a bug
        // in something else. The retired `css:` arrives here too, under the
        // key the deck wrote, so the message names what the author can see.
        let mut file_themes = Vec::new();
        for sheet in meta.theme_sheets() {
            match MapFiles(&self.files).read(Path::new(sheet.path)) {
                Ok(css) => file_themes.push(mirzam_render::FileTheme::new(sheet.path, css)),
                Err(e) => warnings.push(format!("{}: cannot read {}: {e}", sheet.key, sheet.path)),
            }
        }
        warnings.extend(mirzam_render::file_theme_warnings(&file_themes));
        // A slide reads them for one thing only: whether a `theme=` names one.
        ctx.file_themes = file_themes.clone();
        let (cite_style, style_warning) = mirzam_render::citation_style(&meta);
        warnings.extend(style_warning);
        for text in [&mut ctx.footer, &mut ctx.slide_number]
            .into_iter()
            .flatten()
        {
            *text = mirzam_core::substitute_vars(text, &vars);
        }
        warnings.extend(ctx.warnings());
        let mut sections: Vec<String> = slide_srcs
            .iter()
            .enumerate()
            .map(|(i, src)| {
                let slide = mirzam_syntax::parse_slide(src);
                let out = mirzam_render::render_slide_html_with(&slide, i, &assets, &ctx);
                warnings.extend(out.warnings);
                out.html
            })
            .collect();
        // A `toc` block needs the whole deck, so it resolves once every slide
        // has rendered - here as in the CLI pipeline. Without this the browser
        // build would silently drop a table of contents the CLI produces.
        mirzam_render::resolve_deck(&mut sections);
        // And so does a citation, which additionally needs to know which slide
        // the reference list ended up on.
        warnings.extend(mirzam_render::resolve_citations(
            &mut sections,
            &bib,
            cite_style,
        ));
        // Content that produced no slides renders as a blank preview with no
        // hint why - frontmatter and nothing after it is the usual way in.
        // A source that is *entirely* empty is not reported: that is the state
        // an editor starts a new deck in, and the host says so its own way.
        if sections.is_empty() && !source.trim().is_empty() {
            warnings.push("no slides: nothing outside the frontmatter".to_string());
        }
        let page_fingerprint =
            mirzam_render::page_fingerprint(&meta, &sections, &page_options(file_themes.clone()));
        Built {
            meta,
            sections,
            warnings,
            file_themes,
            page_fingerprint,
        }
    }
}

fn parse_table(json: &str) -> Result<BTreeMap<String, String>, JsError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| JsError::new(&format!("failed to parse JSON: {e}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| JsError::new("expected an object of the form `{path: value}`"))?;
    Ok(obj
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (normalize(Path::new(k)), s.to_string())))
        .collect())
}

/// Substitutes variables outside code fences, matching the CLI's rule.
fn substitute_outside_fences(body: &str, vars: &BTreeMap<String, mirzam_core::Value>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_code = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line);
        } else {
            out.push_str(&mirzam_core::substitute_vars(line, vars));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_page_with_includes_and_assets() {
        let mut r = Renderer::new();
        r.set_files("{\"sections/a.md\": \"## Included heading\\n\"}")
            .unwrap();
        r.set_assets(r#"{"img/x.svg": "data:image/svg+xml;base64,QUJD"}"#)
            .unwrap();
        let src =
            "---\ntitle: T\n---\n\n# First\n\n![figure](img/x.svg)\n\n---\n\n![[sections/a.md]]\n";
        let out = r.render_page(src);
        assert_eq!(out.slide_count, 2);
        assert!(out.html.contains("Included heading"));
        assert!(out.html.contains("data:image/svg+xml;base64,QUJD"));
        assert_eq!(out.warnings, "[]");
    }

    /// The whole reason a masters file is read through `FileProvider` rather
    /// than the filesystem: the browser has no filesystem, and a preview that
    /// silently drew every slide as a single pane while the CLI drew the deck
    /// correctly would be worse than either being wrong on its own.
    #[test]
    fn a_masters_file_comes_out_of_the_host_table() {
        let mut r = Renderer::new();
        r.set_files(
            "{\"shapes.md\": \"## body\\n\\n```pane\\n+-------+\\n| head  |\\n+-------+\\n| main  |\\n+-------+\\n```\\n\"}",
        )
        .unwrap();
        let out = r.render_page(
            "---\ntitle: T\nmasters: shapes.md\nlayout: body\n---\n\n::: pane head\n# H\n:::\n",
        );
        assert!(out.html.contains(r#"grid-template-areas:"head" "main""#));
        assert_eq!(out.warnings, "[]");
    }

    /// A bibliography reaches the preview the same way, out of the same table.
    /// The CLI and the browser disagreeing here is the failure that is hard to
    /// see: the deck still renders, every `[@key]` just quietly stops being a
    /// citation in one of the two.
    #[test]
    fn a_bibliography_comes_out_of_the_host_table() {
        let mut r = Renderer::new();
        r.set_files("{\"refs.bib\": \"@misc{a, author={Ito, Ken}, title={One}, year={2020}}\"}")
            .unwrap();
        let out = r.render_page(
            "---\ntitle: T\nbibliography: refs.bib\ncitation-style: author\n---\n\nA claim[@a].\n\n---\n\n```bibliography\n```\n",
        );
        assert!(out.html.contains(">Ito20</a>"), "{}", out.html);
        assert!(out.html.contains("mz-bib-back"), "{}", out.html);
        assert_eq!(out.warnings, "[]");
    }

    /// So do the stylesheets in `theme:` — the last of the four kinds of file
    /// a deck names in its frontmatter, and the one this renderer used to drop
    /// on the floor. A deck written against a theme of its own would otherwise
    /// preview with none of its type or colour and say nothing about why.
    #[test]
    fn the_stylesheet_comes_out_of_the_host_table() {
        let mut r = Renderer::new();
        r.set_files(r#"{"themes/deck.css": ".metric { font-size: 4rem }"}"#)
            .unwrap();
        let out = r.render_page("---\ntitle: T\ntheme: themes/deck.css\n---\n\n# H\n");
        assert!(
            out.html.contains(".metric { font-size: 4rem }"),
            "{}",
            out.html
        );
        assert_eq!(out.warnings, "[]");

        // A list is cascade order, and the built-in comes first because its
        // tokens are what the shared stylesheet reads.
        let out = r.render_page("---\ntitle: T\ntheme: [nord, themes/deck.css]\n---\n\n# H\n");
        assert!(out.html.contains("data-theme=\"nord\""), "{}", out.html);
        assert!(out.html.contains(".metric { font-size: 4rem }"));
        assert_eq!(out.warnings, "[]");
    }

    /// The retired key is the same path with an older name, and says so.
    #[test]
    fn the_css_alias_still_loads_the_stylesheet_and_warns() {
        let mut r = Renderer::new();
        r.set_files(r#"{"themes/deck.css": ".metric { font-size: 4rem }"}"#)
            .unwrap();
        let out = r.render_page("---\ntitle: T\ncss: themes/deck.css\n---\n\n# H\n");
        assert!(out.html.contains(".metric { font-size: 4rem }"));
        assert!(
            out.warnings.contains("theme: themes/deck.css"),
            "{}",
            out.warnings
        );
    }

    /// A stylesheet edit changes no slide at all, so the diff has to come back
    /// as a rebuild or the preview keeps the look it opened with.
    #[test]
    fn a_stylesheet_edit_asks_for_a_rebuild() {
        let mut r = Renderer::new();
        let src = "---\ntheme: deck.css\n---\n\n# A\n";
        r.set_files(r#"{"deck.css": "h1 { color: red }"}"#).unwrap();
        r.render_changed(src);
        r.set_files(r#"{"deck.css": "h1 { color: blue }"}"#)
            .unwrap();
        let out: serde_json::Value = serde_json::from_str(&r.render_changed(src)).unwrap();
        assert_eq!(out["structural"], true);
        assert_eq!(out["changes"].as_array().unwrap().len(), 0);
    }

    /// And a host that did not supply it says so in the same words the CLI
    /// uses, rather than showing an unstyled deck and leaving the author to
    /// wonder which of their classes stopped working.
    #[test]
    fn a_stylesheet_the_host_did_not_supply_warns() {
        let r = Renderer::new();
        let out = r.render_page("---\ntheme: deck.css\n---\n\n# H\n");
        assert!(
            out.warnings.contains("theme: cannot read deck.css"),
            "{}",
            out.warnings
        );
    }

    /// A host that did not supply the file gets a warning saying what the deck
    /// will look like, not a silent fallback nobody can diagnose.
    #[test]
    fn a_masters_file_the_host_did_not_supply_warns() {
        let r = Renderer::new();
        let out = r.render_page("---\ntitle: T\nmasters: shapes.md\n---\n\n# H\n");
        assert!(out.warnings.contains("single pane"), "{}", out.warnings);
    }

    #[test]
    fn render_changed_reports_only_diff() {
        let r = Renderer::new();
        let v1 = "# A\n\n---\n\n# B\n";
        let first: serde_json::Value = serde_json::from_str(&r.render_changed(v1)).unwrap();
        assert_eq!(first["changes"].as_array().unwrap().len(), 2);

        // Only the second slide changes.
        let v2 = "# A\n\n---\n\n# B2\n";
        let second: serde_json::Value = serde_json::from_str(&r.render_changed(v2)).unwrap();
        let changes = second["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0][0], 1);
        assert_eq!(second["structural"], false);

        // No change means no diff.
        let third: serde_json::Value = serde_json::from_str(&r.render_changed(v2)).unwrap();
        assert_eq!(third["changes"].as_array().unwrap().len(), 0);
    }

    /// The preview patches changed slides into a page it assembled earlier, so
    /// a `theme:` swap - which changes no slide at all - has to come back as a
    /// rebuild. Without this the deck keeps the palette it opened with and the
    /// edit looks like it did nothing.
    #[test]
    fn a_page_level_setting_asks_for_a_rebuild() {
        let r = Renderer::new();
        r.render_changed("---\ntheme: wuwei\n---\n\n# A\n");
        let out: serde_json::Value =
            serde_json::from_str(&r.render_changed("---\ntheme: nord\n---\n\n# A\n")).unwrap();
        assert_eq!(out["structural"], true);
        assert_eq!(out["count"], 1);
        // The slide itself is unchanged, which is exactly why the flag matters.
        assert_eq!(out["changes"].as_array().unwrap().len(), 0);

        // And an ordinary edit still patches.
        let out: serde_json::Value =
            serde_json::from_str(&r.render_changed("---\ntheme: nord\n---\n\n# B\n")).unwrap();
        assert_eq!(out["structural"], false);
        assert_eq!(out["changes"].as_array().unwrap().len(), 1);
    }

    /// A slide that switches palette needs tokens the page was not assembled
    /// with, so it is a page change even though the frontmatter never moved.
    #[test]
    fn a_slide_reaching_for_another_palette_asks_for_a_rebuild() {
        let r = Renderer::new();
        r.render_changed("# A\n");
        let out: serde_json::Value =
            serde_json::from_str(&r.render_changed("<!-- theme: nord -->\n\n# A\n")).unwrap();
        assert_eq!(out["structural"], true);
    }

    /// `split: h2` is how an ordinary document becomes a deck. The preview read
    /// it as one slide while `mirzam build` made several - the two disagreeing
    /// about what a slide even is.
    #[test]
    fn frontmatter_split_makes_slides_here_too() {
        let r = Renderer::new();
        let src = "---\nsplit: h2\n---\n\n## One\n\na\n\n## Two\n\nb\n";
        assert_eq!(r.render_page(src).slide_count, 2);
        let out: serde_json::Value = serde_json::from_str(&r.outline(src)).unwrap();
        assert_eq!(out["slides"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn structural_change_flagged() {
        let r = Renderer::new();
        r.render_changed("# A\n");
        let out: serde_json::Value =
            serde_json::from_str(&r.render_changed("# A\n\n---\n\n# B\n")).unwrap();
        assert_eq!(out["structural"], true);
        assert_eq!(out["count"], 2);
    }

    /// Where the editor's cursor is, in slides. Counting the root's own `---`
    /// rules is right only until a deck is split across files: here the second
    /// section is slide 3, and the rule-counting answer was 2.
    #[test]
    fn the_cursor_finds_its_slide_across_transcluded_files() {
        let mut r = Renderer::new();
        r.set_files("{\"a.md\": \"# A1\\n\\n---\\n\\n# A2\\n\", \"b.md\": \"# B1\\n\"}")
            .unwrap();
        let src = "---\ntitle: T\n---\n\n# Cover\n\n---\n\n![[a.md]]\n\n---\n\n![[b.md]]\n";
        assert_eq!(r.render_page(src).slide_count, 4); // cover, A1, A2, B1

        let at = |needle: &str| r.slide_at_offset(src, src.find(needle).unwrap(), None);
        assert_eq!(at("title: T"), 0); // frontmatter: the deck starts at the top
        assert_eq!(at("# Cover"), 0);
        assert_eq!(at("![[a.md]]"), 1); // the include *is* what it pulls in
        assert_eq!(at("![[b.md]]"), 3);
        assert_eq!(r.slide_at_offset(src, src.len(), None), 3);
    }

    /// The other half of a split deck: the cursor is in a section file, and the
    /// preview open on the deck still has to follow it. The section knows
    /// nothing of the slides before it, and does not need to — the map does.
    #[test]
    fn the_cursor_finds_its_slide_from_inside_a_section_file() {
        let mut r = Renderer::new();
        let a = "# A1\n\n---\n\n# A2\n";
        r.set_files(&format!(
            "{{\"a.md\": {a:?}, \"b.md\": \"# B1\\n\\n---\\n\\n# B2\\n\"}}"
        ))
        .unwrap();
        let src = "---\ntitle: T\n---\n\n# Cover\n\n---\n\n![[a.md]]\n\n---\n\n![[./b.md]]\n";
        assert_eq!(r.render_page(src).slide_count, 5);

        let at = |file: &str, source: &str, needle: &str| {
            r.slide_at_offset(src, source.find(needle).unwrap(), Some(file.into()))
        };
        assert_eq!(at("a.md", a, "# A1"), 1);
        assert_eq!(at("a.md", a, "# A2"), 2);
        // Written `![[./b.md]]`, keyed `b.md`: one file said two ways.
        assert_eq!(at("b.md", "# B1\n\n---\n\n# B2\n", "# B2"), 4);
        // A file this deck does not read at all is not an error, just slide 1.
        assert_eq!(r.slide_at_offset(src, 0, Some("elsewhere.md".into())), 0);
    }

    /// The same file with nothing transcluded still has to agree with the old
    /// rule-counting answer, or every single-file deck moves.
    #[test]
    fn the_cursor_finds_its_slide_in_a_deck_of_one_file() {
        let r = Renderer::new();
        let src =
            "---\ntitle: T\n---\n\n# One\n\n---\n\n# Two\n\n```md\n---\n```\n\n---\n\n# Three\n";
        let at = |needle: &str| r.slide_at_offset(src, src.find(needle).unwrap(), None);
        assert_eq!(at("# One"), 0);
        assert_eq!(at("# Two"), 1);
        assert_eq!(at("```md"), 1); // a rule inside a fence is not a rule
        assert_eq!(at("# Three"), 2);
    }

    /// The offset is counted in bytes, which is what the host converts to — a
    /// deck in Japanese is where counting anything else stops agreeing. An
    /// offset that lands mid-character is answered rather than panicking:
    /// nothing here slices the text with it.
    #[test]
    fn the_cursor_finds_its_slide_in_a_deck_that_is_not_ascii() {
        let mut r = Renderer::new();
        r.set_files("{\"章.md\": \"# 第一章\\n\\n---\\n\\n# 第二章\\n\"}")
            .unwrap();
        let src = "# 表紙\n\n---\n\n![[章.md]]\n\n---\n\n# 結び\n";
        assert_eq!(r.render_page(src).slide_count, 4);
        let at = |needle: &str| r.slide_at_offset(src, src.find(needle).unwrap(), None);
        assert_eq!(at("# 表紙"), 0);
        assert_eq!(at("![[章.md]]"), 1);
        assert_eq!(at("# 結び"), 3);
        // Mid-character, the way a stale offset arrives while the deck is
        // being typed into.
        assert_eq!(
            r.slide_at_offset(src, src.find("結び").unwrap() + 1, None),
            3
        );
    }

    #[test]
    fn outline_reports_structure() {
        let r = Renderer::new();
        let src = "## S\n\n```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n::: pane a\nx\n:::\n\n```shape\nrect #r at(50,50) size(10,10)\n```\n\n<!-- note: a memo -->\n";
        let out: serde_json::Value = serde_json::from_str(&r.outline(src)).unwrap();
        let slide = &out["slides"][0];
        assert_eq!(slide["panes"][0], "a");
        assert_eq!(slide["hasShapes"], true);
        assert_eq!(slide["notes"][0], "a memo");
    }

    #[test]
    fn missing_asset_warns_but_renders() {
        let r = Renderer::new();
        let out = r.render_page("![figure](missing.png)\n");
        assert!(out.warnings.contains("missing.png"));
        assert!(out.html.contains("<img")); // replaced with the placeholder
    }
}
