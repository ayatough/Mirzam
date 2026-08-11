//! Build-time validation for `connect` endpoints.
//!
//! Endpoint coordinates are resolved by the viewer at runtime (see the module
//! doc in `mirzam-connect`), so an id that matches nothing draws no arrow and
//! nothing in the render pipeline catches it - only the browser-side checker
//! used to. This reports the same thing `anim`/`annotate` already do for
//! their own targets: a warning, numbered to the slide, with the connector
//! still emitted exactly as written - only the warning is new.

use regex::Regex;
use std::sync::OnceLock;

/// Warns about every connector endpoint that matches no `id="..."` in
/// `haystack` (the slide's rendered body plus its shape layer - the same
/// haystack `anim`/`annotate` check their own targets against).
/// `annot_ids` are the `id=` names declared inside this slide's `annotate`
/// blocks: an annotation mark exists only once the browser draws the overlay
/// (see `mirzam-annot::Item::id`), so a connector legitimately pointing at
/// one has nothing to find in `haystack`, which is static markup.
pub fn validate(
    slide_index: usize,
    doc: &mirzam_connect::ConnectDoc,
    haystack: &str,
    annot_ids: &[String],
    warnings: &mut Vec<String>,
) {
    for c in &doc.connectors {
        for id in [c.from.as_str(), c.to.as_str()] {
            if !endpoint_exists(haystack, annot_ids, id) {
                warnings.push(format!(
                    "slide {}: connect endpoint `#{id}` matches nothing on this slide",
                    slide_index + 1
                ));
            }
        }
    }
}

/// Whether `id` can be shown to exist: either directly in `haystack`, as a
/// name an `annotate` mark on this slide will draw, or - for a chart element
/// id (`<chart-id>-<series>-<point>`; `chart-id` is `chart<slide>-<n>` by
/// default but `id:` in the `chart` block can rename it) - by the chart
/// itself existing. The exact series/point is not checked: that would mean
/// re-deriving how many the chart actually plotted, and getting it wrong
/// would flag a real reference as broken. Checking the chart's own id,
/// always present on its root `<svg>`, is enough to avoid that false
/// positive.
fn endpoint_exists(haystack: &str, annot_ids: &[String], id: &str) -> bool {
    if haystack.contains(&format!("id=\"{id}\"")) {
        return true;
    }
    if annot_ids.iter().any(|a| a == id) {
        return true;
    }
    match chart_base_id(id) {
        Some(base) => haystack.contains(&format!("id=\"{base}\"")),
        None => false,
    }
}

fn chart_base_id(id: &str) -> Option<&str> {
    chart_element_id_regex()
        .captures(id)
        .map(|c| c.get(1).unwrap().as_str())
}

fn chart_element_id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+)-\d+-\d+$").expect("static regex"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirzam_connect::parse_connectors;

    fn warnings_for(src: &str, haystack: &str) -> Vec<String> {
        warnings_for_with_annot_ids(src, haystack, &[])
    }

    fn warnings_for_with_annot_ids(src: &str, haystack: &str, annot_ids: &[&str]) -> Vec<String> {
        let doc = parse_connectors(src);
        let annot_ids: Vec<String> = annot_ids.iter().map(|s| s.to_string()).collect();
        let mut warnings = Vec::new();
        validate(0, &doc, haystack, &annot_ids, &mut warnings);
        warnings
    }

    #[test]
    fn existing_ids_produce_no_warning() {
        let w = warnings_for("#a -> #b", "<span id=\"a\"></span><span id=\"b\"></span>");
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn a_missing_id_is_reported() {
        let w = warnings_for("#a -> #ghost", "<span id=\"a\"></span>");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("slide 1"));
        assert!(w[0].contains("#ghost"));
        assert!(w[0].contains("matches nothing"));
    }

    #[test]
    fn an_edge_suffix_is_stripped_before_checking() {
        let w = warnings_for(
            "#a.n -> #b.s",
            "<span id=\"a\"></span><span id=\"b\"></span>",
        );
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn a_chart_element_id_checks_only_that_the_chart_exists() {
        let haystack = "<svg class=\"mz-chart\" id=\"chart1-1\">...</svg>";
        let w = warnings_for("#chart1-1-0-2 -> #a", haystack);
        // #chart1-1-0-2 resolves via the chart's own id; #a is still missing.
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("#a"));
    }

    #[test]
    fn a_custom_chart_id_from_the_chart_block_also_resolves() {
        let haystack = "<svg class=\"mz-chart\" id=\"adoption\">...</svg>";
        let w = warnings_for("#growth -> #adoption-0-5", "<span id=\"growth\"></span>");
        // Without the chart's svg in the haystack, the reference is unresolved.
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("#adoption-0-5"));
        let w = warnings_for(
            "#growth -> #adoption-0-5",
            &format!("<span id=\"growth\"></span>{haystack}"),
        );
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn a_missing_chart_is_still_reported() {
        let w = warnings_for("#chart1-1-0-0 -> #a", "<span id=\"a\"></span>");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("#chart1-1-0-0"));
    }

    #[test]
    fn an_annotate_marks_id_resolves_even_though_it_draws_nothing_statically() {
        // The mark exists only once the viewer draws the overlay at runtime
        // (see mirzam-annot::Item::id), so it is never in the static HTML.
        let w =
            warnings_for_with_annot_ids("#t-hot -> #hot", "<span id=\"t-hot\"></span>", &["hot"]);
        assert!(w.is_empty(), "{w:?}");
    }
}
