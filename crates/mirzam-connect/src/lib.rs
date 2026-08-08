//! `connect` ブロックの DSL パーサ。
//!
//! ```text
//! #lat -> #cache      : arrow color=@accent1
//! #cache -- #note1    : line  style=dotted
//! #a <-> #b
//! ```
//!
//! 端点(文章中のアンカー span や図形要素)の実座標はブラウザの
//! レイアウト確定後にしか決まらないため、Rust 側は宣言を JSON に
//! 変換してスライドに埋め込み、ビューアランタイムが表示時・リサイズ時・
//! ホットリロード時に端点を解決して描画する。レイアウトが変わっても
//! コネクタが自動追従するのはこの構造による。

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowMode {
    /// `->` 終端に矢印
    End,
    /// `<->` 両端に矢印
    Both,
    /// `--` 線のみ
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connector {
    /// 始点: 要素 ID(`#` なし)+ 任意の辺(`.n` 等)
    pub from: String,
    pub from_edge: Option<char>,
    pub to: String,
    pub to_edge: Option<char>,
    pub arrow: ArrowMode,
    pub kv: BTreeMap<String, String>,
}

pub struct ConnectDoc {
    pub connectors: Vec<Connector>,
    pub errors: Vec<String>,
}

pub fn parse_connectors(src: &str) -> ConnectDoc {
    let mut connectors = Vec::new();
    let mut errors = Vec::new();
    for (ln, line) in src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(c) => connectors.push(c),
            Err(e) => errors.push(format!("connect 行 {}: {e}", ln + 1)),
        }
    }
    ConnectDoc { connectors, errors }
}

fn parse_line(line: &str) -> Result<Connector, String> {
    // `<端点> <op> <端点> [: 属性...]`
    let (link, attrs) = match line.split_once(':') {
        Some((l, a)) => (l.trim(), a.trim()),
        None => (line, ""),
    };
    let (arrow, op) = if link.contains("<->") {
        (ArrowMode::Both, "<->")
    } else if link.contains("->") {
        (ArrowMode::End, "->")
    } else if link.contains("--") {
        (ArrowMode::None, "--")
    } else {
        return Err("接続演算子(-> / <-> / --)がありません".into());
    };
    let (lhs, rhs) = link.split_once(op).unwrap();
    let (from, from_edge) = parse_endpoint(lhs.trim())?;
    let (to, to_edge) = parse_endpoint(rhs.trim())?;

    let mut kv = BTreeMap::new();
    for token in attrs.split_whitespace() {
        if let Some((k, v)) = token.split_once('=') {
            kv.insert(k.to_string(), v.trim_matches('"').to_string());
        }
        // `arrow` / `line` の種別語は演算子と重複するため無視する
    }
    Ok(Connector {
        from,
        from_edge,
        to,
        to_edge,
        arrow,
        kv,
    })
}

fn parse_endpoint(s: &str) -> Result<(String, Option<char>), String> {
    let id = s
        .strip_prefix('#')
        .ok_or_else(|| format!("端点は `#id` 形式で指定してください: `{s}`"))?;
    if id.is_empty() {
        return Err("空の端点 ID".into());
    }
    match id.rsplit_once('.') {
        Some((base, e)) if matches!(e, "n" | "s" | "e" | "w" | "c") => {
            Ok((base.to_string(), e.chars().next()))
        }
        _ => Ok((id.to_string(), None)),
    }
}

/// テーマカラートークンを CSS 変数へ解決
fn color(v: &str) -> String {
    if let Some(name) = v.strip_prefix('@') {
        return format!("var(--mz-{name})");
    }
    v.chars()
        .filter(|c| c.is_alphanumeric() || "#(),.%- ".contains(*c))
        .collect()
}

/// ランタイムへ渡す JSON(HTML data 属性に埋め込む)
pub fn to_json(doc: &ConnectDoc) -> String {
    let arr: Vec<serde_json::Value> = doc
        .connectors
        .iter()
        .map(|c| {
            serde_json::json!({
                "from": c.from,
                "fromEdge": c.from_edge.map(String::from),
                "to": c.to,
                "toEdge": c.to_edge.map(String::from),
                "arrow": match c.arrow {
                    ArrowMode::End => "end",
                    ArrowMode::Both => "both",
                    ArrowMode::None => "none",
                },
                "color": c.kv.get("color").map(|v| color(v)),
                "dashed": matches!(c.kv.get("style").map(String::as_str), Some("dashed" | "dotted")),
                "curve": c.kv.get("curve").and_then(|v| v.parse::<f64>().ok()),
            })
        })
        .collect();
    serde_json::Value::Array(arr).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_arrow_with_attrs() {
        let doc = parse_connectors("#lat -> #cache.s : arrow color=@accent1 style=dashed");
        assert!(doc.errors.is_empty());
        let c = &doc.connectors[0];
        assert_eq!(c.from, "lat");
        assert_eq!(c.to, "cache");
        assert_eq!(c.to_edge, Some('s'));
        assert_eq!(c.arrow, ArrowMode::End);
        let json = to_json(&doc);
        assert!(json.contains("var(--mz-accent1)"));
        assert!(json.contains("\"dashed\":true"));
    }

    #[test]
    fn bidirectional_and_plain() {
        let doc = parse_connectors("#a <-> #b\n#a -- #c");
        assert_eq!(doc.connectors[0].arrow, ArrowMode::Both);
        assert_eq!(doc.connectors[1].arrow, ArrowMode::None);
    }

    #[test]
    fn bad_line_reports_error() {
        let doc = parse_connectors("#a #b");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("接続演算子"));
    }
}
