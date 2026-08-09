//! `toc` blocks: a table of contents built from the deck's own headings.
//!
//! This is the first block that needs to know about slides other than its own,
//! and slides are rendered independently and cached by content hash. So it works
//! in two passes, the same shape as a chart placeholder but one level up:
//!
//! 1. [`extract`] replaces the fence with a comment marker while the slide is
//!    still Markdown. The marker carries the options, so nothing has to be
//!    remembered between the passes — which is what lets a cached slide keep an
//!    unresolved marker and still be correct.
//! 2. [`resolve_deck`] runs once every slide has rendered, reads the headings
//!    out of the finished HTML, and substitutes the list into every marker.
//!
//! Because pass 2 runs over the assembled deck rather than over one slide, it
//! belongs to whoever assembles the deck: the CLI pipeline and the WASM
//! renderer both call it, and the print page gets the same list with page
//! numbers instead of links.

/// Turns ```toc fences into markers. Returns the Markdown with the fences
/// replaced; a marker that is never resolved renders as nothing, which is the
/// right outcome for a slide previewed on its own.
pub fn extract(md: &str, errors: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(md.len());
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "```toc" {
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.trim() == "```" {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            let spec = parse(&body, errors);
            out.push_str(&format!(
                "\n<!--mz-toc:{}:{}:{}-->\n",
                spec.from,
                spec.depth,
                u8::from(spec.current)
            ));
        } else if let Some(open) = mirzam_syntax::fence_len(trimmed).filter(|n| *n > 3) {
            // A longer fence quotes the syntax instead of using it, which is how
            // this file's own documentation shows a `toc` block.
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
    out
}

/// What a `toc` block asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spec {
    /// Shallowest heading level listed. `2` skips the deck's `#` title, which
    /// is the usual shape: the title of the talk is not an item on its agenda.
    from: u8,
    /// Deepest heading level listed. `2` lists `##` and everything above it.
    depth: u8,
    /// Mark the entry the presenter is currently inside.
    current: bool,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            from: 1,
            depth: 2,
            current: false,
        }
    }
}

fn parse(body: &str, errors: &mut Vec<String>) -> Spec {
    let mut spec = Spec::default();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            errors.push(format!("toc: `{line}` is not `key: value`"));
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "from" => match value.parse::<u8>() {
                Ok(n) if (1..=6).contains(&n) => spec.from = n,
                _ => errors.push(format!("toc: from `{value}` must be 1 to 6")),
            },
            "depth" => match value.parse::<u8>() {
                Ok(n) if (1..=6).contains(&n) => spec.depth = n,
                _ => errors.push(format!("toc: depth `{value}` must be 1 to 6")),
            },
            "current" => match value {
                "true" | "yes" | "on" => spec.current = true,
                "false" | "no" | "off" => spec.current = false,
                _ => errors.push(format!("toc: current `{value}` must be true or false")),
            },
            other => errors.push(format!("toc: unknown key `{other}`")),
        }
    }
    spec
}

/// One line of the table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub level: u8,
    pub text: String,
    /// Zero-based index of the slide that carries the heading.
    pub slide: usize,
}

/// Every heading in the deck, in order, each attributed to the first slide that
/// carries it. Speaker notes are skipped: a note is what the presenter says, not
/// part of the deck's structure.
pub fn headings(sections: &[String]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        for (level, text) in headings_in(&without_notes(section)) {
            // A heading repeated across continuation slides is one heading.
            if out.iter().any(|e| e.level == level && e.text == text) {
                continue;
            }
            out.push(Entry {
                level,
                text,
                slide: i,
            });
        }
    }
    out
}

/// Drops `<aside class="notes">…</aside>` so headings written in a note do not
/// end up in the deck's outline.
fn without_notes(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find("<aside class=\"notes\">") {
        out.push_str(&rest[..at]);
        match rest[at..].find("</aside>") {
            Some(end) => rest = &rest[at + end + "</aside>".len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// `<h1>`…`<h6>` in document order, as (level, plain text).
fn headings_in(html: &str) -> Vec<(u8, String)> {
    let bytes = html.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = html[i..].find("<h") {
        let start = i + at;
        let level = match bytes.get(start + 2) {
            Some(c @ b'1'..=b'6') => c - b'0',
            _ => {
                i = start + 2;
                continue;
            }
        };
        // `<h2 id="x">` and `<h2>` both count; `<hr>` and `<html>` do not.
        match bytes.get(start + 3) {
            Some(b'>') | Some(b' ') => {}
            _ => {
                i = start + 2;
                continue;
            }
        }
        let Some(open_end) = html[start..].find('>').map(|o| start + o + 1) else {
            break;
        };
        let close = format!("</h{level}>");
        let Some(close_at) = html[open_end..].find(&close).map(|o| open_end + o) else {
            i = open_end;
            continue;
        };
        let text = plain_text(&html[open_end..close_at]);
        if !text.is_empty() {
            out.push((level, text));
        }
        i = close_at + close.len();
    }
    out
}

/// Strips tags and unescapes the entities the renderer emits, so a heading
/// carrying `<strong>` or `&amp;` becomes the words a reader would say.
fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut depth = 0usize;
    for c in html.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Substitutes the resolved list into every `toc` marker in the deck.
///
/// Call it once, after every slide has rendered. Idempotent: a deck with no
/// `toc` block is left byte-identical, which is what keeps it free for the
/// decks that do not use it.
pub fn resolve_deck(sections: &mut [String]) {
    if !sections.iter().any(|s| s.contains("<!--mz-toc:")) {
        return;
    }
    let entries = headings(sections);
    for (i, section) in sections.iter_mut().enumerate() {
        *section = substitute(section, &entries, i);
    }
}

fn substitute(section: &str, entries: &[Entry], host: usize) -> String {
    let mut out = String::with_capacity(section.len());
    let mut rest = section;
    while let Some(at) = rest.find("<!--mz-toc:") {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find("-->").map(|o| at + o + 3) else {
            break;
        };
        let args = &rest[at + "<!--mz-toc:".len()..at + rest[at..].find("-->").unwrap()];
        let mut parts = args.split(':');
        let from: u8 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
        let depth: u8 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(2);
        let current = parts.next() == Some("1");
        out.push_str(&render(entries, from, depth, current, host));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn render(entries: &[Entry], from: u8, depth: u8, current: bool, host: usize) -> String {
    // A table of contents does not list itself: the heading on the slide the
    // block sits on is "Agenda", and an agenda is not an item on the agenda.
    let listed: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.level >= from && e.level <= depth && e.slide != host)
        .collect();
    if listed.is_empty() {
        return String::new();
    }
    let top = listed.iter().map(|e| e.level).min().unwrap_or(1);
    let mut out = String::from("<nav class=\"mz-toc\"");
    if current {
        out.push_str(" data-current=\"1\"");
    }
    out.push_str("><ol>");
    for e in &listed {
        // The href is the slide number, which is the address the viewer already
        // keeps in `location.hash` — so an entry works with no JavaScript at
        // all, and the runtime only has to notice the hash changed.
        out.push_str(&format!(
            "<li class=\"mz-toc-l{}\" data-slide=\"{}\" style=\"margin-left:{}em\">\
             <a href=\"#{}\">{}</a><span class=\"mz-toc-page\">{}</span></li>",
            e.level,
            e.slide,
            f32::from(e.level - top) * 1.2,
            e.slide + 1,
            crate::inline::html_escape(&e.text),
            e.slide + 1,
        ));
    }
    out.push_str("</ol></nav>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_errors(md: &str) -> String {
        let mut errors = Vec::new();
        let out = extract(md, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        out
    }

    #[test]
    fn a_bare_block_takes_the_defaults() {
        assert!(no_errors("```toc\n```\n").contains("<!--mz-toc:1:2:0-->"));
    }

    #[test]
    fn options_ride_on_the_marker() {
        let out = no_errors("```toc\nfrom: 2\ndepth: 3\ncurrent: true\n```\n");
        assert!(out.contains("<!--mz-toc:2:3:1-->"), "{out}");
    }

    #[test]
    fn a_longer_fence_quotes_the_syntax() {
        let out = no_errors("````markdown\n```toc\ndepth: 2\n```\n````\n");
        assert!(out.contains("```toc"), "{out}");
        assert!(!out.contains("<!--mz-toc"), "{out}");
    }

    #[test]
    fn bad_options_are_reported_not_guessed() {
        let mut errors = Vec::new();
        extract("```toc\ndepth: 9\nnope: 1\n```\n", &mut errors);
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    #[test]
    fn headings_carry_the_slide_they_are_on() {
        let sections = vec![
            "<section><h1>Title</h1></section>".to_string(),
            "<section><h2>Method</h2><h3>Setup</h3></section>".to_string(),
        ];
        assert_eq!(
            headings(&sections),
            vec![
                Entry {
                    level: 1,
                    text: "Title".into(),
                    slide: 0
                },
                Entry {
                    level: 2,
                    text: "Method".into(),
                    slide: 1
                },
                Entry {
                    level: 3,
                    text: "Setup".into(),
                    slide: 1
                },
            ]
        );
    }

    #[test]
    fn markup_inside_a_heading_becomes_words() {
        let sections = vec!["<h2>Latency <strong>after</strong> the roll&amp;out</h2>".to_string()];
        assert_eq!(headings(&sections)[0].text, "Latency after the roll&out");
    }

    /// `<hr>` starts with `<h` and is not a heading.
    #[test]
    fn only_real_headings_count() {
        let sections = vec!["<hr><h2>Real</h2><html><h7>no</h7>".to_string()];
        assert_eq!(headings(&sections).len(), 1);
    }

    /// A note is what the presenter says, not part of the outline.
    #[test]
    fn headings_written_in_a_note_stay_out() {
        let sections = vec![
            "<h2>On the slide</h2><aside class=\"notes\"><h2>In the note</h2></aside>".to_string(),
        ];
        assert_eq!(headings(&sections).len(), 1);
    }

    /// A slide broken by `<!-- next -->` repeats its heading on every part.
    #[test]
    fn a_repeated_heading_is_listed_once() {
        let sections = vec![
            "<h2>Method</h2><p>one</p>".to_string(),
            "<h2>Method</h2><p>two</p>".to_string(),
        ];
        let h = headings(&sections);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].slide, 0, "it belongs to the first slide that has it");
    }

    #[test]
    fn depth_limits_what_is_listed() {
        let mut sections = vec![
            "<!--mz-toc:1:2:0-->".to_string(),
            "<h1>A</h1><h2>B</h2><h3>C</h3>".to_string(),
        ];
        resolve_deck(&mut sections);
        assert!(sections[0].contains(">A<") && sections[0].contains(">B<"));
        assert!(!sections[0].contains(">C<"), "{}", sections[0]);
    }

    #[test]
    fn an_entry_links_to_the_slide_number() {
        let mut sections = vec![
            "<!--mz-toc:1:2:1-->".to_string(),
            "<h2>Method</h2>".to_string(),
        ];
        resolve_deck(&mut sections);
        assert!(sections[0].contains("href=\"#2\""), "{}", sections[0]);
        assert!(sections[0].contains("data-slide=\"1\""));
        assert!(sections[0].contains("data-current=\"1\""));
    }

    /// The talk's own title is not an item on its agenda, so a deck whose
    /// title is an `h1` can start the list at `h2`.
    #[test]
    fn from_skips_the_shallow_levels() {
        let mut sections = vec![
            "<!--mz-toc:2:2:0-->".to_string(),
            "<h1>The talk</h1><h2>Method</h2>".to_string(),
        ];
        resolve_deck(&mut sections);
        assert!(sections[0].contains(">Method<"), "{}", sections[0]);
        assert!(!sections[0].contains(">The talk<"), "{}", sections[0]);
    }

    /// "Agenda" is not an item on the agenda.
    #[test]
    fn the_slide_carrying_the_list_is_not_in_it() {
        let mut sections = vec![
            "<h2>Agenda</h2><!--mz-toc:1:2:0-->".to_string(),
            "<h2>Method</h2>".to_string(),
        ];
        resolve_deck(&mut sections);
        // Only the list matters; the slide keeps its own heading, of course.
        let nav = &sections[0][sections[0].find("<nav").unwrap()..];
        assert!(nav.contains(">Method<"));
        assert!(!nav.contains(">Agenda<"), "{nav}");
    }

    #[test]
    fn a_deck_without_a_toc_is_untouched() {
        let before = vec!["<h2>A</h2>".to_string(), "<p>b</p>".to_string()];
        let mut after = before.clone();
        resolve_deck(&mut after);
        assert_eq!(before, after);
    }

    #[test]
    fn a_toc_on_a_deck_with_no_headings_renders_nothing() {
        let mut sections = vec!["<p>x</p><!--mz-toc:1:2:0--><p>y</p>".to_string()];
        resolve_deck(&mut sections);
        assert_eq!(sections[0], "<p>x</p><p>y</p>");
    }
}
