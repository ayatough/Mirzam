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
//! ```

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
    }

    /// Renders a complete HTML page with the viewer.
    pub fn render_page(&self, source: &str) -> RenderOutput {
        let built = self.build(source);
        let opts = mirzam_render::PageOptions::default();
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
    /// When `structural` is true the slide count changed, so rebuild the page.
    pub fn render_changed(&self, source: &str) -> String {
        let built = self.build(source);
        let mut prev = self.previous.borrow_mut();
        let structural = prev.len() != built.sections.len();
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
        let slides: Vec<serde_json::Value> = mirzam_syntax::split_slides(&body)
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
        warnings.extend(mirzam_render::theme_warning(meta.theme.as_deref()));
        warnings.extend(mirzam_render::mode_warning(meta.mode.as_deref()));
        if let Err(w) = meta.math_dialect() {
            warnings.push(w);
        }
        // There is no current directory in WASM, so the base is an empty path.
        let body = mirzam_syntax::expand_includes(body, Path::new(""), &MapFiles(&self.files));
        let vars = meta.var_table();
        let body = substitute_outside_fences(&body, &vars);

        let assets = MapAssets(&self.assets);
        let slide_srcs = mirzam_syntax::split_slides(&body);
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
        // Content that produced no slides renders as a blank preview with no
        // hint why - frontmatter and nothing after it is the usual way in.
        // A source that is *entirely* empty is not reported: that is the state
        // an editor starts a new deck in, and the host says so its own way.
        if sections.is_empty() && !source.trim().is_empty() {
            warnings.push("no slides: nothing outside the frontmatter".to_string());
        }
        Built {
            meta,
            sections,
            warnings,
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

    #[test]
    fn structural_change_flagged() {
        let r = Renderer::new();
        r.render_changed("# A\n");
        let out: serde_json::Value =
            serde_json::from_str(&r.render_changed("# A\n\n---\n\n# B\n")).unwrap();
        assert_eq!(out["structural"], true);
        assert_eq!(out["count"], 2);
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
