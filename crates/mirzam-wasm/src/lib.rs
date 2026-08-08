//! Mirzam コアの WebAssembly バインディング。
//!
//! ネイティブ版(CLI)と**同一のパース・レイアウト・レンダリング実装**を
//! ブラウザ・VSCode Webview・Obsidian から呼び出すためのもの。
//! ファイルシステムが無い環境で動くよう、include 対象ファイルとアセットは
//! ホストがテーブル(JSON オブジェクト)として渡す。
//!
//! ```js
//! import init, { Renderer } from './mirzam_wasm.js';
//! await init();
//! const r = new Renderer();
//! r.set_files(JSON.stringify({ 'sections/a.md': '## 埋め込み\n' }));
//! r.set_assets(JSON.stringify({ 'img/x.svg': 'data:image/svg+xml;base64,...' }));
//! const html = r.render_page(source);          // ページ全体
//! const changed = JSON.parse(r.render_changed(source)); // 差分だけ
//! ```

use mirzam_render::AssetSource;
use mirzam_syntax::FileProvider;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;

/// ホストが渡したファイルテーブルから include を解決する
struct MapFiles<'a>(&'a BTreeMap<String, String>);

impl FileProvider for MapFiles<'_> {
    fn read(&self, path: &Path) -> Result<String, String> {
        let key = normalize(path);
        self.0
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("`{key}` はホストから渡されていません"))
    }
}

/// ホストが渡したアセットテーブル(パス → data URI または URL)
struct MapAssets<'a>(&'a BTreeMap<String, String>);

impl AssetSource for MapAssets<'_> {
    fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
        let key = normalize(Path::new(rel));
        let result = self
            .0
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("`{key}` はホストから渡されていません"));
        (result, None)
    }
}

/// `./a/../b.md` のような相対パスを正規化してテーブルのキーに合わせる
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

/// レンダリング結果(ページ全体)
#[wasm_bindgen(getter_with_clone)]
pub struct RenderOutput {
    pub html: String,
    /// 警告メッセージ(JSON 配列)
    pub warnings: String,
    pub slide_count: usize,
}

/// 差分レンダリング対応のレンダラ。
/// 前回のスライド出力を保持し、変わったスライドだけを返せる。
#[wasm_bindgen]
pub struct Renderer {
    files: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
    /// 前回レンダリング時のスライド HTML(差分計算用)
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

    /// include 対象のファイルテーブルを設定する(JSON: `{path: content}`)
    pub fn set_files(&mut self, json: &str) -> Result<(), JsError> {
        self.files = parse_table(json)?;
        Ok(())
    }

    /// 画像・動画のテーブルを設定する(JSON: `{path: dataUriOrUrl}`)
    pub fn set_assets(&mut self, json: &str) -> Result<(), JsError> {
        self.assets = parse_table(json)?;
        Ok(())
    }

    /// 差分計算の基準をリセットする(次回は全スライドが「変更」として返る)
    pub fn reset(&self) {
        self.previous.borrow_mut().clear();
    }

    /// ビューア入りの完全な HTML ページを生成する
    pub fn render_page(&self, source: &str) -> RenderOutput {
        let built = self.build(source);
        let opts = mirzam_render::PageOptions::default();
        RenderOutput {
            html: mirzam_render::assemble_page(&built.meta, &built.sections, &opts),
            warnings: serde_json::to_string(&built.warnings).unwrap_or_else(|_| "[]".into()),
            slide_count: built.sections.len(),
        }
    }

    /// スライド 1 枚だけを `<section>` HTML として返す(プレビューの部分更新用)
    pub fn render_slide(&self, source: &str, index: usize) -> Option<String> {
        self.build(source).sections.get(index).cloned()
    }

    /// 前回から変わったスライドだけを返す。
    /// JSON: `{"count": n, "changes": [[index, html], ...], "structural": bool}`
    /// `structural` が true のときはスライド枚数が変わったので全体を作り直す。
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

    /// パースだけ行い、デッキ構造の要約を返す(アウトライン表示・診断用)。
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
        // WASM 環境ではカレントディレクトリの概念が無いため基準は空パス
        let body = mirzam_syntax::expand_includes(body, Path::new(""), &MapFiles(&self.files));
        let vars = meta.var_table();
        let body = substitute_outside_fences(&body, &vars);

        let assets = MapAssets(&self.assets);
        let sections = mirzam_syntax::split_slides(&body)
            .iter()
            .enumerate()
            .map(|(i, src)| {
                let slide = mirzam_syntax::parse_slide(src);
                let out = mirzam_render::render_slide_html_with(&slide, i, &assets);
                warnings.extend(out.warnings);
                out.html
            })
            .collect();
        Built {
            meta,
            sections,
            warnings,
        }
    }
}

fn parse_table(json: &str) -> Result<BTreeMap<String, String>, JsError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| JsError::new(&format!("JSON 解析に失敗: {e}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| JsError::new("オブジェクト `{path: value}` を渡してください"))?;
    Ok(obj
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (normalize(Path::new(k)), s.to_string())))
        .collect())
}

/// コードフェンス外の行にのみ変数置換を適用する(CLI と同じ規則)
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
        // 中身が `"##` を含むので raw string ではなく通常のエスケープで書く
        r.set_files("{\"sections/a.md\": \"## 埋め込み見出し\\n\"}")
            .unwrap();
        r.set_assets(r#"{"img/x.svg": "data:image/svg+xml;base64,QUJD"}"#)
            .unwrap();
        let src =
            "---\ntitle: T\n---\n\n# 一枚目\n\n![図](img/x.svg)\n\n---\n\n![[sections/a.md]]\n";
        let out = r.render_page(src);
        assert_eq!(out.slide_count, 2);
        assert!(out.html.contains("埋め込み見出し"));
        assert!(out.html.contains("data:image/svg+xml;base64,QUJD"));
        assert_eq!(out.warnings, "[]");
    }

    #[test]
    fn render_changed_reports_only_diff() {
        let r = Renderer::new();
        let v1 = "# A\n\n---\n\n# B\n";
        let first: serde_json::Value = serde_json::from_str(&r.render_changed(v1)).unwrap();
        assert_eq!(first["changes"].as_array().unwrap().len(), 2);

        // 2 枚目だけ変更
        let v2 = "# A\n\n---\n\n# B2\n";
        let second: serde_json::Value = serde_json::from_str(&r.render_changed(v2)).unwrap();
        let changes = second["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0][0], 1);
        assert_eq!(second["structural"], false);

        // 変更なしなら差分ゼロ
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
        let src = "## S\n\n```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n::: pane a\nx\n:::\n\n```shape\nrect #r at(50,50) size(10,10)\n```\n\n<!-- note: メモ -->\n";
        let out: serde_json::Value = serde_json::from_str(&r.outline(src)).unwrap();
        let slide = &out["slides"][0];
        assert_eq!(slide["panes"][0], "a");
        assert_eq!(slide["hasShapes"], true);
        assert_eq!(slide["notes"][0], "メモ");
    }

    #[test]
    fn missing_asset_warns_but_renders() {
        let r = Renderer::new();
        let out = r.render_page("![図](missing.png)\n");
        assert!(out.warnings.contains("missing.png"));
        assert!(out.html.contains("<img")); // プレースホルダに置換される
    }
}
