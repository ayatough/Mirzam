//! The deck's own Markdown, carried inside the deck.
//!
//! A published deck shows what the markup *does* and never what it *says*, so
//! a reader looking at a slide full of charts and arrows has no way back to
//! the eight lines that produced it. `mirzam build --embed-source` bakes those
//! lines into the page: the viewer's `V` key then shows the current slide's
//! source beside the slide, and `--editor-url` turns that panel into a way out
//! — one click hands the slide to the browser editor, where it can be changed
//! and re-rendered by the same core that drew it here.
//!
//! What travels is the Markdown as authored, not as expanded: variables are
//! still `{{like this}}` and a transcluded file is still `![[part.md]]`,
//! because the point is to show the text somebody typed. The deck's
//! frontmatter rides along for the same reason the stylesheets `theme:` names
//! do — a slide rendered without its `vars:` and its own type is not the slide
//! on screen.
//!
//! Images do not travel. They are inlined in this page as data URIs with the
//! path they came from long gone, so a handed-over slide that referenced one
//! arrives in the editor with the reference intact and the file missing, which
//! the editor reports the way it reports any missing asset.

use crate::json;
use std::hash::Hash;

/// A deck's Markdown, indexed the way the viewer counts slides.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct DeckSource {
    /// The deck's frontmatter, as written, without the `---` fences. Absent
    /// when the deck declared none.
    pub frontmatter: Option<String>,
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
    /// Each authored slide's Markdown, in order.
    pub slides: Vec<String>,
    /// The authored slide each rendered section came from. A slide broken by
    /// `<!-- next -->` renders as several sections and appears here several
    /// times; an empty list means the two lists are the same.
    pub section_slides: Vec<usize>,
    /// Where the browser editor lives, absolute or relative to the deck. No
    /// URL means no way out of the panel, which is the right answer for a deck
    /// built somewhere with no editor to point at.
    pub editor_url: Option<String>,
}

impl DeckSource {
    /// The payload tag, or nothing at all when there is no source to carry.
    pub fn script(&self) -> String {
        if self.slides.is_empty() {
            return String::new();
        }
        let mut fields = vec![
            format!(
                "\"slides\":[{}]",
                self.slides
                    .iter()
                    .map(|s| json::string(s))
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
        for (name, value) in [("fm", &self.frontmatter), ("editor", &self.editor_url)] {
            if let Some(v) = value {
                fields.push(format!("\"{name}\":{}", json::string(v)));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn some(slides: &[&str]) -> DeckSource {
        DeckSource {
            slides: slides.iter().map(|s| s.to_string()).collect(),
            section_slides: (0..slides.len()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_deck_with_no_source_carries_no_tag() {
        assert!(DeckSource::default().script().is_empty());
    }

    #[test]
    fn slides_travel_as_written() {
        let out = some(&["# One\n", "# Two\n"]).script();
        assert!(out.starts_with("<script type=\"application/json\" id=\"mz-source\">"));
        assert!(out.contains(r##""slides":["# One\n","# Two\n"]"##), "{out}");
        assert!(out.contains(r#""of":[0,1]"#), "{out}");
    }

    #[test]
    fn the_optional_fields_appear_only_when_they_are_set() {
        let bare = some(&["# One\n"]).script();
        assert!(!bare.contains("\"fm\""), "{bare}");
        assert!(!bare.contains("\"files\""), "{bare}");
        assert!(!bare.contains("\"editor\""), "{bare}");

        let full = DeckSource {
            frontmatter: Some("title: T".into()),
            editor_url: Some("../../try/".into()),
            files: vec![("refs.bib".into(), "@book{a}".into())],
            ..some(&["# One\n"])
        }
        .script();
        assert!(full.contains(r#""fm":"title: T""#), "{full}");
        assert!(full.contains(r#""editor":"../../try/""#), "{full}");
        assert!(
            full.contains(r#""files":{"refs.bib":"@book{a}"}"#),
            "{full}"
        );
    }

    /// A slide is Markdown, and Markdown can say `</script>` in a code fence.
    #[test]
    fn a_close_tag_in_a_slide_cannot_break_out_of_the_block() {
        let out = some(&["```html\n</script><img onerror=x>\n```\n"]).script();
        assert!(!out.contains("</script><img"), "{out}");
        assert!(out.contains("\\u003c/script"), "{out}");
    }
}
