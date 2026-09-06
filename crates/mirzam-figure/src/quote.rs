//! A quoted passage on a laid-out page: which lines it covers, where on the
//! first and last of them it starts and ends, and what to cut out around it.
//!
//! This is the geometry under `mirzam import pdf --quote` and the check that
//! verifies a `quote=`. Like the rest of the crate it knows nothing about PDF:
//! it is handed the page's lines with their boxes and asked *where are these
//! words?* — which is a string search with a few concessions to how a page
//! sets text, each of them earned by a paper that broke the naive version:
//!
//! - **A page has columns.** Lines are read column by column, top to bottom,
//!   so a passage that wraps stays in one column and never runs into the one
//!   beside it.
//! - **A word may break at a line end** (`real-` / `time`) and a quote may
//!   spell it either way. Hyphens are dropped on both sides before comparing.
//! - **Ligatures and curly quotes** are what the font drew, not what the
//!   author typed: `ﬁ` and `fi` are the same word.
//! - **A drop cap is its own line** — a `T` at three times the body size
//!   beside `HE state estimation` — and is folded into the line it opens.
//! - **The quote may start mid-line**, and two quotes may share a line. A
//!   match records how far along its first and last line it starts and ends,
//!   as a fraction of the line's characters — good to about a glyph in
//!   justified body text, since one box per line is all a [`Line`] carries.
//! - **A near miss is worth reporting.** One paper spells "sturcture"; a quote
//!   with the correct spelling finds nothing exact, and the right answer is
//!   "no, but this is close", not silence.

use crate::{Line, Rect};

/// The lines of one page, in reading order, ready to be searched.
pub struct Text {
    page: Rect,
    lines: Vec<Line>,
    /// [`normalize`] of each line, computed once.
    normed: Vec<String>,
    /// Which column each line is in: `false` left of the page's middle,
    /// `true` right of it. A passage never runs from one to the other.
    right: Vec<bool>,
}

/// One quote's place on the page: the lines it covers, and where on the first
/// and last of them it starts and ends (0 is the line's left edge, 1 its right).
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub first: usize,
    pub last: usize,
    pub start: f64,
    pub end: f64,
}

/// What a search came back with.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Exact(Span),
    /// No exact match, but a run of words this many edits away — a typo in
    /// the paper, or in the quote.
    Near {
        text: String,
        distance: usize,
    },
    Nothing,
}

/// A mark over one line of a match, as percentages of a crop: the centre of
/// the box and its size, `y` counted from the top the way a slide counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Lines smaller than this are marks, not words: a rotated watermark measured
/// at no size, a footnote rule, a superscript on its own.
const SMALLEST_TEXT: f64 = 4.0;

/// How much taller than the body a one-letter line has to be to count as a
/// drop cap rather than a stray initial.
const DROP_CAP: f64 = 1.8;

impl Text {
    /// Puts a page's lines in reading order and folds its drop caps.
    pub fn new(page: Rect, lines: &[Line]) -> Text {
        let mid = page.x0 + page.width() / 2.0;
        let mut body: Vec<Line> = lines
            .iter()
            .filter(|l| !l.text.trim().is_empty() && l.size >= SMALLEST_TEXT)
            // A line taller than it is wide is set sideways — a journal name up
            // the margin — unless it is a single letter, which is just tall.
            .filter(|l| l.rect.width() >= l.rect.height() || l.text.trim().chars().count() == 1)
            .cloned()
            .collect();
        body.sort_by(|a, b| {
            let (ca, cb) = (a.rect.x0 >= mid, b.rect.x0 >= mid);
            ca.cmp(&cb).then(
                b.rect
                    .y1
                    .partial_cmp(&a.rect.y1)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        fold_drop_caps(&mut body);
        let normed = body.iter().map(|l| normalize(&l.text)).collect();
        let right = body.iter().map(|l| l.rect.x0 >= mid).collect();
        Text {
            page,
            lines: body,
            normed,
            right,
        }
    }

    /// The lines as they will be searched: reading order, drop caps folded.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Where the quote is on the page.
    pub fn find(&self, quote: &str) -> Found {
        let wanted = normalize(quote);
        if wanted.is_empty() {
            return Found::Nothing;
        }
        let wanted_len = wanted.chars().count();

        for first in 0..self.lines.len() {
            let mut joined = String::new();
            let mut starts: Vec<usize> = Vec::new();
            for last in first..self.lines.len() {
                if last > first && self.right[last] != self.right[last - 1] {
                    break;
                }
                if last > first && !continues(&self.lines[last - 1]) {
                    joined.push(' ');
                }
                starts.push(joined.chars().count());
                joined.push_str(&self.normed[last]);
                if let Some(byte_at) = joined.find(&wanted) {
                    let at = joined[..byte_at].chars().count();
                    let end_at = at + wanted_len;
                    // The line holding the first character, and the one
                    // holding the last: the search may have started a line
                    // or two early.
                    let s = starts.iter().rposition(|&st| st <= at).unwrap_or(0);
                    let e = starts.iter().rposition(|&st| st < end_at).unwrap_or(s);
                    let len = |i: usize| self.normed[first + i].chars().count().max(1) as f64;
                    return Found::Exact(Span {
                        first: first + s,
                        last: first + e,
                        start: ((at - starts[s]) as f64 / len(s)).clamp(0.0, 1.0),
                        end: ((end_at - starts[e]) as f64 / len(e)).clamp(0.0, 1.0),
                    });
                }
                // Enough text to hold the quote and then some: a longer run
                // could only match starting from a later line.
                if joined.chars().count() > wanted_len + 200 {
                    break;
                }
            }
        }
        self.nearest(&wanted)
    }

    /// The run of words closest to `wanted`, when it is close enough to be
    /// the same sentence misspelt rather than a different sentence.
    fn nearest(&self, wanted: &str) -> Found {
        let mut page = String::new();
        for i in 0..self.lines.len() {
            if i > 0 && self.right[i] != self.right[i - 1] {
                // A column break: nothing the quote could contain, so no
                // window straddles it cheaply.
                page.push_str(" \u{0} ");
            } else if i > 0 && !continues(&self.lines[i - 1]) {
                page.push(' ');
            }
            page.push_str(&self.normed[i]);
        }
        let page: Vec<char> = page.chars().collect();
        let wanted: Vec<char> = wanted.chars().collect();
        if wanted.len() < 8 || page.len() < wanted.len() / 2 {
            return Found::Nothing;
        }
        // Candidates: wherever one of the quote's first three words appears
        // as a word, the quote would start that word's offset earlier.
        let words: Vec<(usize, Vec<char>)> = {
            let s: String = wanted.iter().collect();
            let mut out = Vec::new();
            let mut at = 0;
            for w in s.split(' ').take(3) {
                if w.chars().count() >= 3 {
                    out.push((at, w.chars().collect::<Vec<char>>()));
                }
                at += w.chars().count() + 1;
            }
            out
        };
        let mut best: Option<(usize, usize)> = None; // (distance, start)
        for (offset, word) in &words {
            let mut p = 0;
            while p + word.len() <= page.len() {
                let boundary = p == 0 || page[p - 1] == ' ';
                if boundary && page[p..p + word.len()] == word[..] && p >= *offset {
                    let start = p - offset;
                    let end = (start + wanted.len()).min(page.len());
                    let d = levenshtein(&wanted, &page[start..end]);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, start));
                    }
                }
                p += 1;
            }
        }
        let allowed = (wanted.len() / 20).max(2);
        match best {
            Some((distance, start)) if distance <= allowed => {
                let end = (start + wanted.len()).min(page.len());
                Found::Near {
                    text: page[start..end]
                        .iter()
                        .collect::<String>()
                        .trim()
                        .to_string(),
                    distance,
                }
            }
            _ => Found::Nothing,
        }
    }

    /// The rectangle to cut out: the column the passage is in, at the column's
    /// full width, from `context` lines above the first span to `context` lines
    /// below the last — never crossing into the next column.
    pub fn crop(&self, spans: &[Span], context: usize) -> Option<Rect> {
        let first = spans.iter().map(|s| s.first).min()?;
        let last = spans.iter().map(|s| s.last).max()?;
        let anchor = self.lines.get(first)?;
        let mid = self.page.x0 + self.page.width() / 2.0;
        let side = |l: &Line| l.rect.x0 >= mid;
        let same_side = |i: usize| side(&self.lines[i]) == side(anchor);

        // Context is the lines *next to* the passage: in the same column, and
        // no further away than a blank line would put them. A passage set
        // above a figure would otherwise take the figure - and everything
        // down to the next line of text - with it.
        let near = |upper: &Line, lower: &Line| upper.rect.y0 - lower.rect.y1 <= 2.0 * anchor.size;
        let mut lo = first;
        for _ in 0..context {
            if lo == 0 || !same_side(lo - 1) || !near(&self.lines[lo - 1], &self.lines[lo]) {
                break;
            }
            lo -= 1;
        }
        let mut hi = last;
        for _ in 0..context {
            if hi + 1 >= self.lines.len()
                || !same_side(hi + 1)
                || !near(&self.lines[hi], &self.lines[hi + 1])
            {
                break;
            }
            hi += 1;
        }
        let mut band = self.lines[lo].rect;
        for l in &self.lines[lo..=hi] {
            band = band.union(&l.rect);
        }

        // The column's edges, from the lines set at the passage's size on its
        // side of the page: the most common left edge, and the furthest right
        // edge among lines that start there. A title or a caption on the same
        // side would otherwise widen the cut-out to its own measure.
        let column: Vec<&Line> = self
            .lines
            .iter()
            .filter(|l| side(l) == side(anchor) && (l.size - anchor.size).abs() < 0.6)
            .collect();
        let mut edges: Vec<i64> = column.iter().map(|l| l.rect.x0.round() as i64).collect();
        edges.sort_unstable();
        let left = edges
            .iter()
            .map(|&e| (edges.iter().filter(|&&x| (x - e).abs() <= 1).count(), -e))
            .max()
            .map(|(_, e)| -e as f64)
            .unwrap_or(anchor.rect.x0);
        let right = column
            .iter()
            .filter(|l| (l.rect.x0 - left).abs() <= 1.5)
            .map(|l| l.rect.x1)
            .fold(anchor.rect.x1, f64::max);

        // Room at the sides for the column; a hair above and below, so the
        // neighbouring lines' descenders stay out of the picture.
        let crop = Rect::new(left - 6.0, band.y0 - 1.5, right + 6.0, band.y1 + 1.5);
        Some(crop.intersect(&self.page).unwrap_or(crop))
    }

    /// One mark per line of the span, as percentages of `crop`, trimmed on the
    /// first and last line to where the quote starts and ends.
    pub fn marks(&self, span: &Span, crop: &Rect) -> Vec<Mark> {
        let mut out = Vec::new();
        for i in span.first..=span.last.min(self.lines.len().saturating_sub(1)) {
            let mut r = self.lines[i].rect;
            let width = r.width();
            if i == span.last {
                r.x1 = r.x0 + width * span.end;
            }
            if i == span.first {
                r.x0 += width * span.start;
            }
            if r.x1 <= r.x0 {
                continue;
            }
            out.push(Mark {
                x: ((r.x0 + r.x1) / 2.0 - crop.x0) / crop.width() * 100.0,
                y: (crop.y1 - (r.y0 + r.y1) / 2.0) / crop.height() * 100.0,
                w: r.width() / crop.width() * 100.0,
                h: r.height() / crop.height() * 100.0,
            });
        }
        out
    }
}

/// Whether the next line continues this one's last word: it ends in a hyphen.
fn continues(line: &Line) -> bool {
    line.text.trim_end().ends_with('-')
}

/// A one-letter line far larger than the body, set beside the line it opens,
/// is folded into that line so `THE` can be searched for as a word.
fn fold_drop_caps(lines: &mut Vec<Line>) {
    let mut sizes: Vec<f64> = lines.iter().map(|l| l.size).collect();
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let Some(&median) = sizes.get(sizes.len() / 2) else {
        return;
    };
    let mut i = 0;
    while i < lines.len() {
        let cap = &lines[i];
        let is_cap = cap.text.trim().chars().count() == 1 && cap.size >= DROP_CAP * median;
        if !is_cap {
            i += 1;
            continue;
        }
        let cap_rect = cap.rect;
        let letter = cap.text.trim().to_string();
        // The topmost line that sits against the cap's right edge and
        // overlaps it vertically.
        let target = lines
            .iter()
            .enumerate()
            .filter(|(j, l)| {
                *j != i
                    && l.rect.y1 > cap_rect.y0
                    && l.rect.y0 < cap_rect.y1
                    && l.rect.x0 >= cap_rect.x0
                    && l.rect.x0 <= cap_rect.x1 + median
            })
            .max_by(|(_, a), (_, b)| {
                a.rect
                    .y1
                    .partial_cmp(&b.rect.y1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(j, _)| j);
        match target {
            Some(j) => {
                lines[j].text = format!("{letter}{}", lines[j].text);
                lines[j].rect.x0 = lines[j].rect.x0.min(cap_rect.x0);
                lines.remove(i);
            }
            None => i += 1,
        }
    }
}

/// The form two spellings of the same words are compared in: ligatures
/// expanded, typographic quotes and dashes plain, hyphens and soft hyphens
/// dropped, whitespace collapsed, case folded.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ﬁ' => out.push_str("fi"),
            'ﬂ' => out.push_str("fl"),
            'ﬀ' => out.push_str("ff"),
            'ﬃ' => out.push_str("ffi"),
            'ﬄ' => out.push_str("ffl"),
            '’' | '‘' | 'ʼ' => out.push('\''),
            '“' | '”' | '„' => out.push('"'),
            '-' | '‐' | '‑' | '–' | '—' | '−' | '\u{ad}' => {}
            c if c.is_whitespace() => out.push(' '),
            c => out.extend(c.to_lowercase()),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Edit distance between two character sequences.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line of body text: left edge, baseline-ish bottom, width, size.
    fn line(x0: f64, y0: f64, width: f64, size: f64, text: &str) -> Line {
        Line {
            rect: Rect::new(x0, y0, x0 + width, y0 + size),
            size,
            text: text.to_string(),
        }
    }

    /// A letter-sized page, two columns of 10 pt text twelve points apart.
    fn page() -> Rect {
        Rect::new(0.0, 0.0, 612.0, 792.0)
    }

    fn left(row: usize, text: &str) -> Line {
        line(49.0, 700.0 - 12.0 * row as f64, 251.0, 10.0, text)
    }

    fn right(row: usize, text: &str) -> Line {
        line(312.0, 700.0 - 12.0 * row as f64, 251.0, 10.0, text)
    }

    fn span(found: Found) -> Span {
        match found {
            Found::Exact(s) => s,
            other => panic!("expected an exact match, got {other:?}"),
        }
    }

    #[test]
    fn a_passage_across_lines_in_one_column_is_found() {
        let text = Text::new(
            page(),
            &[
                left(0, "In this paper we present an extension to"),
                left(1, "OctoMap which we call UFOMap. UFOMap uses an explicit"),
                left(2, "representation of all three states in the map."),
                right(0, "something else entirely, in the other column"),
            ],
        );
        let s = span(text.find("we present an extension to OctoMap which we call UFOMap"));
        assert_eq!((s.first, s.last), (0, 1));
        assert!(s.start > 0.3 && s.start < 0.4, "{s:?}");
        assert!(s.end > 0.4 && s.end < 0.6, "{s:?}");
    }

    #[test]
    fn columns_are_read_one_after_the_other() {
        // The right column's first line is *above* the left column's last;
        // reading order still keeps each column whole.
        let text = Text::new(
            page(),
            &[
                left(0, "end of the left column"),
                right(0, "start of the right column"),
                right(1, "which continues here"),
            ],
        );
        let s = span(text.find("start of the right column which continues here"));
        assert_eq!((s.first, s.last), (1, 2));
        assert_eq!(
            text.find("end of the left column start of the right"),
            Found::Nothing
        );
    }

    #[test]
    fn a_word_broken_at_a_line_end_matches_either_spelling() {
        let text = Text::new(
            page(),
            &[
                left(0, "This enables real-"),
                left(1, "time colored volumetric mapping."),
            ],
        );
        assert!(matches!(
            text.find("enables real-time colored"),
            Found::Exact(_)
        ));
        assert!(matches!(
            text.find("enables realtime colored"),
            Found::Exact(_)
        ));
        // Two words where the page has one is not the same text - close, but
        // not exact.
        assert!(!matches!(text.find("real time colored"), Found::Exact(_)));
    }

    #[test]
    fn ligatures_and_curly_quotes_are_the_typed_characters() {
        let text = Text::new(page(), &[left(0, "the ﬁrst “eﬃcient” method")]);
        assert!(matches!(
            text.find("the first \"efficient\" method"),
            Found::Exact(_)
        ));
    }

    #[test]
    fn a_drop_cap_is_folded_into_the_line_it_opens() {
        let cap = line(49.0, 671.0, 19.6, 29.4, "T");
        let text = Text::new(
            page(),
            &[
                cap,
                line(
                    70.0,
                    688.0,
                    230.0,
                    10.0,
                    "HE state estimation is of great concern",
                ),
                line(70.0, 676.0, 230.0, 10.0, "for nonlinear dynamic systems."),
                line(49.0, 664.0, 251.0, 10.0, "Although offline optimization"),
            ],
        );
        assert_eq!(
            text.lines().len(),
            3,
            "the cap is no longer a line of its own"
        );
        assert!(text.lines()[0].text.starts_with("THE state"));
        let s = span(text.find("The state estimation is of great concern for nonlinear"));
        assert_eq!((s.first, s.last), (0, 1));
        assert_eq!(s.start, 0.0);
    }

    #[test]
    fn two_quotes_sharing_a_line_split_it_between_them() {
        let text = Text::new(
            page(),
            &[
                left(
                    0,
                    "and unknown. This gives, surprisingly, a more memory efficient",
                ),
                left(1, "representation. Furthermore, we provide methods"),
            ],
        );
        let a = span(text.find("and unknown."));
        let b =
            span(text.find("This gives, surprisingly, a more memory efficient representation."));
        assert_eq!((a.first, a.last), (0, 0));
        assert!(a.start == 0.0 && a.end < 0.25, "{a:?}");
        assert_eq!((b.first, b.last), (0, 1));
        assert!(b.start > 0.15 && b.start < 0.25, "{b:?}");
        assert!(b.end > 0.25 && b.end < 0.4, "{b:?}");
    }

    #[test]
    fn a_misspelling_is_reported_as_near_rather_than_missing() {
        let text = Text::new(
            page(),
            &[
                left(
                    0,
                    "However, recent I-EKF framework requires a special Lie group",
                ),
                left(
                    1,
                    "sturcture which may be difficult to find. It seems empirical",
                ),
            ],
        );
        match text.find("recent I-EKF framework requires a special Lie group structure which may be difficult to find") {
            Found::Near { text, distance } => {
                assert!(text.contains("sturcture"), "{text}");
                assert!(distance <= 2, "{distance}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_sentence_that_is_not_there_is_nothing() {
        let text = Text::new(
            page(),
            &[
                left(0, "UFOMap is contributed as a C++ library that can be used"),
                left(1, "standalone but is also integrated into ROS."),
            ],
        );
        assert_eq!(
            text.find("UFOMap is ten times faster than OctoMap"),
            Found::Nothing
        );
        assert_eq!(text.find(""), Found::Nothing);
    }

    #[test]
    fn a_sideways_watermark_and_a_title_do_not_widen_the_column() {
        let mut lines = vec![
            line(
                121.8,
                716.0,
                368.0,
                16.9,
                "UFOMap: An Efficient Probabilistic 3D Mapping",
            ),
            // arXiv's stamp up the left margin: measured at no size, taller
            // than wide.
            Line {
                rect: Rect::new(16.4, 232.0, 36.4, 611.0),
                size: 0.0,
                text: "arXiv:2003.04749v1 [cs.RO] 10 Mar 2020".into(),
            },
        ];
        for (i, t) in [
            "Abstract—3D models are an essential part of many robotic",
            "applications. In applications where the environment is unknown",
            "a-priori, or where only a part of the environment is known, it",
            "is important that the 3D model can handle the unknown space",
        ]
        .iter()
        .enumerate()
        {
            lines.push(left(i, t));
        }
        let text = Text::new(page(), &lines);
        let s = span(text.find("the 3D model can handle the unknown space"));
        let crop = text.crop(std::slice::from_ref(&s), 1).unwrap();
        // The column's own edges plus six points, not the title's.
        assert!((crop.x0 - 43.0).abs() < 0.01, "{crop:?}");
        assert!((crop.x1 - 306.0).abs() < 0.01, "{crop:?}");
        // One line of context above, none below (there is none).
        assert!(crop.y1 > 686.0 && crop.y1 < 690.0, "{crop:?}");
    }

    #[test]
    fn marks_are_percentages_of_the_crop_trimmed_to_the_words() {
        let text = Text::new(
            page(),
            &[
                left(0, "aaaa bbbb cccc dddd"),
                left(1, "eeee ffff gggg hhhh"),
            ],
        );
        let s = span(text.find("cccc dddd eeee"));
        let crop = Rect::new(43.0, 686.5, 306.0, 711.5);
        let marks = text.marks(&s, &crop);
        assert_eq!(marks.len(), 2);
        // The first mark starts a little past half way along the line and
        // runs to its end; the second covers the first word only.
        assert!(
            marks[0].x > 70.0 && marks[0].w > 40.0 && marks[0].w < 50.0,
            "{:?}",
            marks[0]
        );
        assert!(marks[1].x < 20.0 && marks[1].w < 25.0, "{:?}", marks[1]);
        assert!(
            marks[0].y < marks[1].y,
            "the first line is higher, so nearer the top"
        );
    }

    #[test]
    fn normalization_is_what_the_rules_say() {
        assert_eq!(normalize("Real-\u{ad}time  “ﬁne”"), "realtime \"fine\"");
        assert_eq!(normalize("  A – B — C "), "a b c");
    }
}
