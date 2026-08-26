//! The crop as an SVG, with nothing installed.
//!
//! [hayro] interprets and renders PDF in pure Rust under Apache-2.0 OR MIT, so
//! the conversion that used to need `mutool` or `pdftocairo` — the one step in
//! `import pdf` that asked the author to go and install something — happens
//! here instead. Text comes out as outline paths, which is what `text=path`
//! asked of `mutool`: a deck embeds its pictures as data URIs, where a font
//! referenced by name is a font that is not there.
//!
//! # Why there is a second pass
//!
//! hayro converts a **page**, and keeps what falls outside it. A crop here *is*
//! a page — the paper's own, with its box narrowed to the figure — so a
//! straight conversion carries every glyph in both columns along with the
//! figure: 1.49 MB against `mutool`'s 4 to 17 KB, and the same 1.49 MB
//! whichever figure it is. A deck inlines its pictures under a 20 MB ceiling,
//! so that is not a detail.
//!
//! [`cull`] drops what the `viewBox` cannot show. It is deliberately timid: it
//! removes only a `<path>` or `<use>` that sits directly under `<svg>` and can
//! be *proven* outside, plus the definitions nothing refers to any more.
//! Anything it does not understand — a group, a clip, a shorthand it cannot
//! measure — is kept whole. A picture that is larger than it needs to be is a
//! nuisance; a picture missing a line is a lie.
//!
//! [hayro]: https://github.com/LaurenzV/hayro

use hayro_interpret::{InterpreterSettings, InterpreterWarning};
use std::sync::{Arc, Mutex};

/// Converts a one-page crop into an SVG.
///
/// The error is a sentence for the author, because every one of them ends the
/// same way: the crop is written instead and the command says why.
pub fn convert(pdf: &[u8]) -> Result<String, String> {
    let data = Arc::new(pdf.to_vec());
    let pdf =
        hayro_syntax::Pdf::new(data).map_err(|e| format!("hayro cannot read the crop: {e:?}"))?;
    let pages = pdf.pages();
    let page = pages
        .iter()
        .next()
        .ok_or_else(|| "the crop has no page in it".to_string())?;

    // Warnings are the refusal path. hayro reports the two things that would
    // leave a hole in the picture - a font it cannot read, an image it cannot
    // decode - and a figure with a hole in it is worse than a figure the
    // author converts by hand.
    let seen: Arc<Mutex<Vec<InterpreterWarning>>> = Arc::new(Mutex::new(Vec::new()));
    let collected = seen.clone();
    let settings = InterpreterSettings {
        warning_sink: Arc::new(move |warning| {
            if let Ok(mut seen) = collected.lock() {
                seen.push(warning);
            }
        }),
        // The crop drops `/Annots` already; this says so twice, since a link
        // from the paper is not part of the picture.
        render_annotations: false,
        ..InterpreterSettings::default()
    };

    let svg = hayro_svg::convert(
        page,
        &hayro_svg::RenderCache::new(),
        &settings,
        &hayro_svg::SvgRenderSettings::default(),
    );

    let seen = seen
        .lock()
        .map_err(|_| "the converter stopped".to_string())?;
    if let Some(warning) = seen.first() {
        return Err(match warning {
            InterpreterWarning::UnsupportedFont => {
                "a font hayro cannot read (a CID font with its own encoding)".to_string()
            }
            InterpreterWarning::ImageDecodeFailure => {
                "a picture inside the figure that would not decode".to_string()
            }
        });
    }
    Ok(scalable(&cull(&svg)))
}

/// Takes the fixed size off the root element, leaving the `viewBox`.
///
/// A converter writes the page's own measurements — `width="511.47"` — and an
/// `<img>` then draws the figure at 511 CSS pixels whatever box it was given.
/// On a slide that is a picture sitting at half the width of its pane with no
/// way to say otherwise. The `viewBox` carries the shape, which is the part
/// worth keeping; how big to draw it is the deck's business.
pub fn scalable(svg: &str) -> String {
    let Some(open) = svg.find("<svg") else {
        return svg.to_string();
    };
    let Some(end) = svg[open..].find('>').map(|at| open + at) else {
        return svg.to_string();
    };
    let head = &svg[open..end];
    if !head.contains("viewBox=") {
        return svg.to_string();
    }
    let mut kept = String::with_capacity(svg.len());
    kept.push_str(&svg[..open]);
    kept.push_str("<svg");
    let mut rest = &head[4..];
    while let Some(at) = rest.find(|c: char| !c.is_whitespace()) {
        let attribute = &rest[at..];
        let Some(name_end) = attribute.find('=') else {
            break;
        };
        let name = attribute[..name_end].trim();
        // The value runs to the closing quote of whichever quote opens it.
        let after = &attribute[name_end + 1..];
        let quote = after.chars().next().unwrap_or('"');
        let Some(close) = after[1..].find(quote).map(|at| at + 2) else {
            break;
        };
        if name != "width" && name != "height" {
            kept.push(' ');
            kept.push_str(&attribute[..name_end + 1 + close]);
        }
        rest = &after[close..];
    }
    kept.push_str(&svg[end..]);
    kept
}

/// Drops what the `viewBox` cannot show.
///
/// Two passes and a fixed point: measure the definitions, decide which drawn
/// elements are visible, then keep the definitions those still name — a clip
/// path may name a gradient, so following the names once is not enough.
pub fn cull(svg: &str) -> String {
    let Some(view) = view_box(svg) else {
        return svg.to_string();
    };
    let elements = scan(svg);
    let boxes = measure(&elements);

    let mut kept: Vec<bool> = vec![true; elements.len()];
    for (i, element) in elements.iter().enumerate() {
        // Only what is drawn straight onto the page: inside a group, one
        // element's box says nothing about where the group puts it, so the
        // group is judged as a whole or not at all.
        if element.depth != 1 || id_of(element.text).is_some() {
            continue;
        }
        let visible = match boxes[i] {
            Extent::Nothing => false,
            Extent::Box(own) => own.overlaps(&view),
            Extent::Unknown => true,
        };
        if !visible {
            // A container takes its contents with it.
            for (j, other) in elements.iter().enumerate() {
                if other.start >= element.start && other.close_end <= element.close_end {
                    kept[j] = false;
                }
            }
        }
    }

    // A definition survives while anything kept still names it.
    loop {
        let names = named(&elements, &kept);
        let mut changed = false;
        for (i, element) in elements.iter().enumerate() {
            if let Some(id) = id_of(element.text).filter(|_| element.empty) {
                let wanted = names.iter().any(|n| n == id);
                if kept[i] != wanted {
                    kept[i] = wanted;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut out = String::with_capacity(svg.len() / 4);
    let mut at = 0;
    for (i, element) in elements.iter().enumerate() {
        if kept[i] {
            continue;
        }
        // The line goes with it. Three thousand dropped glyphs otherwise leave
        // three thousand blank lines, which cost more than the glyphs did.
        let mut from = element.start;
        while from > at && matches!(svg.as_bytes()[from - 1], b' ' | b'\t') {
            from -= 1;
        }
        if from > at && svg.as_bytes()[from - 1] == b'\n' {
            from -= 1;
        }
        if element.start < at {
            // Already inside something dropped.
            continue;
        }
        out.push_str(&svg[at..from]);
        at = element.close_end;
    }
    out.push_str(&svg[at..]);
    out
}

/// Measures every element in the coordinates of whatever holds it.
///
/// Children first, so a group can be the union of what it draws: hayro wraps a
/// clipped drawing, and every placed image, in a `<g>`, and an image is the
/// one thing here big enough that carrying an unseen one costs megabytes.
fn measure(elements: &[Element]) -> Vec<Extent> {
    let mut boxes = vec![Extent::Unknown; elements.len()];
    let defs: Vec<(&str, usize)> = elements
        .iter()
        .enumerate()
        .filter_map(|(i, e)| Some((id_of(e.text)?, i)))
        .collect();

    for i in (0..elements.len()).rev() {
        let element = &elements[i];
        let own = if element.empty {
            match element.name {
                "use" => href_of(element.text)
                    .and_then(|id| defs.iter().find(|(d, _)| *d == id))
                    .map(|(_, at)| boxes[*at])
                    .unwrap_or(Extent::Unknown),
                "image" => image_box(element.text),
                _ => extent(element.text),
            }
        } else {
            children(elements, i).fold(Extent::Nothing, |acc, child| match (acc, boxes[child]) {
                (Extent::Unknown, _) | (_, Extent::Unknown) => Extent::Unknown,
                (Extent::Nothing, other) => other,
                (own, Extent::Nothing) => own,
                (Extent::Box(a), Extent::Box(b)) => Extent::Box(a.union(&b)),
            })
        };
        boxes[i] = match (own, transform_of(element.text)) {
            (Extent::Box(own), Some(transform)) => Extent::Box(map_rect(transform, own)),
            (Extent::Box(_), None) => Extent::Unknown,
            (other, _) => other,
        };
    }
    boxes
}

/// The elements one element holds directly.
fn children<'a>(elements: &'a [Element<'a>], parent: usize) -> impl Iterator<Item = usize> + 'a {
    let (start, end, depth) = (
        elements[parent].end,
        elements[parent].close_end,
        elements[parent].depth,
    );
    (parent + 1..elements.len()).filter(move |&i| {
        elements[i].depth == depth + 1 && elements[i].start >= start && elements[i].close_end <= end
    })
}

/// An `<image>` is placed by its own box and its transform.
fn image_box(text: &str) -> Extent {
    let number = |name: &str, fallback: f64| {
        attribute(text, name)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    let (Some(width), Some(height)) = (
        attribute(text, "width").and_then(|v| v.parse::<f64>().ok()),
        attribute(text, "height").and_then(|v| v.parse::<f64>().ok()),
    ) else {
        return Extent::Unknown;
    };
    let (x, y) = (number("x", 0.0), number("y", 0.0));
    Extent::Box(Rect {
        x0: x,
        y0: y,
        x1: x + width,
        y1: y + height,
    })
}

/// Every `#name` the kept elements refer to — a `use`, a clip, a fill that
/// points at a gradient.
fn named(elements: &[Element], kept: &[bool]) -> Vec<String> {
    let mut names = Vec::new();
    for (i, element) in elements.iter().enumerate() {
        if !kept[i] || id_of(element.text).is_some() {
            continue;
        }
        let mut rest = element.text;
        while let Some(at) = rest.find('#') {
            rest = &rest[at + 1..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names
}

/// One element as it appears in the source, with how deep it sits.
struct Element<'a> {
    name: &'a str,
    text: &'a str,
    start: usize,
    /// The end of the opening tag.
    end: usize,
    /// The end of the element — past its closing tag, when it has one. Removing
    /// a container means removing what it holds.
    close_end: usize,
    depth: usize,
    /// Whether it closes itself. Only these may be dropped: an element that
    /// opens a scope takes its children with it, and hayro's own `<defs>`
    /// carries an id, which would otherwise read as a definition nothing
    /// refers to.
    empty: bool,
}

/// Splits the document into tags, keeping track of nesting.
///
/// Written by hand rather than with a parser, because what it reads is one
/// generator's output: tags, quoted attributes, no comments, no CDATA. What it
/// does not understand it leaves alone, and leaving something alone here means
/// keeping it.
fn scan(svg: &str) -> Vec<Element<'_>> {
    let bytes = svg.as_bytes();
    let mut elements: Vec<Element> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut depth = 0usize;
    let mut at = 0usize;
    while let Some(open_at) = svg[at..].find('<').map(|i| at + i) {
        let mut end = open_at + 1;
        let mut quote: Option<u8> = None;
        while end < bytes.len() {
            match (quote, bytes[end]) {
                (Some(q), c) if c == q => quote = None,
                (None, c @ (b'"' | b'\'')) => quote = Some(c),
                (None, b'>') => break,
                _ => {}
            }
            end += 1;
        }
        if end >= bytes.len() {
            break;
        }
        end += 1;
        let text = &svg[open_at..end];
        let closing = text.starts_with("</");
        let empty = text.ends_with("/>");
        let name = text
            .trim_start_matches(['<', '/'])
            .split([' ', '\t', '\n', '>', '/'])
            .next()
            .unwrap_or("");

        if closing {
            depth = depth.saturating_sub(1);
            if let Some(opened) = open.pop() {
                elements[opened].close_end = end;
            }
        } else {
            // The root `<svg>` is depth zero, so what it draws directly is
            // depth one - the only depth this dares to remove anything at.
            elements.push(Element {
                name,
                text,
                start: open_at,
                end,
                close_end: end,
                depth,
                empty,
            });
            if !empty {
                open.push(elements.len() - 1);
                depth += 1;
            }
        }
        at = end;
    }
    elements
}

/// One attribute's value.
///
/// The name has to end where it starts: `d` may not be found inside `id`, and
/// `href` has to be found inside `xlink:href`, which is how every `use` hayro
/// writes names its glyph.
fn attribute<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let mut at = 0;
    while let Some(found) = text[at..].find(name).map(|i| at + i) {
        let after = &text[found + name.len()..];
        let before = text[..found].chars().next_back();
        let boundary = matches!(before, None | Some(' ' | '\t' | '\n' | ':' | '<'));
        if boundary && after.starts_with("=\"") {
            return after[2..].split('"').next();
        }
        at = found + name.len();
    }
    None
}

fn id_of(text: &str) -> Option<&str> {
    attribute(text, "id")
}

fn href_of(text: &str) -> Option<&str> {
    attribute(text, "href").map(|h| h.trim_start_matches('#'))
}

fn view_box(svg: &str) -> Option<Rect> {
    let n: Vec<f64> = attribute(svg.split('>').next()?, "viewBox")?
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    (n.len() == 4).then(|| Rect {
        x0: n[0],
        y0: n[1],
        x1: n[0] + n[2],
        y1: n[1] + n[3],
    })
}

/// How much room an element takes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Extent {
    /// It paints nothing at all — the outline of a space, most often, of which
    /// a page of prose has hundreds.
    Nothing,
    /// Every point it is drawn through, control points included, so the box is
    /// never smaller than the curve inside it.
    Box(Rect),
    /// Not measurable here: a relative or shorthand command, an arc, an
    /// element with no path at all. Whatever it is, it stays.
    Unknown,
}

fn extent(text: &str) -> Extent {
    let Some(d) = attribute(text, "d") else {
        return Extent::Unknown;
    };
    if d.trim().is_empty() {
        return Extent::Nothing;
    }
    if d.contains([
        'a', 'A', 'h', 'H', 'v', 'V', 's', 'S', 't', 'T', 'm', 'l', 'c', 'q', 'z',
    ]) {
        return Extent::Unknown;
    }
    let mut points = Vec::new();
    for chunk in d.split(['M', 'L', 'C', 'Q', 'Z', ' ']) {
        let mut pair = chunk.split(',');
        if let (Some(Ok(x)), Some(Ok(y))) = (
            pair.next().map(str::trim).map(str::parse::<f64>),
            pair.next().map(str::trim).map(str::parse::<f64>),
        ) {
            points.push((x, y));
        }
    }
    let Some(first) = points.first() else {
        return Extent::Nothing;
    };
    let mut rect = Rect {
        x0: first.0,
        y0: first.1,
        x1: first.0,
        y1: first.1,
    };
    for (x, y) in &points {
        rect.x0 = rect.x0.min(*x);
        rect.y0 = rect.y0.min(*y);
        rect.x1 = rect.x1.max(*x);
        rect.y1 = rect.y1.max(*y);
    }
    Extent::Box(rect)
}

/// `transform="matrix(a b c d e f)"`, and nothing else — an element carrying a
/// transform this cannot read is one this may not measure. An element with no
/// transform at all is measured as it stands.
fn transform_of(text: &str) -> Option<[f64; 6]> {
    let Some(transform) = attribute(text, "transform") else {
        return Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    };
    let inside = transform.strip_prefix("matrix(")?.strip_suffix(')')?;
    let n: Vec<f64> = inside
        .split([' ', ','])
        .filter(|v| !v.is_empty())
        .filter_map(|v| v.parse().ok())
        .collect();
    (n.len() == 6).then(|| [n[0], n[1], n[2], n[3], n[4], n[5]])
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn union(&self, other: &Rect) -> Rect {
        Rect {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    fn overlaps(&self, other: &Rect) -> bool {
        self.x1 >= other.x0 && self.x0 <= other.x1 && self.y1 >= other.y0 && self.y0 <= other.y1
    }
}

fn map_rect(m: [f64; 6], r: Rect) -> Rect {
    let corners = [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)]
        .map(|(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]));
    Rect {
        x0: corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min),
        y0: corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min),
        x1: corners
            .iter()
            .map(|c| c.0)
            .fold(f64::NEG_INFINITY, f64::max),
        y1: corners
            .iter()
            .map(|c| c.1)
            .fold(f64::NEG_INFINITY, f64::max),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_keeps_its_shape_and_loses_its_size() {
        let svg = r##"<svg viewBox="0 0 511 133" width="511" height="133.4" xmlns="http://www.w3.org/2000/svg"><g/></svg>"##;
        assert_eq!(
            scalable(svg),
            r##"<svg viewBox="0 0 511 133" xmlns="http://www.w3.org/2000/svg"><g/></svg>"##
        );
    }

    #[test]
    fn a_root_with_no_view_box_keeps_the_only_size_it_has() {
        // Without one there is nothing left to say how tall the figure is.
        let svg = r##"<svg width="511" height="133"><g/></svg>"##;
        assert_eq!(scalable(svg), svg);
    }

    #[test]
    fn an_attribute_in_single_quotes_survives() {
        let svg = r##"<svg viewBox='0 0 4 2' width='4' id='fig'><g/></svg>"##;
        assert_eq!(
            scalable(svg),
            r##"<svg viewBox='0 0 4 2' id='fig'><g/></svg>"##
        );
    }

    /// A page the shape hayro writes one: drawn elements first, then the glyph
    /// outlines they point at, inside a `<defs>` that carries an id of its own.
    const PAGE: &str = r##"<svg viewBox="0 0 100 50" xmlns="http://www.w3.org/2000/svg">
    <path d="M10,10 L90,10 L90,40 L10,40 Z" fill="#eeeeee" transform="matrix(1 0 0 1 0 0)"/>
    <path d="M10,500 L90,500 L90,540 L10,540 Z" fill="#ff0000" transform="matrix(1 0 0 1 0 0)"/>
    <path d="M0,0 h10 v10" fill="#00ff00" transform="matrix(1 0 0 1 0 0)"/>
    <use xlink:href="#g0" transform="matrix(1 0 0 1 0 0)" fill="#000000"/>
    <use xlink:href="#g1" transform="matrix(1 0 0 1 0 0)" fill="#000000"/>
    <use xlink:href="#g2" transform="matrix(1 0 0 1 0 0)" fill="#000000"/>
    <g transform="matrix(1 0 0 1 0 0)">
        <path d="M10,900 L20,900 L20,910 Z" fill="#0000ff"/>
    </g>
    <g transform="matrix(1 0 0 1 0 0)">
        <path d="M10,10 m5,5 l3,3" fill="#00ffff"/>
    </g>
    <g>
        <image transform="matrix(100 0 0 30 2 900)" xlink:href="data:image/png;base64,AAAA" width="2" height="2"/>
    </g>
    <defs id="outline-glyph">
        <path id="g0" d="M20,20 L30,20 L30,30 Z"/>
        <path id="g1" d="M20,900 L30,900 L30,910 Z"/>
        <path id="g2" d=""/>
    </defs>
</svg>"##;

    /// Tags in, tags out, and every one of them closed — the failure this
    /// caught in the making was an emptied `<defs>` whose opening tag went
    /// with its contents, which a browser renders as a parse error rather than
    /// as a figure.
    fn balanced(svg: &str) -> bool {
        let mut open: Vec<&str> = Vec::new();
        for element in scan(svg) {
            if !element.empty {
                open.push(element.name);
            }
        }
        let closes = svg.matches("</").count();
        closes == open.len()
    }

    #[test]
    fn what_the_view_cannot_show_goes() {
        let culled = cull(PAGE);
        assert!(culled.contains("#eeeeee"), "the figure stays: {culled}");
        assert!(
            !culled.contains("#ff0000"),
            "the far-off rectangle goes: {culled}"
        );
        assert!(culled.contains(r##"<use xlink:href="#g0""##), "{culled}");
        assert!(!culled.contains(r##"<use xlink:href="#g1""##), "{culled}");
        assert!(balanced(&culled), "{culled}");
    }

    /// A definition survives exactly as long as something still names it.
    #[test]
    fn the_glyphs_nobody_draws_go_with_them() {
        let culled = cull(PAGE);
        assert!(culled.contains(r##"<path id="g0""##), "{culled}");
        assert!(!culled.contains(r##"<path id="g1""##), "{culled}");
        assert!(
            culled.contains(r##"<defs id="outline-glyph">"##) && culled.contains("</defs>"),
            "the container is not a definition: {culled}"
        );
    }

    /// The outline of a space is a path with nothing in it, and a page of prose
    /// has hundreds. They are the one thing dropped without being measured.
    #[test]
    fn a_glyph_that_paints_nothing_is_dropped() {
        let culled = cull(PAGE);
        assert!(!culled.contains(r##"<use xlink:href="#g2""##), "{culled}");
        assert!(!culled.contains(r##"<path id="g2""##), "{culled}");
    }

    /// Timid on purpose: a shorthand this cannot measure stays, and so does
    /// the group that holds it. A picture larger than it needs to be is a
    /// nuisance; a picture missing a line is a lie.
    #[test]
    fn what_it_cannot_measure_it_keeps() {
        let culled = cull(PAGE);
        assert!(
            culled.contains("#00ff00"),
            "the `h` shorthand stays: {culled}"
        );
        assert!(
            culled.contains("#00ffff"),
            "and the group holding one stays with it: {culled}"
        );
    }

    /// A group *is* measurable when everything in it is, and then it goes as
    /// one. hayro wraps each placed image in a group, so this is what stops a
    /// figure from carrying the photograph in the next column — the one thing
    /// on the page big enough to cost megabytes.
    #[test]
    fn a_group_is_measured_by_what_it_holds() {
        let culled = cull(PAGE);
        assert!(
            !culled.contains("#0000ff"),
            "a group of far-off paths goes: {culled}"
        );
        assert!(
            !culled.contains("data:image/png"),
            "and so does one holding a far-off image: {culled}"
        );
        assert!(balanced(&culled), "{culled}");
    }

    /// Nothing to measure against, nothing to remove.
    #[test]
    fn a_page_with_no_view_box_is_left_alone() {
        let odd = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0,0 L1,1"/></svg>"#;
        assert_eq!(cull(odd), odd);
    }

    #[test]
    fn the_lines_go_with_the_elements() {
        let culled = cull(PAGE);
        assert!(
            !culled.lines().any(|l| l.trim().is_empty()),
            "a dropped element leaves no blank line behind: {culled}"
        );
    }
}
