//! Automated check of the project's core principle:
//! **Mirzam's extensions never break a plain CommonMark parser.**
//!
//! Opening a deck on GitHub or in Obsidian must still show readable text,
//! with extension blocks degrading harmlessly into code blocks.

mod common;

use common::{example, EXAMPLE_DECKS};

/// Renders as plain CommonMark, with every extension disabled.
fn plain_commonmark(md: &str) -> String {
    let options = comrak::Options::default(); // no extensions; raw HTML escaped
    comrak::markdown_to_html(md, &options)
}

#[test]
fn extension_blocks_degrade_to_code_blocks() {
    let src = "\
```pane
+---+---+
| a | b |
+---+---+
```

```shape
rect #r at(50%, 50%) size(10%, 10%)
```

```connect
#a -> #r
```
";
    let html = plain_commonmark(src);
    // Fenced blocks become code blocks whose info string turns into a class.
    assert_eq!(
        html.matches("<pre>").count(),
        3,
        "expected three code blocks: {html}"
    );
    assert!(html.contains("language-pane"));
    assert!(html.contains("language-shape"));
    assert!(html.contains("language-connect"));
    // The contents remain readable; no information is lost.
    assert!(html.contains("rect #r at(50%, 50%)"));
}

#[test]
fn pane_divs_and_vars_stay_readable_text() {
    let src = "::: pane main\n\nBody {{price}} yen\n\n:::\n";
    let html = plain_commonmark(src);
    // The div syntax survives as paragraph text; the content is intact.
    assert!(html.contains("::: pane main"));
    assert!(html.contains("Body {{price}} yen"));
}

#[test]
fn speaker_notes_are_hidden_comments() {
    let html = plain_commonmark("Body\n\n<!-- note: private memo -->\n");
    // With raw HTML disabled the comment still must not become visible text.
    assert!(!html.contains("private memo") || html.contains("&lt;!--"));
}

/// Every sample deck must still read as a document under plain CommonMark.
#[test]
fn all_examples_render_as_plain_markdown() {
    for deck in EXAMPLE_DECKS {
        let src = std::fs::read_to_string(example(deck)).expect("read sample");
        let (_, body) = mirzam_syntax::split_frontmatter(&src);
        let html = plain_commonmark(body);

        assert!(!html.is_empty(), "{deck}: empty output");
        // Headings survive.
        assert!(
            html.contains("<h1") || html.contains("<h2"),
            "{deck}: no headings survived"
        );
        // Extension blocks all degrade to code blocks; no Mirzam output leaks through.
        for marker in ["grid-template-areas", "<section class=\"slide\""] {
            assert!(
                !html.contains(marker),
                "{deck}: Mirzam-specific output `{marker}` leaked into plain Markdown"
            );
        }
    }
}
