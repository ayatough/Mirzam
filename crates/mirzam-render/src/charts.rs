//! Renders `chart` blocks into inline SVG.
//!
//! Charts live inside a pane like any other content, so they are substituted
//! into the pane's Markdown before it is rendered. Referenced CSV files are
//! resolved through the same `AssetSource` used for images, which keeps the
//! filesystem-free WASM path working.

use crate::AssetSource;

/// Extracts ```chart fences from a chunk of Markdown, replacing each with a
/// placeholder comment. Charts are written inline in a pane, so they must be
/// substituted in place rather than collected at the slide level.
pub fn extract(md: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(md.len());
    let mut blocks = Vec::new();
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "```chart" {
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.trim() == "```" {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            out.push_str(&format!("\n<!--mz-chart:{}-->\n", blocks.len()));
            blocks.push(body);
        } else if let Some(open) = mirzam_syntax::fence_len(trimmed).filter(|n| *n > 3) {
            // A longer fence quotes chart syntax instead of using it.
            out.push_str(line);
            out.push('\n');
            for inner in lines.by_ref() {
                out.push_str(inner);
                out.push('\n');
                let t = inner.trim();
                if t.chars().all(|c| c == '`') && t.len() >= open {
                    break;
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    (out, blocks)
}

/// Replaces chart placeholders in `html` with rendered SVG.
/// Returns the referenced data files so the caller can watch them.
pub fn render_charts_in(
    html: &str,
    blocks: &[String],
    slide_index: usize,
    assets: &dyn AssetSource,
    errors: &mut Vec<String>,
) -> (String, Vec<std::path::PathBuf>) {
    let mut data_files = Vec::new();
    let mut out = html.to_string();
    for (i, src) in blocks.iter().enumerate() {
        let resolved_path = std::cell::RefCell::new(None);
        let doc = mirzam_chart::parse_chart(src, |path| {
            let (result, p) = assets.resolve(path);
            *resolved_path.borrow_mut() = p;
            // CSV arrives as a data URI from the asset layer; decode it back.
            result.ok().and_then(|uri| decode_text(&uri))
        });
        if let Some(p) = resolved_path.into_inner() {
            data_files.push(p);
        }
        for e in &doc.errors {
            errors.push(format!("slide {}: {e}", slide_index + 1));
        }
        let id = format!("chart{}-{}", slide_index + 1, i + 1);
        let svg = mirzam_chart::render_svg(&doc, &id);
        let marker = format!("<!--mz-chart:{i}-->");
        let replacement = if svg.is_empty() {
            String::new()
        } else {
            format!("<div class=\"mz-chart-wrap\">{svg}</div>")
        };
        out = out.replace(&marker, &replacement);
    }
    (out, data_files)
}

/// Decodes a `data:` URI produced by the asset layer back into text.
fn decode_text(uri: &str) -> Option<String> {
    use base64::Engine as _;
    let b64 = uri.split(";base64,").nth(1)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct NoAssets;
    impl AssetSource for NoAssets {
        fn resolve(&self, _: &str) -> (Result<String, String>, Option<PathBuf>) {
            (Err("missing".into()), None)
        }
    }

    struct CsvAssets;
    impl AssetSource for CsvAssets {
        fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
            use base64::Engine as _;
            let csv = "k,v\na,1\nb,2\n";
            let b64 = base64::engine::general_purpose::STANDARD.encode(csv);
            (
                Ok(format!("data:text/csv;base64,{b64}")),
                Some(PathBuf::from(rel)),
            )
        }
    }

    #[test]
    fn extracts_chart_fences_in_place() {
        let (md, blocks) = extract("before\n\n```chart\ntype: bar\n```\n\nafter\n");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("type: bar"));
        assert!(md.contains("<!--mz-chart:0-->"));
        assert!(md.contains("before") && md.contains("after"));
        assert!(!md.contains("```chart"));
    }

    #[test]
    fn longer_fence_quotes_chart_syntax() {
        let (md, blocks) = extract("````markdown\n```chart\ntype: bar\n```\n````\n");
        assert!(blocks.is_empty(), "quoted chart must not be rendered");
        assert!(md.contains("```chart"));
    }

    #[test]
    fn replaces_placeholder_with_svg() {
        let blocks = vec!["type: bar\ndata: |\n  k, v\n  a, 1\n  b, 2\n".to_string()];
        let mut errors = Vec::new();
        let (out, files) = render_charts_in(
            "<p><!--mz-chart:0--></p>",
            &blocks,
            0,
            &NoAssets,
            &mut errors,
        );
        assert!(out.contains("<svg class=\"mz-chart\""));
        assert!(out.contains("id=\"chart1-1-0-0\""));
        assert!(errors.is_empty());
        assert!(files.is_empty());
    }

    #[test]
    fn resolves_csv_through_asset_source() {
        let blocks = vec!["type: line\ndata: data.csv\n".to_string()];
        let mut errors = Vec::new();
        let (out, files) =
            render_charts_in("<!--mz-chart:0-->", &blocks, 0, &CsvAssets, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(out.contains("mz-chart-line"));
        assert_eq!(files, vec![PathBuf::from("data.csv")]);
    }

    #[test]
    fn missing_data_reports_error_and_drops_chart() {
        let blocks = vec!["type: bar\ndata: gone.csv\n".to_string()];
        let mut errors = Vec::new();
        let (out, _) = render_charts_in("<!--mz-chart:0-->", &blocks, 2, &NoAssets, &mut errors);
        assert!(errors[0].contains("slide 3"));
        assert!(!out.contains("<svg"));
    }
}
