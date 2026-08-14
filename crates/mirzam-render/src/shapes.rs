//! Extracts `shape` fences written inside a `::: pane`.
//!
//! A pane-anchored shape block draws in its pane's coordinate space: `at(50%,
//! 50%)` is the centre of the pane, not of the slide. The fence is removed
//! from the pane's Markdown here — unlike a chart it is not flow content, so
//! nothing marks its place — and the sources are rendered into the slide's
//! one shape layer with the pane's rectangle as their frame. Coordinates are
//! not clipped to it: a shape past 100% deliberately reaches out of the pane,
//! exactly as a page-level shape may reach across one.

/// Splits ```` ```shape ```` fences out of a chunk of pane Markdown, returning
/// what is left and the fence bodies. A fence quoted inside a longer fence
/// (````` ```` `````) stays where it is: that is how a document *shows* shape
/// syntax rather than using it.
pub fn extract(md: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(md.len());
    let mut blocks = Vec::new();
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "```shape" {
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.trim() == "```" {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            blocks.push(body);
        } else if let Some(open) = mirzam_syntax::fence_len(trimmed).filter(|n| *n > 3) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_fence_is_removed_and_collected() {
        let (md, blocks) =
            extract("before\n\n```shape\nrect #a at(50,50) size(20,20)\n```\n\nafter\n");
        assert!(md.contains("before") && md.contains("after"));
        assert!(!md.contains("```shape"));
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("rect #a"));
    }

    #[test]
    fn a_quoted_fence_stays_markdown() {
        let (md, blocks) =
            extract("````markdown\n```shape\nrect #a at(0,0) size(1,1)\n```\n````\n");
        assert!(blocks.is_empty());
        assert!(md.contains("```shape"));
    }
}
