//! Parser for the `shape` block DSL, plus build-time SVG layer generation.
//!
//! Coordinates are percentages (0-100) of a [`Frame`] — the whole slide for a
//! top-level block, a pane's rectangle for a block written inside one. They
//! are converted to the slide's logical pixels inside a fixed `viewBox`, so
//! shapes scale with the slide; labels, stroke widths and arrowheads are in
//! those pixels directly, so a small frame scales a drawing's geometry
//! without shrinking its typography.
//!
//! ```text
//! rect    #cache at(70%, 30%) size(30%, 14%) label="Cache layer" fill=@accent2
//! ellipse #db    at(70%, 70%) size(26%, 16%) label="DB"
//! arrow   #a1    from(#cache.s) to(#db.n) style=dashed
//! line           from(10%, 90%) to(40%, 90%)
//! text    #cap   at(20%, 85%) "95% hit rate" .small
//! ```

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Text,
    Arrow,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum Edge {
    N,
    S,
    E,
    W,
    C,
}

/// An endpoint: either a literal point or an edge of another shape.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum EndRef {
    Point(f64, f64),
    Anchor { id: String, edge: Edge },
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShapeDoc {
    pub shapes: Vec<Shape>,
    pub errors: Vec<String>,
}

/// Parses a shape block; one shape per line.
pub fn parse_shapes(src: &str) -> ShapeDoc {
    let mut shapes = Vec::new();
    let mut errors = Vec::new();
    for (ln, line) in src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') && line.len() > 1 && line.as_bytes()[1] == b' '
        {
            // Skip blank lines and `# comment` lines.
            continue;
        }
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(Some(s)) => shapes.push(s),
            Ok(None) => {}
            Err(e) => errors.push(format!("shape line {}: {e}", ln + 1)),
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
        other => return Err(format!("unknown shape kind `{other}`")),
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
                other => return Err(format!("unknown setting `{other}(...)`")),
            }
        } else if let Some((k, v)) = t.split_once('=') {
            if k == "label" {
                s.label = Some(v.trim_matches('"').to_string());
            } else {
                s.kv.insert(k.to_string(), v.trim_matches('"').to_string());
            }
        } else {
            return Err(format!("unrecognized token `{t}`"));
        }
    }
    validate(&s)?;
    Ok(Some(s))
}

fn validate(s: &Shape) -> Result<(), String> {
    match s.kind.unwrap() {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            if s.at.is_none() || s.size.is_none() {
                return Err("rect/ellipse require at(x,y) and size(w,h)".into());
            }
        }
        ShapeKind::Text => {
            if s.at.is_none() || s.label.is_none() {
                return Err("text requires at(x,y) and a quoted string".into());
            }
        }
        ShapeKind::Arrow | ShapeKind::Line => {
            if s.from.is_none() || s.to.is_none() {
                return Err("arrow/line require from(...) and to(...)".into());
            }
        }
    }
    Ok(())
}

/// Tokenizes a line, keeping quoted strings and parenthesized groups intact.
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
                    return Err("unclosed parenthesis".into());
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

/// Splits a `name(args)` call.
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
        .map_err(|_| format!("not a number: `{s}`"))
}

fn parse_pair(args: &str) -> Result<(f64, f64), String> {
    let (a, b) = args
        .split_once(',')
        .ok_or_else(|| format!("`{args}` must be written as `x, y`"))?;
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
                    other => return Err(format!("unknown edge `.{other}` (expected n/s/e/w/c)")),
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

// ---- SVG generation ----

/// Resolves a theme token such as `@accent1` to a CSS variable; literal colors are sanitized.
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

/// A shape's bounding box in pixels, used to resolve id references.
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

/// The rectangle a block's percentages map into: the whole slide for a
/// top-level `shape` block, the pane's rectangle for one written inside a
/// `::: pane`. Coordinates are not clamped to it — a shape may deliberately
/// reach past the frame, the way a page-level shape may reach past a pane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Frame {
    /// The whole slide.
    pub fn page(w: u32, h: u32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: w as f64,
            h: h as f64,
        }
    }

    fn px(&self, p: (f64, f64)) -> (f64, f64) {
        (self.x + p.0 / 100.0 * self.w, self.y + p.1 / 100.0 * self.h)
    }

    /// A size has no origin: only the frame's scale applies.
    fn px_size(&self, p: (f64, f64)) -> (f64, f64) {
        (p.0 / 100.0 * self.w, p.1 / 100.0 * self.h)
    }
}

/// Generates the SVG layer for one page-coordinate block. `(w, h)` is the
/// slide's logical pixel size.
pub fn render_svg(doc: &ShapeDoc, w: u32, h: u32) -> (String, Vec<String>) {
    render_layer(&[(doc, Frame::page(w, h))], w, h)
}

/// Generates one SVG layer from several blocks, each drawn in its own frame.
/// Ids are resolved across the whole layer, so an arrow in one block may end
/// on a shape another block drew — a pane-anchored box and a page-anchored
/// caption are still one picture.
pub fn render_layer(blocks: &[(&ShapeDoc, Frame)], w: u32, h: u32) -> (String, Vec<String>) {
    let mut errors: Vec<String> = Vec::new();

    // Pass 1: map ids to boxes, in final pixels, across every block.
    let mut boxes: BTreeMap<&str, Box_> = BTreeMap::new();
    for (doc, frame) in blocks {
        errors.extend(doc.errors.iter().cloned());
        for s in &doc.shapes {
            if let (Some(id), Some(at)) = (&s.id, s.at) {
                let (cx, cy) = frame.px(at);
                let (bw, bh) = s.size.map(|p| frame.px_size(p)).unwrap_or((0.0, 0.0));
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
    }

    // Pass 2: emit the elements.
    let mut body = String::new();
    for (doc, frame) in blocks {
        let px = |p: (f64, f64)| frame.px(p);
        let resolve = |r: &EndRef, errors: &mut Vec<String>| -> Option<(f64, f64)> {
            match r {
                EndRef::Point(x, y) => Some(px((*x, *y))),
                EndRef::Anchor { id, edge } => match boxes.get(id.as_str()) {
                    Some(b) => Some(b.edge(*edge)),
                    None => {
                        errors.push(format!("shape: no element with id `#{id}`"));
                        None
                    }
                },
            }
        };
        for s in &doc.shapes {
            // A shape's parts are emitted together and, when it has an id, wrapped
            // in a group carrying it. A labelled box is a box *and* its text, and
            // an arrow is a line *and* its head: an id on the geometry alone would
            // let `anim` move half a shape and leave the rest behind.
            let mut part = String::new();
            let id_attr = String::new();
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
                    let (bw, bh) = frame.px_size(s.size.unwrap());
                    let fill = color(
                        s.kv.get("fill")
                            .map(String::as_str)
                            .unwrap_or("@shape-fill"),
                    );
                    let stroke =
                        color(s.kv.get("stroke").map(String::as_str).unwrap_or("@accent1"));
                    if s.kind == Some(ShapeKind::Rect) {
                        let rx = s.kv.get("radius").map(String::as_str).unwrap_or("10");
                        part.push_str(&format!(
                        "<rect{id_attr}{cls_attr} x=\"{:.1}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"{bh:.1}\" rx=\"{rx}\" style=\"fill:{fill};stroke:{stroke}\" stroke-width=\"{stroke_w}\"{dash}/>\n",
                        cx - bw / 2.0,
                        cy - bh / 2.0,
                    ));
                    } else {
                        part.push_str(&format!(
                        "<ellipse{id_attr}{cls_attr} cx=\"{cx:.1}\" cy=\"{cy:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" style=\"fill:{fill};stroke:{stroke}\" stroke-width=\"{stroke_w}\"{dash}/>\n",
                        bw / 2.0,
                        bh / 2.0,
                    ));
                    }
                    if let Some(label) = &s.label {
                        part.push_str(&format!(
                        "<text x=\"{cx:.1}\" y=\"{cy:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" class=\"mz-shape-label\">{}</text>\n",
                        esc(label)
                    ));
                    }
                }
                ShapeKind::Text => {
                    let (cx, cy) = px(s.at.unwrap());
                    let fill = color(s.kv.get("color").map(String::as_str).unwrap_or("@fg"));
                    part.push_str(&format!(
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
                    part.push_str(&format!(
                    "<line{id_attr}{cls_attr} x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" style=\"stroke:{stroke}\" stroke-width=\"{stroke_w}\"{dash}/>\n",
                    a.0, a.1, b.0, b.1
                ));
                    if s.kind == Some(ShapeKind::Arrow) {
                        part.push_str(&arrow_head(a, b, &stroke));
                    }
                }
            }
            match &s.id {
                Some(id) => {
                    body.push_str(&format!("<g id=\"{}\">\n", esc(id)));
                    body.push_str(&part);
                    body.push_str("</g>\n");
                }
                None => body.push_str(&part),
            }
        }
    }

    let svg = format!(
        "<svg class=\"mz-shapes\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">\n{body}</svg>\n"
    );
    (svg, errors)
}

/// The arrowhead triangle, oriented by the line's end angle.
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
        let doc =
            parse_shapes(r#"rect #cache at(70%, 30%) size(30%, 14%) label="Cache" fill=@accent2"#);
        assert!(doc.errors.is_empty());
        let s = &doc.shapes[0];
        assert_eq!(s.kind, Some(ShapeKind::Rect));
        assert_eq!(s.id.as_deref(), Some("cache"));
        assert_eq!(s.at, Some((70.0, 30.0)));
        assert_eq!(s.label.as_deref(), Some("Cache"));
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
        let doc = parse_shapes(r#"text #t at(50, 90) "95% hit rate" .small"#);
        assert!(doc.errors.is_empty());
        assert_eq!(doc.shapes[0].label.as_deref(), Some("95% hit rate"));
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
        // East edge of #a = (25% + 5%) * 1280 = 384
        assert!(svg.contains("x1=\"384.0\""));
        assert!(svg.contains("<polygon")); // arrowhead
    }

    #[test]
    fn unknown_ref_reports_error() {
        let doc = parse_shapes("arrow from(#nope) to(50,50)");
        let (_, errors) = render_svg(&doc, 1280, 720);
        assert!(errors.iter().any(|e| e.contains("#nope")));
    }

    /// A framed block's percentages map into its frame — origin for positions,
    /// scale alone for sizes — and nothing clamps to the frame's edges.
    #[test]
    fn a_framed_block_draws_in_its_frame_without_clamping() {
        let doc = parse_shapes("rect #a at(50,50) size(20,20)\nrect #b at(110,50) size(20,20)");
        let frame = Frame {
            x: 640.0,
            y: 360.0,
            w: 500.0,
            h: 300.0,
        };
        let (svg, errors) = render_layer(&[(&doc, frame)], 1280, 720);
        assert!(errors.is_empty(), "{errors:?}");
        // #a: centre (640 + 250, 360 + 150), size (100, 60) → x = 840, y = 480.
        assert!(svg.contains("x=\"840.0\" y=\"480.0\" width=\"100.0\" height=\"60.0\""));
        // #b sits at 110% of the frame — past its edge, kept as written.
        assert!(svg.contains("x=\"1140.0\""));
    }

    /// Ids resolve across the whole layer: an arrow in a page block may end on
    /// a shape a pane block drew, and the endpoint is in final pixels.
    #[test]
    fn refs_resolve_across_blocks_in_different_frames() {
        let pane = parse_shapes("rect #target at(50,50) size(20,20)");
        let page = parse_shapes("arrow from(10%, 50%) to(#target.w)");
        let frame = Frame {
            x: 640.0,
            y: 0.0,
            w: 640.0,
            h: 720.0,
        };
        let (svg, errors) = render_layer(
            &[(&page, Frame::page(1280, 720)), (&pane, frame)],
            1280,
            720,
        );
        assert!(errors.is_empty(), "{errors:?}");
        // #target centre x = 640 + 320 = 960, west edge = 960 - 64 = 896.
        assert!(svg.contains("x2=\"896.0\""), "{svg}");
    }
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use super::*;

    /// The parsed model survives a JSON round trip byte-for-byte: what the
    /// consumer deserializes is what the parser produced.
    #[test]
    fn shape_doc_round_trips_through_json() {
        let doc = parse_shapes(
            "rect #cache at(70%, 30%) size(30%, 14%) label=\"Cache\" fill=@accent2\narrow from(#cache.s) to(75%, 80%) style=dashed\nbadline",
        );
        assert!(!doc.shapes.is_empty());
        assert!(!doc.errors.is_empty());
        let json = serde_json::to_string(&doc).unwrap();
        // Enums serialize kebab-case: a `rect` is "rect", an edge is "s".
        assert!(json.contains("\"kind\":\"rect\""), "{json}");
        assert!(json.contains("\"edge\":\"s\""), "{json}");
        let back: ShapeDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}
