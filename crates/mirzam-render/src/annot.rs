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

use crate::{anim::selector_exists, inline};

/// Emits one `<script class="mz-annot">` tag per valid block. `haystack` is
/// the slide's rendered HTML (body plus shape layer), searched to check that
/// the targets exist.
pub fn extract(
    slide_index: usize,
    blocks: &[String],
    haystack: &str,
    warnings: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    for src in blocks {
        let doc = mirzam_annot::parse(src);
        let mut problems: Vec<String> = doc.errors.clone();

        let sel = doc.target.as_deref().map(target_selector);
        if let (Some(target), Some(sel)) = (doc.target.as_deref(), &sel) {
            if !target_exists(haystack, target, sel) {
                problems.push(format!(
                    "annotate target `{target}` matches nothing on this slide"
                ));
            }
        }
        for id in mirzam_annot::referenced_ids(&doc) {
            if !selector_exists(haystack, &format!("#{id}")) {
                problems.push(format!(
                    "annotate anchors #{id}, but no element with that id exists"
                ));
            }
        }

        if !problems.is_empty() {
            for p in problems {
                warnings.push(format!("slide {}: {p}", slide_index + 1));
            }
            continue;
        }
        let Some(sel) = sel.as_deref() else { continue }; // empty block: nothing to draw
        if doc.items.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "<script type=\"application/json\" class=\"mz-annot\" data-target=\"{}\">{}</script>\n",
            inline::html_escape(sel),
            mirzam_annot::to_json(&doc)
        ));
    }
    out
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
        assert!(extract(0, &[], "<div></div>", &mut w).is_empty());
        assert!(w.is_empty());
    }

    #[test]
    fn pane_target_becomes_a_data_pane_selector() {
        let mut w = Vec::new();
        let blocks = vec!["target: fig\ncircle 40,30 20x20 : label=\"here\"\n".to_string()];
        let out = extract(
            0,
            &blocks,
            "<div class=\"pane\" data-pane=\"fig\"></div>",
            &mut w,
        );
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
        let out = extract(0, &blocks, "<img id=\"shot\">", &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("data-target=\"#shot\""));
    }

    #[test]
    fn missing_target_warns_and_drops_the_block() {
        let mut w = Vec::new();
        let blocks = vec!["target: ghost\nrect 10,10 20x20\n".to_string()];
        let out = extract(2, &blocks, "<div></div>", &mut w);
        assert!(out.is_empty());
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("slide 3"));
        assert!(w[0].contains("matches nothing"));
    }

    #[test]
    fn missing_anchor_warns() {
        let mut w = Vec::new();
        let blocks = vec!["target: fig\ncircle #nope : pad=4\n".to_string()];
        let out = extract(0, &blocks, "<div data-pane=\"fig\"></div>", &mut w);
        assert!(out.is_empty());
        assert!(w[0].contains("#nope"));
    }

    #[test]
    fn anchor_to_a_chart_mark_resolves() {
        let mut w = Vec::new();
        let blocks = vec!["target: chart\ncircle #rev-0-1 : pad=6\n".to_string()];
        let haystack = "<div data-pane=\"chart\"><svg><g id=\"rev-0-1\"></g></svg></div>";
        let out = extract(0, &blocks, haystack, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("\"anchor\":\"rev-0-1\""));
    }

    #[test]
    fn parse_errors_become_warnings() {
        let mut w = Vec::new();
        let blocks = vec!["target: fig\nblob 1,1\n".to_string()];
        let out = extract(0, &blocks, "<div data-pane=\"fig\"></div>", &mut w);
        assert!(out.is_empty());
        assert!(w[0].contains("blob"));
    }
}
