//! Parser for the `annotate` block DSL: circle the interesting part of a
//! screenshot, point an arrow at it, label it.
//!
//! ```text
//! target: shot                 # a pane name, or a #id
//! rect   40,22 18x12 : label="cache miss"
//! circle 62,40 20x20 : color=@accent1
//! circle #latency-1-2 : pad=6 label="the spike"
//! arrow  12,70 -> 38,30 : style=dashed
//! text   10,80 "throughput doubles here"
//! ```
//!
//! Coordinates are **percentages of the target's painted box** — the picture
//! itself for an image, the drawn chart for a chart, the border box for
//! anything else — so an annotation follows its target when the layout
//! changes. An item that names an `#id` instead of coordinates is anchored to
//! that element's live bounding box and needs no coordinates at all.
//!
//! This crate is pure: text in, the C2 model out
//! (see `docs/workstreams.md#c2-annotation-model`). Locating the target in
//! the rendered slide, and drawing, are the renderer's and the runtime's jobs.

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Rect,
    Circle,
    Arrow,
    Text,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Rect => "rect",
            Kind::Circle => "circle",
            Kind::Arrow => "arrow",
            Kind::Text => "text",
        }
    }
}

/// One end of an arrow, or the placement of a shape: literal percentages of
/// the target box, or the live bounding box of another element.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    At(f64, f64),
    Anchor(String),
}

#[derive(Debug, Clone)]
pub struct Item {
    pub kind: Kind,
    /// `id=` on the item, put on the drawn shape so the rest of the deck can
    /// refer to it — a `connect` arrow from a sentence to the circle, for
    /// instance. The mark is drawn from the live layout, so pointing at it
    /// keeps working when the layout moves.
    pub id: Option<String>,
    pub place: Place,
    /// `w x h` for rect/circle placed by coordinates; unused for anchors.
    pub size: Option<(f64, f64)>,
    /// Arrow head end.
    pub to: Option<Place>,
    /// Breathing room in slide pixels around an anchored box.
    pub pad: Option<f64>,
    pub label: Option<String>,
    pub color: Option<String>,
    pub dashed: bool,
    /// The click step this item appears on; 0 means it is there from the
    /// start. A deck read without the viewer — the PDF included — shows every
    /// item regardless, the way an animated slide prints fully revealed.
    pub step: u32,
}

#[derive(Debug, Default)]
pub struct AnnotDoc {
    /// `#id`, or a bare pane name; the renderer resolves it to a selector.
    pub target: Option<String>,
    pub items: Vec<Item>,
    pub errors: Vec<String>,
}

pub fn parse(src: &str) -> AnnotDoc {
    let mut doc = AnnotDoc::default();
    for (ln, raw) in src.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if let Some(t) = line.strip_prefix("target:") {
            let t = t.trim();
            if t.is_empty() {
                doc.errors
                    .push(format!("annotate line {}: empty target", ln + 1));
            } else {
                doc.target = Some(t.to_string());
            }
            continue;
        }
        match parse_item(line) {
            Ok(item) => doc.items.push(item),
            Err(e) => doc.errors.push(format!("annotate line {}: {e}", ln + 1)),
        }
    }
    if doc.target.is_none() && !doc.items.is_empty() {
        doc.errors
            .push("annotate block has no `target:` line".to_string());
    }
    doc
}

fn parse_item(line: &str) -> Result<Item, String> {
    // `kind geometry [: attributes]` — but a quoted string may contain `:`,
    // so split on the *last* ` : ` outside quotes... quotes only appear in
    // text content and attribute values, both after the separator, so the
    // first ` : ` is the separator.
    let (head, attrs) = match line.find(" : ") {
        Some(i) => (&line[..i], line[i + 3..].trim()),
        None => (line, ""),
    };
    let mut words = head.split_whitespace();
    let kind = match words.next() {
        Some("rect") => Kind::Rect,
        Some("circle") => Kind::Circle,
        Some("arrow") => Kind::Arrow,
        Some("text") => Kind::Text,
        Some(other) => return Err(format!("unknown annotation `{other}`")),
        None => unreachable!("blank lines are skipped"),
    };
    let rest = words.collect::<Vec<_>>().join(" ");
    let rest = rest.trim();

    let mut item = Item {
        kind,
        id: None,
        place: Place::At(0.0, 0.0),
        size: None,
        to: None,
        pad: None,
        label: None,
        color: None,
        dashed: false,
        step: 0,
    };

    match kind {
        Kind::Rect | Kind::Circle => {
            let mut parts = rest.split_whitespace();
            let first = parts.next().ok_or("missing position")?;
            item.place = parse_place(first)?;
            match (&item.place, parts.next()) {
                (Place::At(..), Some(sz)) => item.size = Some(parse_size(sz)?),
                (Place::At(..), None) => {
                    return Err("coordinates need a size, e.g. `40,22 18x12`".into())
                }
                (Place::Anchor(_), Some(extra)) => {
                    return Err(format!(
                        "an anchored {} takes no size (got `{extra}`); use pad=",
                        kind.as_str()
                    ))
                }
                (Place::Anchor(_), None) => {}
            }
            if parts.next().is_some() {
                return Err("too many fields before `:`".into());
            }
        }
        Kind::Arrow => {
            let (from, to) = rest
                .split_once("->")
                .ok_or("arrow is written `from -> to`")?;
            item.place = parse_place(from.trim())?;
            item.to = Some(parse_place(to.trim())?);
        }
        Kind::Text => {
            let (pos, text) = rest
                .split_once(char::is_whitespace)
                .ok_or("text is written `x,y \"content\"`")?;
            item.place = parse_place(pos)?;
            let text = text.trim();
            let unquoted = text
                .strip_prefix('"')
                .and_then(|t| t.strip_suffix('"'))
                .ok_or("text content must be quoted")?;
            item.label = Some(unquoted.to_string());
        }
    }

    for (k, v) in parse_attrs(attrs)? {
        match k {
            "label" => item.label = Some(v),
            "color" => item.color = Some(v),
            "id" => {
                if v.is_empty()
                    || !v
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(format!("`{v}` is not a valid id"));
                }
                item.id = Some(v);
            }
            "pad" => {
                item.pad = Some(
                    v.parse::<f64>()
                        .map_err(|_| format!("pad is a number of pixels, got `{v}`"))?,
                )
            }
            "step" => {
                item.step = v
                    .parse::<u32>()
                    .map_err(|_| format!("step is a click number, got `{v}`"))?
            }
            "style" if v == "dashed" => item.dashed = true,
            "style" => return Err(format!("unknown style `{v}` (only `dashed`)")),
            other => return Err(format!("unknown attribute `{other}=`")),
        }
    }
    if item.pad.is_some() && !matches!(item.place, Place::Anchor(_)) {
        return Err("pad= only applies to an anchored item".into());
    }
    Ok(item)
}

fn parse_place(s: &str) -> Result<Place, String> {
    if let Some(id) = s.strip_prefix('#') {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("`{s}` is not a valid id"));
        }
        return Ok(Place::Anchor(id.to_string()));
    }
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| format!("`{s}` is neither `x,y` nor `#id`"))?;
    Ok(Place::At(parse_num(x)?, parse_num(y)?))
}

fn parse_size(s: &str) -> Result<(f64, f64), String> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| format!("`{s}` is not a `WxH` size"))?;
    Ok((parse_num(w)?, parse_num(h)?))
}

fn parse_num(s: &str) -> Result<f64, String> {
    s.trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .map_err(|_| format!("not a number: `{}`", s.trim()))
}

/// `key=value` pairs; a value may be quoted to contain spaces.
fn parse_attrs(src: &str) -> Result<Vec<(&str, String)>, String> {
    let mut out = Vec::new();
    let mut rest = src.trim();
    while !rest.is_empty() {
        let eq = rest
            .find('=')
            .ok_or_else(|| format!("expected key=value, got `{rest}`"))?;
        let key = rest[..eq].trim();
        let after = &rest[eq + 1..];
        let (value, tail) = if let Some(q) = after.strip_prefix('"') {
            let end = q.find('"').ok_or("unclosed quote")?;
            (q[..end].to_string(), &q[end + 1..])
        } else {
            match after.find(char::is_whitespace) {
                Some(i) => (after[..i].to_string(), &after[i..]),
                None => (after.to_string(), ""),
            }
        };
        out.push((key, value));
        rest = tail.trim_start();
    }
    Ok(out)
}

/// Resolves `@token` colors to CSS variables; literals are sanitized the same
/// way `shape` colors are.
fn color_css(v: &str) -> String {
    if let Some(name) = v.strip_prefix('@') {
        return format!("var(--mz-{name})");
    }
    v.chars()
        .filter(|c| c.is_alphanumeric() || "#(),.%- ".contains(*c))
        .collect()
}

fn esc_json(s: &str) -> String {
    serde_json::to_string(s).expect("strings always serialize")
}

/// The C2 JSON for one block's items. The `target` is not in the blob — it
/// rides on the `<script>` tag's `data-target` attribute, where the runtime
/// needs it before parsing anything.
pub fn to_json(doc: &AnnotDoc) -> String {
    let mut out = String::from("{\"items\":[");
    for (i, item) in doc.items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{{\"kind\":\"{}\"", item.kind.as_str());
        if let Some(id) = &item.id {
            let _ = write!(out, ",\"id\":{}", esc_json(id));
        }
        match &item.place {
            Place::At(x, y) => {
                let _ = write!(out, ",\"x\":{x},\"y\":{y}");
            }
            Place::Anchor(id) => {
                let _ = write!(out, ",\"anchor\":{}", esc_json(id));
            }
        }
        if let Some((w, h)) = item.size {
            let _ = write!(out, ",\"w\":{w},\"h\":{h}");
        }
        match &item.to {
            Some(Place::At(x, y)) => {
                let _ = write!(out, ",\"x2\":{x},\"y2\":{y}");
            }
            Some(Place::Anchor(id)) => {
                let _ = write!(out, ",\"anchor2\":{}", esc_json(id));
            }
            None => {}
        }
        if let Some(p) = item.pad {
            let _ = write!(out, ",\"pad\":{p}");
        }
        if let Some(l) = &item.label {
            let _ = write!(out, ",\"label\":{}", esc_json(l));
        }
        if let Some(c) = &item.color {
            let _ = write!(out, ",\"color\":{}", esc_json(&color_css(c)));
        }
        if item.dashed {
            out.push_str(",\"dashed\":true");
        }
        if item.step > 0 {
            let _ = write!(out, ",\"step\":{}", item.step);
        }
        out.push('}');
    }
    out.push_str("]}");
    out
}

/// Every `#id` the block references: anchors, arrow endpoints. The renderer
/// warns when one matches nothing on the slide.
pub fn referenced_ids(doc: &AnnotDoc) -> Vec<&str> {
    let mut ids = Vec::new();
    for item in &doc.items {
        if let Place::Anchor(id) = &item.place {
            ids.push(id.as_str());
        }
        if let Some(Place::Anchor(id)) = &item.to {
            ids.push(id.as_str());
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_workstream_example() {
        let doc = parse(
            "target: #fig1\n\
             rect   40,22 18x12 : label=\"cache miss\"\n\
             arrow  12,70 -> 38,30\n\
             text   10,80 \"throughput doubles here\"\n",
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.target.as_deref(), Some("#fig1"));
        assert_eq!(doc.items.len(), 3);
        assert_eq!(doc.items[0].kind, Kind::Rect);
        assert_eq!(doc.items[0].place, Place::At(40.0, 22.0));
        assert_eq!(doc.items[0].size, Some((18.0, 12.0)));
        assert_eq!(doc.items[0].label.as_deref(), Some("cache miss"));
        assert_eq!(doc.items[1].to, Some(Place::At(38.0, 30.0)));
        assert_eq!(
            doc.items[2].label.as_deref(),
            Some("throughput doubles here")
        );
    }

    #[test]
    fn anchored_circle_with_pad() {
        let doc = parse("target: chart\ncircle #latency-1-2 : pad=6 label=\"the spike\"\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].place, Place::Anchor("latency-1-2".into()));
        assert_eq!(doc.items[0].pad, Some(6.0));
        assert_eq!(referenced_ids(&doc), vec!["latency-1-2"]);
    }

    #[test]
    fn arrow_may_end_on_an_anchor() {
        let doc = parse("target: shot\narrow 10,80 -> #mark\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].to, Some(Place::Anchor("mark".into())));
    }

    #[test]
    fn missing_target_is_an_error() {
        let doc = parse("rect 10,10 5x5\n");
        assert!(doc.errors.iter().any(|e| e.contains("no `target:`")));
    }

    #[test]
    fn coordinates_without_size_are_an_error() {
        let doc = parse("target: x\nrect 10,10\n");
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].contains("size"));
    }

    #[test]
    fn pad_on_a_coordinate_item_is_an_error() {
        let doc = parse("target: x\nrect 10,10 5x5 : pad=4\n");
        assert!(doc.errors[0].contains("anchored"));
    }

    #[test]
    fn unknown_kind_reports_the_line() {
        let doc = parse("target: x\nblob 10,10\n");
        assert!(doc.errors[0].contains("line 2"));
        assert!(doc.errors[0].contains("blob"));
    }

    #[test]
    fn json_has_the_c2_shape() {
        let doc = parse(
            "target: shot\n\
             rect 40,22 18x12 : label=\"cache miss\" color=@accent2\n\
             circle #m-3 : pad=6\n\
             arrow 12,70 -> 38,30 : style=dashed\n",
        );
        let json: serde_json::Value = serde_json::from_str(&to_json(&doc)).unwrap();
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["kind"], "rect");
        assert_eq!(items[0]["x"], 40.0);
        assert_eq!(items[0]["w"], 18.0);
        assert_eq!(items[0]["label"], "cache miss");
        assert_eq!(items[0]["color"], "var(--mz-accent2)");
        assert_eq!(items[1]["anchor"], "m-3");
        assert_eq!(items[1]["pad"], 6.0);
        assert_eq!(items[2]["x2"], 38.0);
        assert_eq!(items[2]["dashed"], true);
    }

    #[test]
    fn literal_colors_are_sanitized() {
        let doc = parse("target: x\nrect 1,1 2x2 : color=\"red;} body{\"\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let json: serde_json::Value = serde_json::from_str(&to_json(&doc)).unwrap();
        // `;`, `{` and `}` are gone: nothing an attribute value can smuggle
        // into a style survives the filter.
        assert_eq!(json["items"][0]["color"], "red body");
    }

    #[test]
    fn an_item_may_carry_an_id_for_the_rest_of_the_deck_to_point_at() {
        let doc = parse("target: fig\ncircle 40,30 20x20 : id=a-hot label=\"here\"\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].id.as_deref(), Some("a-hot"));
        assert!(to_json(&doc).contains("\"id\":\"a-hot\""));
    }

    #[test]
    fn an_id_that_would_not_survive_a_selector_is_refused() {
        let doc = parse("target: fig\ncircle 40,30 20x20 : id=\"a b\"\n");
        assert!(doc.errors[0].contains("not a valid id"), "{:?}", doc.errors);
    }

    #[test]
    fn an_item_may_wait_for_a_click() {
        let doc = parse("target: fig\ncircle 40,30 20x20 : step=2 label=\"here\"\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].step, 2);
        assert!(to_json(&doc).contains("\"step\":2"));
    }

    #[test]
    fn an_item_with_no_step_is_there_from_the_start() {
        let doc = parse("target: fig\ncircle 40,30 20x20\n");
        assert_eq!(doc.items[0].step, 0);
        // Absent rather than zero: the runtime's default is "always shown".
        assert!(!to_json(&doc).contains("step"));
    }

    #[test]
    fn a_step_that_is_not_a_number_is_refused() {
        let doc = parse("target: fig\ncircle 40,30 20x20 : step=soon\n");
        assert!(doc.errors[0].contains("click number"), "{:?}", doc.errors);
    }

    #[test]
    fn quoted_label_may_contain_colons_and_spaces() {
        let doc = parse("target: x\ntext 5,5 \"p95: down 40%\"\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].label.as_deref(), Some("p95: down 40%"));
    }
}
