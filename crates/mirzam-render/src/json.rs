//! JSON string literals for the payloads decks carry in `<script>` tags.
//!
//! Every one of them is embedded in an HTML document, so `<` is escaped as
//! well as the characters JSON requires — otherwise a `</script>` inside a
//! danmaku line, or inside a slide's own Markdown, would close the block
//! early and hand whatever followed to the HTML parser.

/// A JSON string literal, safe to place inside a `<script>` element.
pub fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_backslashes_and_newlines_survive() {
        assert_eq!(string("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
    }

    #[test]
    fn a_script_close_tag_cannot_break_out_of_the_block() {
        let out = string("</script><img onerror=x>");
        assert!(!out.contains("</script>"), "{out}");
        assert!(out.contains("\\u003c/script"), "{out}");
    }

    #[test]
    fn a_stray_control_character_is_escaped_rather_than_emitted() {
        assert_eq!(string("a\u{1}b"), r#""a\u0001b""#);
    }
}
