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
    /// `quote=`: the words this mark lies over, as the source prints them.
    /// Read by `mirzam check`, which opens the block's `source:` and looks
    /// for them on `page`. On a mark placed by coordinates it is what
    /// `mirzam import pdf --quote` wrote; on the phrase's own mark it is what
    /// the author typed, and the build then cuts the passage out of the
    /// source and marks it - see [`AnnotDoc::is_card`]. The overlay never
    /// sees it; a card prints it under the picture.
    pub quote: Option<String>,
    /// `page=`: the page of the source the quote is on. Optional on a phrase's
    /// mark, where the build finds the page and the check searches for it.
    pub page: Option<u32>,
}

#[derive(Debug, Default)]
pub struct AnnotDoc {
    /// `#id`, or a bare pane name; the renderer resolves it to a selector. Or
    /// the path of a picture that is *not* on the slide — see [`AnnotDoc::picture`].
    pub target: Option<String>,
    /// `source:` — where the picture's words come from, for the check that
    /// verifies a `quote=`: `@key` for an entry in the deck's bibliography
    /// whose `file` field names the PDF, or a path to the PDF itself.
    pub source: Option<String>,
    pub items: Vec<Item>,
    pub errors: Vec<String>,
}

impl AnnotDoc {
    /// The picture a *card* block shows: a `target:` that names an image file
    /// rather than something on the slide.
    ///
    /// A cut-out of a source page need not sit beside the summary. Written
    /// this way, the picture stays off the slide and each anchored mark
    /// becomes a chip after its phrase; the chip opens the picture in a card
    /// with the coordinate marks drawn on it, and the exported PDF collects
    /// the cards in a generated "Sources" appendix. Same block, same marks,
    /// same `quote=` for the check - only the presentation differs.
    pub fn picture(&self) -> Option<&str> {
        self.target.as_deref().filter(|t| is_picture(t))
    }

    /// Whether the block is a *card*: nothing drawn on the slide, and a chip
    /// after each phrase that opens the source in a card.
    ///
    /// Two shapes of block say so. One names a picture as its `target:` and
    /// places marks on it by coordinates - the block `mirzam import pdf
    /// --quote` prints. The other names no target at all and puts the
    /// `quote=` on the phrase's own mark:
    ///
    /// ```text
    /// source: @devi2022
    /// highlight #lim : quote="Outside the range over which …"
    /// ```
    ///
    /// which is the whole of what a person has to write. On its own it is a
    /// card holding the words and the source; a build that can open the PDF
    /// cuts the passage out, marks its lines and adds the picture - the same
    /// block as the first shape, written for the author by the build instead
    /// of by a command.
    pub fn is_card(&self) -> bool {
        self.picture().is_some()
            || (self.target.is_none() && self.chips().any(|c| c.quote.is_some()))
    }

    /// The anchored marks of a card block, each of which becomes a chip.
    pub fn chips(&self) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .filter(|i| matches!(i.place, Place::Anchor(_)))
    }
}

/// Whether a `target:` names an image file. A pane name or an `#id` has no
/// extension; a picture has one of the browser's.
pub fn is_picture(target: &str) -> bool {
    let ext = target
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    target.contains('.')
        && !target.starts_with('#')
        && matches!(
            ext.as_str(),
            "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif"
        )
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
        if let Some(src) = line.strip_prefix("source:") {
            let src = src.trim();
            if src.is_empty() {
                doc.errors
                    .push(format!("annotate line {}: empty source", ln + 1));
            } else {
                doc.source = Some(src.to_string());
            }
            continue;
        }
        match parse_item(line) {
            Ok(item) => doc.items.push(item),
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
    if doc.is_card() {
        check_card(&mut doc);
    }
    doc
}

/// What a card block may hold. The picture is not on the slide, so the only
/// things that can be drawn *on* it are the marks a card knows how to place by
/// percentage, and the only way to open it is a chip on a phrase.
fn check_card(doc: &mut AnnotDoc) {
    if !doc.items.is_empty() && doc.chips().next().is_none() {
        doc.errors.push(
            "the target is a picture that is not on the slide, so the block needs a phrase \
             to open it from: an anchored mark such as `highlight #q1`"
                .to_string(),
        );
    }
    // Without a picture, the quote is all a chip has to show, so a chip
    // without one would open on nothing.
    if doc.picture().is_none() {
        if doc.source.is_none() {
            doc.errors.push(
                "a quote on a phrase says the phrase quotes a source, but the block names \
                 none: `source: @key` for a bibliography entry whose `file` field names the \
                 PDF, or `source: path/to/paper.pdf`"
                    .to_string(),
            );
        }
        let silent: Vec<String> = doc
            .chips()
            .filter(|c| c.quote.is_none())
            .filter_map(|c| match &c.place {
                Place::Anchor(id) => Some(id.clone()),
                Place::At(..) => None,
            })
            .collect();
        for id in silent {
            doc.errors.push(format!(
                "the block is a card with no picture, so `#{id}` needs the words it quotes: \
                 `quote=\"…\"` on its mark, or a `target:` naming a picture of them"
            ));
        }
    }
    for item in &doc.items {
        let problem = match (&item.place, item.kind) {
            (Place::Anchor(_), k) if !k.marks_text() => Some(format!(
                "an anchored `{}` in a card block would mark something on the slide, but the \
                 picture is not there; a chip is a `highlight`, `underline` or `box` on a phrase",
                k.as_str()
            )),
            (Place::At(..), Kind::Arrow | Kind::Text) => Some(format!(
                "`{}` cannot be drawn in a card; a card places highlights, underlines, rects \
                 and circles on the picture",
                item.kind.as_str()
            )),
            _ => None,
        };
        if let Some(p) = problem {
            doc.errors.push(p);
        }
    }
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
        quote: None,
        page: None,
    };

    match kind {
        // A box outlines a phrase and nothing else: where the words are is the
        // browser's business, and a percentage would be a guess that goes
        // stale the moment the sentence is edited.
        Kind::Box => {
            let mut parts = rest.split_whitespace();
            let first = parts
                .next()
                .ok_or("`box` marks a phrase, so it needs an `#id`")?;
            item.place = parse_place(first)?;
            if !matches!(item.place, Place::Anchor(_)) {
                return Err(
                    "`box` marks a phrase written `[like this]{#id}`, so it takes an `#id` \
                     rather than coordinates"
                        .into(),
                );
            }
            if parts.next().is_some() {
                return Err("too many fields before `:`".into());
            }
        }
        // A highlight or an underline marks a phrase the same way - and may
        // also mark words *in a picture*, by coordinates: a passage cut out of
        // a paper does not reflow when the sentence beside it is edited, so a
        // percentage of the picture stays true. `mirzam import pdf --quote`
        // writes these.
        Kind::Highlight | Kind::Underline | Kind::Rect | Kind::Circle => {
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
            "quote" => {
                if v.trim().is_empty() {
                    return Err("quote= needs the words to look for".into());
                }
                item.quote = Some(v);
            }
            "page" => {
                item.page = Some(
                    v.parse::<u32>()
                        .ok()
                        .filter(|&p| p > 0)
                        .ok_or_else(|| format!("page is a page number, got `{v}`"))?,
                )
            }
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
pub fn color_css(v: &str) -> String {
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

    /// Where the words of a *sentence* are is the browser's business; a
    /// percentage would be a guess that goes stale as soon as the sentence is
    /// edited. A box is only ever drawn round a sentence, so it refuses them.
    #[test]
    fn a_box_refuses_coordinates() {
        let doc = parse("target: t\nbox 10,20 30x5\n");
        assert_eq!(doc.items.len(), 0);
        assert!(doc.errors[0].contains("#id"), "{:?}", doc.errors);
    }

    /// Words in a picture do not reflow, so a highlight or an underline may be
    /// placed over them by coordinates - which is how a passage cut out of a
    /// paper gets its lines marked.
    #[test]
    fn a_highlight_or_underline_may_be_placed_by_coordinates() {
        let doc = parse("target: p\nhighlight 50,32.1 95.5x8 : step=1\nunderline 10,20 30x5\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.items[0].place, Place::At(50.0, 32.1));
        assert_eq!(doc.items[0].size, Some((95.5, 8.0)));
        assert_eq!(doc.items[1].kind, Kind::Underline);
        // Coordinates need a size, as they do on a rect.
        let doc = parse("target: p\nhighlight 50,32\n");
        assert!(doc.errors[0].contains("size"), "{:?}", doc.errors);
    }

    /// The words a mark lies over, and where the picture came from, ride
    /// along for `mirzam check` and stay out of what the viewer is sent.
    #[test]
    fn a_quote_and_its_source_are_kept_for_the_check() {
        let doc = parse(
            "target: #p1\nsource: @duberg2020\n\
             highlight 50,32 95x8 : step=1 quote=\"This gives, surprisingly, a more memory efficient representation.\" page=1\n\
             highlight 50,41 95x8 : step=1\n",
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.source.as_deref(), Some("@duberg2020"));
        assert_eq!(
            doc.items[0].quote.as_deref(),
            Some("This gives, surprisingly, a more memory efficient representation.")
        );
        assert_eq!(doc.items[0].page, Some(1));
        assert_eq!(doc.items[1].quote, None);
        let json = to_json(&doc);
        assert!(!json.contains("quote"), "not the viewer's business: {json}");

        let doc = parse("highlight #t : quote=\"words\"\n");
        assert!(doc.errors[0].contains("source:"), "{:?}", doc.errors);
        let doc = parse("target: p\nhighlight 1,1 1x1 : page=0\n");
        assert!(doc.errors[0].contains("page number"), "{:?}", doc.errors);
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

    // ---- A card: the picture is not on the slide ----

    /// A `target:` naming an image file is a picture the slide does not show;
    /// the anchored marks are the chips that open it.
    #[test]
    fn a_picture_target_makes_a_card_block() {
        let doc = parse(
            "target: img/devi2022-p1.svg\nsource: @devi2022\n\
             highlight #q1 : color=@accent1 step=1\n\
             highlight 54.6,43.8 84.2x9.6 : color=@accent1 step=1 quote=\"words\" page=1\n",
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(doc.picture(), Some("img/devi2022-p1.svg"));
        assert_eq!(doc.chips().count(), 1);
        assert!(is_picture("shots/a.PNG"));
        assert!(!is_picture("#fig"), "an id is not a file");
        assert!(!is_picture("fig"), "a pane name is not a file");
        assert!(!is_picture("notes.txt"), "not a picture the browser draws");
        assert_eq!(parse("target: #fig\nrect 1,1 2x2\n").picture(), None);
    }

    /// Nothing on the slide can open a card but a chip, and nothing but a
    /// percentage mark can be drawn on a picture that is not laid out.
    #[test]
    fn a_card_needs_a_chip_and_draws_only_marks() {
        let doc = parse("target: p.svg\nhighlight 50,32 95x8 : step=1\n");
        assert!(doc.errors[0].contains("phrase"), "{:?}", doc.errors);

        let doc = parse("target: p.svg\nhighlight #q1\narrow 10,80 -> 38,30\n");
        assert!(doc.errors[0].contains("arrow"), "{:?}", doc.errors);

        let doc = parse("target: p.svg\nhighlight #q1\ncircle #mark : pad=4\n");
        assert!(doc.errors[0].contains("chip"), "{:?}", doc.errors);

        let doc = parse("target: p.svg\nbox #q1\nrect 10,10 20x10\ncircle 50,50 10x10\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
    }

    /// The form a person writes: no target, the words on the phrase's own
    /// mark. It is a card - and a chip beside it that quotes nothing has
    /// nothing to open on.
    #[test]
    fn a_quote_on_the_phrase_makes_a_card_with_no_picture() {
        let doc = parse("source: @devi2022\nhighlight #lim : quote=\"Outside the range\"\n");
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert!(doc.is_card());
        assert_eq!(doc.picture(), None);
        assert_eq!(doc.chips().count(), 1);

        let doc = parse("source: @devi2022\nhighlight #lim : quote=\"words\"\nunderline #other\n");
        assert!(doc.is_card());
        assert!(doc.errors[0].contains("#other"), "{:?}", doc.errors);

        // The same lines with a target are an overlay on the slide, as ever.
        let doc = parse("target: #chart\nhighlight #lim : quote=\"words\"\n");
        assert!(!doc.is_card());
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let doc = parse("highlight #lim : color=@accent1\n");
        assert!(!doc.is_card());
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
