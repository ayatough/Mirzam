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
use mirzam_figure::Line;
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

/// Puts the figure's own words back on top of it, where they cannot be seen.
///
/// A converter draws every glyph as an outline path, because the reader does
/// not have the paper's fonts and substituting one would change every width in
/// a table. The picture is then exact and the text is gone: a table of results
/// cannot be selected, copied, searched or read aloud.
///
/// So the words are laid over the picture a second time, from what the
/// measuring pass already read off the page, at an alpha of one part in 255.
/// Nothing is visible and nothing moves — `textLength` holds each line to the
/// width it was drawn at, whatever font the viewer sets it in — but the
/// characters are there for anything that looks for characters. It is the same
/// arrangement a scanned page gets when it is put through OCR.
///
/// One part in 255 rather than none: `fill="none"`, `fill-opacity="0"` and
/// `opacity="0"` are all dropped on the way into a PDF, and the exported deck
/// is the place this matters most.
pub fn with_text(svg: &str, lines: &[Line], crop: mirzam_figure::Rect) -> String {
    let Some(close) = svg.rfind("</svg>") else {
        return svg.to_string();
    };
    let mut layer = String::new();
    for line in lines {
        let text = line.text.trim();
        // A code the fonts could not explain is worse than nothing: a reader
        // who copies it gets a replacement character where a digit was.
        if text.is_empty() || line.rect.width() <= 0.0 || text.contains('\u{fffd}') {
            continue;
        }
        layer.push_str(&format!(
            "    <text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{:.2}\" textLength=\"{:.2}\" \
             lengthAdjust=\"spacingAndGlyphs\" fill=\"#000000\" fill-opacity=\"0.004\">{}</text>\n",
            line.rect.x0 - crop.x0,
            crop.y1 - crate::pdfpage::baseline_of(&line.rect, line.size),
            line.size,
            line.rect.width(),
            escape(text),
        ));
    }
    if layer.is_empty() {
        return svg.to_string();
    }
    format!("{}{layer}{}", &svg[..close], &svg[close..])
}

/// Marks the ink in a converted figure so a dark deck can recolour it without
/// touching the picture's file.
///
/// A cut-out is the paper's black ink on a transparent ground: legible on the
/// white a printed page assumes, a dark rectangle on a dark slide. Recolouring
/// every dark shape would be wrong just as often — a fill means something, and
/// a caption that says "black means occupied" is lying the moment the node
/// turns white. So this writes `class="mz-ink"` (recoloured to the theme's
/// foreground) and `class="mz-ink-outline"` (a hairline this pass adds, on a
/// filled shape that had no stroke to recolour) on exactly the elements five
/// rules select, and leaves everything else - most of all a fill - exactly as
/// hayro drew it. `mirzam-render` turns the marked classes into `var(--mz-fg)`
/// when it inlines the picture; see W27 in `docs/workstreams.md` for what each
/// rule answers and the figure that needed it.
///
/// Scoped to what [`cull`] already judges: an element hayro draws straight
/// onto the page, one level under `<svg>`. A shape nested inside a `<g>` - a
/// clipped drawing, a placed image - is left exactly as printed rather than
/// guessed at, the same trade `cull` makes for the same reason. `with_text`'s
/// own invisible layer is `<text>...</text>`, never self-closing, so it is
/// never a candidate here.
pub fn mark_ink(svg: &str, lines: &[Line], crop: mirzam_figure::Rect) -> String {
    let Some(view) = view_box(svg) else {
        return svg.to_string();
    };
    let elements = scan(svg);
    let boxes = measure(&elements);
    let line_boxes: Vec<Rect> = lines
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .map(|l| local_box(l.rect, crop))
        .collect();
    // A page has no line taller than its display heading, so this is a loose
    // ceiling on purpose: a rotated label with no line box to fall inside
    // still has to read as a glyph by size alone.
    let glyph_ceiling = lines
        .iter()
        .map(|l| l.size)
        .fold(0.0_f64, f64::max)
        .max(6.0)
        * 1.8;
    let outline_width = common_stroke_width(&elements)
        .unwrap_or_else(|| (view.x1 - view.x0).max(view.y1 - view.y0) * 0.004);

    let mut kept = vec![true; elements.len()];
    let mut replace: Vec<Option<String>> = vec![None; elements.len()];

    for (i, element) in elements.iter().enumerate() {
        if element.depth != 1 || !element.empty {
            continue;
        }
        let Extent::Box(own) = boxes[i] else {
            continue;
        };
        let fill = fill_of(element.text);
        let stroke = stroke_of(element.text);

        // Rule 5: a rectangle the size of the picture is the page it was cut
        // from, not the drawing - drop it rather than judge it as a shape.
        if element.name != "use"
            && matches!(fill, Some(Paint::Solid(c)) if c.is_light())
            && matches!(stroke, Paint::None)
            && is_page_sized(own, &view)
        {
            kept[i] = false;
            continue;
        }

        // Rule 1: a glyph is a `<use>` onto the glyph outlines, or a fill-only
        // shape the size of a character, sitting on one of the page's own
        // lines or shaped like a letter on its own.
        let is_glyph = matches!(fill, Some(Paint::Solid(_)))
            && matches!(stroke, Paint::None)
            && (element.name == "use"
                || glyph_sized(own, glyph_ceiling)
                || in_any_line(own, &line_boxes));
        if is_glyph {
            let Some(Paint::Solid(color)) = fill else {
                unreachable!("is_glyph requires a solid fill")
            };
            // Rule 2: a word set against its own light fill - a table cell -
            // is read against that fill, not the slide, and recolouring it
            // made it vanish into the one the slide is now painted in.
            if color.is_dark() && !covered_by_light_fill(i, &elements, &boxes, own, &view) {
                replace[i] = Some(with_class(element.text, "mz-ink"));
            }
            continue;
        }

        // Rule 3: a stroke with no fill of its own is the outline of a shape,
        // and outlines are read as ink whatever their colour - achromatic ones
        // take the foreground outright, a coloured one keeps its hue and is
        // lifted so it still reads on a dark ground.
        // Rule 4: fills are left alone, except a dark achromatic one with
        // no stroke at all, which the slide would otherwise lose entirely -
        // it gets the hairline outline a stroked version of the same shape
        // already has.
        match (fill, stroke) {
            (fill, Paint::Solid(stroke_color))
                if matches!(fill, None | Some(Paint::None)) && stroke_color.is_dark() =>
            {
                replace[i] = Some(if stroke_color.is_achromatic() {
                    with_class(element.text, "mz-ink")
                } else {
                    with_chroma_class(element.text, stroke_color)
                });
            }
            (Some(Paint::Solid(fill_color)), Paint::None)
                if fill_color.is_dark() && fill_color.is_achromatic() =>
            {
                replace[i] = Some(add_outline(element.text, outline_width));
            }
            _ => {}
        }
    }

    rewrite(svg, &elements, &kept, &replace)
}

/// A line's box (page space, y up) in the crop's own local space (y down),
/// the frame every coordinate in the converted SVG is already drawn in.
fn local_box(pdf: mirzam_figure::Rect, crop: mirzam_figure::Rect) -> Rect {
    Rect {
        x0: pdf.x0 - crop.x0,
        y0: crop.y1 - pdf.y1,
        x1: pdf.x1 - crop.x0,
        y1: crop.y1 - pdf.y0,
    }
}

fn glyph_sized(own: Rect, ceiling: f64) -> bool {
    (own.x1 - own.x0).max(own.y1 - own.y0) <= ceiling
}

fn in_any_line(own: Rect, lines: &[Rect]) -> bool {
    let cx = (own.x0 + own.x1) / 2.0;
    let cy = (own.y0 + own.y1) / 2.0;
    lines
        .iter()
        .any(|l| cx >= l.x0 && cx <= l.x1 && cy >= l.y0 && cy <= l.y1)
}

/// Whether an earlier-drawn, depth-one, light fill already covers this
/// element's centre - the shape of a table cell under one of its numbers.
/// Earlier in the document is earlier on the page: hayro paints in that
/// order, so a fill a word sits on is always drawn before the word is.
///
/// The page background is excluded on purpose: it is a light fill under
/// every glyph on the page by construction, and rule 2 means a *local* fill -
/// a cell, a highlight - not "this page happens to be printed on white".
fn covered_by_light_fill(
    i: usize,
    elements: &[Element],
    boxes: &[Extent],
    own: Rect,
    view: &Rect,
) -> bool {
    let cx = (own.x0 + own.x1) / 2.0;
    let cy = (own.y0 + own.y1) / 2.0;
    elements[..i].iter().enumerate().any(|(j, e)| {
        e.depth == 1
            && matches!(fill_of(e.text), Some(Paint::Solid(c)) if c.is_light())
            && matches!(boxes[j], Extent::Box(b) if !is_page_sized(b, view)
                && cx >= b.x0 && cx <= b.x1 && cy >= b.y0 && cy <= b.y1)
    })
}

/// Whether `own` fills the picture the way a page background does: flush with
/// every edge of the `viewBox`, within a tolerance loose enough for the odd
/// half-point a PDF's own rounding leaves.
fn is_page_sized(own: Rect, view: &Rect) -> bool {
    let tol = (view.x1 - view.x0).max(view.y1 - view.y0) * 0.03 + 0.5;
    (own.x0 - view.x0).abs() <= tol
        && (own.y0 - view.y0).abs() <= tol
        && (own.x1 - view.x1).abs() <= tol
        && (own.y1 - view.y1).abs() <= tol
}

/// The `stroke-width` the drawing itself uses most, so the outline this pass
/// adds looks like one already in the figure rather than like a repair.
fn common_stroke_width(elements: &[Element]) -> Option<f64> {
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for e in elements {
        if let Some(w) = attribute(e.text, "stroke-width") {
            match seen.iter_mut().find(|(v, _)| *v == w) {
                Some((_, n)) => *n += 1,
                None => seen.push((w, 1)),
            }
        }
    }
    seen.into_iter()
        .max_by_key(|(_, n)| *n)
        .and_then(|(w, _)| w.parse().ok())
}

/// One paint attribute, read for what it means to the eye rather than what it
/// is stored as. A gradient reference or a pattern is not a colour this pass
/// can judge, so it is left alone rather than guessed at.
#[derive(Clone, Copy)]
enum Paint {
    Solid(Rgb),
    None,
}

fn fill_of(tag: &str) -> Option<Paint> {
    attribute(tag, "fill").and_then(paint_of)
}

/// A missing `stroke` is `none` by the SVG spec's own default, so absence and
/// the literal word are the same paint here.
fn stroke_of(tag: &str) -> Paint {
    match attribute(tag, "stroke") {
        None => Paint::None,
        Some(v) => paint_of(v).unwrap_or(Paint::None),
    }
}

fn paint_of(v: &str) -> Option<Paint> {
    let v = v.trim();
    if v == "none" {
        return Some(Paint::None);
    }
    Rgb::parse(v).map(Paint::Solid)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    fn parse(v: &str) -> Option<Rgb> {
        let v = v.trim();
        if let Some(hex) = v.strip_prefix('#') {
            let expand = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).ok();
            return match hex.len() {
                6 => Some(Rgb(
                    u8::from_str_radix(&hex[0..2], 16).ok()?,
                    u8::from_str_radix(&hex[2..4], 16).ok()?,
                    u8::from_str_radix(&hex[4..6], 16).ok()?,
                )),
                3 => {
                    let mut cs = hex.chars();
                    Some(Rgb(
                        expand(cs.next()?)?,
                        expand(cs.next()?)?,
                        expand(cs.next()?)?,
                    ))
                }
                _ => None,
            };
        }
        match v {
            "black" => Some(Rgb(0, 0, 0)),
            "white" => Some(Rgb(255, 255, 255)),
            _ => None,
        }
    }

    fn lightness(&self) -> f64 {
        (0.299 * self.0 as f64 + 0.587 * self.1 as f64 + 0.114 * self.2 as f64) / 255.0
    }

    fn is_dark(&self) -> bool {
        self.lightness() < 0.35
    }

    fn is_light(&self) -> bool {
        self.lightness() > 0.6
    }

    fn is_achromatic(&self) -> bool {
        let (r, g, b) = (self.0 as i32, self.1 as i32, self.2 as i32);
        r.max(g).max(b) - r.min(g).min(b) <= 12
    }

    /// Toward white by `amount`, every channel moving the same fraction of
    /// the way there. That keeps the hue - pure blue lightened this way is
    /// still recognisably blue - which is the one thing a full desaturating
    /// lightness lift would not do.
    fn lifted(self, amount: f64) -> Rgb {
        let lift = |c: u8| -> u8 {
            (c as f64 + (255.0 - c as f64) * amount)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgb(lift(self.0), lift(self.1), lift(self.2))
    }

    fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Adds (or joins) a class on a self-closing tag.
fn with_class(tag: &str, class: &str) -> String {
    match attribute(tag, "class") {
        Some(existing) if existing.split_whitespace().any(|c| c == class) => tag.to_string(),
        Some(existing) => set_attr(tag, "class", &format!("{existing} {class}")),
        None => set_attr(tag, "class", class),
    }
}

/// The hairline rule 4 adds to a fill with no stroke of its own. The colour
/// written here is only ever seen without the renderer's CSS - `mz-ink-outline`
/// is what actually paints the foreground - so it is set to the fill's own
/// colour and nothing depends on it.
fn add_outline(tag: &str, width: f64) -> String {
    let tag = set_attr(tag, "stroke", "#1a1a1a");
    let tag = set_attr(&tag, "stroke-width", &format!("{width:.2}"));
    with_class(&tag, "mz-ink-outline")
}

/// Rule 3's coloured stroke: the lifted colour rides along as a custom
/// property so the renderer's dark-mode rule can use it without knowing what
/// colour the drawing started from.
fn with_chroma_class(tag: &str, color: Rgb) -> String {
    let lifted = color.lifted(0.5).to_hex();
    let tag = match attribute(tag, "style") {
        Some(existing) => set_attr(
            tag,
            "style",
            &format!("{existing};--mz-ink-chroma:{lifted}"),
        ),
        None => set_attr(tag, "style", &format!("--mz-ink-chroma:{lifted}")),
    };
    with_class(&tag, "mz-ink-chroma")
}

/// Sets one attribute on a tag, replacing its value if it is already there.
fn set_attr(tag: &str, name: &str, value: &str) -> String {
    if let Some(existing) = attribute(tag, name) {
        tag.replacen(
            &format!("{name}=\"{existing}\""),
            &format!("{name}=\"{value}\""),
            1,
        )
    } else {
        let at = tag
            .rfind("/>")
            .or_else(|| tag.rfind('>'))
            .unwrap_or(tag.len());
        format!("{} {name}=\"{value}\"{}", &tag[..at], &tag[at..])
    }
}

/// Removes what [`cull`] and [`mark_ink`] both drop, and rewrites what
/// [`mark_ink`] replaces - a self-closing tag for another self-closing tag, so
/// nothing about nesting or closing tags is at stake.
fn rewrite(svg: &str, elements: &[Element], kept: &[bool], replace: &[Option<String>]) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut at = 0;
    for (i, element) in elements.iter().enumerate() {
        if !kept[i] {
            if element.start < at {
                continue;
            }
            out.push_str(&svg[at..trim_removed(svg, at, element.start)]);
            at = element.close_end;
            continue;
        }
        if let Some(rep) = &replace[i] {
            out.push_str(&svg[at..element.start]);
            out.push_str(rep);
            at = element.close_end;
        }
    }
    out.push_str(&svg[at..]);
    out
}

/// Where a removed element's own line starts, so the line goes with it rather
/// than leaving a blank line for every element `cull` or `mark_ink` drops.
fn trim_removed(svg: &str, at: usize, start: usize) -> usize {
    let mut from = start;
    while from > at && matches!(svg.as_bytes()[from - 1], b' ' | b'\t') {
        from -= 1;
    }
    if from > at && svg.as_bytes()[from - 1] == b'\n' {
        from -= 1;
    }
    from
}

/// The three characters that cannot stand for themselves between two tags.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
        if element.start < at {
            // Already inside something dropped.
            continue;
        }
        // The line goes with it. Three thousand dropped glyphs otherwise leave
        // three thousand blank lines, which cost more than the glyphs did.
        out.push_str(&svg[at..trim_removed(svg, at, element.start)]);
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

    /// One line, in the space the measuring pass leaves it: the page's own,
    /// with y running up from the bottom of the sheet.
    fn measured(x0: f64, baseline: f64, width: f64, size: f64, text: &str) -> Line {
        Line {
            rect: mirzam_figure::Rect::new(
                x0,
                baseline - size * 0.22,
                x0 + width,
                baseline + size * 0.78,
            ),
            size,
            text: text.to_string(),
        }
    }

    #[test]
    fn the_words_are_placed_where_the_page_had_them() {
        let crop = mirzam_figure::Rect::new(40.0, 300.0, 240.0, 380.0);
        let lines = vec![measured(60.0, 328.0, 40.0, 8.0, "rows & columns")];

        let out = with_text(r##"<svg viewBox="0 0 200 80"></svg>"##, &lines, crop);
        // Twenty points in from the crop's left edge, and fifty-two down from
        // its top: the same baseline, counted the way an SVG counts.
        assert!(out.contains(r##"x="20.00" y="52.00""##), "{out}");
        assert!(out.contains(r##"textLength="40.00""##), "{out}");
        assert!(out.contains(">rows &amp; columns</text>"), "{out}");
        assert!(out.ends_with("</svg>"), "the layer goes inside the root");
    }

    #[test]
    fn a_code_no_font_explained_is_left_out() {
        // A reader who copies a replacement character where a digit was is
        // worse off than one who copies nothing.
        let crop = mirzam_figure::Rect::new(0.0, 0.0, 100.0, 100.0);
        let lines = vec![measured(10.0, 50.0, 20.0, 8.0, "8\u{fffd}5")];
        let svg = r##"<svg viewBox="0 0 100 100"></svg>"##;
        assert_eq!(with_text(svg, &lines, crop), svg);
    }

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

    // W27: a dark deck's answer for a figure that is otherwise the paper's
    // black ink on a transparent ground. `INK_PAGE` is built to exercise the
    // rules against fixtures a real page produced: a glyph by `<use>`, a
    // glyph drawn as a plain `<path>` (the shape hayro takes when a figure
    // sets its own labels rather than the page's body text), a word over its
    // own light table cell, an unstroked dark node too big to be a letter, a
    // coloured stroke, the page's own background rectangle under a transform,
    // and an embedded raster image.
    const INK_PAGE: &str = r##"<svg viewBox="0 0 200 100" xmlns="http://www.w3.org/2000/svg">
    <path d="M0,0 L100,0 L100,50 Z" fill="#ffffff" transform="matrix(2 0 0 2 0 0)"/>
    <use xlink:href="#g0" transform="matrix(1 0 0 1 0 0)" fill="#000000"/>
    <path d="M50,10 L54,10 L54,20 Z" fill="#111111"/>
    <path d="M100,10 L140,10 L140,30 Z" fill="#ffffff"/>
    <path d="M110,15 L114,15 L114,25 Z" fill="#000000"/>
    <path d="M20,60 L60,60 L60,90 L20,90 Z" fill="#000000"/>
    <path d="M20,60 L60,90" stroke="#0000ff" fill="none" stroke-width="0.8"/>
    <image transform="matrix(20 0 0 20 140 60)" xlink:href="data:image/png;base64,AAAA" width="1" height="1"/>
    <defs id="outline-glyph">
        <path id="g0" d="M10,10 L14,10 L14,20 Z"/>
    </defs>
</svg>"##;

    /// `mark_ink` reasons in the SVG's own local space, which `INK_PAGE` is
    /// already written in; a crop at the origin the same size as the
    /// `viewBox` makes [`local_box`]'s conversion the identity on `x` and a
    /// flip on `y`, so a line box is written directly against `INK_PAGE`'s
    /// own coordinates rather than through a baseline.
    fn ink_crop() -> mirzam_figure::Rect {
        mirzam_figure::Rect::new(0.0, 0.0, 200.0, 100.0)
    }

    /// A line box, given in `INK_PAGE`'s own (SVG, y-down) coordinates: page
    /// y runs the other way, so this is `local_box`'s inverse.
    fn line_over(x0: f64, y0: f64, x1: f64, y1: f64) -> Line {
        Line {
            rect: mirzam_figure::Rect::new(x0, 100.0 - y1, x1, 100.0 - y0),
            size: y1 - y0,
            text: "w".to_string(),
        }
    }

    fn marked(lines: Vec<Line>) -> String {
        mark_ink(INK_PAGE, &lines, ink_crop())
    }

    #[test]
    fn a_glyph_by_use_is_ink() {
        let out = marked(vec![line_over(0.0, 5.0, 80.0, 15.0)]);
        assert!(
            out.contains(r##"<use xlink:href="#g0" transform="matrix(1 0 0 1 0 0)" fill="#000000" class="mz-ink"/>"##),
            "{out}"
        );
    }

    #[test]
    fn a_glyph_drawn_as_a_path_is_ink() {
        let out = marked(vec![line_over(0.0, 5.0, 80.0, 15.0)]);
        assert!(
            out.contains(r##"<path d="M50,10 L54,10 L54,20 Z" fill="#111111" class="mz-ink"/>"##),
            "{out}"
        );
    }

    #[test]
    fn a_word_on_its_own_light_fill_is_left_as_printed() {
        let out = marked(vec![line_over(100.0, 15.0, 145.0, 25.0)]);
        // The glyph over the cell keeps its own fill unmarked...
        assert!(
            out.contains(r##"<path d="M110,15 L114,15 L114,25 Z" fill="#000000"/>"##),
            "the word over its own cell is untouched: {out}"
        );
        // ...and the cell itself, being far smaller than the page, survives.
        assert!(
            out.contains(r##"<path d="M100,10 L140,10 L140,30 Z" fill="#ffffff"/>"##),
            "{out}"
        );
    }

    #[test]
    fn an_unstroked_dark_shape_too_big_for_a_letter_gets_an_outline() {
        let out = marked(vec![line_over(0.0, 5.0, 80.0, 15.0)]);
        assert!(
            out.contains(r##"stroke="#1a1a1a" stroke-width="0.80" class="mz-ink-outline""##),
            "{out}"
        );
        // Its own fill is untouched - rule 4 adds an outline, nothing more.
        assert!(
            out.contains(r##"fill="#000000" stroke="#1a1a1a""##),
            "{out}"
        );
    }

    #[test]
    fn a_coloured_stroke_is_lifted_for_dark_mode_and_keeps_its_hue() {
        let out = marked(vec![]);
        assert!(out.contains("mz-ink-chroma"), "{out}");
        assert!(out.contains("--mz-ink-chroma:#"), "{out}");
        // Pure blue lightened toward white stays blue: red and green rise
        // together, blue stays saturated at the channel ceiling.
        let hex = out
            .split("--mz-ink-chroma:")
            .nth(1)
            .and_then(|s| s.get(0..7))
            .unwrap();
        assert_eq!(&hex[5..7], "ff", "blue channel stays at its ceiling: {hex}");
        assert_ne!(&hex[1..3], "00", "red is lifted off zero: {hex}");
    }

    #[test]
    fn the_page_background_is_dropped_even_under_a_transform() {
        let out = marked(vec![]);
        assert!(
            !out.contains(r##"fill="#ffffff" transform="matrix(2 0 0 2 0 0)""##),
            "the page rectangle goes: {out}"
        );
        assert!(balanced(&out), "{out}");
    }

    #[test]
    fn an_embedded_image_is_never_touched() {
        let out = marked(vec![]);
        assert!(
            out.contains(r##"<image transform="matrix(20 0 0 20 140 60)" xlink:href="data:image/png;base64,AAAA" width="1" height="1"/>"##),
            "{out}"
        );
    }
}
