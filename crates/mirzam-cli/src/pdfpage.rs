//! Reading one page of a PDF into the shape [`mirzam_figure`] works on: text
//! lines with their boxes, and the box of everything painted.
//!
//! This is a *measuring* pass, not a renderer. It walks the content stream
//! keeping the current transform, the text matrix and enough of each font to
//! know how wide a string is and what it says — and it throws away colour,
//! shading, patterns and anything else that cannot move a box. That is why it
//! fits in one file: a figure's rectangle depends on where the ink is, never on
//! what colour it was.
//!
//! Two things are worth knowing about what comes out.
//!
//! **Coordinates are in reading space.** A page with `/Rotate 90` is measured
//! the way a reader sees it, so "above the caption" means what it says. The
//! matrix back to PDF user space rides along in [`Page::to_pdf`], because a
//! `/CropBox` has to be written in the space the file uses, not the one the
//! reader sees.
//!
//! **Widths are approximate when a font declines to say.** A standard-14 font
//! carries no `/Widths`, so its glyphs are measured at half an em. That moves a
//! line's right edge, not its left edge or its baseline, and every rule in
//! `mirzam-figure` that depends on the right edge — a caption's last line
//! stopping short — has slack for it.

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};
use mirzam_figure::{Line, Rect};
use std::collections::HashMap;

/// An affine transform, in PDF's order: `[a b c d e f]`.
pub type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` applied first, then `n`.
fn mul(m: Matrix, n: Matrix) -> Matrix {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn apply(m: Matrix, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// The box holding all four transformed corners — right for the axis-aligned
/// transforms a page actually uses, and safe for the ones it does not.
pub fn map_rect(m: Matrix, r: Rect) -> Rect {
    let corners = [
        apply(m, r.x0, r.y0),
        apply(m, r.x1, r.y0),
        apply(m, r.x0, r.y1),
        apply(m, r.x1, r.y1),
    ];
    let xs = corners.map(|c| c.0);
    let ys = corners.map(|c| c.1);
    Rect {
        x0: xs.iter().copied().fold(f64::INFINITY, f64::min),
        y0: ys.iter().copied().fold(f64::INFINITY, f64::min),
        x1: xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        y1: ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    }
}

fn invert(m: Matrix) -> Option<Matrix> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-9 {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

/// One page, measured.
pub struct Page {
    pub number: u32,
    pub id: ObjectId,
    /// The visible area, in reading space.
    pub rect: Rect,
    pub lines: Vec<Line>,
    pub ink: Vec<Rect>,
    pub images: Vec<Image>,
    /// Reading space back to PDF user space.
    pub to_pdf: Matrix,
}

/// A placed image, and the object holding its samples.
pub struct Image {
    pub id: ObjectId,
    pub rect: Rect,
}

/// Measures one page.
///
/// A content stream that will not parse is an error; a font that will not, or
/// an XObject that is missing, is not — a page half measured still finds its
/// figures, and refusing the whole file over one broken object would fail a
/// paper for a reason its reader cannot see.
pub fn read(doc: &Document, number: u32, id: ObjectId) -> Result<Page, String> {
    let page = doc
        .get_dictionary(id)
        .map_err(|e| format!("page {number}: {e}"))?;
    let media = box_of(doc, page, b"MediaBox").unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
    let crop = box_of(doc, page, b"CropBox")
        .and_then(|c| c.intersect(&media))
        .unwrap_or(media);
    let rotate = inherited(doc, page, b"Rotate")
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0)
        .rem_euclid(360);

    // Reading space: the page as a reader sees it, with the origin at the
    // bottom left of the visible area.
    let base = match rotate {
        90 => [0.0, -1.0, 1.0, 0.0, -crop.y0, crop.x1],
        180 => [-1.0, 0.0, 0.0, -1.0, crop.x1, crop.y1],
        270 => [0.0, 1.0, -1.0, 0.0, crop.y1, -crop.x0],
        _ => [1.0, 0.0, 0.0, 1.0, -crop.x0, -crop.y0],
    };
    let rect = map_rect(base, crop);

    let content = doc.get_page_content(id);
    let content = Content::decode(&content).map_err(|e| format!("page {number}: {e}"))?;
    // Not `get_page_resources`: it hands back the dictionary only when the page
    // writes one inline, and a page that references one — most of them — would
    // come back with no fonts and no images.
    let resources = inherited(doc, page, b"Resources").and_then(|o| o.as_dict().ok());

    let mut walk = Walk {
        doc,
        runs: Vec::new(),
        ink: Vec::new(),
        images: Vec::new(),
        fonts: Vec::new(),
        loaded: HashMap::new(),
    };
    walk.run(
        &content,
        resources,
        State {
            ctm: base,
            clip: rect,
        },
        0,
    );

    Ok(Page {
        number,
        id,
        rect,
        lines: lines_from(walk.runs),
        ink: walk.ink,
        images: walk.images,
        to_pdf: invert(base).unwrap_or(IDENTITY),
    })
}

/// A `/MediaBox` or `/CropBox`, which may be written on an ancestor.
fn box_of(doc: &Document, page: &Dictionary, key: &[u8]) -> Option<Rect> {
    let array = inherited(doc, page, key)?.as_array().ok()?;
    let n: Vec<f64> = array.iter().filter_map(num).collect();
    (n.len() == 4).then(|| Rect::new(n[0], n[1], n[2], n[3]))
}

/// A page attribute, following `/Parent` when the page does not carry it.
fn inherited<'a>(doc: &'a Document, page: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    let mut dict = page;
    for _ in 0..32 {
        if let Ok(value) = dict.get(key) {
            return deref(doc, value);
        }
        dict = deref(doc, dict.get(b"Parent").ok()?)?.as_dict().ok()?;
    }
    None
}

fn deref<'a>(doc: &'a Document, object: &'a Object) -> Option<&'a Object> {
    let mut object = object;
    for _ in 0..32 {
        match object {
            Object::Reference(id) => object = doc.get_object(*id).ok()?,
            other => return Some(other),
        }
    }
    None
}

fn num(object: &Object) -> Option<f64> {
    match object {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// A run of glyphs shown by one operator, already placed on the page.
struct Run {
    rect: Rect,
    size: f64,
    text: String,
}

/// The graphics state this pass cares about: where things land, and what has
/// been clipped away.
#[derive(Clone, Copy)]
struct State {
    ctm: Matrix,
    clip: Rect,
}

struct Walk<'a> {
    doc: &'a Document,
    runs: Vec<Run>,
    ink: Vec<Rect>,
    images: Vec<Image>,
    fonts: Vec<Font>,
    /// Fonts already loaded, by the resource name they were reached through.
    /// A name means a different font inside a form XObject, so the depth is
    /// part of the key.
    loaded: HashMap<(usize, Vec<u8>), usize>,
}

impl Walk<'_> {
    /// Walks one content stream. `depth` counts form XObjects, which nest.
    fn run(
        &mut self,
        content: &Content,
        resources: Option<&Dictionary>,
        start: State,
        depth: usize,
    ) {
        let mut state = start;
        let mut stack: Vec<State> = Vec::new();
        let mut text = TextState::default();
        let mut path: Option<Rect> = None;
        let mut pending_clip = false;
        let mut line_width = 1.0;
        let mut font: Option<usize> = None;
        let unknown = Font::guessed();

        for op in &content.operations {
            let a = &op.operands;
            match op.operator.as_str() {
                "q" => stack.push(state),
                "Q" => state = stack.pop().unwrap_or(start),
                "cm" => {
                    if let Some(m) = matrix(a) {
                        state.ctm = mul(m, state.ctm);
                    }
                }

                // Path construction. Every point is transformed as it arrives,
                // because the transform may be replaced before the path is
                // painted.
                "m" | "l" => {
                    if let (Some(x), Some(y)) = (a.first().and_then(num), a.get(1).and_then(num)) {
                        path = Some(grow(path, apply(state.ctm, x, y)));
                    }
                }
                // `v` and `y` leave one control point implicit; it is the
                // current point, which whatever drew it already counted.
                "c" | "v" | "y" => {
                    for pair in a.chunks(2) {
                        if let (Some(x), Some(y)) =
                            (pair.first().and_then(num), pair.get(1).and_then(num))
                        {
                            path = Some(grow(path, apply(state.ctm, x, y)));
                        }
                    }
                }
                "re" => {
                    let n: Vec<f64> = a.iter().filter_map(num).collect();
                    if n.len() == 4 {
                        let r =
                            map_rect(state.ctm, Rect::new(n[0], n[1], n[0] + n[2], n[1] + n[3]));
                        path = Some(match path {
                            Some(p) => p.union(&r),
                            None => r,
                        });
                    }
                }
                "h" => {}
                "w" => line_width = a.first().and_then(num).unwrap_or(line_width),
                "W" | "W*" => pending_clip = true,

                // Painting. `n` paints nothing, but it is how a clip is ended,
                // so it closes the path like the rest.
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                    if let Some(p) = path {
                        if op.operator != "n" {
                            // A stroke is drawn *around* the path, so a rule
                            // written as one line from here to there is half a
                            // width tall on each side of nothing.
                            let stroked =
                                matches!(op.operator.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*");
                            let pen = if stroked {
                                let scale = state.ctm[0].hypot(state.ctm[1]);
                                (line_width * scale).max(0.4) / 2.0
                            } else {
                                0.0
                            };
                            if let Some(visible) = p.grow(pen).intersect(&state.clip) {
                                self.ink.push(visible);
                            }
                        }
                        if pending_clip {
                            state.clip = p.intersect(&state.clip).unwrap_or(state.clip);
                        }
                    }
                    path = None;
                    pending_clip = false;
                }
                "sh" => {}

                "BT" => text = TextState::default(),
                "ET" => {}
                "Tf" => {
                    text.size = a.get(1).and_then(num).unwrap_or(text.size);
                    if let Some(name) = a.first().and_then(|o| o.as_name().ok()) {
                        let key = (depth, name.to_vec());
                        font = Some(match self.loaded.get(&key) {
                            Some(&at) => at,
                            None => {
                                self.fonts.push(Font::load(self.doc, resources, name));
                                self.loaded.insert(key, self.fonts.len() - 1);
                                self.fonts.len() - 1
                            }
                        });
                    }
                }
                "Td" => {
                    if let (Some(x), Some(y)) = (a.first().and_then(num), a.get(1).and_then(num)) {
                        text.tlm = mul([1.0, 0.0, 0.0, 1.0, x, y], text.tlm);
                        text.tm = text.tlm;
                    }
                }
                "TD" => {
                    if let (Some(x), Some(y)) = (a.first().and_then(num), a.get(1).and_then(num)) {
                        text.leading = -y;
                        text.tlm = mul([1.0, 0.0, 0.0, 1.0, x, y], text.tlm);
                        text.tm = text.tlm;
                    }
                }
                "Tm" => {
                    if let Some(m) = matrix(a) {
                        text.tlm = m;
                        text.tm = m;
                    }
                }
                "T*" => {
                    text.tlm = mul([1.0, 0.0, 0.0, 1.0, 0.0, -text.leading], text.tlm);
                    text.tm = text.tlm;
                }
                "TL" => text.leading = a.first().and_then(num).unwrap_or(0.0),
                "Tc" => text.char_spacing = a.first().and_then(num).unwrap_or(0.0),
                "Tw" => text.word_spacing = a.first().and_then(num).unwrap_or(0.0),
                "Tz" => text.hscale = a.first().and_then(num).unwrap_or(100.0) / 100.0,
                "Ts" => text.rise = a.first().and_then(num).unwrap_or(0.0),
                "Tj" | "'" | "\"" => {
                    if op.operator != "Tj" {
                        text.tlm = mul([1.0, 0.0, 0.0, 1.0, 0.0, -text.leading], text.tlm);
                        text.tm = text.tlm;
                        if op.operator == "\"" {
                            text.word_spacing =
                                a.first().and_then(num).unwrap_or(text.word_spacing);
                            text.char_spacing = a.get(1).and_then(num).unwrap_or(text.char_spacing);
                        }
                    }
                    if let Some(Object::String(bytes, _)) = a.last() {
                        let face = font.map_or(&unknown, |at| &self.fonts[at]);
                        show(&mut self.runs, bytes, face, &mut text, &state);
                    }
                }
                "TJ" => {
                    let Some(Object::Array(items)) = a.first() else {
                        continue;
                    };
                    for item in items {
                        match item {
                            Object::String(bytes, _) => {
                                let face = font.map_or(&unknown, |at| &self.fonts[at]);
                                show(&mut self.runs, bytes, face, &mut text, &state);
                            }
                            other => {
                                if let Some(adjust) = num(other) {
                                    let tx = -adjust / 1000.0 * text.size * text.hscale;
                                    text.tm = mul([1.0, 0.0, 0.0, 1.0, tx, 0.0], text.tm);
                                }
                            }
                        }
                    }
                }

                "Do" => {
                    if let Some(name) = a.first().and_then(|o| o.as_name().ok()) {
                        self.xobject(name, resources, &state, depth);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Places one string, advancing the text matrix as it goes.
fn show(runs: &mut Vec<Run>, bytes: &[u8], font: &Font, text: &mut TextState, state: &State) {
    let mut shown = String::new();
    let mut width = 0.0;
    for (code, single) in font.codes(bytes) {
        shown.push_str(&font.text(code));
        let mut advance = font.width(code) / 1000.0 * text.size + text.char_spacing;
        if single == Some(b' ') {
            advance += text.word_spacing;
        }
        width += advance * text.hscale;
    }

    // The run's box in the space the text matrix maps from: the font size
    // is already in `width`, so only the ascent and descent are guessed.
    let box_here = Rect::new(
        0.0,
        text.rise - text.size * 0.22,
        width,
        text.rise + text.size * 0.78,
    );
    let placed = map_rect(mul(text.tm, state.ctm), box_here);
    text.tm = mul([1.0, 0.0, 0.0, 1.0, width, 0.0], text.tm);

    if shown.trim().is_empty() {
        return;
    }
    if let Some(visible) = placed.intersect(&state.clip) {
        // Clipped to a sliver, the glyphs are decoration behind something
        // else; measured whole, the box is the line the reader sees.
        if visible.area() > placed.area() * 0.5 {
            let scale = (mul(text.tm, state.ctm)[3]).abs().max(1e-6);
            runs.push(Run {
                rect: placed,
                size: text.size * scale,
                text: shown,
            });
        }
    }
}

impl Walk<'_> {
    /// An image is ink and a candidate for extraction; a form is more content
    /// stream, in its own space.
    fn xobject(
        &mut self,
        name: &[u8],
        resources: Option<&Dictionary>,
        state: &State,
        depth: usize,
    ) {
        if depth > 8 {
            return;
        }
        let Some(xobjects) = resources
            .and_then(|r| r.get(b"XObject").ok())
            .and_then(|o| deref(self.doc, o))
            .and_then(|o| o.as_dict().ok())
        else {
            return;
        };
        let Ok(reference) = xobjects.get(name) else {
            return;
        };
        let id = reference.as_reference().ok();
        let Some(stream) = deref(self.doc, reference).and_then(|o| o.as_stream().ok()) else {
            return;
        };
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok());

        match subtype {
            Some(b"Image") => {
                let placed = map_rect(state.ctm, Rect::new(0.0, 0.0, 1.0, 1.0));
                if let Some(visible) = placed.intersect(&state.clip) {
                    self.ink.push(visible);
                    if let Some(id) = id {
                        self.images.push(Image { id, rect: visible });
                    }
                }
            }
            Some(b"Form") => {
                let matrix = stream
                    .dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .and_then(|a| matrix(a))
                    .unwrap_or(IDENTITY);
                let mut inner = State {
                    ctm: mul(matrix, state.ctm),
                    clip: state.clip,
                };
                if let Some(bbox) = stream
                    .dict
                    .get(b"BBox")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .map(|a| a.iter().filter_map(num).collect::<Vec<f64>>())
                    .filter(|n| n.len() == 4)
                {
                    let clip = map_rect(inner.ctm, Rect::new(bbox[0], bbox[1], bbox[2], bbox[3]));
                    inner.clip = clip.intersect(&state.clip).unwrap_or(clip);
                }
                let Ok(data) = stream.decompressed_content() else {
                    return;
                };
                let Ok(content) = Content::decode(&data) else {
                    return;
                };
                let inner_resources = stream
                    .dict
                    .get(b"Resources")
                    .ok()
                    .and_then(|o| deref(self.doc, o))
                    .and_then(|o| o.as_dict().ok())
                    .or(resources);
                self.run(&content, inner_resources, inner, depth + 1);
            }
            _ => {}
        }
    }
}

fn grow(path: Option<Rect>, point: (f64, f64)) -> Rect {
    let here = Rect::new(point.0, point.1, point.0, point.1);
    match path {
        Some(p) => p.union(&here),
        None => here,
    }
}

fn matrix(operands: &[Object]) -> Option<Matrix> {
    let n: Vec<f64> = operands.iter().filter_map(num).collect();
    (n.len() == 6).then(|| [n[0], n[1], n[2], n[3], n[4], n[5]])
}

#[derive(Clone)]
struct TextState {
    tm: Matrix,
    tlm: Matrix,
    size: f64,
    leading: f64,
    char_spacing: f64,
    word_spacing: f64,
    hscale: f64,
    rise: f64,
}

impl Default for TextState {
    fn default() -> Self {
        TextState {
            tm: IDENTITY,
            tlm: IDENTITY,
            size: 0.0,
            leading: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            hscale: 1.0,
            rise: 0.0,
        }
    }
}

/// As much of a font as a measuring pass needs: how wide each code is, and
/// what it says.
#[derive(Default)]
struct Font {
    widths: HashMap<u32, f64>,
    default_width: f64,
    two_byte: bool,
    unicode: HashMap<u32, String>,
}

/// Helvetica's widths for ASCII, in thousandths of an em.
///
/// One of the fourteen fonts every PDF reader is required to have, and
/// therefore one of the fourteen a file may use without embedding metrics for.
/// Something has to be assumed for those, and half an em for every glyph is a
/// poor assumption: a line of `l`s and `i`s comes out half again too wide, and
/// a text box that overshoots into the next column is read as one line
/// spanning both. These are the real numbers, and they are close enough for
/// Times and Courier too — closer, at any rate, than a flat guess.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

impl Font {
    /// A font nothing is known about: measured as Helvetica, its codes read as
    /// Latin-1. Better than dropping the string, which would lose the caption
    /// the whole pass is looking for.
    fn guessed() -> Font {
        Font {
            widths: HELVETICA
                .iter()
                .enumerate()
                .map(|(i, w)| (i as u32 + 32, *w as f64))
                .collect(),
            default_width: 500.0,
            ..Font::default()
        }
    }

    fn load(doc: &Document, resources: Option<&Dictionary>, name: &[u8]) -> Font {
        let dict = resources
            .and_then(|r| r.get(b"Font").ok())
            .and_then(|o| deref(doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|fonts| fonts.get(name).ok())
            .and_then(|o| deref(doc, o))
            .and_then(|o| o.as_dict().ok());
        let Some(dict) = dict else {
            return Font::guessed();
        };

        let mut font = Font::guessed();
        if let Some(stream) = dict
            .get(b"ToUnicode")
            .ok()
            .and_then(|o| deref(doc, o))
            .and_then(|o| o.as_stream().ok())
            .and_then(|s| s.decompressed_content().ok())
        {
            font.unicode = to_unicode(&stream);
        }

        let composite = dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            .is_some_and(|s| s == b"Type0");
        if composite {
            font.two_byte = true;
            font.default_width = 1000.0;
            // The guessed widths are keyed by character code; in a composite
            // font the same numbers are glyph ids, and `65` is whatever glyph
            // the subset put there.
            font.widths.clear();
            let descendant = dict
                .get(b"DescendantFonts")
                .ok()
                .and_then(|o| deref(doc, o))
                .and_then(|o| o.as_array().ok())
                .and_then(|a| a.first())
                .and_then(|o| deref(doc, o))
                .and_then(|o| o.as_dict().ok());
            if let Some(descendant) = descendant {
                if let Some(dw) = descendant.get(b"DW").ok().and_then(num) {
                    font.default_width = dw;
                }
                if let Some(w) = descendant
                    .get(b"W")
                    .ok()
                    .and_then(|o| deref(doc, o))
                    .and_then(|o| o.as_array().ok())
                {
                    font.read_cid_widths(doc, w);
                }
            }
            return font;
        }

        let first = dict.get(b"FirstChar").ok().and_then(num).unwrap_or(0.0) as u32;
        if let Some(widths) = dict
            .get(b"Widths")
            .ok()
            .and_then(|o| deref(doc, o))
            .and_then(|o| o.as_array().ok())
        {
            for (i, w) in widths.iter().filter_map(num).enumerate() {
                font.widths.insert(first + i as u32, w);
            }
        }
        if let Some(missing) = dict
            .get(b"FontDescriptor")
            .ok()
            .and_then(|o| deref(doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"MissingWidth").ok())
            .and_then(num)
        {
            font.default_width = missing;
        }
        font
    }

    /// `/W` is `[ c [w w w] cfirst clast w ]`, in either form, repeated.
    fn read_cid_widths(&mut self, doc: &Document, w: &[Object]) {
        let mut i = 0;
        while i < w.len() {
            let Some(first) = num(&w[i]) else { break };
            match w.get(i + 1).and_then(|o| deref(doc, o)) {
                Some(Object::Array(list)) => {
                    for (k, width) in list.iter().filter_map(num).enumerate() {
                        self.widths.insert(first as u32 + k as u32, width);
                    }
                    i += 2;
                }
                Some(other) => {
                    let (Some(last), Some(width)) = (num(other), w.get(i + 2).and_then(num)) else {
                        break;
                    };
                    // A run of thousands of identical widths is normal in a CJK
                    // font; it is also a memory trap, so it is capped.
                    for code in first as u32..=(last as u32).min(first as u32 + 65_535) {
                        self.widths.insert(code, width);
                    }
                    i += 3;
                }
                None => break,
            }
        }
    }

    /// The codes in a string, with the byte when the code is a single one —
    /// word spacing applies to a single-byte code 32 and to nothing else.
    fn codes(&self, bytes: &[u8]) -> Vec<(u32, Option<u8>)> {
        if self.two_byte {
            bytes
                .chunks(2)
                .map(|c| {
                    let hi = *c.first().unwrap_or(&0) as u32;
                    let lo = *c.get(1).unwrap_or(&0) as u32;
                    ((hi << 8) | lo, None)
                })
                .collect()
        } else {
            bytes.iter().map(|&b| (b as u32, Some(b))).collect()
        }
    }

    fn width(&self, code: u32) -> f64 {
        *self.widths.get(&code).unwrap_or(&self.default_width)
    }

    /// What a code says. Without a `/ToUnicode` map a single-byte code is read
    /// as Latin-1, which is right for the ASCII a caption's label is written
    /// in whatever encoding the font declares; a two-byte code without one is
    /// unknowable, and comes back as a placeholder so the width still counts.
    fn text(&self, code: u32) -> String {
        if let Some(mapped) = self.unicode.get(&code) {
            return mapped.clone();
        }
        if self.two_byte {
            return "\u{fffd}".to_string();
        }
        char::from_u32(code).unwrap_or('\u{fffd}').to_string()
    }
}

/// Parses the `beginbfchar` and `beginbfrange` sections of a `/ToUnicode` CMap.
fn to_unicode(cmap: &[u8]) -> HashMap<u32, String> {
    let text = String::from_utf8_lossy(cmap);
    let mut map = HashMap::new();
    for (start, end, ranged) in [
        ("beginbfchar", "endbfchar", false),
        ("beginbfrange", "endbfrange", true),
    ] {
        let mut rest = text.as_ref();
        while let Some(from) = rest.find(start) {
            let section = &rest[from + start.len()..];
            let to = section.find(end).unwrap_or(section.len());
            let tokens = cmap_tokens(&section[..to]);
            let step = if ranged { 3 } else { 2 };
            for entry in tokens.chunks(step) {
                if entry.len() < step {
                    break;
                }
                let Some(code) = hex_code(&entry[0]) else {
                    continue;
                };
                if !ranged {
                    if let Token::Hex(value) = &entry[1] {
                        map.insert(code, utf16be(value));
                    }
                    continue;
                }
                let Some(last) = hex_code(&entry[1]) else {
                    continue;
                };
                match &entry[2] {
                    Token::Hex(value) => {
                        let base = utf16be(value);
                        // A range maps consecutive codes to consecutive
                        // characters, which only makes sense one character at
                        // a time; anything longer maps whole and stops.
                        let mut chars = base.chars();
                        match (chars.next(), chars.next()) {
                            (Some(c), None) => {
                                for (k, code) in (code..=last.min(code + 65_535)).enumerate() {
                                    let shifted = char::from_u32(c as u32 + k as u32);
                                    map.insert(code, shifted.unwrap_or(c).to_string());
                                }
                            }
                            _ => {
                                map.insert(code, base);
                            }
                        }
                    }
                    Token::List(values) => {
                        for (k, value) in values.iter().enumerate() {
                            map.insert(code + k as u32, utf16be(value));
                        }
                    }
                }
            }
            rest = &section[to..];
        }
    }
    map
}

enum Token {
    Hex(Vec<u8>),
    List(Vec<Vec<u8>>),
}

fn hex_code(token: &Token) -> Option<u32> {
    let Token::Hex(bytes) = token else {
        return None;
    };
    Some(bytes.iter().fold(0u32, |acc, &b| (acc << 8) | b as u32))
}

/// A CMap section is `<hex>` strings and `[ <hex> … ]` lists, and nothing else
/// this pass reads.
fn cmap_tokens(section: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = section.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '<' => {
                let end = section[i..]
                    .find('>')
                    .map(|e| i + e)
                    .unwrap_or(section.len());
                tokens.push(Token::Hex(hex_bytes(&section[i + 1..end])));
                while chars.peek().is_some_and(|&(j, _)| j < end) {
                    chars.next();
                }
            }
            '[' => {
                let end = section[i..]
                    .find(']')
                    .map(|e| i + e)
                    .unwrap_or(section.len());
                let list = section[i + 1..end]
                    .split('<')
                    .filter_map(|part| part.split('>').next())
                    .filter(|part| !part.trim().is_empty())
                    .map(hex_bytes)
                    .collect();
                tokens.push(Token::List(list));
                while chars.peek().is_some_and(|&(j, _)| j < end) {
                    chars.next();
                }
            }
            _ => {}
        }
    }
    tokens
}

fn hex_bytes(src: &str) -> Vec<u8> {
    let digits: Vec<u8> = src
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_digit(16).unwrap_or(0) as u8)
        .collect();
    digits
        .chunks(2)
        .map(|p| (p[0] << 4) | p.get(1).copied().unwrap_or(0))
        .collect()
}

fn utf16be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|c| ((*c.first().unwrap_or(&0) as u16) << 8) | *c.get(1).unwrap_or(&0) as u16)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Runs into lines.
///
/// A PDF has no lines: it has strings placed one after another, sometimes one
/// per glyph. Two runs belong to the same line when they share a baseline and
/// are not separated by more than a space or so — the gap test is what keeps
/// two columns, or two cells of a table, from being read as one line.
fn lines_from(mut runs: Vec<Run>) -> Vec<Line> {
    runs.retain(|r| r.rect.width().is_finite() && r.rect.height().is_finite());
    runs.sort_by(|a, b| {
        b.rect
            .y0
            .partial_cmp(&a.rect.y0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.rect
                    .x0
                    .partial_cmp(&b.rect.x0)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    let mut lines: Vec<Line> = Vec::new();
    let mut current: Option<(Line, f64)> = None;
    for run in runs {
        match current.take() {
            Some((mut line, right)) => {
                let same_baseline =
                    (line.rect.y0 - run.rect.y0).abs() < line.size.max(run.size) * 0.4;
                let gap = run.rect.x0 - right;
                // The overlap allowed is a kerned glyph's worth, not a
                // column's: a width estimated from a font that gave no metrics
                // can run past where the text really ended, and a generous
                // rule here reads the two columns of a paper as single lines.
                if same_baseline && gap < run.size.max(1.0) * 1.5 && gap > -run.size.max(1.0) {
                    if gap > run.size * 0.18 && !line.text.ends_with(' ') {
                        line.text.push(' ');
                    }
                    line.text.push_str(&run.text);
                    line.rect = line.rect.union(&run.rect);
                    line.size = line.size.max(run.size);
                    current = Some((line, run.rect.x1));
                    continue;
                }
                lines.push(line);
                current = Some((
                    Line {
                        rect: run.rect,
                        size: run.size,
                        text: run.text,
                    },
                    run.rect.x1,
                ));
            }
            None => {
                current = Some((
                    Line {
                        rect: run.rect,
                        size: run.size,
                        text: run.text,
                    },
                    run.rect.x1,
                ))
            }
        }
    }
    lines.extend(current.map(|(line, _)| line));
    lines.retain(|l| !l.text.trim().is_empty());
    lines
}
