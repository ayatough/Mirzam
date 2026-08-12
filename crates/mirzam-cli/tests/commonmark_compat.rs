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

/// A realistic body for each block form, so the test reads what an author
/// would actually write rather than an empty fence.
fn sample_block(kind: &str) -> &'static str {
    match kind {
        "pane" => "+---+---+\n| a | b |\n+---+---+\n",
        "shape" => "rect #r at(50%, 50%) size(10%, 10%)\n",
        "connect" => "#a -> #r : color=@accent2\n",
        "anim" => "[enter] .title : fade-in 400ms\n",
        "annotate" => "target: fig\ncircle 40,30 20x20 : label=\"here\"\n",
        "chart" => "type: bar\ndata: |\n  k, v\n  a, 1\n",
        "effects" => "1 : flash\n",
        "toc" => "from: 2\ndepth: 3\n",
        "bibliography" => "show: cited\nback: true\n",
        other => panic!("no sample written for the `{other}` block"),
    }
}

/// Walks [`mirzam_syntax::BLOCK_KINDS`] rather than a list of its own, so a
/// block form added to the language without a compatibility story fails here
/// instead of shipping.
#[test]
fn extension_blocks_degrade_to_code_blocks() {
    let mut src = String::new();
    for kind in mirzam_syntax::BLOCK_KINDS {
        src.push_str(&format!("```{kind}\n{}```\n\n", sample_block(kind)));
    }
    let html = plain_commonmark(&src);
    // Fenced blocks become code blocks whose info string turns into a class.
    assert_eq!(
        html.matches("<pre>").count(),
        mirzam_syntax::BLOCK_KINDS.len(),
        "one code block per form: {html}"
    );
    for kind in mirzam_syntax::BLOCK_KINDS {
        assert!(
            html.contains(&format!("language-{kind}")),
            "`{kind}` did not degrade to a code block: {html}"
        );
    }
    // The contents remain readable; no information is lost.
    assert!(html.contains("rect #r at(50%, 50%)"));
}

/// The forms that are not fenced blocks. Each one has to survive a plain
/// parser too, and each fails differently: a comment must stay invisible, an
/// attribute list must stay inert, an include must not turn into a broken
/// image the reader is told to go and find.
#[test]
fn the_inline_forms_stay_harmless() {
    let src = "\
A phrase [carrying an id]{#win .u} and a {{ price * 12 }} figure.

<!-- next -->

Body after the break, with a citation[^src] and a reference[@vaswani2017].

![[section.md]]

[^src]: The source.
";
    let html = plain_commonmark(src);
    // The continuation marker is a comment: a plain parser shows nothing.
    assert!(
        !html.contains("next") || html.contains("&lt;!--"),
        "the continuation marker became visible: {html}"
    );
    // Attribute lists and variables read as the literal text they are.
    assert!(html.contains("carrying an id"), "{html}");
    assert!(html.contains("{{ price * 12 }}"), "{html}");
    // A transclusion degrades to an image-shaped link. Obsidian embeds it;
    // GitHub shows the filename. Neither loses the reference.
    assert!(html.contains("section.md"), "{html}");
    // Footnotes are a CommonMark extension, so plainly they stay as text —
    // which is still readable, and still says where the claim came from.
    assert!(html.contains("[^src]"), "{html}");
    // A citation is the same trade one step further out: the key stays on the
    // page, so a reader on GitHub can still see which paper is meant even
    // though the reference list is not built for them.
    assert!(html.contains("[@vaswani2017]"), "{html}");
}

/// The three W14 text marks name a phrase and nothing else, so the phrase has
/// to be readable on its own — the mark is decoration a plain reader loses,
/// not meaning.
#[test]
fn a_marked_phrase_still_reads_without_its_mark() {
    let src = "\
Origin traffic keeps falling — [by Q3 it is the smaller half]{#c-q3}

```annotate
highlight #c-q3     : color=@accent2 step=1
rect      #cook-1-2 : color=@accent2 step=1 pad=6
```
";
    let html = plain_commonmark(src);
    assert!(html.contains("by Q3 it is the smaller half"), "{html}");
    assert!(html.contains("language-annotate"), "{html}");
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

/// The same principle read the other way round: Markdown carrying none of
/// Mirzam's syntax has to come out of Mirzam the way a reference parser
/// renders it. The extensions a deck wants — `==marked==`, `++inserted++` —
/// are switched on for every document, and one of them changed how *plain*
/// emphasis was read: `**+ alpha**` reached the slide as literal asterisks
/// because `+` is an extension delimiter and the scan for what follows `**`
/// stepped over it onto the space.
#[test]
fn plain_markdown_renders_like_the_reference_parser() {
    for src in [
        "A **+ alpha** here.\n",
        "A **+alpha** here.\n",
        "A **- alpha** here.\n",
        "A *= alpha* here.\n",
        "A **~ alpha** here.\n",
        "A ** + alpha** here.\n",
        "A **x alpha** here.\n",
        "| **+ wheel odometry** | added |\n",
        "Write `**+ a**` and \\*\\*+ b\\*\\* plainly.\n",
        "_+ under_ and __+ strong__.\n",
    ] {
        let ours = mirzam_render::render_markdown(&mirzam_render::preprocess(src));
        assert_eq!(
            ours.trim(),
            plain_commonmark(src).trim(),
            "plain Markdown read differently: {src}"
        );
    }
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
