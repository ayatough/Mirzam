//! The deck's own Markdown, carried inside the deck.
//!
//! A published deck shows what the markup *does* and never what it *says*, so
//! a reader looking at a slide full of charts and arrows has no way back to
//! the eight lines that produced it. `mirzam build --embed-source` bakes the
//! document into the page: the viewer's `V` key then shows the current slide's
//! source beside the slide, and `--editor-url` turns that panel into a way out
//! — one click hands the **whole deck** to the browser editor, positioned at
//! the slide you were looking at, where it can be changed and re-rendered by
//! the same core that drew it here.
//!
//! The whole deck rather than the one slide, because a slide is not a document:
//! it has no frontmatter of its own, its citations are listed on another slide,
//! and somebody who came to fix a typo would have had to paste the result back
//! by hand. What the panel shows is still one slide — that is the question it
//! answers — and both are slices of the same text, so they cannot disagree.
//!
//! What travels is the document **as rendered**: transclusions expanded and
//! variables substituted, which is what makes it something the editor can
//! render on its own without the files it was assembled from. A deck that
//! writes `{{price}}` therefore hands over the number.
//!
//! Images do not travel. They are inlined in this page as data URIs with the
//! path they came from long gone, so a handed-over deck that referenced one
//! arrives in the editor with the reference intact and the file missing, which
//! the editor reports the way it reports any missing asset.

use crate::json;
use std::hash::Hash;

/// A deck's Markdown, indexed the way the viewer counts slides.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct DeckSource {
    /// The whole deck: frontmatter, then the body the slides were split from.
    pub doc: String,
    /// Where each *authored* slide starts in `doc`, as a byte offset. A slide
    /// runs to the next entry, or to the end of the document.
    pub starts: Vec<usize>,
    /// The authored slide each rendered section came from. A slide broken by
    /// `<!-- next -->` renders as several sections and appears here several
    /// times; an empty list means the two lists are the same.
    pub section_slides: Vec<usize>,
    /// Text files the deck read by name, under the names it read them by: the
    /// stylesheets `theme:` points at, the `bibliography:` and the `masters:`.
    /// Everything the core resolves through a file provider rather than
    /// through the asset table, which is exactly the set the editor resolves
    /// the same way.
    ///
    /// A theme is inlined in this page already, so this is a second copy of
    /// it. Reading it back out of the page instead would save those bytes and
    /// cost the thing they are being spent on: what is in the page is the
    /// stylesheet plus the scope defaults the renderer prepended, and the
    /// handover is supposed to be the file somebody wrote.
    pub files: Vec<(String, String)>,
    /// Where the browser editor lives, absolute or relative to the deck. No
    /// URL means no way out of the panel, which is the right answer for a deck
    /// built somewhere with no editor to point at.
    pub editor_url: Option<String>,
}

impl DeckSource {
    /// The payload tag, or nothing at all when there is no source to carry.
    ///
    /// Offsets are emitted in UTF-16 code units rather than bytes, because the
    /// only thing that reads them is JavaScript and that is what its strings
    /// are indexed in. A deck written in Japanese would otherwise show every
    /// slide from the wrong place.
    pub fn script(&self) -> String {
        if self.doc.is_empty() || self.starts.is_empty() {
            return String::new();
        }
        let mut fields = vec![
            format!("\"doc\":{}", json::string(&self.doc)),
            format!(
                "\"at\":[{}]",
                self.starts
                    .iter()
                    .map(|b| utf16_offset(&self.doc, *b).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            format!(
                "\"of\":[{}]",
                self.section_slides
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ];
        if let Some(url) = &self.editor_url {
            fields.push(format!("\"editor\":{}", json::string(url)));
        }
        if !self.files.is_empty() {
            fields.push(format!(
                "\"files\":{{{}}}",
                self.files
                    .iter()
                    .map(|(name, body)| format!("{}:{}", json::string(name), json::string(body)))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        format!(
            "<script type=\"application/json\" id=\"mz-source\">{{{}}}</script>\n",
            fields.join(",")
        )
    }
}

/// A byte offset into `s`, counted the way JavaScript counts one. An offset
/// past the end, or inside a character, lands on the nearest boundary below —
/// a slide always starts on one, so this only guards against a caller that
/// does not.
fn utf16_offset(s: &str, byte: usize) -> usize {
    let mut end = byte.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(doc: &str, starts: &[usize]) -> DeckSource {
        DeckSource {
            doc: doc.to_string(),
            starts: starts.to_vec(),
            section_slides: (0..starts.len()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_deck_with_no_source_carries_no_tag() {
        assert!(DeckSource::default().script().is_empty());
    }

    #[test]
    fn the_document_travels_with_every_slide_marked() {
        let out = some("# One\n\n---\n\n# Two\n", &[0, 10]).script();
        assert!(out.starts_with("<script type=\"application/json\" id=\"mz-source\">"));
        assert!(
            out.contains(r##""doc":"# One\n\n---\n\n# Two\n""##),
            "{out}"
        );
        assert!(out.contains(r#""at":[0,10]"#), "{out}");
        assert!(out.contains(r#""of":[0,1]"#), "{out}");
    }

    /// The offsets are read by JavaScript, whose strings are UTF-16.
    #[test]
    fn offsets_count_the_way_the_reader_of_them_counts() {
        let doc = "# 日本\n\n---\n\n# Two\n";
        let second = doc.find("# Two").unwrap();
        let out = some(doc, &[0, second]).script();
        let expected = doc[..second].encode_utf16().count();
        assert_ne!(expected, second, "the sample text has to be multi-byte");
        assert!(out.contains(&format!("\"at\":[0,{expected}]")), "{out}");
    }

    #[test]
    fn the_optional_fields_appear_only_when_they_are_set() {
        let bare = some("# One\n", &[0]).script();
        assert!(!bare.contains("\"files\""), "{bare}");
        assert!(!bare.contains("\"editor\""), "{bare}");

        let full = DeckSource {
            editor_url: Some("../../try/".into()),
            files: vec![("refs.bib".into(), "@book{a}".into())],
            ..some("# One\n", &[0])
        }
        .script();
        assert!(full.contains(r#""editor":"../../try/""#), "{full}");
        assert!(
            full.contains(r#""files":{"refs.bib":"@book{a}"}"#),
            "{full}"
        );
    }

    /// A deck is Markdown, and Markdown can say `</script>` in a code fence.
    #[test]
    fn a_close_tag_in_the_document_cannot_break_out_of_the_block() {
        let out = some("```html\n</script><img onerror=x>\n```\n", &[0]).script();
        assert!(!out.contains("</script><img"), "{out}");
        assert!(out.contains("\\u003c/script"), "{out}");
    }
}
