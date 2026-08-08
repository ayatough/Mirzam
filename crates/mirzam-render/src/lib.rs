//! SlideSource 列 + DeckMeta → 単一ファイル HTML(ビューア内蔵)を生成する。

mod assets;
mod inline;
mod theme;

pub use assets::{AssetSource, FsAssets};
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

/// スライド 1 枚のレンダリング結果(`<section>` 要素、アセット埋め込み済み)
pub struct RenderedSlide {
    pub html: String,
    pub warnings: Vec<String>,
    /// このスライドが参照したローカルアセット(キャッシュ鮮度検証・監視用)
    pub assets: Vec<std::path::PathBuf>,
}

/// スライド 1 枚を `<section>` HTML にレンダリングする(serve の差分更新単位)。
/// アセットはファイルシステムから解決する。
pub fn render_slide_html(slide: &SlideSource, index: usize, asset_dir: &Path) -> RenderedSlide {
    render_slide_html_with(slide, index, &assets::FsAssets(asset_dir))
}

/// アセット解決を差し替えられる版(WASM ではホストのテーブルを注入する)
pub fn render_slide_html_with(
    slide: &SlideSource,
    index: usize,
    asset_source: &dyn AssetSource,
) -> RenderedSlide {
    let mut warnings = Vec::new();
    let mut assets_used = Vec::new();
    let html = render_slide(slide, index, &mut warnings);
    let html = assets::embed_assets(&html, asset_source, &mut warnings, &mut assets_used);
    RenderedSlide {
        html,
        warnings,
        assets: assets_used,
    }
}

/// ページ組み立てオプション
#[derive(Default)]
pub struct PageOptions {
    /// Some(version) でホットリロードクライアント(serve モード)を注入
    pub live_version: Option<u64>,
    /// frontmatter `css:` で指定されたカスタム CSS(内容)
    pub custom_css: Option<String>,
}

/// セクション列に数式が含まれるか(数式フォント同梱の要否)
pub fn sections_have_math(sections: &[String]) -> bool {
    sections.iter().any(|s| s.contains("<math"))
}

/// レンダリング済みセクション列をビューア入りの完全な HTML ページに組み立てる。
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
<html lang="ja">
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
<div id="hint">← → : 移動 / N : ノート / F : 全画面</div>
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

/// 印刷用に `<video>` を静止画へ置換する。
/// poster が指定されていればその画像を、無ければ再生アイコン付きの
/// プレースホルダを出す(PDF は静的なので動画は再生できないため)。
fn videos_to_stills(html: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"<video\b([^>]*)></video>"#).expect("static regex")
    });
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR.get_or_init(|| {
        Regex::new(r#"(\w[\w-]*)="([^"]*)""#).expect("static regex")
    });
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

/// PDF 印刷用ページ(全スライドを固定サイズで縦に並べ、1 枚 = 1 ページ)
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
<html lang="ja">
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

/// デッキ全体を単一 HTML にレンダリングする。
/// `asset_dir` は画像等の相対パスの基準ディレクトリ(data URI 埋め込みに使用)。
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

fn render_slide(slide: &SlideSource, index: usize, warnings: &mut Vec<String>) -> String {
    let mut errors: Vec<String> = Vec::new();

    // レイアウト解決
    let grid: Option<GridSpec> = match &slide.layout {
        Some(src) => match parse_grid(src) {
            Ok(g) => Some(g),
            Err(e) => {
                errors.push(format!("スライド {}: {e}", index + 1));
                None
            }
        },
        None => None,
    };

    let body = match &grid {
        Some(g) => render_grid_slide(g, slide, index, &mut errors),
        None => render_single_pane_slide(slide),
    };

    // shape ブロック → 静的 SVG レイヤ(ページ座標系、スケール自動追従)
    let mut shapes_html = String::new();
    if !slide.shapes.is_empty() {
        let src = slide.shapes.join("\n");
        let doc = mirzam_shape::parse_shapes(&src);
        let (svg, shape_errors) = mirzam_shape::render_svg(&doc, 1280, 720);
        for e in shape_errors {
            errors.push(format!("スライド {}: {e}", index + 1));
        }
        shapes_html = svg;
    }

    // connect ブロック → JSON をスライドに埋め込み、ランタイムが
    // レイアウト確定後に端点を解決して描画する(リサイズ・更新に追従)
    let mut connect_attr = String::new();
    if !slide.connects.is_empty() {
        let src = slide.connects.join("\n");
        let doc = mirzam_connect::parse_connectors(&src);
        for e in &doc.errors {
            errors.push(format!("スライド {}: {e}", index + 1));
        }
        if !doc.connectors.is_empty() {
            connect_attr = format!(
                " data-connectors=\"{}\"",
                inline::html_escape(&mirzam_connect::to_json(&doc))
            );
        }
    }

    // 未実装フェーズの予約ブロック(前方互換のためパースだけして表示)
    let mut reserved_html = String::new();
    for (kind, src) in &slide.reserved {
        let phase = match kind {
            mirzam_syntax::BlockKind::Anim => "Phase 3",
        };
        reserved_html.push_str(&format!(
            "<details class=\"mz-reserved\"><summary>```{kind} ブロック({phase} で対応予定)</summary><pre>{body}</pre></details>\n",
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
) -> String {
    let names = grid.pane_names();
    let mut panes_html = String::new();

    // loose コンテンツの行き先: `main` があれば main、なければ最初のペイン
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
        // ペイン属性: align(text-align)/ valign(縦位置)/ 追加クラス
        let attrs = parse_attrs(attrs_src);
        let mut style = format!("grid-area:{name}");
        if let Some(a) = attrs.kv.get("align") {
            if matches!(a.as_str(), "center" | "right" | "left") {
                style.push_str(&format!(";text-align:{a}"));
            }
        }
        match attrs.kv.get("valign").map(String::as_str) {
            Some("middle") => style.push_str(";display:flex;flex-direction:column;justify-content:center"),
            Some("bottom") => style.push_str(";display:flex;flex-direction:column;justify-content:flex-end"),
            _ => {}
        }
        let extra_cls = attrs
            .classes
            .iter()
            .map(|c| format!(" {c}"))
            .collect::<String>();
        panes_html.push_str(&format!(
            "<div class=\"pane pane-{name}{extra_cls}\" style=\"{style}\">{}</div>\n",
            render_markdown(&preprocess(&content))
        ));
    }

    // グリッドに存在しないペインへの割り当ては警告してスキップ
    for pb in &slide.panes {
        if !names.contains(&pb.name) {
            errors.push(format!(
                "スライド {}: ペイン `{}` はレイアウトに存在しません",
                index + 1,
                pb.name
            ));
        }
    }

    // 注意: grid-template-areas の値は二重引用符を含むため style 属性は単引用符で囲む
    format!(
        "<div class=\"grid\" style='grid-template-columns:{cols};grid-template-rows:{rows};grid-template-areas:{areas}'>\n{panes_html}</div>\n",
        cols = grid.css_columns(),
        rows = grid.css_rows(),
        areas = grid.css_areas(),
    )
}

fn render_single_pane_slide(slide: &SlideSource) -> String {
    let mut content = slide.loose.clone();
    // レイアウトが無いのに ::: pane があれば順に連結
    for pb in &slide.panes {
        content.push('\n');
        content.push_str(&pb.body);
    }
    format!(
        "<div class=\"grid\" style='grid-template-columns:1fr;grid-template-rows:1fr;grid-template-areas:\"main\"'>\n<div class=\"pane pane-main\" style=\"grid-area:main\">{}</div>\n</div>\n",
        render_markdown(&preprocess(&content))
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
        assert!(out.html.contains("grid-template-areas:\"main main\" \"a b\""));
        assert!(out.html.contains("pane-a"));
        assert!(out.html.contains("hello"));
    }

    #[test]
    fn unknown_pane_warns() {
        let slide = parse_slide(
            "```pane\n+---+\n| a |\n+---+\n```\n\n::: pane zzz\nlost\n:::\n",
        );
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
        let out = videos_to_stills("<video src=\"a.mp4\" title=\"デモ動画\"></video>");
        assert!(out.contains("mz-video-still"));
        assert!(out.contains("デモ動画"));
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
