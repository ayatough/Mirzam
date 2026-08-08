//! `shape` ブロックの DSL パーサと、ビルド時 SVG レイヤ生成。
//!
//! 座標系はページ全体の % (0–100)。スライドの論理ピクセルへ変換して
//! `viewBox` 固定の SVG を生成するため、表示スケールに自動追従する。
//!
//! ```text
//! rect    #cache at(70%, 30%) size(30%, 14%) label="キャッシュ層" fill=@accent2
//! ellipse #db    at(70%, 70%) size(26%, 16%) label="DB"
//! arrow   #a1    from(#cache.s) to(#db.n) style=dashed
//! line           from(10%, 90%) to(40%, 90%)
//! text    #cap   at(20%, 85%) "ヒット率 95%" .small
//! ```

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Text,
    Arrow,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    N,
    S,
    E,
    W,
    C,
}

/// 端点参照: 座標そのもの、または他の図形の辺
#[derive(Debug, Clone, PartialEq)]
pub enum EndRef {
    Point(f64, f64),
    Anchor { id: String, edge: Edge },
}

#[derive(Debug, Clone, Default)]
pub struct Shape {
    pub kind: Option<ShapeKind>,
    pub id: Option<String>,
    pub at: Option<(f64, f64)>,
    pub size: Option<(f64, f64)>,
    pub from: Option<EndRef>,
    pub to: Option<EndRef>,
    pub label: Option<String>,
    pub classes: Vec<String>,
    pub kv: BTreeMap<String, String>,
}

pub struct ShapeDoc {
    pub shapes: Vec<Shape>,
    pub errors: Vec<String>,
}

/// shape ブロックのソースをパースする(1 行 = 1 図形)
pub fn parse_shapes(src: &str) -> ShapeDoc {
    let mut shapes = Vec::new();
    let mut errors = Vec::new();
    for (ln, line) in src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') && line.len() > 1 && line.as_bytes()[1] == b' '
        {
            // 空行と `# コメント` はスキップ
            continue;
        }
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(Some(s)) => shapes.push(s),
            Ok(None) => {}
            Err(e) => errors.push(format!("shape 行 {}: {e}", ln + 1)),
        }
    }
    ShapeDoc { shapes, errors }
}

fn parse_line(line: &str) -> Result<Option<Shape>, String> {
    let tokens = tokenize(line)?;
    let Some(first) = tokens.first() else {
        return Ok(None);
    };
    let kind = match first.as_str() {
        "rect" => ShapeKind::Rect,
        "ellipse" => ShapeKind::Ellipse,
        "text" => ShapeKind::Text,
        "arrow" => ShapeKind::Arrow,
        "line" => ShapeKind::Line,
        other => return Err(format!("不明な図形種別 `{other}`")),
    };
    let mut s = Shape {
        kind: Some(kind),
        ..Shape::default()
    };
    for t in &tokens[1..] {
        if let Some(id) = t.strip_prefix('#') {
            s.id = Some(id.to_string());
        } else if let Some(cls) = t.strip_prefix('.') {
            s.classes.push(cls.to_string());
        } else if let Some(q) = t.strip_prefix('"') {
            s.label = Some(q.trim_end_matches('"').to_string());
        } else if let Some((name, args)) = parse_call(t) {
            match name {
                "at" => s.at = Some(parse_pair(&args)?),
                "size" => s.size = Some(parse_pair(&args)?),
                "from" => s.from = Some(parse_endref(&args)?),
                "to" => s.to = Some(parse_endref(&args)?),
                other => return Err(format!("不明な指定 `{other}(...)`")),
            }
        } else if let Some((k, v)) = t.split_once('=') {
            if k == "label" {
                s.label = Some(v.trim_matches('"').to_string());
            } else {
                s.kv.insert(k.to_string(), v.trim_matches('"').to_string());
            }
        } else {
            return Err(format!("解釈できないトークン `{t}`"));
        }
    }
    validate(&s)?;
    Ok(Some(s))
}

fn validate(s: &Shape) -> Result<(), String> {
    match s.kind.unwrap() {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            if s.at.is_none() || s.size.is_none() {
                return Err("rect/ellipse には at(x,y) と size(w,h) が必要です".into());
            }
        }
        ShapeKind::Text => {
            if s.at.is_none() || s.label.is_none() {
                return Err("text には at(x,y) と \"内容\" が必要です".into());
            }
        }
        ShapeKind::Arrow | ShapeKind::Line => {
            if s.from.is_none() || s.to.is_none() {
                return Err("arrow/line には from(...) と to(...) が必要です".into());
            }
        }
    }
    Ok(())
}

/// トークン分割: 引用符と括弧の内側は空白で切らない
fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                cur.push('"');
                for q in chars.by_ref() {
                    cur.push(q);
                    if q == '"' {
                        break;
                    }
                }
            }
            '(' => {
                cur.push('(');
                let mut depth = 1;
                for q in chars.by_ref() {
                    cur.push(q);
                    if q == '(' {
                        depth += 1;
                    }
                    if q == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                if depth != 0 {
                    return Err("括弧が閉じていません".into());
                }
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    Ok(tokens)
}

/// `name(args)` 形式を分解
fn parse_call(t: &str) -> Option<(&str, String)> {
    let open = t.find('(')?;
    if !t.ends_with(')') {
        return None;
    }
    Some((&t[..open], t[open + 1..t.len() - 1].to_string()))
}

fn parse_pct(s: &str) -> Result<f64, String> {
    s.trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .map_err(|_| format!("数値を解釈できません: `{s}`"))
}

fn parse_pair(args: &str) -> Result<(f64, f64), String> {
    let (a, b) = args
        .split_once(',')
        .ok_or_else(|| format!("`{args}` は `x, y` 形式で指定してください"))?;
    Ok((parse_pct(a)?, parse_pct(b)?))
}

fn parse_endref(args: &str) -> Result<EndRef, String> {
    let args = args.trim();
    if let Some(rest) = args.strip_prefix('#') {
        let (id, edge) = match rest.rsplit_once('.') {
            Some((id, e)) => {
                let edge = match e {
                    "n" => Edge::N,
                    "s" => Edge::S,
                    "e" => Edge::E,
                    "w" => Edge::W,
                    "c" => Edge::C,
                    other => return Err(format!("不明な辺 `.{other}`(n/s/e/w/c)")),
                };
                (id.to_string(), edge)
            }
            None => (rest.to_string(), Edge::C),
        };
        Ok(EndRef::Anchor { id, edge })
    } else {
        let (x, y) = parse_pair(args)?;
        Ok(EndRef::Point(x, y))
    }
}

// ---- SVG 生成 ----

/// テーマカラートークン(@accent1 等)を CSS 変数へ解決。リテラル色は無害化して通す
fn color(v: &str) -> String {
    if let Some(name) = v.strip_prefix('@') {
        return format!("var(--mz-{name})");
    }
    v.chars()
        .filter(|c| c.is_alphanumeric() || "#(),.%- ".contains(*c))
        .collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

/// 図形の外接矩形(px)。id 参照の解決に使う
#[derive(Clone, Copy)]
struct Box_ {
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
}

impl Box_ {
    fn edge(&self, e: Edge) -> (f64, f64) {
        match e {
            Edge::N => (self.cx, self.cy - self.h / 2.0),
            Edge::S => (self.cx, self.cy + self.h / 2.0),
            Edge::E => (self.cx + self.w / 2.0, self.cy),
            Edge::W => (self.cx - self.w / 2.0, self.cy),
            Edge::C => (self.cx, self.cy),
        }
    }
}

/// SVG レイヤを生成する。`(w, h)` はスライドの論理ピクセル。
pub fn render_svg(doc: &ShapeDoc, w: u32, h: u32) -> (String, Vec<String>) {
    let mut errors = doc.errors.clone();
    let (wf, hf) = (w as f64, h as f64);
    let px = |p: (f64, f64)| (p.0 / 100.0 * wf, p.1 / 100.0 * hf);

    // 1 パス目: id → 矩形
    let mut boxes: BTreeMap<&str, Box_> = BTreeMap::new();
    for s in &doc.shapes {
        if let (Some(id), Some(at)) = (&s.id, s.at) {
            let (cx, cy) = px(at);
            let (bw, bh) = s.size.map(px).unwrap_or((0.0, 0.0));
            boxes.insert(
                id,
                Box_ {
                    cx,
                    cy,
                    w: bw,
                    h: bh,
                },
            );
        }
    }
    let resolve = |r: &EndRef, errors: &mut Vec<String>| -> Option<(f64, f64)> {
        match r {
            EndRef::Point(x, y) => Some(px((*x, *y))),
            EndRef::Anchor { id, edge } => match boxes.get(id.as_str()) {
                Some(b) => Some(b.edge(*edge)),
                None => {
                    errors.push(format!("shape: 参照先 `#{id}` が見つかりません"));
                    None
                }
            },
        }
    };

    // 2 パス目: 要素を出力
    let mut body = String::new();
    for s in &doc.shapes {
        let id_attr =
            s.id.as_ref()
                .map(|i| format!(" id=\"{}\"", esc(i)))
                .unwrap_or_default();
        let cls_attr = if s.classes.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", esc(&s.classes.join(" ")))
        };
        let stroke_w = s.kv.get("width").map(String::as_str).unwrap_or("2.5");
        let dash = if s.kv.get("style").map(String::as_str) == Some("dashed") {
            " stroke-dasharray=\"8 6\""
        } else {
            ""
        };
        match s.kind.unwrap() {
            ShapeKind::Rect | ShapeKind::Ellipse => {
                let (cx, cy) = px(s.at.unwrap());
                let (bw, bh) = px(s.size.unwrap());
                let fill = color(
                    s.kv.get("fill")
                        .map(String::as_str)
                        .unwrap_or("@shape-fill"),
                );
                let stroke = color(s.kv.get("stroke").map(String::as_str).unwrap_or("@accent1"));
                if s.kind == Some(ShapeKind::Rect) {
                    let rx = s.kv.get("radius").map(String::as_str).unwrap_or("10");
                    body.push_str(&format!(
                        "<rect{id_attr}{cls_attr} x=\"{:.1}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"{bh:.1}\" rx=\"{rx}\" style=\"fill:{fill};stroke:{stroke}\" stroke-width=\"{stroke_w}\"{dash}/>\n",
                        cx - bw / 2.0,
                        cy - bh / 2.0,
                    ));
                } else {
                    body.push_str(&format!(
                        "<ellipse{id_attr}{cls_attr} cx=\"{cx:.1}\" cy=\"{cy:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" style=\"fill:{fill};stroke:{stroke}\" stroke-width=\"{stroke_w}\"{dash}/>\n",
                        bw / 2.0,
                        bh / 2.0,
                    ));
                }
                if let Some(label) = &s.label {
                    body.push_str(&format!(
                        "<text x=\"{cx:.1}\" y=\"{cy:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" class=\"mz-shape-label\">{}</text>\n",
                        esc(label)
                    ));
                }
            }
            ShapeKind::Text => {
                let (cx, cy) = px(s.at.unwrap());
                let fill = color(s.kv.get("color").map(String::as_str).unwrap_or("@fg"));
                body.push_str(&format!(
                    "<text{id_attr}{cls_attr} x=\"{cx:.1}\" y=\"{cy:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" class=\"mz-shape-label{}\" style=\"fill:{fill}\">{}</text>\n",
                    s.classes
                        .iter()
                        .map(|c| format!(" {c}"))
                        .collect::<String>(),
                    esc(s.label.as_deref().unwrap_or(""))
                ));
            }
            ShapeKind::Arrow | ShapeKind::Line => {
                let (Some(a), Some(b)) = (
                    resolve(s.from.as_ref().unwrap(), &mut errors),
                    resolve(s.to.as_ref().unwrap(), &mut errors),
                ) else {
                    continue;
                };
                let stroke = color(s.kv.get("color").map(String::as_str).unwrap_or("@accent1"));
                body.push_str(&format!(
                    "<line{id_attr}{cls_attr} x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" style=\"stroke:{stroke}\" stroke-width=\"{stroke_w}\"{dash}/>\n",
                    a.0, a.1, b.0, b.1
                ));
                if s.kind == Some(ShapeKind::Arrow) {
                    body.push_str(&arrow_head(a, b, &stroke));
                }
            }
        }
    }

    let svg = format!(
        "<svg class=\"mz-shapes\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">\n{body}</svg>\n"
    );
    (svg, errors)
}

/// 矢印の先端(三角形)。線の終端角度から計算する
fn arrow_head(a: (f64, f64), b: (f64, f64), stroke: &str) -> String {
    let ang = (b.1 - a.1).atan2(b.0 - a.0);
    let len = 12.0;
    let spread = 0.45;
    let p1 = (
        b.0 - len * (ang - spread).cos(),
        b.1 - len * (ang - spread).sin(),
    );
    let p2 = (
        b.0 - len * (ang + spread).cos(),
        b.1 - len * (ang + spread).sin(),
    );
    format!(
        "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" style=\"fill:{stroke}\"/>\n",
        b.0, b.1, p1.0, p1.1, p2.0, p2.1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rect_with_label() {
        let doc = parse_shapes(
            r#"rect #cache at(70%, 30%) size(30%, 14%) label="キャッシュ" fill=@accent2"#,
        );
        assert!(doc.errors.is_empty());
        let s = &doc.shapes[0];
        assert_eq!(s.kind, Some(ShapeKind::Rect));
        assert_eq!(s.id.as_deref(), Some("cache"));
        assert_eq!(s.at, Some((70.0, 30.0)));
        assert_eq!(s.label.as_deref(), Some("キャッシュ"));
    }

    #[test]
    fn parse_arrow_between_shapes() {
        let doc = parse_shapes(
            "rect #a at(20,20) size(10,10)\nrect #b at(80,80) size(10,10)\narrow from(#a.s) to(#b.n) style=dashed",
        );
        assert!(doc.errors.is_empty());
        assert_eq!(
            doc.shapes[2].from,
            Some(EndRef::Anchor {
                id: "a".into(),
                edge: Edge::S
            })
        );
    }

    #[test]
    fn quoted_text_shape() {
        let doc = parse_shapes(r#"text #t at(50, 90) "ヒット率 95%" .small"#);
        assert!(doc.errors.is_empty());
        assert_eq!(doc.shapes[0].label.as_deref(), Some("ヒット率 95%"));
        assert_eq!(doc.shapes[0].classes, vec!["small"]);
    }

    #[test]
    fn missing_required_is_error() {
        let doc = parse_shapes("rect #a at(20,20)");
        assert_eq!(doc.errors.len(), 1);
    }

    #[test]
    fn svg_renders_and_resolves_refs() {
        let doc = parse_shapes("rect #a at(25,50) size(10,20)\narrow from(#a.e) to(75%, 50%)");
        let (svg, errors) = render_svg(&doc, 1280, 720);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(svg.contains("viewBox=\"0 0 1280 720\""));
        // #a の東端 = (25% + 5%) * 1280 = 384
        assert!(svg.contains("x1=\"384.0\""));
        assert!(svg.contains("<polygon")); // 矢印の先端
    }

    #[test]
    fn unknown_ref_reports_error() {
        let doc = parse_shapes("arrow from(#nope) to(50,50)");
        let (_, errors) = render_svg(&doc, 1280, 720);
        assert!(errors.iter().any(|e| e.contains("#nope")));
    }
}
