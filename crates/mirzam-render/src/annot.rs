//! The `annotate` extraction pass: turns a slide's `annotate` blocks into the
//! [C2] JSON the runtime draws from.
//!
//! Like `connect`, an `annotate` block sits at the slide level and names what
//! it points at, rather than living inside the pane it decorates. The two
//! features are the same shape — an overlay resolved against the live layout —
//! and writing them the same way means one mental model, not two.
//!
//! The block is dropped with a warning, never a build failure, when its
//! `target:` or an anchored `#id` matches nothing on the slide.
//!
//! [C2]: ../../../docs/workstreams.md#c2-annotation-model

use crate::{
    anim::{after_element, selector_exists},
    inline,
};
use mirzam_annot::{AnnotDoc, Item, Kind, Place};
use std::fmt::Write as _;

/// Emits one `<script class="mz-annot">` tag per valid block, or - for a
/// block whose `target:` is a picture that is not on the slide - one hidden
/// `<aside class="mz-card">` per block, with a chip written into `body` after
/// each phrase the block anchors to. `shapes` is the slide's shape layer,
/// searched with the body to check that the targets exist. `citations` says
/// whether the deck has a bibliography, so a `source: @key` can become a
/// citation mark rather than a key nothing resolves.
pub fn extract(
    slide_index: usize,
    blocks: &[String],
    body: &mut String,
    shapes: &str,
    citations: bool,
    warnings: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    let mut cards = 0usize;
    for src in blocks {
        let doc = mirzam_annot::parse(src);
        let mut problems: Vec<String> = doc.errors.clone();
        let haystack = format!("{body}{shapes}");

        let sel = doc.target.as_deref().map(target_selector);
        if let (Some(target), Some(sel), None) = (doc.target.as_deref(), &sel, doc.picture()) {
            if !target_exists(&haystack, target, sel) {
                problems.push(format!(
                    "annotate target `{target}` matches nothing on this slide"
                ));
            }
        }
        for id in mirzam_annot::referenced_ids(&doc) {
            if !selector_exists(&haystack, &format!("#{id}")) {
                problems.push(format!(
                    "annotate anchors #{id}, but no element with that id exists"
                ));
            }
        }
        if doc.is_card() && problems.is_empty() {
            // A chip goes after the phrase, so the phrase has to be in the
            // slide's text - an id on a shape or a chart mark is somewhere a
            // chip cannot follow.
            for chip in doc.chips() {
                if let Place::Anchor(id) = &chip.place {
                    if after_element(body, id).is_none() {
                        problems.push(format!(
                            "annotate: a chip goes after #{id}, but it is not an element in \
                             the slide's text that a chip can follow"
                        ));
                    }
                }
            }
        }

        if !problems.is_empty() {
            for p in problems {
                warnings.push(format!("slide {}: {p}", slide_index + 1));
            }
            continue;
        }
        if doc.items.is_empty() {
            continue;
        }
        if doc.is_card() {
            cards += 1;
            let id = format!("mz-card-{}-{cards}", slide_index + 1);
            out.push_str(&card(&doc, &id, citations, body));
            continue;
        }
        // A block whose items are all anchored measures nothing against a box,
        // so it names no target. The overlay still needs *somewhere* to hang,
        // and the slide itself is the honest answer: an anchored mark is
        // resolved from the element it names, wherever on the slide that is.
        let sel = sel.as_deref().unwrap_or(":scope");
        out.push_str(&format!(
            "<script type=\"application/json\" class=\"mz-annot\" data-target=\"{}\">{}</script>\n",
            inline::html_escape(sel),
            mirzam_annot::to_json(&doc)
        ));
    }
    out
}

/// The group a card item belongs to: which chip shows it. Marks arrive with
/// the chip whose `step` they share, the pairing rule every `annotate` block
/// uses; a block with one chip shows that chip everything, whatever the
/// steps say, since there is nothing to tell apart. A card with no picture
/// has nothing to pair: each chip is its own group and shows its own words.
fn group_of(doc: &AnnotDoc, item: &Item) -> String {
    let mut chips = doc.chips();
    match (chips.next(), chips.next(), &item.place) {
        (Some(only), None, _) => only.step.to_string(),
        (_, _, Place::Anchor(id)) if doc.picture().is_none() => id.clone(),
        _ => item.step.to_string(),
    }
}

/// One hidden card for the block, and a chip after each phrase it anchors to.
///
/// The card is real HTML rather than JSON for the overlay to draw from: the
/// marks are percentages of the picture, so they are boxes positioned in
/// percent inside the picture's own box and need no layout to be measured -
/// which is what lets the same element print, in the appendix the export
/// gathers the cards into, with no script at all.
///
/// A block with no picture makes a card of the words and the source alone.
/// That is what a hand-written quote is to a build that cannot open the PDF
/// (the editor's preview, a machine without the paper), and it is still a
/// chip that opens on the passage and the page.
fn card(doc: &AnnotDoc, id: &str, citations: bool, body: &mut String) -> String {
    let color = |item: &Item| {
        item.color
            .as_deref()
            .map(mirzam_annot::color_css)
            .unwrap_or_else(|| "var(--mz-accent1)".to_string())
    };
    // The page a group quotes from, for the chip's label: the first `page=`
    // among its items.
    let page_of = |group: &str| {
        doc.items
            .iter()
            .find(|i| i.page.is_some() && group_of(doc, i) == group)
            .and_then(|i| i.page)
    };

    for chip in doc.chips() {
        let Place::Anchor(anchor) = &chip.place else {
            continue;
        };
        let group = group_of(doc, chip);
        let label = match (&chip.label, page_of(&group)) {
            (Some(l), _) => l.clone(),
            (None, Some(p)) => format!("p. {p}"),
            (None, None) => "¶".to_string(),
        };
        let step = if chip.step > 0 {
            format!(" data-step=\"{}\"", chip.step)
        } else {
            String::new()
        };
        let html = format!(
            "<a class=\"mz-chip\" href=\"#{id}\" data-card=\"{id}\" data-for=\"{}\" \
             data-group=\"{group}\"{step} style=\"--mz-chip:{}\">{}</a>",
            inline::html_escape(anchor),
            inline::html_escape(&color(chip)),
            inline::html_escape(&label)
        );
        // Checked before this was called, so the chip always has a place.
        if let Some(at) = after_element(body, anchor) {
            body.insert_str(at, &html);
        }
    }

    let mut marks = String::new();
    let mut quotes = String::new();
    for item in doc
        .items
        .iter()
        .filter(|i| matches!(i.place, Place::At(..)))
    {
        let Place::At(x, y) = item.place else {
            continue;
        };
        let (w, h) = item.size.unwrap_or((0.0, 0.0));
        let group = group_of(doc, item);
        let kind = match item.kind {
            Kind::Highlight => "highlight",
            Kind::Underline => "underline",
            Kind::Circle => "circle",
            _ => "rect",
        };
        let _ = write!(
            marks,
            "<i class=\"mz-card-mark mz-card-{kind}\" data-group=\"{group}\" \
             style=\"left:{:.2}%;top:{:.2}%;width:{w:.2}%;height:{h:.2}%;--mz-chip:{}\"></i>",
            x - w / 2.0,
            y - h / 2.0,
            inline::html_escape(&color(item))
        );
    }
    // The words, wherever the block keeps them: on the first mark of a
    // passage `import` cut out, or on the phrase's own mark when the author
    // wrote them there.
    for item in doc.items.iter().filter(|i| i.quote.is_some()) {
        let _ = write!(
            quotes,
            "<blockquote class=\"mz-card-quote\" data-group=\"{}\">{}</blockquote>",
            group_of(doc, item),
            inline::html_escape(item.quote.as_deref().unwrap_or_default())
        );
    }

    // Where the words come from: a citation when the deck can resolve one,
    // the key or the file's name when it cannot.
    let source = doc.source.as_deref().map(|s| match s.strip_prefix('@') {
        Some(key) if citations => format!("<!--mz-cite:{}-->", key.replace("-->", "")),
        Some(_) => inline::html_escape(s),
        None => inline::html_escape(s.rsplit('/').next().unwrap_or(s)),
    });
    let mut lines = String::new();
    let mut seen: Vec<String> = Vec::new();
    for item in &doc.items {
        let group = group_of(doc, item);
        if seen.contains(&group) {
            continue;
        }
        seen.push(group.clone());
        let line = match (page_of(&group), &source) {
            (Some(p), Some(s)) => format!("p. {p} of {s}"),
            (Some(p), None) => format!("p. {p}"),
            (None, Some(s)) => s.clone(),
            (None, None) => continue,
        };
        let _ = write!(
            lines,
            "<p class=\"mz-card-src\" data-group=\"{group}\">{line}</p>"
        );
    }
    let pic = match doc.picture() {
        Some(picture) => format!(
            "<div class=\"mz-card-pic\"><img src=\"{}\" \
             alt=\"The source, cut out at the quoted passage\">{marks}</div>",
            inline::html_escape(picture)
        ),
        None => String::new(),
    };
    format!("<aside class=\"mz-card\" id=\"{id}\" hidden>{pic}{quotes}{lines}</aside>\n")
}

/// A `target:` is either a `#id` written as such, or a bare pane name.
fn target_selector(target: &str) -> String {
    if target.starts_with('#') || target.starts_with('.') {
        return target.to_string();
    }
    format!("[data-pane=\"{}\"]", target.replace('"', ""))
}

/// Whether the target is on the slide. A pane name is checked against the
/// `data-pane` attribute the renderer writes, which `selector_exists` cannot
/// see — it only knows `#id` and `.class`, and assumes anything else is valid.
fn target_exists(haystack: &str, target: &str, sel: &str) -> bool {
    if target.starts_with('#') || target.starts_with('.') {
        return selector_exists(haystack, sel);
    }
    haystack.contains(&format!("data-pane=\"{target}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_blocks_emit_nothing() {
        let mut w = Vec::new();
        assert!(extract(0, &[], &mut "<div></div>".to_string(), "", false, &mut w).is_empty());
        assert!(w.is_empty());
    }

    #[test]
    fn pane_target_becomes_a_data_pane_selector() {
        let mut w = Vec::new();
        let blocks = vec!["target: fig\ncircle 40,30 20x20 : label=\"here\"\n".to_string()];
        let mut body = "<div class=\"pane\" data-pane=\"fig\"></div>".to_string();
        let out = extract(0, &blocks, &mut body, "", false, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(
            out.contains("data-target=\"[data-pane=&quot;fig&quot;]\""),
            "{out}"
        );
        assert!(out.contains("\"kind\":\"circle\""));
    }

    #[test]
    fn id_target_is_used_as_written() {
        let mut w = Vec::new();
        let blocks = vec!["target: #shot\nrect 10,10 20x20\n".to_string()];
        let out = extract(
            0,
            &blocks,
            &mut "<img id=\"shot\">".to_string(),
            "",
            false,
            &mut w,
        );
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("data-target=\"#shot\""));
    }

    #[test]
    fn missing_target_warns_and_drops_the_block() {
        let mut w = Vec::new();
        let blocks = vec!["target: ghost\nrect 10,10 20x20\n".to_string()];
        let out = extract(
            2,
            &blocks,
            &mut "<div></div>".to_string(),
            "",
            false,
            &mut w,
        );
        assert!(out.is_empty());
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("slide 3"));
        assert!(w[0].contains("matches nothing"));
    }

    #[test]
    fn missing_anchor_warns() {
        let mut w = Vec::new();
        let blocks = vec!["target: fig\ncircle #nope : pad=4\n".to_string()];
        let out = extract(
            0,
            &blocks,
            &mut "<div data-pane=\"fig\"></div>".to_string(),
            "",
            false,
            &mut w,
        );
        assert!(out.is_empty());
        assert!(w[0].contains("#nope"));
    }

    #[test]
    fn anchor_to_a_chart_mark_resolves() {
        let mut w = Vec::new();
        let blocks = vec!["target: chart\ncircle #rev-0-1 : pad=6\n".to_string()];
        let haystack = "<div data-pane=\"chart\"><svg><g id=\"rev-0-1\"></g></svg></div>";
        let out = extract(0, &blocks, &mut haystack.to_string(), "", false, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("\"anchor\":\"rev-0-1\""));
    }

    /// Pairing a phrase with a chart mark measures nothing against a box, so
    /// the block names no target and the overlay hangs on the slide.
    #[test]
    fn an_all_anchored_block_targets_the_slide() {
        let mut w = Vec::new();
        let blocks = vec!["highlight #t-ap : step=1\nrect #lat-0-2 : step=1 pad=8\n".to_string()];
        let haystack = "<span id=\"t-ap\">ap-ne</span><g id=\"lat-0-2\"></g>";
        let out = extract(0, &blocks, &mut haystack.to_string(), "", false, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("data-target=\":scope\""), "{out}");
        assert!(out.contains("\"kind\":\"highlight\""), "{out}");
    }

    // ---- Cards: the picture is not on the slide ----

    const CARD: &str = "target: img/p1.svg\nsource: @devi2022\n\
                        highlight #q1 : color=@accent2\n\
                        highlight 50,20 90x8 : color=@accent2 quote=\"the words\" page=4\n\
                        underline 50,30 80x8 : color=@accent2\n";

    /// A chip goes after the phrase, in the block's colour, and the card
    /// holds the picture with its marks placed in percent, the quote, and
    /// where it came from - as a citation, since this deck has a bibliography.
    #[test]
    fn a_picture_target_writes_a_chip_and_a_hidden_card() {
        let mut w = Vec::new();
        let mut body = "<p>Says <span id=\"q1\">this</span>, then more.</p>".to_string();
        let out = extract(7, &[CARD.to_string()], &mut body, "", true, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(!out.contains("mz-annot"), "a card draws no overlay: {out}");
        assert!(
            body.contains("<span id=\"q1\">this</span><a class=\"mz-chip\" href=\"#mz-card-8-1\""),
            "the chip follows the phrase: {body}"
        );
        assert!(body.contains("data-for=\"q1\""), "{body}");
        assert!(
            body.contains("style=\"--mz-chip:var(--mz-accent2)\">p. 4</a>"),
            "{body}"
        );
        assert!(
            out.starts_with("<aside class=\"mz-card\" id=\"mz-card-8-1\" hidden>"),
            "{out}"
        );
        assert!(out.contains("<img src=\"img/p1.svg\""), "{out}");
        // Centre 50,20 and size 90x8: the box starts at 5,16.
        assert!(
            out.contains(
                "<i class=\"mz-card-mark mz-card-highlight\" data-group=\"0\" \
                          style=\"left:5.00%;top:16.00%;width:90.00%;height:8.00%;"
            ),
            "{out}"
        );
        assert!(out.contains("mz-card-underline"), "{out}");
        assert!(
            out.contains(
                "<blockquote class=\"mz-card-quote\" data-group=\"0\">the words</blockquote>"
            ),
            "{out}"
        );
        assert!(
            out.contains(
                "<p class=\"mz-card-src\" data-group=\"0\">p. 4 of <!--mz-cite:devi2022--></p>"
            ),
            "{out}"
        );
    }

    /// Without a bibliography the key stays as written, and a path shows
    /// its file name.
    #[test]
    fn a_card_source_without_a_bibliography_stays_literal() {
        let mut w = Vec::new();
        let mut body = "<span id=\"q1\">x</span>".to_string();
        let out = extract(0, &[CARD.to_string()], &mut body, "", false, &mut w);
        assert!(out.contains(">p. 4 of @devi2022</p>"), "{out}");
        let mut body = "<span id=\"q1\">x</span>".to_string();
        let block = CARD.replace("source: @devi2022", "source: papers/devi.pdf");
        let out = extract(0, &[block], &mut body, "", true, &mut w);
        assert!(out.contains(">p. 4 of devi.pdf</p>"), "{out}");
    }

    /// Two chips share one picture: each shows the marks of its own step,
    /// and a chip that waits for a click says so.
    #[test]
    fn chips_are_grouped_by_step() {
        let mut w = Vec::new();
        let mut body = "<span id=\"a\">a</span> <span id=\"b\">b</span>".to_string();
        let block = "target: p.svg\nsource: @k\n\
                     highlight #a : step=1\nhighlight #b : step=2\n\
                     highlight 50,20 90x8 : step=1 quote=\"one\" page=1\n\
                     highlight 50,40 90x8 : step=2 quote=\"two\" page=2\n";
        let out = extract(0, &[block.to_string()], &mut body, "", false, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(
            body.contains("data-for=\"a\" data-group=\"1\" data-step=\"1\""),
            "{body}"
        );
        assert!(
            body.contains("data-for=\"b\" data-group=\"2\" data-step=\"2\""),
            "{body}"
        );
        assert!(
            body.contains(">p. 1</a>") && body.contains(">p. 2</a>"),
            "{body}"
        );
        assert!(
            out.contains("data-group=\"1\">one<") && out.contains("data-group=\"2\">two<"),
            "{out}"
        );
        assert!(
            out.contains("data-group=\"1\">p. 1 of") && out.contains("data-group=\"2\">p. 2 of"),
            "{out}"
        );
    }

    /// The hand-written form: no picture, the words on the phrase's mark. The
    /// chip opens on the words and the source, and two such phrases in one
    /// block each open on their own.
    #[test]
    fn a_quote_on_the_phrase_makes_a_card_of_the_words() {
        let mut w = Vec::new();
        let mut body = "<span id=\"a\">a</span> <span id=\"b\">b</span>".to_string();
        let block = "source: @devi2022\n\
                     highlight #a : quote=\"one\" page=3\n\
                     underline #b : quote=\"two\" color=@accent2\n";
        let out = extract(0, &[block.to_string()], &mut body, "", true, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(!out.contains("mz-card-pic"), "nothing to picture: {out}");
        assert!(!out.contains("mz-card-mark"), "{out}");
        assert!(
            body.contains(
                "data-for=\"a\" data-group=\"a\" style=\"--mz-chip:var(--mz-accent1)\">p. 3</a>"
            ),
            "{body}"
        );
        assert!(
            body.contains(
                "data-for=\"b\" data-group=\"b\" style=\"--mz-chip:var(--mz-accent2)\">¶</a>"
            ),
            "no page yet, so no page on the chip: {body}"
        );
        assert!(
            out.contains("<blockquote class=\"mz-card-quote\" data-group=\"a\">one</blockquote>"),
            "{out}"
        );
        assert!(out.contains("data-group=\"b\">two</blockquote>"), "{out}");
        assert!(
            out.contains(
                "<p class=\"mz-card-src\" data-group=\"a\">p. 3 of <!--mz-cite:devi2022--></p>"
            ),
            "{out}"
        );
        assert!(
            out.contains("<p class=\"mz-card-src\" data-group=\"b\"><!--mz-cite:devi2022--></p>"),
            "{out}"
        );
    }

    /// A phrase the chip cannot follow - because it is not in the slide's
    /// text - drops the block with a warning, like any missing anchor.
    #[test]
    fn a_chip_needs_a_phrase_in_the_text() {
        let mut w = Vec::new();
        let mut body = "<p>nothing</p>".to_string();
        let out = extract(
            0,
            &[CARD.to_string()],
            &mut body,
            "<g id=\"q1\"></g>",
            true,
            &mut w,
        );
        assert!(out.is_empty());
        assert!(w[0].contains("a chip goes after #q1"), "{w:?}");
        assert!(!body.contains("mz-chip"));
    }

    #[test]
    fn parse_errors_become_warnings() {
        let mut w = Vec::new();
        let blocks = vec!["target: fig\nblob 1,1\n".to_string()];
        let out = extract(
            0,
            &blocks,
            &mut "<div data-pane=\"fig\"></div>".to_string(),
            "",
            false,
            &mut w,
        );
        assert!(out.is_empty());
        assert!(w[0].contains("blob"));
    }
}
