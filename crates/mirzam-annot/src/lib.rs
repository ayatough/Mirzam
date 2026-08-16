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
//! An annotation may also mark **words**, which is how a phrase in a sentence
//! is tied to a mark on a chart without an arrow crossing the slide:
//!
//! ```text
//! highlight #t-ap    : color=@accent2 step=1
//! rect      #lat-0-2 : color=@accent2 step=1 pad=8
//! ```
//!
//! Both are annotation items with the same `step`, so they arrive together and
//! in the same colour — which a room reads as a pairing instantly, with
//! nothing travelling between them.
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
    /// The three below mark *words* rather than a region of a picture, and
    /// follow the lines the words are laid out on rather than one union box.
    Highlight,
    Underline,
    Box,
}

impl Kind {
    /// Whether the mark follows text. A phrase that wraps is two line boxes,
    /// not one rectangle with a hole in the middle of the sentence.
    pub fn marks_text(self) -> bool {
        matches!(self, Kind::Highlight | Kind::Underline | Kind::Box)
    }
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Rect => "rect",
            Kind::Circle => "circle",
            Kind::Arrow => "arrow",
            Kind::Text => "text",
            Kind::Highlight => "highlight",
            Kind::Underline => "underline",
            Kind::Box => "box",
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

/// Where an item's numbers were written, as byte ranges within the block body
/// [`parse`] was handed.
///
/// Only the coordinate forms have any: an item anchored to `#id` names an
/// element and takes its box from the live layout, so there is nothing in the
/// source for a drag to move. Composed with a slide's offset and
/// `mirzam_syntax::SourceMap::resolve`, each range is the exact span of the
/// file that one dragged handle would rewrite — nothing else on the line, and
/// nothing of the author's spacing or attribute order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Numbers {
    /// `40,22` — the position, which for a rect or circle is its centre.
    pub at: Option<std::ops::Range<usize>>,
    /// `18x12` — the size.
    pub size: Option<std::ops::Range<usize>>,
    /// The arrow head's `x,y`.
    pub to: Option<std::ops::Range<usize>>,
}

impl Numbers {
    /// Whether anything here can be moved by dragging. False for every
    /// anchored item, and for the three text marks, which never carry numbers.
    pub fn draggable(&self) -> bool {
        self.at.is_some() || self.size.is_some() || self.to.is_some()
    }
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
    /// The whole line this item was written on, as a byte range within the
    /// block body — the unit to rewrite when a change is more than a number.
    pub line: std::ops::Range<usize>,
    /// The byte ranges of this item's numbers within that same block body.
    pub numbers: Numbers,
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
    // Walked rather than `lines()`ed because every item records where it was
    // written: the byte range is what lets a change made in the preview go
    // back to the file as a change to *that line*, leaving the rest of the
    // block — its spacing, its comments, the columns the author aligned —
    // exactly as typed.
    let mut at = 0usize;
    for (ln, raw) in src.split_inclusive('\n').enumerate() {
        let start = at;
        at += raw.len();
        let raw = raw.trim_end_matches(['\n', '\r']);
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
            Ok(mut item) => {
                item.line = start..start + raw.len();
                // `line` is `raw` trimmed, so a span found within it is offset
                // by however much whitespace the author indented with.
                let indent = start + (raw.len() - raw.trim_start().len());
                item.numbers = shift(number_spans(line, &item), indent);
                doc.items.push(item);
            }
            Err(e) => doc.errors.push(format!("annotate line {}: {e}", ln + 1)),
        }
    }
    // `target:` says which box percentages are measured against. A block whose
    // items are all anchored measures nothing, so requiring one would mean
    // naming a box the author never refers to - which is exactly the shape of a
    // block that pairs a phrase with a chart mark.
    let all_anchored = doc
        .items
        .iter()
        .all(|i| matches!(i.place, Place::Anchor(_)) && !matches!(i.to, Some(Place::At(..))));
    if doc.target.is_none() && !doc.items.is_empty() && !all_anchored {
        doc.errors.push(
            "annotate block has no `target:` line, and an item is placed by coordinates \
             (which are percentages of the target)"
                .to_string(),
        );
    }
    doc
}

/// Whitespace-separated tokens with their byte offsets in `s`. The parser
/// below reads the same tokens through `split_whitespace`, which drops the
/// offsets; this walks the string once to get them back.
fn tokens(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            if let Some(b) = start.take() {
                out.push((b, &s[b..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(b) = start {
        out.push((b, &s[b..]));
    }
    out
}

fn shift(n: Numbers, by: usize) -> Numbers {
    let go = |r: Option<std::ops::Range<usize>>| r.map(|r| r.start + by..r.end + by);
    Numbers {
        at: go(n.at),
        size: go(n.size),
        to: go(n.to),
    }
}

/// Locates an already-parsed item's numbers in the line it was written on.
///
/// Kept separate from `parse_item` rather than threaded through it: the parse
/// has already decided what shape the line is, so this only has to walk the
/// same tokens again knowing the answer. One walk that can be wrong is better
/// than a parser rewritten to carry offsets through every branch.
fn number_spans(line: &str, item: &Item) -> Numbers {
    let head = match line.find(" : ") {
        Some(i) => &line[..i],
        None => line,
    };
    let span = |t: &(usize, &str)| t.0..t.0 + t.1.len();
    let mut n = Numbers::default();
    if item.kind == Kind::Arrow {
        // `from -> to`, written with or without spaces around the arrow.
        let Some(i) = head.find("->") else {
            return n;
        };
        let left = tokens(&head[..i]);
        // The first token is the word `arrow`; the position is what follows it.
        if matches!(item.place, Place::At(..)) && left.len() >= 2 {
            n.at = left.last().map(span);
        }
        if matches!(item.to, Some(Place::At(..))) {
            let right = &head[i + 2..];
            n.to = tokens(right)
                .first()
                .map(|t| i + 2 + t.0..i + 2 + t.0 + t.1.len());
        }
        return n;
    }
    if !matches!(item.place, Place::At(..)) {
        return n; // anchored: the element's own box, nothing written down
    }
    let toks = tokens(head);
    n.at = toks.get(1).map(span);
    if matches!(item.kind, Kind::Rect | Kind::Circle) {
        n.size = toks.get(2).map(span);
    }
    n
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
        Some("highlight") => Kind::Highlight,
        Some("underline") => Kind::Underline,
        Some("box") => Kind::Box,
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
        line: 0..0,
        numbers: Numbers::default(),
    };

    match kind {
        // A text mark names the phrase and nothing else: where the words are is
        // the browser's business, and a percentage would be a guess that goes
        // stale the moment the sentence is edited.
        Kind::Highlight | Kind::Underline | Kind::Box => {
            let mut parts = rest.split_whitespace();
            let first = parts.next().ok_or_else(|| {
                format!("`{}` marks a phrase, so it needs an `#id`", kind.as_str())
            })?;
            item.place = parse_place(first)?;
            if !matches!(item.place, Place::Anchor(_)) {
                return Err(format!(
                    "`{}` marks a phrase written `[like this]{{#id}}`, so it takes an `#id` \
                     rather than coordinates",
                    kind.as_str()
                ));
            }
            if parts.next().is_some() {
                return Err("too many fields before `:`".into());
            }
        }
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
        // The item's ordinal in its block, which is the only name it has that
        // the source and the drawn mark agree on: `id=` is optional and the
        // author's, and two items may sit at the same coordinates. Written
        // out rather than left to the runtime to count, so that what points
        // back at a line of Markdown is stated by the thing that read it.
        let _ = write!(out, "{{\"i\":{i},\"kind\":\"{}\"", item.kind.as_str());
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

    /// The bytes a span picks out of the source it was parsed from. Every
    /// assertion about a range below reads it back this way rather than
    /// comparing numbers, because a number that is wrong by two is a number
    /// that still looks plausible.
    fn at<'a>(src: &'a str, r: &Option<std::ops::Range<usize>>) -> Option<&'a str> {
        r.as_ref().map(|r| &src[r.clone()])
    }

    #[test]
    fn a_rect_records_where_its_numbers_are() {
        let src = "target: fig\nrect 40,22 18x12 : label=\"cache miss\"\n";
        let doc = parse(src);
        let n = &doc.items[0].numbers;
        assert_eq!(at(src, &n.at), Some("40,22"));
        assert_eq!(at(src, &n.size), Some("18x12"));
        assert_eq!(n.to, None);
        assert!(n.draggable());
        assert_eq!(
            &src[doc.items[0].line.clone()],
            "rect 40,22 18x12 : label=\"cache miss\""
        );
    }

    #[test]
    fn an_arrow_records_both_ends() {
        let src = "target: fig\narrow  12,70 -> 38,30 : step=2\n";
        let doc = parse(src);
        let n = &doc.items[0].numbers;
        assert_eq!(at(src, &n.at), Some("12,70"));
        assert_eq!(at(src, &n.to), Some("38,30"));
        assert_eq!(n.size, None);
    }

    #[test]
    fn an_arrow_written_without_spaces_still_records_both_ends() {
        let src = "target: fig\narrow 12,70->38,30\n";
        let doc = parse(src);
        let n = &doc.items[0].numbers;
        assert_eq!(at(src, &n.at), Some("12,70"));
        assert_eq!(at(src, &n.to), Some("38,30"));
    }

    #[test]
    fn an_anchored_item_has_no_numbers_to_move() {
        let src = "circle #peak : pad=6\nhighlight #s-moment : color=@accent2\n";
        let doc = parse(src);
        for item in &doc.items {
            assert!(!item.numbers.draggable(), "{:?}", item.numbers);
        }
    }

    #[test]
    fn an_arrow_from_an_anchor_records_only_the_end_that_is_written_down() {
        let src = "target: fig\narrow #peak -> 38,30\n";
        let doc = parse(src);
        let n = &doc.items[0].numbers;
        assert_eq!(n.at, None);
        assert_eq!(at(src, &n.to), Some("38,30"));
    }

    #[test]
    fn a_text_mark_records_its_position_and_not_its_words() {
        let src = "target: fig\ntext 10,80 \"throughput doubles here\"\n";
        let doc = parse(src);
        let n = &doc.items[0].numbers;
        assert_eq!(at(src, &n.at), Some("10,80"));
        assert_eq!(n.size, None);
    }

    #[test]
    fn spans_survive_indentation_comments_and_blank_lines() {
        // Nothing here is the shape a formatter would produce, which is the
        // point: an author's own spacing has to come back untouched.
        let src = "target: fig\n\n// the hot corner\n    circle   62,38   34x34 : id=hot\n";
        let doc = parse(src);
        let n = &doc.items[0].numbers;
        assert_eq!(at(src, &n.at), Some("62,38"));
        assert_eq!(at(src, &n.size), Some("34x34"));
        assert_eq!(
            &src[doc.items[0].line.clone()],
            "    circle   62,38   34x34 : id=hot"
        );
    }

    #[test]
    fn spans_are_right_on_a_block_with_crlf_endings() {
        let src = "target: fig\r\nrect 40,22 18x12\r\narrow 1,2 -> 3,4\r\n";
        let doc = parse(src);
        assert_eq!(at(src, &doc.items[0].numbers.at), Some("40,22"));
        assert_eq!(at(src, &doc.items[1].numbers.to), Some("3,4"));
    }

    #[test]
    fn every_item_of_a_real_block_lands_on_its_own_line() {
        let src = "target: shot\n\
                   circle 62,38 34x34 : id=hot label=\"the hot corner\" step=1\n\
                   arrow  16,88 -> 55,48 : step=2\n";
        let doc = parse(src);
        assert_eq!(doc.items.len(), 2);
        for item in &doc.items {
            let line = &src[item.line.clone()];
            assert!(line.starts_with(item.kind.as_str()), "{line}");
            for r in [&item.numbers.at, &item.numbers.size, &item.numbers.to]
                .into_iter()
                .flatten()
            {
                assert!(
                    item.line.contains(&r.start) && r.end <= item.line.end,
                    "a number span escaped its own line: {r:?} vs {:?}",
                    item.line
                );
            }
        }
    }

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
    // ---- Marking words rather than a region of a picture ----

    #[test]
    fn a_text_mark_takes_an_id() {
        let doc = parse("highlight #t-ap : color=@accent2 step=1\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items.len(), 1);
        assert_eq!(doc.items[0].kind, Kind::Highlight);
        assert!(doc.items[0].kind.marks_text());
        assert_eq!(doc.items[0].place, Place::Anchor("t-ap".into()));
        assert_eq!(doc.items[0].step, 1);
    }

    #[test]
    fn underline_and_box_are_text_marks_too() {
        let doc = parse("underline #a\nbox #b : pad=6\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].kind, Kind::Underline);
        assert_eq!(doc.items[1].kind, Kind::Box);
        assert_eq!(doc.items[1].pad, Some(6.0));
    }

    /// Where the words are is the browser's business; a percentage would be a
    /// guess that goes stale as soon as the sentence is edited.
    #[test]
    fn a_text_mark_refuses_coordinates() {
        let doc = parse("highlight 10,20 30x5\n");
        assert_eq!(doc.items.len(), 0);
        assert!(doc.errors[0].contains("#id"), "{:?}", doc.errors);
    }

    /// A block that pairs a phrase with a chart mark measures nothing against a
    /// box, so making it name one would be ceremony.
    #[test]
    fn an_all_anchored_block_needs_no_target() {
        let doc = parse("highlight #t-ap : step=1\nrect #lat-0-2 : step=1 pad=8\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items.len(), 2);
        assert!(doc.target.is_none());
    }

    /// But a coordinate is a percentage *of something*, and that something has
    /// to be named.
    #[test]
    fn coordinates_still_need_a_target() {
        let doc = parse("circle 40,30 20x20\n");
        assert!(
            doc.errors.iter().any(|e| e.contains("target")),
            "{:?}",
            doc.errors
        );
    }

    #[test]
    fn a_text_mark_reaches_the_json() {
        let doc = parse("highlight #t-ap : color=@accent2 step=2\n");
        let json = to_json(&doc);
        assert!(json.contains("\"kind\":\"highlight\""), "{json}");
        assert!(json.contains("\"anchor\":\"t-ap\""), "{json}");
        assert!(json.contains("var(--mz-accent2)"), "{json}");
        assert!(json.contains("\"step\":2"), "{json}");
    }
}
