//! A laid-out slide as a PowerPoint file.
//!
//! The input is a *scene*: what the browser painted, read back off the
//! laid-out DOM — every text block with its box, font and colour; every
//! surface with a fill or an edge; every table; and, for whatever has no
//! DrawingML equivalent (a chart, a formula, a diagram, a photograph in a
//! format PowerPoint may not open), a picture of exactly that element. The
//! output is a `.pptx` in which the words are words: selectable, searchable,
//! editable, in text boxes where the slide put them.
//!
//! This crate knows nothing about browsers or files. The CLI drives a
//! headless Chromium, runs the extractor script in each slide, photographs
//! the elements the scene asks for, and hands the result here; the scene
//! itself is the JSON contract between the two, and [`Slide`] is its schema.
//! Keeping the writer pure is what makes it testable against scenes written
//! by hand, and what keeps the WebAssembly build free of it.
//!
//! **No dependency beyond serde.** The package is a ZIP (the writer in
//! [`zip`]) of hand-written OOXML parts following ECMA-376's minimal
//! presentation package. The crates that write PowerPoint drag in XML
//! frameworks to emit a dozen fixed elements; the shapes below are those
//! elements, spelled out once.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write as _;

pub mod zip;

/// EMUs per CSS pixel: 914400 EMU to the inch, 96 px to the inch.
const EMU_PER_PX: f64 = 9525.0;

/// Hundredths of a point per CSS pixel: 72 points to the inch, 96 px.
const CPT_PER_PX: f64 = 75.0;

const NS: &str = concat!(
    "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\""
);

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
const REL_NOTES_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const REL_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const REL_THEME: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

/// The empty shape-tree header every `cSld` opens with.
const EMPTY_TREE_HEAD: &str = concat!(
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>",
    "<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>",
    "<a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>"
);

/// The colour map every master declares, naming the theme's twelve slots.
const CLR_MAP: &str = concat!(
    "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" ",
    "accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" ",
    "accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>"
);

// ---------------------------------------------------------------------------
// The scene: what the extractor read off the laid-out slide.
// ---------------------------------------------------------------------------

/// A box on the slide, in CSS pixels from the slide's top-left corner.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A colour: six hex digits, and an opacity from 0 to 1.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Color {
    pub hex: String,
    #[serde(default = "one")]
    pub alpha: f64,
}

fn one() -> f64 {
    1.0
}

/// One stop of a gradient: where it sits, 0 to 1, and its colour.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Stop {
    pub pos: f64,
    pub color: Color,
}

/// A linear gradient, at the angle CSS gives it: 0 runs upward, 90 to the
/// right, 180 down.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Gradient {
    pub angle: f64,
    pub stops: Vec<Stop>,
}

/// A stroke: its width in CSS pixels and its colour, solid unless `dash`
/// names one of DrawingML's patterns (`sysDot`, `dash`, …).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Line {
    pub width: f64,
    pub color: Color,
    #[serde(default)]
    pub dash: Option<String>,
}

/// Where the text sits in its box when it does not fill it.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// A text box's content. The insets are the distance from the shape's edge
/// to the text, in CSS pixels: left, top, right, bottom.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct TextBody {
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub insets: [f64; 4],
    /// `false` for a box whose text never wraps (`white-space: nowrap`).
    #[serde(default = "yes")]
    pub wrap: bool,
    #[serde(default)]
    pub paragraphs: Vec<Paragraph>,
}

fn yes() -> bool {
    true
}

/// How a paragraph is bulleted.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Bullet {
    /// A literal character, `•` or `◦` or `☑`.
    Char {
        text: String,
        #[serde(default)]
        color: Option<Color>,
    },
    /// Automatic numbering, in one of DrawingML's schemes (`arabicPeriod`,
    /// `alphaLcPeriod`, `romanUcPeriod`, …).
    Auto {
        scheme: String,
        #[serde(default = "one_u32")]
        start: u32,
        #[serde(default)]
        color: Option<Color>,
    },
}

fn one_u32() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Paragraph {
    #[serde(default)]
    pub align: Align,
    /// Line height in CSS pixels, applied as exact spacing.
    #[serde(default)]
    pub line_height: f64,
    /// Space above the paragraph, in CSS pixels.
    #[serde(default)]
    pub space_before: f64,
    /// Nesting level, 0 to 8.
    #[serde(default)]
    pub level: u8,
    /// Left margin of the text, from the box's inset edge, in CSS pixels.
    #[serde(default)]
    pub margin_left: f64,
    /// Right margin of the text, from the box's inset edge, in CSS pixels.
    #[serde(default)]
    pub margin_right: f64,
    /// First-line offset from the margin: negative hangs a bullet.
    #[serde(default)]
    pub indent: f64,
    #[serde(default)]
    pub bullet: Option<Bullet>,
    #[serde(default)]
    pub runs: Vec<Run>,
}

/// One run of a paragraph: text in one style, or a line break.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Run {
    Break {
        #[allow(dead_code)]
        br: bool,
    },
    Text(TextRun),
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct TextRun {
    #[serde(rename = "t")]
    pub text: String,
    /// The family the browser used.
    #[serde(default)]
    pub font: String,
    /// Font size in CSS pixels.
    #[serde(default)]
    pub size: f64,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub strike: bool,
    /// A background behind the run — an inline code span's paper.
    #[serde(default)]
    pub highlight: Option<Color>,
    /// Superscript (positive) or subscript (negative), in thousandths of a
    /// percent of the size: 30000 lifts a footnote mark.
    #[serde(default)]
    pub baseline: i32,
    /// `text-transform: uppercase`, kept as a property so the words keep
    /// their case for whoever edits them.
    #[serde(default)]
    pub caps: bool,
    /// Letter spacing in CSS pixels.
    #[serde(default)]
    pub spacing: f64,
    #[serde(default)]
    pub href: Option<String>,
}

/// A surface — a card, a code block's paper, a border drawn as a bar — that
/// may also carry text.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Shape {
    pub rect: Rect,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fill: Option<Color>,
    /// A gradient fill, which wins over `fill` when both are given.
    #[serde(default)]
    pub gradient: Option<Gradient>,
    #[serde(default)]
    pub line: Option<Line>,
    /// Corner radius in CSS pixels; half the shorter side or more is a
    /// circle.
    #[serde(default)]
    pub radius: f64,
    /// `line` for a stroke from the box's top-left to its bottom-right —
    /// a rule or a leader — instead of a box.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub text: Option<TextBody>,
}

/// A picture: one of the slide's photographs (by the id the extractor gave
/// it), cropped to what was visible and shaped like its element.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Picture {
    pub rect: Rect,
    #[serde(default)]
    pub name: String,
    pub image: u32,
    /// Fractions of the source cut from each edge: left, top, right, bottom.
    #[serde(default)]
    pub crop: [f64; 4],
    #[serde(default)]
    pub radius: f64,
    #[serde(default = "one")]
    pub alpha: f64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Cell {
    #[serde(default)]
    pub text: Option<TextBody>,
    #[serde(default)]
    pub fill: Option<Color>,
    /// Left, right, top, bottom.
    #[serde(default)]
    pub borders: [Option<Line>; 4],
    #[serde(default)]
    pub insets: [f64; 4],
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default = "one_u32")]
    pub col_span: u32,
    #[serde(default = "one_u32")]
    pub row_span: u32,
    /// A slot covered by a span to its left or above it: present in the
    /// grid, but drawn by the cell that spans it.
    #[serde(default)]
    pub merged_h: bool,
    #[serde(default)]
    pub merged_v: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Row {
    pub height: f64,
    #[serde(default)]
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Table {
    pub rect: Rect,
    #[serde(default)]
    pub name: String,
    pub cols: Vec<f64>,
    pub rows: Vec<Row>,
}

/// One thing on the slide, in painting order.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum Node {
    Shape(Shape),
    Picture(Picture),
    Table(Table),
}

/// What the extractor wants photographed, or embedded as it found it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Raster {
    pub id: u32,
    /// The region of the page to capture, in CSS pixels from the slide's
    /// corner. Absent when `data` carries the bytes already.
    #[serde(default)]
    pub rect: Option<Rect>,
    /// `png` for anything with transparency or fine lines, `jpeg` for a
    /// photograph, `data` for an image the page had inline.
    pub kind: String,
    /// A `data:` URI, when the image can go in as it is.
    #[serde(default)]
    pub data: Option<String>,
    /// What to leave visible for the photograph: the element and everything
    /// in it (`tree`), or the element's own paint alone (`self`) — a
    /// gradient scrim, say, without the words laid over it.
    #[serde(default)]
    pub mode: String,
}

/// The scene of one slide, as the extractor reports it.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Slide {
    #[serde(default)]
    pub background: Option<Color>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub rasters: Vec<Raster>,
    /// Speaker notes, as text; not part of the extractor's output.
    #[serde(default)]
    pub notes: Option<String>,
}

impl Slide {
    /// Parses the extractor's JSON for one slide.
    pub fn from_json(json: &str) -> Result<Slide, String> {
        serde_json::from_str(json).map_err(|e| format!("the slide scene is malformed: {e}"))
    }
}

/// The bytes of one picture and the extension they should be stored under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub bytes: Vec<u8>,
    /// `png`, `jpeg` or `gif`.
    pub ext: &'static str,
}

/// What a `data:` URI carries: its media type and its base64 payload,
/// resolved to the extension the package stores it under.
pub fn data_uri_media(uri: &str) -> Option<Media> {
    let rest = uri.strip_prefix("data:")?;
    let (head, payload) = rest.split_once(',')?;
    let (mime, encoding) = head.split_once(';').unwrap_or((head, ""));
    if encoding != "base64" {
        return None;
    }
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/gif" => "gif",
        _ => return None,
    };
    Some(Media {
        bytes: base64_decode(payload)?,
        ext,
    })
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u8;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// XML.
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Control characters are not XML 1.0; a stray one in a code
            // block would make the whole part unreadable.
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

fn emu(px: f64) -> i64 {
    (px * EMU_PER_PX).round() as i64
}

/// Hundredths of a point, never below the smallest size PowerPoint accepts.
fn cpt(px: f64) -> i64 {
    ((px * CPT_PER_PX).round() as i64).max(100)
}

fn hex6(c: &Color) -> String {
    let h: String = c
        .hex
        .trim_start_matches('#')
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(6)
        .collect::<String>()
        .to_ascii_uppercase();
    if h.len() == 6 {
        h
    } else {
        "000000".into()
    }
}

/// `<a:srgbClr>` with its alpha, when the colour is not opaque.
fn srgb(c: &Color) -> String {
    let alpha = c.alpha.clamp(0.0, 1.0);
    if alpha >= 0.999 {
        format!("<a:srgbClr val=\"{}\"/>", hex6(c))
    } else {
        format!(
            "<a:srgbClr val=\"{}\"><a:alpha val=\"{}\"/></a:srgbClr>",
            hex6(c),
            (alpha * 100_000.0).round() as i64
        )
    }
}

fn solid_fill(c: &Color) -> String {
    format!("<a:solidFill>{}</a:solidFill>", srgb(c))
}

/// `<a:gradFill>`: CSS measures its angle clockwise from "up", DrawingML
/// clockwise from "to the right", so the two differ by a quarter turn.
fn gradient_fill(g: &Gradient) -> String {
    let stops: String = g
        .stops
        .iter()
        .map(|s| {
            format!(
                "<a:gs pos=\"{}\">{}</a:gs>",
                (s.pos.clamp(0.0, 1.0) * 100_000.0).round() as i64,
                srgb(&s.color)
            )
        })
        .collect();
    let ang = ((g.angle - 90.0).rem_euclid(360.0) * 60_000.0).round() as i64;
    format!("<a:gradFill rotWithShape=\"1\"><a:gsLst>{stops}</a:gsLst><a:lin ang=\"{ang}\" scaled=\"0\"/></a:gradFill>")
}

fn line_xml(line: Option<&Line>) -> String {
    match line {
        Some(l) if l.width > 0.0 && l.color.alpha > 0.0 => format!(
            "<a:ln w=\"{}\">{}{}</a:ln>",
            emu(l.width).max(1),
            solid_fill(&l.color),
            match &l.dash {
                Some(d) => format!("<a:prstDash val=\"{}\"/>", xml_escape(d)),
                None => String::new(),
            }
        ),
        _ => "<a:ln><a:noFill/></a:ln>".into(),
    }
}

fn xfrm(r: &Rect) -> String {
    format!(
        "<a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>",
        emu(r.x),
        emu(r.y),
        emu(r.w).max(0),
        emu(r.h).max(0)
    )
}

/// The preset geometry a corner radius calls for: a rectangle, a rounded
/// one with the same radius, or — once the corners meet — an ellipse.
fn geometry(rect: &Rect, radius: f64) -> String {
    let short = rect.w.min(rect.h);
    if radius <= 0.0 || short <= 0.0 {
        return "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>".into();
    }
    if radius >= short / 2.0 - 0.5 {
        if (rect.w - rect.h).abs() < 1.0 {
            return "<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom>".into();
        }
        // A pill: `roundRect` at its limit, which is what the browser draws
        // for a radius past half the short side.
        return "<a:prstGeom prst=\"roundRect\"><a:avLst><a:gd name=\"adj\" fmla=\"val 50000\"/></a:avLst></a:prstGeom>".into();
    }
    let adj = (radius / short * 100_000.0).round() as i64;
    format!(
        "<a:prstGeom prst=\"roundRect\"><a:avLst><a:gd name=\"adj\" fmla=\"val {adj}\"/></a:avLst></a:prstGeom>"
    )
}

fn anchor_attr(a: Anchor) -> &'static str {
    match a {
        Anchor::Top => "t",
        Anchor::Middle => "ctr",
        Anchor::Bottom => "b",
    }
}

fn align_attr(a: Align) -> &'static str {
    match a {
        Align::Left => "l",
        Align::Center => "ctr",
        Align::Right => "r",
        Align::Justify => "just",
    }
}

/// Hyperlinks a slide's runs point at, each given a relationship id once.
/// A link to another slide of the deck (`#3`, as the table of contents and
/// a citation write them) becomes a jump to that slide rather than a URL.
#[derive(Default)]
struct Links {
    ids: HashMap<String, String>,
    order: Vec<String>,
    next: usize,
    slide_count: usize,
}

impl Links {
    fn new(first_id: usize, slide_count: usize) -> Links {
        Links {
            next: first_id,
            slide_count,
            ..Default::default()
        }
    }

    /// The slide a `#n` link names, when the deck has one.
    fn slide_target(&self, href: &str) -> Option<usize> {
        let n: usize = href.strip_prefix('#')?.parse().ok()?;
        (n >= 1 && n <= self.slide_count).then_some(n)
    }

    fn id_for(&mut self, href: &str) -> String {
        if let Some(id) = self.ids.get(href) {
            return id.clone();
        }
        let id = format!("rId{}", self.next);
        self.next += 1;
        self.ids.insert(href.to_string(), id.clone());
        self.order.push(href.to_string());
        id
    }
}

fn run_props_xml(r: &TextRun, links: &mut Links) -> String {
    let mut s = String::from("<a:rPr");
    if r.size > 0.0 {
        let _ = write!(s, " sz=\"{}\"", cpt(r.size));
    }
    if r.bold {
        s.push_str(" b=\"1\"");
    }
    if r.italic {
        s.push_str(" i=\"1\"");
    }
    if r.underline {
        s.push_str(" u=\"sng\"");
    }
    if r.strike {
        s.push_str(" strike=\"sngStrike\"");
    }
    if r.caps {
        s.push_str(" cap=\"all\"");
    }
    if r.spacing.abs() > 0.01 {
        let _ = write!(s, " spc=\"{}\"", (r.spacing * CPT_PER_PX).round() as i64);
    }
    if r.baseline != 0 {
        let _ = write!(s, " baseline=\"{}\"", r.baseline);
    }
    s.push('>');
    if let Some(c) = &r.color {
        s.push_str(&solid_fill(c));
    }
    if let Some(h) = &r.highlight {
        let _ = write!(s, "<a:highlight>{}</a:highlight>", srgb(h));
    }
    if !r.font.is_empty() {
        let f = xml_escape(&r.font);
        let _ = write!(
            s,
            "<a:latin typeface=\"{f}\"/><a:ea typeface=\"{f}\"/><a:cs typeface=\"{f}\"/>"
        );
    }
    if let Some(href) = r.href.as_deref().filter(|h| !h.is_empty()) {
        let id = links.id_for(href);
        if links.slide_target(href).is_some() {
            let _ = write!(
                s,
                "<a:hlinkClick r:id=\"{id}\" action=\"ppaction://hlinksldjump\"/>"
            );
        } else {
            let _ = write!(s, "<a:hlinkClick r:id=\"{id}\"/>");
        }
    }
    s.push_str("</a:rPr>");
    s
}

fn paragraph_xml(p: &Paragraph, links: &mut Links) -> String {
    let mut s = String::from("<a:p><a:pPr");
    if p.margin_left.abs() > 0.01 {
        let _ = write!(s, " marL=\"{}\"", emu(p.margin_left).max(0));
    }
    if p.margin_right.abs() > 0.01 {
        let _ = write!(s, " marR=\"{}\"", emu(p.margin_right).max(0));
    }
    if p.indent.abs() > 0.01 {
        let _ = write!(s, " indent=\"{}\"", emu(p.indent));
    }
    if p.level > 0 {
        let _ = write!(s, " lvl=\"{}\"", p.level.min(8));
    }
    let _ = write!(s, " algn=\"{}\">", align_attr(p.align));
    if p.line_height > 0.0 {
        let _ = write!(
            s,
            "<a:lnSpc><a:spcPts val=\"{}\"/></a:lnSpc>",
            cpt(p.line_height)
        );
    }
    let _ = write!(
        s,
        "<a:spcBef><a:spcPts val=\"{}\"/></a:spcBef>",
        (p.space_before.max(0.0) * CPT_PER_PX).round() as i64
    );
    match &p.bullet {
        None => s.push_str("<a:buNone/>"),
        Some(Bullet::Char { text, color }) => {
            if let Some(c) = color {
                let _ = write!(s, "<a:buClr>{}</a:buClr>", srgb(c));
            }
            let _ = write!(
                s,
                "<a:buSzPct val=\"100000\"/><a:buFont typeface=\"Arial\"/><a:buChar char=\"{}\"/>",
                xml_escape(text)
            );
        }
        Some(Bullet::Auto {
            scheme,
            start,
            color,
        }) => {
            if let Some(c) = color {
                let _ = write!(s, "<a:buClr>{}</a:buClr>", srgb(c));
            }
            let _ = write!(
                s,
                "<a:buSzPct val=\"100000\"/><a:buAutoNum type=\"{}\" startAt=\"{}\"/>",
                xml_escape(scheme),
                (*start).max(1)
            );
        }
    }
    s.push_str("</a:pPr>");
    let mut last_size = 0.0;
    for run in &p.runs {
        match run {
            Run::Break { .. } => s.push_str("<a:br/>"),
            Run::Text(r) => {
                last_size = r.size;
                let _ = write!(
                    s,
                    "<a:r>{}<a:t>{}</a:t></a:r>",
                    run_props_xml(r, links),
                    xml_escape(&r.text)
                );
            }
        }
    }
    // The size an empty paragraph is measured at: without it a blank line
    // in a code block is a blank line of default-size text.
    if last_size > 0.0 {
        let _ = write!(s, "<a:endParaRPr sz=\"{}\"/>", cpt(last_size));
    }
    s.push_str("</a:p>");
    s
}

fn body_xml(t: &TextBody, links: &mut Links) -> String {
    let mut s = format!(
        "<p:txBody><a:bodyPr wrap=\"{}\" lIns=\"{}\" tIns=\"{}\" rIns=\"{}\" bIns=\"{}\" anchor=\"{}\" rtlCol=\"0\"><a:noAutofit/></a:bodyPr><a:lstStyle/>",
        if t.wrap { "square" } else { "none" },
        emu(t.insets[0]).max(0),
        emu(t.insets[1]).max(0),
        emu(t.insets[2]).max(0),
        emu(t.insets[3]).max(0),
        anchor_attr(t.anchor)
    );
    if t.paragraphs.is_empty() {
        s.push_str("<a:p/>");
    }
    for p in &t.paragraphs {
        s.push_str(&paragraph_xml(p, links));
    }
    s.push_str("</p:txBody>");
    s
}

fn shape_xml(sh: &Shape, id: u32, links: &mut Links) -> String {
    let name = if sh.name.is_empty() {
        "Shape".to_string()
    } else {
        sh.name.clone()
    };
    let fill = match (&sh.gradient, &sh.fill) {
        (Some(g), _) if g.stops.len() >= 2 => gradient_fill(g),
        (_, Some(c)) if c.alpha > 0.0 => solid_fill(c),
        _ => "<a:noFill/>".into(),
    };
    let text = match &sh.text {
        Some(t) => body_xml(t, links),
        None => String::new(),
    };
    let geom = if sh.kind.as_deref() == Some("line") {
        "<a:prstGeom prst=\"line\"><a:avLst/></a:prstGeom>".to_string()
    } else {
        geometry(&sh.rect, sh.radius)
    };
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}{geom}{fill}{}</p:spPr>{text}</p:sp>",
        xml_escape(&name),
        xfrm(&sh.rect),
        line_xml(sh.line.as_ref())
    )
}

fn picture_xml(pic: &Picture, id: u32, rel: &str) -> String {
    let name = if pic.name.is_empty() {
        "Picture".to_string()
    } else {
        pic.name.clone()
    };
    let pct = |f: f64| (f.clamp(0.0, 1.0) * 100_000.0).round() as i64;
    let crop = if pic.crop.iter().any(|f| *f > 0.0) {
        format!(
            "<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>",
            pct(pic.crop[0]),
            pct(pic.crop[1]),
            pct(pic.crop[2]),
            pct(pic.crop[3])
        )
    } else {
        String::new()
    };
    let alpha = if pic.alpha < 0.999 {
        format!(
            "<a:alphaModFix amt=\"{}\"/>",
            (pic.alpha.clamp(0.0, 1.0) * 100_000.0).round() as i64
        )
    } else {
        String::new()
    };
    format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"{}\"/>\
         <p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
         <p:blipFill><a:blip r:embed=\"{rel}\">{alpha}</a:blip>{crop}<a:stretch><a:fillRect/></a:stretch></p:blipFill>\
         <p:spPr>{}{}</p:spPr></p:pic>",
        xml_escape(&name),
        xfrm(&pic.rect),
        geometry(&pic.rect, pic.radius)
    )
}

fn cell_xml(cell: &Cell, links: &mut Links) -> String {
    let mut s = String::from("<a:tc");
    if cell.col_span > 1 {
        let _ = write!(s, " gridSpan=\"{}\"", cell.col_span);
    }
    if cell.row_span > 1 {
        let _ = write!(s, " rowSpan=\"{}\"", cell.row_span);
    }
    if cell.merged_h {
        s.push_str(" hMerge=\"1\"");
    }
    if cell.merged_v {
        s.push_str(" vMerge=\"1\"");
    }
    s.push('>');
    // A cell's text body has no bodyPr of its own to speak of: the insets
    // and anchor live on tcPr.
    s.push_str("<a:txBody><a:bodyPr/><a:lstStyle/>");
    match &cell.text {
        Some(t) if !t.paragraphs.is_empty() => {
            for p in &t.paragraphs {
                s.push_str(&paragraph_xml(p, links));
            }
        }
        _ => s.push_str("<a:p><a:endParaRPr/></a:p>"),
    }
    s.push_str("</a:txBody>");
    let _ = write!(
        s,
        "<a:tcPr marL=\"{}\" marR=\"{}\" marT=\"{}\" marB=\"{}\" anchor=\"{}\">",
        emu(cell.insets[0]).max(0),
        emu(cell.insets[2]).max(0),
        emu(cell.insets[1]).max(0),
        emu(cell.insets[3]).max(0),
        anchor_attr(cell.anchor)
    );
    for (tag, line) in ["lnL", "lnR", "lnT", "lnB"].iter().zip(cell.borders.iter()) {
        match line {
            Some(l) if l.width > 0.0 && l.color.alpha > 0.0 => {
                let _ = write!(
                    s,
                    "<a:{tag} w=\"{}\">{}</a:{tag}>",
                    emu(l.width).max(1),
                    solid_fill(&l.color)
                );
            }
            _ => {
                let _ = write!(s, "<a:{tag} w=\"0\"><a:noFill/></a:{tag}>");
            }
        }
    }
    match &cell.fill {
        Some(c) if c.alpha > 0.0 => s.push_str(&solid_fill(c)),
        _ => s.push_str("<a:noFill/>"),
    }
    s.push_str("</a:tcPr></a:tc>");
    s
}

fn table_xml(t: &Table, id: u32, links: &mut Links) -> String {
    let name = if t.name.is_empty() {
        "Table".to_string()
    } else {
        t.name.clone()
    };
    let mut s = format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"{}\"/>\
         <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp=\"1\"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>\
         <p:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></p:xfrm>\
         <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\">\
         <a:tbl><a:tblPr/><a:tblGrid>",
        xml_escape(&name),
        emu(t.rect.x),
        emu(t.rect.y),
        emu(t.rect.w).max(0),
        emu(t.rect.h).max(0)
    );
    for w in &t.cols {
        let _ = write!(s, "<a:gridCol w=\"{}\"/>", emu(*w).max(1));
    }
    s.push_str("</a:tblGrid>");
    for row in &t.rows {
        let _ = write!(s, "<a:tr h=\"{}\">", emu(row.height).max(1));
        for cell in &row.cells {
            s.push_str(&cell_xml(cell, links));
        }
        // A short row is padded to the grid: every row must hold every
        // column, or the table does not open.
        for _ in row.cells.len()..t.cols.len() {
            s.push_str("<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr/></a:tc>");
        }
        s.push_str("</a:tr>");
    }
    s.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
    s
}

/// A complete, minimal DrawingML theme — the part PowerPoint refuses to open
/// a package without. Neutral values: every colour on a slide is written on
/// the shape that uses it, so nothing reads a theme slot.
fn theme_xml() -> String {
    let scheme = concat!(
        "<a:clrScheme name=\"Mirzam\">",
        "<a:dk1><a:srgbClr val=\"0B0E1A\"/></a:dk1>",
        "<a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1>",
        "<a:dk2><a:srgbClr val=\"44546A\"/></a:dk2>",
        "<a:lt2><a:srgbClr val=\"E7E6E6\"/></a:lt2>",
        "<a:accent1><a:srgbClr val=\"6557D9\"/></a:accent1>",
        "<a:accent2><a:srgbClr val=\"38B2AC\"/></a:accent2>",
        "<a:accent3><a:srgbClr val=\"A5A5A5\"/></a:accent3>",
        "<a:accent4><a:srgbClr val=\"FFC000\"/></a:accent4>",
        "<a:accent5><a:srgbClr val=\"5B9BD5\"/></a:accent5>",
        "<a:accent6><a:srgbClr val=\"70AD47\"/></a:accent6>",
        "<a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink>",
        "<a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink>",
        "</a:clrScheme>"
    );
    let fonts = concat!(
        "<a:fontScheme name=\"Mirzam\">",
        "<a:majorFont><a:latin typeface=\"Arial\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>",
        "<a:minorFont><a:latin typeface=\"Arial\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>",
        "</a:fontScheme>"
    );
    // The format scheme wants three fills, three lines, three effects and two
    // background fills; the plainest legal ones will do.
    let line = "<a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>";
    let fmt = format!(
        concat!(
            "<a:fmtScheme name=\"Mirzam\">",
            "<a:fillStyleLst>{f}{f}{f}</a:fillStyleLst>",
            "<a:lnStyleLst>{l}{l}{l}</a:lnStyleLst>",
            "<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle>",
            "<a:effectStyle><a:effectLst/></a:effectStyle>",
            "<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>",
            "<a:bgFillStyleLst>{f}{f}{f}</a:bgFillStyleLst>",
            "</a:fmtScheme>"
        ),
        f = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
        l = line,
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         name=\"Mirzam\"><a:themeElements>{scheme}{fonts}{fmt}</a:themeElements>\
         <a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"
    )
}

/// One relationship: id, type, target, and whether the target is outside
/// the package.
struct Rel {
    id: String,
    ty: &'static str,
    target: String,
    external: bool,
}

fn rel(id: impl Into<String>, ty: &'static str, target: impl Into<String>) -> Rel {
    Rel {
        id: id.into(),
        ty,
        target: target.into(),
        external: false,
    }
}

fn rels(entries: &[Rel]) -> String {
    let body: String = entries
        .iter()
        .map(|r| {
            format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
                r.id,
                r.ty,
                xml_escape(&r.target),
                if r.external {
                    " TargetMode=\"External\""
                } else {
                    ""
                }
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{REL_NS}\">{body}</Relationships>"
    )
}

/// One slide part and the relationships it needs: the pictures it embeds,
/// under the media names it was given, and the links its runs carry.
fn slide_xml(
    slide: &Slide,
    number: usize,
    slide_count: usize,
    media_rel: &HashMap<u32, (String, String)>,
) -> (String, Vec<Rel>) {
    let mut rels_out = vec![rel("rId1", REL_LAYOUT, "../slideLayouts/slideLayout1.xml")];
    let mut tree = String::new();
    let mut id = 2u32;
    // Picture relationships come after the layout and the notes, links
    // after those; ids are handed out in tree order.
    if slide.notes.is_some() {
        rels_out.push(rel(
            "rId2",
            REL_NOTES_SLIDE,
            format!("../notesSlides/notesSlide{number}.xml"),
        ));
    }
    let mut next_rel = rels_out.len() + 1;
    let mut image_rels: HashMap<u32, String> = HashMap::new();
    for node in &slide.nodes {
        if let Node::Picture(p) = node {
            if let (Some((path, _)), false) =
                (media_rel.get(&p.image), image_rels.contains_key(&p.image))
            {
                let rid = format!("rId{next_rel}");
                next_rel += 1;
                rels_out.push(rel(rid.clone(), REL_IMAGE, format!("../media/{path}")));
                image_rels.insert(p.image, rid);
            }
        }
    }
    let mut links = Links::new(next_rel, slide_count);
    for node in &slide.nodes {
        match node {
            Node::Shape(sh) => {
                tree.push_str(&shape_xml(sh, id, &mut links));
                id += 1;
            }
            Node::Picture(p) => {
                if let Some(rid) = image_rels.get(&p.image) {
                    tree.push_str(&picture_xml(p, id, rid));
                    id += 1;
                }
            }
            Node::Table(t) => {
                if !t.cols.is_empty() && !t.rows.is_empty() {
                    tree.push_str(&table_xml(t, id, &mut links));
                    id += 1;
                }
            }
        }
    }
    for href in &links.order {
        match links.slide_target(href) {
            Some(n) => rels_out.push(rel(
                links.ids[href].clone(),
                REL_SLIDE,
                format!("slide{n}.xml"),
            )),
            None => {
                let mut r = rel(links.ids[href].clone(), REL_HYPERLINK, href.clone());
                r.external = true;
                rels_out.push(r);
            }
        }
    }
    let bg = match &slide.background {
        Some(c) if c.alpha > 0.0 => format!(
            "<p:bg><p:bgPr>{}<a:effectLst/></p:bgPr></p:bg>",
            solid_fill(c)
        ),
        _ => String::new(),
    };
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:sld {NS}><p:cSld>{bg}<p:spTree>{EMPTY_TREE_HEAD}{tree}\
         </p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    );
    (xml, rels_out)
}

/// A notes page: the note text in the body placeholder, one paragraph per
/// line the author wrote.
fn notes_xml(text: &str) -> String {
    let paragraphs: String = text
        .lines()
        .map(|l| {
            let l = l.trim_end();
            if l.is_empty() {
                "<a:p/>".to_string()
            } else {
                format!("<a:p><a:r><a:t>{}</a:t></a:r></a:p>", xml_escape(l))
            }
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:notes {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}\
         <p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr>\
         <p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr><p:spPr/>\
         <p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>\
         </p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"
    )
}

/// Builds the whole `.pptx`. `w`/`h` are the deck's slide size in CSS pixels;
/// `media` holds the bytes for every picture id the slides refer to, and a
/// picture whose id is missing is left out rather than written broken.
pub fn package(w: u32, h: u32, slides: &[Slide], media: &HashMap<u32, Media>) -> Vec<u8> {
    let cx = emu(f64::from(w));
    let cy = emu(f64::from(h));
    let n = slides.len();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let text = |s: String| s.into_bytes();

    // Media names: one file per picture id, in first-use order across the
    // deck, so a picture two slides share is stored once.
    let mut media_rel: HashMap<u32, (String, String)> = HashMap::new();
    let mut media_files: Vec<(String, Vec<u8>)> = Vec::new();
    for slide in slides {
        for node in &slide.nodes {
            if let Node::Picture(p) = node {
                if media_rel.contains_key(&p.image) {
                    continue;
                }
                if let Some(m) = media.get(&p.image) {
                    let name = format!("image{}.{}", media_files.len() + 1, m.ext);
                    media_files.push((format!("ppt/media/{name}"), m.bytes.clone()));
                    media_rel.insert(p.image, (name, m.ext.to_string()));
                }
            }
        }
    }

    // [Content_Types].xml
    let mut overrides = String::new();
    let over = |part: &str, ty: &str| {
        format!("<Override PartName=\"{part}\" ContentType=\"application/vnd.openxmlformats-officedocument.{ty}+xml\"/>")
    };
    overrides.push_str(&over(
        "/ppt/presentation.xml",
        "presentationml.presentation.main",
    ));
    overrides.push_str(&over(
        "/ppt/slideMasters/slideMaster1.xml",
        "presentationml.slideMaster",
    ));
    overrides.push_str(&over(
        "/ppt/slideLayouts/slideLayout1.xml",
        "presentationml.slideLayout",
    ));
    overrides.push_str(&over(
        "/ppt/notesMasters/notesMaster1.xml",
        "presentationml.notesMaster",
    ));
    overrides.push_str(&over("/ppt/theme/theme1.xml", "theme"));
    overrides.push_str(&over("/ppt/theme/theme2.xml", "theme"));
    for (i, slide) in slides.iter().enumerate() {
        let i = i + 1;
        overrides.push_str(&over(
            &format!("/ppt/slides/slide{i}.xml"),
            "presentationml.slide",
        ));
        if slide.notes.is_some() {
            overrides.push_str(&over(
                &format!("/ppt/notesSlides/notesSlide{i}.xml"),
                "presentationml.notesSlide",
            ));
        }
    }
    files.push((
        "[Content_Types].xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Default Extension=\"png\" ContentType=\"image/png\"/>\
             <Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/>\
             <Default Extension=\"gif\" ContentType=\"image/gif\"/>\
             {overrides}</Types>"
        )),
    ));

    files.push((
        "_rels/.rels".into(),
        text(rels(&[rel("rId1", REL_OFFICE, "ppt/presentation.xml")])),
    ));

    // presentation.xml and its relationships.
    let slide_ids: String = (1..=n)
        .map(|i| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + i, i + 2))
        .collect();
    files.push((
        "ppt/presentation.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:presentation {NS} saveSubsetFonts=\"1\">\
             <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
             <p:notesMasterIdLst><p:notesMasterId r:id=\"rId2\"/></p:notesMasterIdLst>\
             <p:sldIdLst>{slide_ids}</p:sldIdLst>\
             <p:sldSz cx=\"{cx}\" cy=\"{cy}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/>\
             </p:presentation>"
        )),
    ));
    let mut pres_rels = vec![
        rel("rId1", REL_MASTER, "slideMasters/slideMaster1.xml"),
        rel("rId2", REL_NOTES_MASTER, "notesMasters/notesMaster1.xml"),
    ];
    for i in 1..=n {
        pres_rels.push(rel(
            format!("rId{}", i + 2),
            REL_SLIDE,
            format!("slides/slide{i}.xml"),
        ));
    }
    files.push((
        "ppt/_rels/presentation.xml.rels".into(),
        text(rels(&pres_rels)),
    ));

    // The master, its one blank layout, the notes master, and their themes.
    files.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:sldMaster {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}</p:spTree></p:cSld>{CLR_MAP}\
             <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
             </p:sldMaster>"
        )),
    ));
    files.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        text(rels(&[
            rel("rId1", REL_LAYOUT, "../slideLayouts/slideLayout1.xml"),
            rel("rId2", REL_THEME, "../theme/theme1.xml"),
        ])),
    ));
    files.push((
        "ppt/slideLayouts/slideLayout1.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:sldLayout {NS} type=\"blank\" preserve=\"1\">\
             <p:cSld name=\"Blank\"><p:spTree>{EMPTY_TREE_HEAD}</p:spTree></p:cSld>\
             <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
        )),
    ));
    files.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        text(rels(&[rel(
            "rId1",
            REL_MASTER,
            "../slideMasters/slideMaster1.xml",
        )])),
    ));
    files.push((
        "ppt/notesMasters/notesMaster1.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:notesMaster {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}</p:spTree></p:cSld>{CLR_MAP}\
             </p:notesMaster>"
        )),
    ));
    files.push((
        "ppt/notesMasters/_rels/notesMaster1.xml.rels".into(),
        text(rels(&[rel("rId1", REL_THEME, "../theme/theme2.xml")])),
    ));
    files.push(("ppt/theme/theme1.xml".into(), text(theme_xml())));
    files.push(("ppt/theme/theme2.xml".into(), text(theme_xml())));

    // The slides, their pictures, and the notes beside them.
    for (i, slide) in slides.iter().enumerate() {
        let i = i + 1;
        let (xml, slide_rels) = slide_xml(slide, i, n, &media_rel);
        files.push((format!("ppt/slides/slide{i}.xml"), text(xml)));
        files.push((
            format!("ppt/slides/_rels/slide{i}.xml.rels"),
            text(rels(&slide_rels)),
        ));
        if let Some(note) = &slide.notes {
            files.push((
                format!("ppt/notesSlides/notesSlide{i}.xml"),
                text(notes_xml(note)),
            ));
            files.push((
                format!("ppt/notesSlides/_rels/notesSlide{i}.xml.rels"),
                text(rels(&[
                    rel("rId1", REL_NOTES_MASTER, "../notesMasters/notesMaster1.xml"),
                    rel("rId2", REL_SLIDE, format!("../slides/slide{i}.xml")),
                ])),
            ));
        }
    }
    files.extend(media_files);

    zip::archive_bytes(&files)
}

/// Speaker notes arrive as rendered HTML; PowerPoint's notes pane wants text.
/// Paragraphs and `<br>` become line breaks, tags go, entities come back.
pub fn notes_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find('>') else {
            break;
        };
        let tag = &rest[at + 1..at + end];
        let name = tag
            .trim_start_matches('/')
            .split([' ', '/'])
            .next()
            .unwrap_or("");
        if matches!(name, "p" | "br" | "li" | "div") && !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        rest = &rest[at + end + 1..];
    }
    out.push_str(rest);
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-pixel PNG, enough for the package tests.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R', 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0, b'I', b'E', b'N',
        b'D', 0xAE, 0x42, 0x60, 0x82,
    ];

    fn color(hex: &str) -> Color {
        Color {
            hex: hex.into(),
            alpha: 1.0,
        }
    }

    fn run(t: &str) -> Run {
        Run::Text(TextRun {
            text: t.into(),
            font: "Arial".into(),
            size: 24.0,
            color: Some(color("112233")),
            ..Default::default()
        })
    }

    fn text_shape(t: &str) -> Node {
        Node::Shape(Shape {
            rect: Rect {
                x: 10.0,
                y: 20.0,
                w: 300.0,
                h: 40.0,
            },
            text: Some(TextBody {
                paragraphs: vec![Paragraph {
                    line_height: 32.0,
                    runs: vec![run(t)],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn names(zip: &[u8]) -> String {
        String::from_utf8_lossy(zip).to_string()
    }

    #[test]
    fn the_package_holds_every_required_part() {
        let mut media = HashMap::new();
        media.insert(
            7,
            Media {
                bytes: PNG.to_vec(),
                ext: "png",
            },
        );
        let slides = vec![
            Slide {
                nodes: vec![
                    text_shape("say hello"),
                    Node::Picture(Picture {
                        rect: Rect {
                            x: 0.0,
                            y: 0.0,
                            w: 100.0,
                            h: 50.0,
                        },
                        image: 7,
                        ..Default::default()
                    }),
                ],
                notes: Some("a note".into()),
                ..Default::default()
            },
            Slide::default(),
        ];
        let zip = package(1280, 720, &slides, &media);
        let text = names(&zip);
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/notesMasters/notesMaster1.xml",
            "ppt/theme/theme1.xml",
            "ppt/theme/theme2.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
            "ppt/media/image1.png",
            "ppt/notesSlides/notesSlide1.xml",
        ] {
            assert!(text.contains(part), "missing {part}");
        }
        // The second slide has no notes, so no second notes part.
        assert!(!text.contains("notesSlide2.xml"));
        // 16:9 at 1280x720 is the standard PowerPoint size, in EMUs.
        assert!(text.contains("<p:sldSz cx=\"12192000\" cy=\"6858000\"/>"));
        assert!(text.contains("<a:t>say hello</a:t>"));
        assert!(text.contains("a note"));
        // The picture is embedded through a relationship of its own, after
        // the layout's and the notes'.
        assert!(text.contains("<a:blip r:embed=\"rId3\">"));
        assert!(text.contains("Target=\"../media/image1.png\""));
    }

    #[test]
    fn a_text_box_carries_its_geometry_font_and_colour() {
        let (xml, _) = slide_xml(
            &Slide {
                nodes: vec![text_shape("Hello & <world>")],
                background: Some(color("fafafa")),
                ..Default::default()
            },
            1,
            1,
            &HashMap::new(),
        );
        // 10px is 95250 EMU; 24px is 18pt, which is 1800 hundredths.
        assert!(xml.contains("<a:off x=\"95250\" y=\"190500\"/>"));
        assert!(xml.contains("sz=\"1800\""));
        assert!(xml.contains("<a:srgbClr val=\"112233\"/>"));
        assert!(xml.contains("<a:latin typeface=\"Arial\"/>"));
        // 32px line height is 24pt, exact.
        assert!(xml.contains("<a:lnSpc><a:spcPts val=\"2400\"/></a:lnSpc>"));
        assert!(xml.contains("<a:t>Hello &amp; &lt;world&gt;</a:t>"));
        assert!(xml.contains("<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"FAFAFA\"/>"));
        // A text-only shape has no fill and no edge of its own.
        assert!(xml.contains("<a:noFill/><a:ln><a:noFill/></a:ln>"));
    }

    #[test]
    fn a_surface_is_filled_edged_and_rounded() {
        let sh = Shape {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 100.0,
            },
            fill: Some(Color {
                hex: "#ABCDEF".into(),
                alpha: 0.5,
            }),
            line: Some(Line {
                width: 2.0,
                color: color("000000"),
                dash: None,
            }),
            radius: 12.0,
            ..Default::default()
        };
        let xml = shape_xml(&sh, 2, &mut Links::new(1, 1));
        assert!(xml.contains("<a:srgbClr val=\"ABCDEF\"><a:alpha val=\"50000\"/></a:srgbClr>"));
        assert!(xml.contains("<a:ln w=\"19050\">"));
        // 12px of 100px short side: adj is 12000 of 100000.
        assert!(xml.contains("prst=\"roundRect\""));
        assert!(xml.contains("fmla=\"val 12000\""));
        // Corners past half the short side of a square read as a circle.
        let circle = Rect {
            x: 0.0,
            y: 0.0,
            w: 80.0,
            h: 80.0,
        };
        assert!(geometry(&circle, 40.0).contains("ellipse"));
        assert!(geometry(&circle, 0.0).contains("prst=\"rect\""));
    }

    #[test]
    fn a_leader_is_a_dotted_line() {
        let sh = Shape {
            rect: Rect {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 0.0,
            },
            kind: Some("line".into()),
            line: Some(Line {
                width: 1.0,
                color: color("888888"),
                dash: Some("sysDot".into()),
            }),
            ..Default::default()
        };
        let xml = shape_xml(&sh, 2, &mut Links::new(1, 1));
        assert!(xml.contains("<a:prstGeom prst=\"line\">"));
        assert!(xml.contains("<a:prstDash val=\"sysDot\"/>"));
        assert!(xml.contains("<a:ext cx=\"952500\" cy=\"0\"/>"));
    }

    #[test]
    fn a_gradient_runs_the_way_css_says() {
        let sh = Shape {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 40.0,
                h: 4.0,
            },
            gradient: Some(Gradient {
                angle: 90.0,
                stops: vec![
                    Stop {
                        pos: 0.0,
                        color: color("6557d9"),
                    },
                    Stop {
                        pos: 1.0,
                        color: Color {
                            hex: "38b2ac".into(),
                            alpha: 0.0,
                        },
                    },
                ],
            }),
            ..Default::default()
        };
        let xml = shape_xml(&sh, 2, &mut Links::new(1, 1));
        // 90deg in CSS is left to right, which is DrawingML's angle zero.
        assert!(xml.contains("<a:lin ang=\"0\" scaled=\"0\"/>"));
        assert!(xml.contains("<a:gs pos=\"0\"><a:srgbClr val=\"6557D9\"/></a:gs>"));
        assert!(xml.contains("<a:gs pos=\"100000\"><a:srgbClr val=\"38B2AC\"><a:alpha val=\"0\"/>"));
        // `to top` is 0deg, a quarter turn back from DrawingML's zero.
        let up = Gradient {
            angle: 0.0,
            stops: sh.gradient.clone().unwrap().stops,
        };
        assert!(gradient_fill(&up).contains("ang=\"16200000\""));
    }

    #[test]
    fn bullets_numbering_and_links_are_written() {
        let para = Paragraph {
            level: 1,
            margin_left: 40.0,
            indent: -20.0,
            bullet: Some(Bullet::Char {
                text: "•".into(),
                color: Some(color("ff0000")),
            }),
            runs: vec![Run::Text(TextRun {
                text: "docs".into(),
                href: Some("https://example.com/?a=1&b=2".into()),
                ..Default::default()
            })],
            ..Default::default()
        };
        let numbered = Paragraph {
            bullet: Some(Bullet::Auto {
                scheme: "arabicPeriod".into(),
                start: 3,
                color: None,
            }),
            runs: vec![run("third"), Run::Break { br: true }, run("still third")],
            ..Default::default()
        };
        let slide = Slide {
            nodes: vec![Node::Shape(Shape {
                rect: Rect::default(),
                text: Some(TextBody {
                    paragraphs: vec![para, numbered],
                    ..Default::default()
                }),
                ..Default::default()
            })],
            ..Default::default()
        };
        let (xml, slide_rels) = slide_xml(&slide, 1, 1, &HashMap::new());
        assert!(xml.contains("marL=\"381000\" indent=\"-190500\" lvl=\"1\""));
        assert!(xml.contains("<a:buChar char=\"•\"/>"));
        assert!(xml.contains("<a:buClr><a:srgbClr val=\"FF0000\"/></a:buClr>"));
        assert!(xml.contains("<a:buAutoNum type=\"arabicPeriod\" startAt=\"3\"/>"));
        assert!(xml.contains("<a:br/>"));
        assert!(xml.contains("<a:hlinkClick r:id=\"rId2\"/>"));
        let link = slide_rels.iter().find(|r| r.ty == REL_HYPERLINK).unwrap();
        assert_eq!(link.id, "rId2");
        assert!(link.external);
        assert!(rels(&[Rel {
            id: "rId2".into(),
            ty: REL_HYPERLINK,
            target: link.target.clone(),
            external: true
        }])
        .contains("Target=\"https://example.com/?a=1&amp;b=2\" TargetMode=\"External\""));
    }

    #[test]
    fn a_link_to_another_slide_jumps_there() {
        let jump = |href: &str, count: usize| {
            let slide = Slide {
                nodes: vec![Node::Shape(Shape {
                    text: Some(TextBody {
                        paragraphs: vec![Paragraph {
                            runs: vec![Run::Text(TextRun {
                                text: "contents".into(),
                                href: Some(href.into()),
                                ..Default::default()
                            })],
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                })],
                ..Default::default()
            };
            slide_xml(&slide, 1, count, &HashMap::new())
        };
        let (xml, rels) = jump("#3", 5);
        assert!(xml.contains("action=\"ppaction://hlinksldjump\""));
        let r = rels.iter().find(|r| r.ty == REL_SLIDE).unwrap();
        assert_eq!(r.target, "slide3.xml");
        assert!(!r.external);
        // A slide the deck does not have, or an anchor that is not a
        // number, stays an ordinary link.
        let (xml, rels) = jump("#9", 5);
        assert!(!xml.contains("hlinksldjump"));
        assert!(rels.iter().any(|r| r.ty == REL_HYPERLINK && r.external));
        let (xml, _) = jump("#refs", 5);
        assert!(!xml.contains("hlinksldjump"));
    }

    #[test]
    fn run_properties_cover_the_inline_marks() {
        let r = TextRun {
            text: "x".into(),
            size: 16.0,
            bold: true,
            italic: true,
            underline: true,
            strike: true,
            caps: true,
            spacing: 2.0,
            baseline: 30000,
            highlight: Some(color("eeeeee")),
            ..Default::default()
        };
        let xml = run_props_xml(&r, &mut Links::new(1, 1));
        for attr in [
            "sz=\"1200\"",
            "b=\"1\"",
            "i=\"1\"",
            "u=\"sng\"",
            "strike=\"sngStrike\"",
            "cap=\"all\"",
            "spc=\"150\"",
            "baseline=\"30000\"",
        ] {
            assert!(xml.contains(attr), "missing {attr} in {xml}");
        }
        assert!(xml.contains("<a:highlight><a:srgbClr val=\"EEEEEE\"/></a:highlight>"));
    }

    #[test]
    fn a_table_is_a_graphic_frame_with_a_grid() {
        let cell = |t: &str| Cell {
            text: Some(TextBody {
                paragraphs: vec![Paragraph {
                    runs: vec![run(t)],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            borders: [
                Some(Line {
                    width: 1.0,
                    color: color("cccccc"),
                    dash: None,
                }),
                None,
                None,
                None,
            ],
            fill: Some(color("f0f0f0")),
            insets: [8.0, 4.0, 8.0, 4.0],
            anchor: Anchor::Middle,
            ..Default::default()
        };
        let table = Table {
            rect: Rect {
                x: 1.0,
                y: 2.0,
                w: 200.0,
                h: 60.0,
            },
            cols: vec![100.0, 100.0],
            rows: vec![
                Row {
                    height: 30.0,
                    cells: vec![
                        Cell {
                            col_span: 2,
                            ..cell("head")
                        },
                        Cell {
                            merged_h: true,
                            ..Default::default()
                        },
                    ],
                },
                Row {
                    height: 30.0,
                    cells: vec![cell("a"), cell("b")],
                },
            ],
            ..Default::default()
        };
        let xml = table_xml(&table, 2, &mut Links::new(1, 1));
        assert!(xml.contains("<a:gridCol w=\"952500\"/><a:gridCol w=\"952500\"/>"));
        assert_eq!(xml.matches("<a:tr h=\"285750\">").count(), 2);
        assert!(xml.contains("<a:tc gridSpan=\"2\">"));
        assert!(xml.contains("<a:tc hMerge=\"1\">"));
        assert!(xml.contains("<a:lnL w=\"9525\"><a:solidFill><a:srgbClr val=\"CCCCCC\"/>"));
        assert!(xml.contains("<a:lnR w=\"0\"><a:noFill/></a:lnR>"));
        assert!(xml.contains("anchor=\"ctr\""));
        assert!(xml.contains("<a:t>head</a:t>"));
    }

    #[test]
    fn a_picture_is_cropped_shaped_and_faded() {
        let pic = Picture {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            image: 1,
            crop: [0.1, 0.0, 0.25, 0.0],
            radius: 50.0,
            alpha: 0.5,
            ..Default::default()
        };
        let xml = picture_xml(&pic, 3, "rId9");
        assert!(xml.contains("<a:srcRect l=\"10000\" t=\"0\" r=\"25000\" b=\"0\"/>"));
        assert!(xml.contains("<a:alphaModFix amt=\"50000\"/>"));
        assert!(xml.contains("prst=\"ellipse\""));
        assert!(xml.contains("r:embed=\"rId9\""));
    }

    #[test]
    fn the_scene_json_contract_parses() {
        let json = r##"{
          "background": {"hex": "#ffffff"},
          "nodes": [
            {"k": "shape", "rect": {"x": 1, "y": 2, "w": 3, "h": 4}, "fill": {"hex": "aabbcc", "alpha": 0.5},
             "text": {"anchor": "middle", "paragraphs": [
               {"align": "center", "line_height": 30, "bullet": {"kind": "char", "text": "•"},
                "runs": [{"t": "hi", "size": 20, "bold": true}, {"br": true}, {"t": "there"}]}]}},
            {"k": "picture", "rect": {"x": 0, "y": 0, "w": 10, "h": 10}, "image": 4, "crop": [0, 0, 0.5, 0]},
            {"k": "table", "rect": {"x": 0, "y": 0, "w": 10, "h": 10}, "cols": [5, 5],
             "rows": [{"height": 10, "cells": [{"text": null}, {"col_span": 1, "anchor": "bottom"}]}]}
          ],
          "rasters": [{"id": 4, "rect": {"x": 0, "y": 0, "w": 10, "h": 10}, "kind": "png"},
                      {"id": 5, "kind": "data", "data": "data:image/png;base64,AA=="}]
        }"##;
        let slide = Slide::from_json(json).unwrap();
        assert_eq!(slide.nodes.len(), 3);
        assert_eq!(slide.rasters.len(), 2);
        match &slide.nodes[0] {
            Node::Shape(s) => {
                let t = s.text.as_ref().unwrap();
                assert_eq!(t.anchor, Anchor::Middle);
                assert_eq!(t.paragraphs[0].runs.len(), 3);
                assert!(matches!(t.paragraphs[0].runs[1], Run::Break { .. }));
            }
            other => panic!("expected a shape, got {other:?}"),
        }
        assert!(Slide::from_json("{\"nodes\": [{\"k\": \"blob\"}]}").is_err());
    }

    #[test]
    fn data_uris_become_media() {
        let m = data_uri_media("data:image/png;base64,iVBORw0KGgo=").unwrap();
        assert_eq!(m.ext, "png");
        assert_eq!(m.bytes, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(
            data_uri_media("data:image/jpeg;base64,/9g=").unwrap().ext,
            "jpeg"
        );
        // WebP and SVG are not formats every PowerPoint opens: they are
        // photographed instead.
        assert!(data_uri_media("data:image/webp;base64,AAAA").is_none());
        assert!(data_uri_media("data:image/svg+xml;base64,AAAA").is_none());
        assert!(data_uri_media("data:image/png,notbase64").is_none());
    }

    #[test]
    fn note_text_is_escaped_into_the_xml() {
        let xml = notes_xml("a < b & \"c\"\nsecond");
        assert!(xml.contains("a &lt; b &amp; &quot;c&quot;"));
        assert_eq!(xml.matches("<a:p>").count(), 2);
    }

    #[test]
    fn notes_html_becomes_lines_of_text() {
        assert_eq!(
            notes_text("<p>One &amp; two</p><p>Three<br>four</p>"),
            "One & two\nThree\nfour"
        );
        assert_eq!(notes_text("plain"), "plain");
    }

    #[test]
    fn control_characters_never_reach_the_xml() {
        assert_eq!(xml_escape("a\u{1}b\tc"), "ab\tc");
    }
}
