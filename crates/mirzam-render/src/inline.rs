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

/// Markdown ソースを raw HTML 混在ソースに前処理する
pub fn preprocess(src: &str) -> String {
    let src = map_outside_fences(src, |line| heading_attrs(line));
    let src = map_outside_fences(&src, |line| image_attrs(line));
    let src = map_outside_fences(&src, |line| span_attrs(line));
    map_outside_fences(&src, |line| math_spans(line))
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
        format!(
            "<img src=\"{src}\" alt=\"{alt}\"{}{style_attr}>",
            attrs.html_id_class()
        )
    })
    .into_owned()
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

/// `$$...$$` と `$...$` を数式スパンに変換(中身は TeX ソースのまま保持)
fn math_spans(line: &str) -> String {
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    static INLINE: OnceLock<Regex> = OnceLock::new();
    let b = re(&BLOCK, r"\$\$([^$]+)\$\$");
    let line = b
        .replace_all(line, |c: &regex::Captures| {
            format!(
                "<span class=\"math math-block\">{}</span>",
                html_escape(c[1].trim())
            )
        })
        .into_owned();
    let i = re(&INLINE, r"\$([^$\n]+)\$");
    i.replace_all(&line, |c: &regex::Captures| {
        format!("<span class=\"math\">{}</span>", html_escape(c[1].trim()))
    })
    .into_owned()
}

/// Markdown をインラインとしてレンダリングし、外側の `<p>` を剥がす
pub fn render_inline(md: &str) -> String {
    let html = render_markdown(md);
    let t = html.trim();
    let t = t.strip_prefix("<p>").unwrap_or(t);
    let t = t.strip_suffix("</p>").unwrap_or(t);
    t.to_string()
}

/// comrak による Markdown → HTML 変換(raw HTML 許可、GFM 拡張)
pub fn render_markdown(md: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.render.unsafe_ = true;
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
    fn image_with_fit() {
        let out = preprocess("![alt](img/a.png){fit=contain}\n");
        assert!(out.contains("<img src=\"img/a.png\""));
        assert!(out.contains("object-fit:contain"));
    }

    #[test]
    fn math_tagged() {
        let out = preprocess("式 $E=mc^2$ と $$\\int_0^1 x dx$$\n");
        assert!(out.contains("<span class=\"math\">E=mc^2</span>"));
        assert!(out.contains("math math-block"));
    }

    #[test]
    fn code_fences_untouched() {
        let src = "```\n## not a heading {#x}\n```\n";
        assert_eq!(preprocess(src), src);
    }
}
