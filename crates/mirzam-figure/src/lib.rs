//! Captioned figures on a laid-out page: where the caption is, and which ink
//! belongs to it.
//!
//! This crate knows nothing about PDF. It is given a page that has already been
//! laid out — text lines with their boxes and sizes, and the boxes of every
//! painted thing — and answers one question: *if a line says `Figure 3`, what
//! rectangle is Figure 3?* Reading the file is `mirzam-cli`'s job, because that
//! is where a process may open one; the geometry is here, where it can be
//! tested against a page written by hand.
//!
//! # How a figure is found
//!
//! A paper does not mark its floats. What it does instead is typographic, and
//! consistent enough across publishers to lean on:
//!
//! 1. **A caption starts with its own name.** `Figure 3:`, `Fig. 3`, `TABLE I`.
//!    That is the only anchor in the page, and everything else is measured
//!    from it.
//! 2. **A caption ends where its paragraph does.** Its last line stops short of
//!    the column, so a following full line is the next paragraph, not more
//!    caption. This is what keeps the body text out of a caption.
//! 3. **The picture is on the caption's side of the caption**, and reaches as
//!    far as the nearest line of body text. A figure's caption sits under the
//!    picture; a table's sits over the table. So the search runs up from a
//!    `Figure` and down from a `Table`, and the other way if that side is
//!    empty — plenty of journals do the opposite.
//! 4. **Body text stops the search.** A line as wide as its column, set at the
//!    page's dominant size, is prose. Everything between it and the caption —
//!    rules, curves, images, axis labels — is the float.
//! 5. **A float that reaches into the next column spans them.** The figure
//!    across the top of a two-column paper is found by its ink covering both,
//!    not by its caption looking wide: such a caption is often one short line.
//! 6. **A rectangle the size of the page is the page.** A background wash or a
//!    watermark is the largest thing near every caption, and would make every
//!    figure the whole sheet.
//!
//! Nothing here is certain, which is why [`Figure`] carries the box it decided
//! on rather than a picture: a caller can print it, widen it, or let the author
//! override it.

/// A rectangle in PDF user space, where **y grows upward** and the unit is a
/// point.
///
/// Kept as two corners rather than an origin and a size because every operation
/// below is a union or an overlap, and those read better on edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Rect {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }

    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    /// The smallest rectangle holding both.
    pub fn union(&self, other: &Rect) -> Rect {
        Rect {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    /// The overlap, or `None` when they miss each other.
    ///
    /// A rectangle with no thickness still overlaps: the rule under a table
    /// row and the underline of a heading are rectangles a fraction of a point
    /// tall, and demanding area here would drop every one of them — which is
    /// most of what a table is made of.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let r = Rect {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        };
        (r.x1 >= r.x0 && r.y1 >= r.y0).then_some(r)
    }

    /// How much of `self` lies inside `other`, from 0 to 1. A degenerate
    /// rectangle — a rule with no thickness — is measured on its longer side
    /// instead, since its area is zero and every share of it would be too.
    pub fn share_inside(&self, other: &Rect) -> f64 {
        let Some(hit) = self.intersect(other) else {
            return 0.0;
        };
        if self.area() > 0.0 {
            return hit.area() / self.area();
        }
        let span = self.width().max(self.height());
        if span <= 0.0 {
            return 1.0;
        }
        hit.width().max(hit.height()) / span
    }

    /// The rectangle grown by `pad` on every side.
    pub fn grow(&self, pad: f64) -> Rect {
        Rect {
            x0: self.x0 - pad,
            y0: self.y0 - pad,
            x1: self.x1 + pad,
            y1: self.y1 + pad,
        }
    }
}

/// One line of text as the page sets it.
///
/// `size` is the font size in points. It is what separates a caption from the
/// prose around it in most papers, and what makes a page's dominant size — the
/// body size — computable.
#[derive(Debug, Clone)]
pub struct Line {
    pub rect: Rect,
    pub size: f64,
    pub text: String,
}

/// What a float calls itself. The word matters because it says which side of
/// the caption to look at first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `Figure`, `Fig.`, `Chart`, `Scheme`, `図`.
    Figure,
    /// `Table`, `Tab.`, `表`.
    Table,
}

impl Kind {
    /// The word to use when naming a file or writing the alt text, in the
    /// deck's language rather than the paper's.
    pub fn word(&self) -> &'static str {
        match self {
            Kind::Figure => "fig",
            Kind::Table => "table",
        }
    }
}

/// A captioned float: the caption as written, the sentence under it, and the
/// rectangle the picture occupies.
#[derive(Debug, Clone)]
pub struct Figure {
    pub kind: Kind,
    /// `3`, `1a`, `I` — as the paper writes it, not renumbered.
    pub number: String,
    /// `Figure 3` — the label as the paper writes it, for the alt text and the
    /// credit line.
    pub label: String,
    /// The caption with its label and the separator after it removed, so it can
    /// go straight into `caption=`.
    pub caption: String,
    /// What to cut out of the page.
    pub art: Rect,
    /// The caption's own lines, which are *not* part of `art`: a slide sets the
    /// caption in the deck's font.
    pub caption_box: Rect,
}

/// Every captioned float on one page, in reading order.
///
/// `ink` is the box of everything painted — filled and stroked paths, placed
/// images — in the same space as the lines. A caption with no ink and no small
/// text on either side of it is not reported: it is a cross-reference in the
/// prose (`as Figure 3 shows`) that happens to start a line.
pub fn find(page: Rect, lines: &[Line], ink: &[Rect]) -> Vec<Figure> {
    // A rectangle the size of the page is the page: a background wash, a
    // watermark, the white the printer starts from. Left in, it is the largest
    // thing near every caption and every figure becomes the whole slide.
    let ink: Vec<Rect> = ink
        .iter()
        .copied()
        .filter(|r| r.width() < page.width() * 0.95 || r.height() < page.height() * 0.95)
        .collect();
    let ink = ink.as_slice();

    let mut ordered: Vec<usize> = (0..lines.len()).collect();
    // Reading order: down the page, then across. Sorting by the top edge is
    // enough for one column and near enough for two, since a caption is only
    // ever compared with the lines it shares a column with.
    ordered.sort_by(|&a, &b| {
        let (a, b) = (&lines[a].rect, &lines[b].rect);
        b.y1.partial_cmp(&a.y1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let body = body_size(lines);
    let sheet = Sheet {
        lines,
        ink,
        columns: columns(lines, body, page),
        body,
        rect: page,
    };
    let mut found = Vec::new();
    for (at, &i) in ordered.iter().enumerate() {
        let Some(label) = Label::parse(&lines[i].text) else {
            continue;
        };
        if !label.punctuated && is_prose(&lines[i], &sheet) {
            // `Figure 3 shows the drop in error` at the head of a paragraph is
            // a sentence about a figure. Only a line that is not prose — set
            // smaller, or stopping short of its column — may open a caption
            // without punctuation to prove it.
            continue;
        }
        let column = column_for(&lines[i].rect, &sheet.columns, page);
        let caption = caption_block(lines, &ordered, at, &column);
        let Some(art) = art_box(&label.kind, &caption.rect, &column, &sheet) else {
            continue;
        };
        found.push(Figure {
            kind: label.kind,
            number: label.number,
            label: label.label,
            caption: caption.text[caption.consumed.min(caption.text.len())..]
                .trim()
                .to_string(),
            art,
            caption_box: caption.rect,
        });
    }
    found
}

/// Whether a line reads as prose: the page's dominant size, running the width
/// of its column.
fn is_prose(line: &Line, sheet: &Sheet) -> bool {
    let column = column_for(&line.rect, &sheet.columns, sheet.rect);
    (line.size - sheet.body).abs() < 0.35 && line.rect.width() > (column.1 - column.0) * 0.9
}

/// The page's dominant text size, weighted by how much text is set in it.
///
/// Weighting by characters rather than by lines is what stops a page of section
/// headings and captions from outvoting the prose they interrupt.
fn body_size(lines: &[Line]) -> f64 {
    let mut buckets: Vec<(f64, usize)> = Vec::new();
    for line in lines {
        let key = (line.size * 10.0).round() / 10.0;
        let weight = line.text.chars().count();
        match buckets.iter_mut().find(|(s, _)| (*s - key).abs() < 0.05) {
            Some((_, w)) => *w += weight,
            None => buckets.push((key, weight)),
        }
    }
    buckets
        .into_iter()
        .max_by_key(|&(_, w)| w)
        .map(|(s, _)| s)
        .unwrap_or(10.0)
}

/// The page's columns, as left/right edges.
///
/// Found by grouping body lines that start at the same place: a two-column
/// paper has two such places, a thesis has one. Anything narrower than a fifth
/// of the page is a list indent or a marginal note, not a column.
fn columns(lines: &[Line], body: f64, page: Rect) -> Vec<(f64, f64)> {
    let mut starts: Vec<&Line> = lines
        .iter()
        .filter(|l| (l.size - body).abs() < 0.35 && l.rect.width() > body * 4.0)
        .collect();
    starts.sort_by(|a, b| {
        a.rect
            .x0
            .partial_cmp(&b.rect.x0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut groups: Vec<(f64, f64, usize)> = Vec::new();
    for line in starts {
        // A paragraph's first line is indented, so a column's left edge is the
        // smallest start in its group, not the most common one.
        match groups
            .last_mut()
            .filter(|(x0, _, _)| line.rect.x0 - *x0 < body * 2.0)
        {
            Some((_, x1, n)) => {
                *x1 = x1.max(line.rect.x1);
                *n += 1;
            }
            None => groups.push((line.rect.x0, line.rect.x1, 1)),
        }
    }

    let min_width = page.width() * 0.2;
    let cols: Vec<(f64, f64)> = groups
        .into_iter()
        .filter(|&(x0, x1, n)| n > 1 && x1 - x0 >= min_width)
        .map(|(x0, x1, _)| (x0, x1))
        .collect();
    if cols.is_empty() {
        vec![(page.x0, page.x1)]
    } else {
        cols
    }
}

/// The column a caption belongs to.
///
/// A caption that runs *past* the column it starts in belongs to a float that
/// spans them — the figure across the top of a two-column paper — so it gets
/// the whole text width instead. Past, not merely wide: such a caption is
/// often one short line, and would never look wide enough on its own.
fn column_for(rect: &Rect, columns: &[(f64, f64)], page: Rect) -> (f64, f64) {
    let own = columns
        .iter()
        .max_by(|a, b| {
            let overlap = |c: &(f64, f64)| (rect.x1.min(c.1) - rect.x0.max(c.0)).max(0.0);
            overlap(a)
                .partial_cmp(&overlap(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or((page.x0, page.x1));
    if rect.x1 > own.1 + rect.height() {
        let x0 = columns.iter().map(|c| c.0).fold(page.x1, f64::min);
        let x1 = columns.iter().map(|c| c.1).fold(page.x0, f64::max);
        return (x0.min(rect.x0), x1.max(rect.x1));
    }
    own
}

/// The page as the search reads it: its text, its ink, and what those say
/// about how it is set.
struct Sheet<'a> {
    lines: &'a [Line],
    ink: &'a [Rect],
    columns: Vec<(f64, f64)>,
    /// The dominant text size, which is what makes a line prose.
    body: f64,
    rect: Rect,
}

/// A caption, gathered from the line that names it.
struct Caption {
    text: String,
    rect: Rect,
    /// Bytes at the front of `text` that are the label and its separator.
    consumed: usize,
}

/// Reads a caption forward from its first line.
///
/// It stops on the first of: a line set at another size, a line too far below
/// to be the next line of the same paragraph, a line that starts a caption of
/// its own, or a line following one that stopped short of the column — the last
/// being the one that does the real work, since a caption's final line is
/// short and the prose under it is not.
fn caption_block(lines: &[Line], ordered: &[usize], at: usize, column: &(f64, f64)) -> Caption {
    let first = &lines[ordered[at]];
    let mut text = first.text.trim().to_string();
    let mut rect = first.rect;
    let mut prev = first;
    // What a line has to reach not to be the caption's last.
    //
    // The first line is measured against the column: a paragraph that wraps
    // fills its first line, so a short one is a caption of one line and the
    // text under it is something else. Every line after that is measured
    // against the caption's own widest, because a caption need not fill the
    // column it sits in — the one under a figure spanning a two-column page
    // does not — and comparing those with the column would cut them off.
    let mut widest = first.rect.x1;
    for (joined, &j) in ordered[at + 1..].iter().take(12).enumerate() {
        let line = &lines[j];
        let against = if joined == 0 { column.1 } else { widest };
        let short = prev.rect.x1 < against - prev.size * 2.0;
        let gap = prev.rect.y0 - line.rect.y1;
        let same_column =
            line.rect.x0 >= column.0 - prev.size && line.rect.x1 <= column.1 + prev.size;
        if short
            || gap > prev.size * 0.9
            || gap < -prev.size
            || (line.size - first.size).abs() > 0.3
            || !same_column
            || Label::parse(&line.text).is_some()
        {
            break;
        }
        text.push(' ');
        text.push_str(line.text.trim());
        rect = rect.union(&line.rect);
        widest = widest.max(line.rect.x1);
        prev = line;
    }
    let consumed = Label::parse(&text).map(|l| l.consumed).unwrap_or(0);
    Caption {
        text,
        rect,
        consumed,
    }
}

/// The picture belonging to a caption, or `None` when neither side of it holds
/// anything but prose.
fn art_box(kind: &Kind, caption: &Rect, column: &(f64, f64), sheet: &Sheet) -> Option<Rect> {
    // A figure is above its caption and a table below it, until a page says
    // otherwise — so the other side is tried when the first comes back empty.
    let first = matches!(kind, Kind::Figure);
    band(caption, column, sheet, first).or_else(|| band(caption, column, sheet, !first))
}

/// One side of a caption, bounded by the nearest line of prose.
fn band(caption: &Rect, column: &(f64, f64), sheet: &Sheet, above: bool) -> Option<Rect> {
    let (columns, ink, page) = (&sheet.columns, sheet.ink, sheet.rect);
    // Measured in the caption's own column first. A picture that turns out to
    // cross the column runs the second pass, over the whole text width.
    let full = (
        columns
            .iter()
            .map(|c| c.0)
            .fold(page.x1, f64::min)
            .min(column.0),
        columns
            .iter()
            .map(|c| c.1)
            .fold(page.x0, f64::max)
            .max(column.1),
    );
    let narrow = within(caption, column, sheet, above)?;
    // Spanning means reaching into *another column*, not merely past this
    // one's edge: a table is often a little wider than the prose beside it,
    // and the column's edges are measured from that prose.
    let reaches = |r: &Rect, c: &(f64, f64)| (r.x1.min(c.1) - r.x0.max(c.0)).max(0.0);
    let crosses = ink.iter().any(|r| {
        r.share_inside(&narrow.0) > 0.3
            && columns
                .iter()
                .any(|c| c != column && reaches(r, c) > (c.1 - c.0) * 0.3)
    });
    if !crosses || full == *column {
        return narrow.1;
    }
    within(caption, &full, sheet, above)?.1
}

/// The band on one side of a caption inside one span of the page, and what is
/// in it.
fn within(
    caption: &Rect,
    column: &(f64, f64),
    sheet: &Sheet,
    above: bool,
) -> Option<(Rect, Option<Rect>)> {
    let (lines, ink, body, page) = (sheet.lines, sheet.ink, sheet.body, sheet.rect);
    let in_column = |r: &Rect| r.x1 > column.0 + 1.0 && r.x0 < column.1 - 1.0;
    // Prose: a line at the body size that runs the width of its column. A
    // table's cells are the same size and stop well short of it, which is what
    // lets a band swallow a table but not the paragraph under it.
    let stops = |l: &Line| {
        in_column(&l.rect)
            && ((l.size - body).abs() < 0.35 && l.rect.width() > (column.1 - column.0) * 0.6
                || Label::parse(&l.text).is_some())
    };

    let mut edge = if above { page.y1 } else { page.y0 };
    for line in lines.iter().filter(|l| stops(l)) {
        if above && line.rect.y0 >= caption.y1 - 1.0 {
            edge = edge.min(line.rect.y0);
        } else if !above && line.rect.y1 <= caption.y0 + 1.0 {
            edge = edge.max(line.rect.y1);
        }
    }
    let band = if above {
        Rect::new(column.0, caption.y1, column.1, edge)
    } else {
        Rect::new(column.0, edge, column.1, caption.y0)
    };
    if band.height() < 4.0 {
        return None;
    }

    let mut art: Option<Rect> = None;
    let mut take = |r: &Rect| {
        if r.share_inside(&band) > 0.6 {
            art = Some(match art {
                Some(a) => a.union(r),
                None => *r,
            });
        }
    };
    for r in ink {
        take(r);
    }
    for line in lines {
        if line.rect != *caption {
            take(&line.rect);
        }
    }

    // Trimmed to the band's *height* but not to its width: a table a little
    // wider than the prose beside it is still that table, and cutting its
    // rules off at the column edge would be a worse crop than the screenshot
    // this replaces. What may not grow is the reach up or down, which is what
    // keeps the prose out.
    let keep = Rect::new(page.x0, band.y0, page.x1, band.y1);
    let found = art
        .and_then(|a| a.intersect(&keep))
        // A rule left over from a page border, or one stray label, is not a
        // picture. Six points is the smallest thing worth cutting out.
        .filter(|a| a.height() >= 6.0 && a.width() >= 6.0);
    Some((band, found))
}

/// What a caption calls itself, parsed off the front of its first line.
struct Label {
    kind: Kind,
    number: String,
    label: String,
    consumed: usize,
    /// Whether punctuation separated the label from the sentence. `Figure 3:`
    /// is unmistakably a caption; `Figure 3 shows` is a sentence, and only the
    /// company it keeps says which one `Figure 3 The transformer` is.
    punctuated: bool,
}

impl Label {
    /// `Figure 3: The transformer` → `Figure`, `3`, 8 bytes consumed.
    ///
    /// Hand-rolled rather than a regular expression, because the crate has no
    /// dependencies and the grammar is three tokens long: a word, a number,
    /// and the punctuation between the number and the sentence.
    fn parse(text: &str) -> Option<Label> {
        let src = text.trim_start();
        let lead = text.len() - src.len();
        let (kind, word_len) = Self::word(src)?;

        let after_word = &src[word_len..];
        let dot = usize::from(after_word.starts_with('.'));
        let rest = after_word[dot..].trim_start();
        let spaced = after_word.len() - dot - rest.len();
        let number: String = Self::number(rest)?;

        let tail = &rest[number.len()..];
        // `Figure 3.` and `Figure 3:` both end the label; `Figure 3 shows`
        // does not, and is a sentence about a figure rather than a caption.
        let sep = tail
            .find(|c: char| !matches!(c, ':' | '.' | '·' | '-' | '–' | '—' | ' ' | '\u{3000}'))
            .unwrap_or(tail.len());
        if sep == 0 && !tail.is_empty() {
            return None;
        }
        let consumed = lead + word_len + dot + spaced + number.len() + sep;
        Some(Label {
            kind,
            label: format!("{} {}", &src[..word_len], number),
            number,
            consumed,
            punctuated: tail[..sep].contains(|c: char| c != ' ' && c != '\u{3000}'),
        })
    }

    /// The leading word, if it is one a float uses. Case is ignored on the
    /// Latin words so `TABLE I` reads the same as `Table I`.
    fn word(src: &str) -> Option<(Kind, usize)> {
        const WORDS: [(&str, Kind); 8] = [
            ("figure", Kind::Figure),
            ("fig", Kind::Figure),
            ("chart", Kind::Figure),
            ("scheme", Kind::Figure),
            ("table", Kind::Table),
            ("tab", Kind::Table),
            ("図", Kind::Figure),
            ("表", Kind::Table),
        ];
        // `get` rather than a slice: a word is three bytes wide in Japanese,
        // and asking for the first three bytes of a Latin line would cut a
        // character in half.
        WORDS
            .iter()
            .filter(|(w, _)| {
                src.get(..w.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(w))
            })
            .max_by_key(|(w, _)| w.len())
            .map(|&(w, kind)| (kind, w.len()))
    }

    /// `3`, `12`, `1a`, `I`, `IV`. A roman numeral only counts when the whole
    /// token is one, so `Table Insets` is not table `I`.
    fn number(src: &str) -> Option<String> {
        let token: String = src
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if token.is_empty() {
            return None;
        }
        let digits = token.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 {
            // `3a` is a subfigure; `3rd` is prose that began with a number.
            let suffix = &token[digits..];
            return (suffix.len() <= 1).then(|| token.clone());
        }
        let roman = token.chars().all(|c| "IVXLC".contains(c));
        (roman && !token.is_empty()).then_some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 595.0,
        y1: 842.0,
    };
    const LEFT: f64 = 50.0;
    const RIGHT: f64 = 310.0;
    const COLUMN: f64 = 235.0;

    /// One line, given its left edge, its top edge and how wide it runs.
    fn line(x0: f64, top: f64, width: f64, size: f64, text: &str) -> Line {
        Line {
            rect: Rect::new(x0, top - size, x0 + width, top),
            size,
            text: text.to_string(),
        }
    }

    /// A paragraph of prose: `n` full-width lines at the body size.
    fn prose(x0: f64, top: f64, n: usize) -> Vec<Line> {
        (0..n)
            .map(|i| {
                line(
                    x0,
                    top - i as f64 * 12.0,
                    COLUMN,
                    10.0,
                    "the quick brown fox jumps over the lazy dog again",
                )
            })
            .collect()
    }

    #[test]
    fn a_figure_is_the_ink_above_its_caption() {
        let mut lines = prose(LEFT, 800.0, 3);
        lines.push(line(LEFT, 590.0, COLUMN, 8.0, "Figure 1: The transformer"));
        lines.push(line(
            LEFT,
            580.0,
            100.0,
            8.0,
            "architecture, in two stacks.",
        ));
        lines.extend(prose(LEFT, 560.0, 4));
        let ink = vec![Rect::new(60.0, 600.0, 250.0, 700.0)];

        let found = find(PAGE, &lines, &ink);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, Kind::Figure);
        assert_eq!(found[0].number, "1");
        assert_eq!(found[0].label, "Figure 1");
        assert_eq!(
            found[0].caption,
            "The transformer architecture, in two stacks."
        );
        assert_eq!(found[0].art, Rect::new(60.0, 600.0, 250.0, 700.0));
    }

    /// The caption stops at its own last line. Letting it run on is the failure
    /// this rule exists for: the deck would carry a paragraph of the paper
    /// under the picture.
    #[test]
    fn a_caption_stops_where_its_paragraph_does() {
        let mut lines = vec![line(LEFT, 590.0, COLUMN, 8.0, "Figure 1: A short one.")];
        lines.extend(prose(LEFT, 578.0, 3));
        let ink = vec![Rect::new(60.0, 600.0, 250.0, 700.0)];

        let found = find(PAGE, &lines, &ink);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].caption, "A short one.");
    }

    /// A table's caption sits over the table, so the search runs the other way.
    #[test]
    fn a_table_is_the_ink_below_its_caption() {
        let mut lines = prose(LEFT, 800.0, 3);
        lines.push(line(LEFT, 700.0, 120.0, 8.0, "Table 1: BLEU on WMT 2014."));
        for i in 0..3 {
            lines.push(line(LEFT, 690.0 - i as f64 * 16.0, 80.0, 8.0, "model 27.3"));
        }
        lines.extend(prose(LEFT, 620.0, 3));
        let ink: Vec<Rect> = (0..3)
            .map(|i| {
                Rect::new(
                    LEFT,
                    692.0 - i as f64 * 16.0,
                    LEFT + COLUMN,
                    692.5 - i as f64 * 16.0,
                )
            })
            .collect();

        let found = find(PAGE, &lines, &ink);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, Kind::Table);
        assert_eq!(found[0].caption, "BLEU on WMT 2014.");
        let art = found[0].art;
        assert!(art.y1 <= 692.5 && art.y0 >= 650.0, "{art:?}");
        assert!(
            art.y0 > 620.0,
            "the paragraph under the table is not in it: {art:?}"
        );
    }

    /// `Figure 3 shows the drop in error` opens a paragraph in half the papers
    /// ever written. Cutting the picture above it out and captioning it with a
    /// sentence from the body would be worse than finding nothing.
    #[test]
    fn a_cross_reference_in_prose_is_not_a_caption() {
        let mut lines = prose(LEFT, 800.0, 3);
        lines.push(line(
            LEFT,
            590.0,
            COLUMN,
            10.0,
            "Figure 3 shows the drop in error over the run",
        ));
        lines.extend(prose(LEFT, 578.0, 2));
        let ink = vec![Rect::new(60.0, 600.0, 250.0, 700.0)];

        assert!(find(PAGE, &lines, &ink).is_empty());
    }

    /// A caption with nothing but prose on either side of it is a mention, not
    /// a float.
    #[test]
    fn a_caption_with_no_picture_is_not_reported() {
        let mut lines = prose(LEFT, 800.0, 3);
        lines.push(line(
            LEFT,
            700.0,
            120.0,
            8.0,
            "Figure 9: mentioned in passing.",
        ));
        lines.extend(prose(LEFT, 680.0, 3));

        assert!(find(PAGE, &lines, &[]).is_empty());
    }

    /// A page painted edge to edge — a coloured background, a watermark — is
    /// the page, not the picture above the caption.
    /// A rule is a rectangle with no height. It is still inside the band it
    /// is drawn in, and a table that loses its rules loses its shape.
    #[test]
    fn a_rule_with_no_thickness_still_counts() {
        let rule = Rect::new(50.0, 600.0, 250.0, 600.0);
        let band = Rect::new(40.0, 560.0, 280.0, 640.0);
        assert_eq!(rule.share_inside(&band), 1.0);
        assert_eq!(
            rule.share_inside(&Rect::new(40.0, 100.0, 280.0, 200.0)),
            0.0
        );
        // Half of it, when half of it is outside.
        assert_eq!(
            rule.share_inside(&Rect::new(150.0, 560.0, 280.0, 640.0)),
            0.5
        );
    }

    #[test]
    fn a_page_sized_wash_is_not_a_figure() {
        let mut lines = prose(LEFT, 800.0, 3);
        lines.push(line(
            LEFT,
            590.0,
            120.0,
            8.0,
            "Figure 1: over a washed page.",
        ));
        lines.extend(prose(LEFT, 560.0, 3));
        let ink = vec![PAGE, Rect::new(60.0, 600.0, 250.0, 700.0)];

        let found = find(PAGE, &lines, &ink);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].art, Rect::new(60.0, 600.0, 250.0, 700.0));
    }

    #[test]
    fn a_column_keeps_to_itself() {
        let mut lines = prose(LEFT, 800.0, 6);
        lines.extend(prose(RIGHT, 800.0, 3));
        lines.push(line(RIGHT, 590.0, 120.0, 8.0, "Fig. 2: on the right."));
        lines.extend(prose(RIGHT, 570.0, 3));
        let ink = vec![
            Rect::new(RIGHT + 10.0, 600.0, RIGHT + 200.0, 700.0),
            Rect::new(LEFT, 600.0, LEFT + 200.0, 700.0),
        ];

        let found = find(PAGE, &lines, &ink);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].art,
            Rect::new(RIGHT + 10.0, 600.0, RIGHT + 200.0, 700.0)
        );
    }

    /// A figure across the top of a two-column paper has a caption wider than
    /// either column, and a picture to match.
    #[test]
    fn a_spanning_figure_takes_the_whole_text_width() {
        let mut lines = prose(LEFT, 600.0, 4);
        lines.extend(prose(RIGHT, 600.0, 4));
        lines.push(line(
            LEFT,
            700.0,
            495.0,
            8.0,
            "Figure 1: across both columns.",
        ));
        let ink = vec![Rect::new(LEFT, 720.0, 545.0, 800.0)];

        let found = find(PAGE, &lines, &ink);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].art, Rect::new(LEFT, 720.0, 545.0, 800.0));
    }

    #[test]
    fn the_forms_a_caption_announces_itself_in() {
        let cases = [
            ("Figure 3: A thing", Some((Kind::Figure, "3"))),
            ("Fig. 12 A thing", Some((Kind::Figure, "12"))),
            ("Fig.4—A thing", Some((Kind::Figure, "4"))),
            ("TABLE I", Some((Kind::Table, "I"))),
            ("Table 2. Results", Some((Kind::Table, "2"))),
            ("Figure 3a: A subfigure", Some((Kind::Figure, "3a"))),
            ("図 2: 日本語の図", Some((Kind::Figure, "2"))),
            ("表1: 日本語の表", Some((Kind::Table, "1"))),
            ("Figures 3 and 4 differ", None),
            ("Figure. A thing", None),
            ("In the 3rd run", None),
            ("Tabular results follow", None),
        ];
        for (src, want) in cases {
            let got = Label::parse(src).map(|l| (l.kind, l.number));
            match want {
                Some((kind, number)) => {
                    let got = got.unwrap_or_else(|| panic!("{src:?} was not read as a caption"));
                    assert_eq!((got.0, got.1.as_str()), (kind, number), "{src:?}");
                }
                None => assert!(got.is_none(), "{src:?} was read as {got:?}"),
            }
        }
    }

    #[test]
    fn the_label_is_cut_off_the_caption() {
        let cases = [
            ("Figure 3: A thing", "A thing"),
            ("Fig. 12 A thing", "A thing"),
            ("TABLE I", ""),
            ("表1: 日本語の表", "日本語の表"),
        ];
        for (src, want) in cases {
            let label = Label::parse(src).unwrap();
            assert_eq!(&src[label.consumed..], want, "{src:?}");
        }
    }
}
