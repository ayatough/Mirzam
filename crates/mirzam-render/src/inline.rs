//! ペイン内 Markdown の前処理:
//! - 見出し・画像・スパンの属性記法 `{#id .class k=v}` を raw HTML に変換
//! - 数式 `$...$` / `$$...$$` をタグ付け(KaTeX 統合は MVP で対応、今はスタイル表示)

use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// `#id .class k=v` 属性列のパース結果
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

/// コードフェンスの外側の行にのみ変換 `f` を適用する
fn map_outside_fences(src: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_code = false;
    for line in src.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line);
        } else {
            out.push_str(&f(line));
        }
        out.push('\n');
    }
    out
}

/// コードフェンス外の連続領域(複数行)に変換 `f` を適用する
fn map_fence_segments(src: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(src.len());
    let mut segment = String::new();
    let mut in_code = false;
    for line in src.lines() {
        if line.trim_start().starts_with("```") {
            if !in_code {
                out.push_str(&f(&segment));
                segment.clear();
            }
            in_code = !in_code;
            out.push_str(line);
            out.push('\n');
        } else if in_code {
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

/// Markdown ソースを raw HTML 混在ソースに前処理する。
/// 数式を最初に処理する: TeX 中の `\sqrt[3]{x}` 等がスパン属性記法
/// `[...]{...}` に誤マッチするのを防ぐため。
pub fn preprocess(src: &str) -> String {
    // $$...$$ は複数行にまたがれるため、フェンス外セグメント単位で処理
    let src = map_fence_segments(src, block_math);
    let src = map_outside_fences(&src, inline_math);
    let src = map_outside_fences(&src, heading_attrs);
    let src = map_outside_fences(&src, image_attrs);
    map_outside_fences(&src, span_attrs)
}

/// `## Text {attrs}` → `<h2 ...>Text(inline render)</h2>`
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

/// `![alt](src){attrs}` → `<img ...>`
fn image_attrs(line: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let r = re(&RE, r#"!\[([^\]]*)\]\(([^()\s"]+)\)\{([^{}]*)\}"#);
    r.replace_all(line, |c: &regex::Captures| {
        let alt = html_escape(&c[1]);
        let src = &c[2];
        let attrs = parse_attrs(&c[3]);
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
        format!(
            "<img src=\"{src}\" alt=\"{alt}\"{}{style_attr}>",
            attrs.html_id_class()
        )
    })
    .into_owned()
}

/// 画像記法で参照されたファイルが動画かどうか(GIF は img のまま扱う)
fn is_video(src: &str) -> bool {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    matches!(
        path.rsplit('.').next().map(str::to_ascii_lowercase).as_deref(),
        Some("mp4" | "webm" | "ogv" | "mov" | "m4v")
    )
}

/// `![alt](demo.mp4){autoplay muted loop controls poster=...}` → `<video>`
fn video_html(src: &str, alt: &str, attrs: &Attrs, style_attr: &str) -> String {
    // 真偽属性はクラス記法(`.autoplay`)でも `key=true` でも書ける
    let flag = |name: &str| -> bool {
        attrs.classes.iter().any(|c| c == name)
            || matches!(attrs.kv.get(name).map(String::as_str), Some("" | "true"))
    };
    let mut flags = String::new();
    // ブラウザは音ありの自動再生を拒否するため、autoplay 指定時は muted を強制する
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
    // id/class から動画用の真偽フラグ由来のクラスは除く
    let mut carried = attrs.clone();
    carried
        .classes
        .retain(|c| !matches!(c.as_str(), "autoplay" | "muted" | "loop" | "controls"));
    format!(
        "<video src=\"{src}\" title=\"{alt}\"{}{poster}{flags}{style_attr}></video>",
        carried.html_id_class()
    )
}

/// `[text]{attrs}` → `<span ...>text</span>`(リンク `[t](u)` にはマッチしない)
fn span_attrs(line: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let r = re(&RE, r"(!?)\[([^\[\]]+)\]\{([^{}]*)\}");
    r.replace_all(line, |c: &regex::Captures| {
        if &c[1] == "!" {
            // 属性なし画像記法とバッティングした場合はそのまま
            return c[0].to_string();
        }
        let attrs = parse_attrs(&c[3]);
        let inner = render_inline(&c[2]);
        format!("<span{}>{inner}</span>", attrs.html_id_class())
    })
    .into_owned()
}

/// `$$...$$`(複数行可)を MathML(display=block)に変換する
fn block_math(segment: &str) -> String {
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    let b = re(&BLOCK, r"\$\$([^$]+)\$\$");
    b.replace_all(segment, |c: &regex::Captures| {
        math_html(c[1].trim(), math_core::MathDisplay::Block)
    })
    .into_owned()
}

/// `$...$` を MathML(inline)に変換する
fn inline_math(line: &str) -> String {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    let i = re(&INLINE, r"\$([^$\n]+)\$");
    i.replace_all(line, |c: &regex::Captures| {
        math_html(c[1].trim(), math_core::MathDisplay::Inline)
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

/// LaTeX → MathML 変換。失敗時は TeX ソースをスタイル付きスパンで表示する。
fn math_html(tex: &str, display: math_core::MathDisplay) -> String {
    match math_converter().convert_with_local_state(tex, display) {
        Ok(r) => r.mathml,
        Err(e) => {
            let cls = match display {
                math_core::MathDisplay::Block => "math-error math-block",
                math_core::MathDisplay::Inline => "math-error",
            };
            format!(
                "<span class=\"{cls}\" title=\"{}\">{}</span>",
                html_escape(&e.to_string()),
                html_escape(tex)
            )
        }
    }
}

/// Markdown をインラインとしてレンダリングし、外側の `<p>` を剥がす
pub fn render_inline(md: &str) -> String {
    let html = render_markdown(md);
    let t = html.trim();
    let t = t.strip_prefix("<p>").unwrap_or(t);
    let t = t.strip_suffix("</p>").unwrap_or(t);
    t.to_string()
}

/// comrak による Markdown → HTML 変換(raw HTML 許可、GFM 拡張、CJK 強調対応)
pub fn render_markdown(md: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    // 「**ページ座標(%)**で」のような CJK と記号が隣接する強調を正しく解釈する
    options.extension.cjk_friendly_emphasis = true;
    options.render.r#unsafe = true;
    comrak::markdown_to_html(md, &options)
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
        let out = preprocess("## タイトル {.center #sec1}\n");
        assert!(out.contains("<h2 id=\"sec1\" class=\"center\">タイトル</h2>"));
    }

    #[test]
    fn span_with_attrs_not_link() {
        let out = preprocess("see [word]{#w .u} and [link](http://x)\n");
        assert!(out.contains("<span id=\"w\" class=\"u\">word</span>"));
        assert!(out.contains("[link](http://x)"));
    }

    #[test]
    fn video_from_image_syntax() {
        let out = preprocess("![デモ](media/demo.mp4){.autoplay .loop .controls fit=contain}\n");
        assert!(out.contains("<video src=\"media/demo.mp4\""));
        assert!(out.contains(" autoplay"));
        // autoplay 指定時は muted が強制される(ブラウザの自動再生ポリシー)
        assert!(out.contains(" muted"));
        assert!(out.contains(" loop"));
        assert!(out.contains(" controls"));
        assert!(out.contains("object-fit:contain"));
        // フラグはクラス属性に残さない
        assert!(!out.contains("class=\"autoplay"));
    }

    #[test]
    fn gif_stays_an_image() {
        let out = preprocess("![動き](a.gif){w=60%}\n");
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
        let out = preprocess("式 $E=mc^2$ と $$\\int_0^1 x dx$$\n");
        // inline と block の 2 つの MathML が生成される
        assert_eq!(out.matches("<math").count(), 2);
        assert!(out.contains("display=\"block\""));
        assert!(!out.contains("math-error"));
    }

    #[test]
    fn broken_math_falls_back() {
        let out = preprocess("$\\frac{1$\n");
        assert!(out.contains("math-error"));
        assert!(out.contains("\\frac{1"));
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
        // latex2mathml 0.2 は x_{84}^{7} を msub(msup(...)) に誤変換していた
        let out = preprocess("$x_{84}^{7}$\n");
        assert!(out.contains("<msubsup>"));
    }

    #[test]
    fn tex_brackets_not_eaten_by_span_attrs() {
        // \sqrt[3]{x} の [3]{x} がスパン属性記法に誤マッチしないこと
        let out = preprocess("$\\sqrt[3]{x}$\n");
        assert!(out.contains("<math"));
        assert!(!out.contains("<span>3</span>"));
    }

    #[test]
    fn cjk_emphasis_with_punctuation() {
        let out = render_markdown("図形は**ページ座標(%)**で宣言する\n");
        assert!(out.contains("<strong>ページ座標(%)</strong>"), "{out}");
    }

    #[test]
    fn code_fences_untouched() {
        let src = "```\n## not a heading {#x}\n```\n";
        assert_eq!(preprocess(src), src);
    }
}
