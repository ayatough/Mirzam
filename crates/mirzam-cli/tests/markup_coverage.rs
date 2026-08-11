//! Every inline mark Mirzam understands, in one list, checked three ways.
//!
//! This exists because the question "is X supported?" had no answer anybody
//! could trust. Strikethrough and task lists both worked from the first
//! release and appeared in no reference and no sample — so the only way to
//! find out was to read the renderer, and the author, the documentation and
//! the samples had each drifted to a different answer.
//!
//! A mark listed here must:
//!   1. render, which is the difference between supported and imagined;
//!   2. be in `docs/syntax.md`, or nobody can discover it;
//!   3. be in a sample deck, or nobody can see what it looks like.
//!
//! Adding a mark to the renderer and not to this list is the one hole left,
//! and it is a much smaller one: the list is the first place anyone changing
//! `render_markdown` will look, because it sits next to the tests they have to
//! make pass.
//!
//! The fenced *block* forms have their own registry — `BLOCK_KINDS` in
//! `mirzam-syntax`, walked by `commonmark_compat.rs`. This is the inline half.

mod common;

use common::repo_root;
use mirzam_render::{preprocess, render_markdown};

/// One mark: what you write, what has to come out, and the phrase that has to
/// appear in the reference and in some deck under `examples/`.
struct Mark {
    /// What the reader types.
    source: &'static str,
    /// A fragment the rendered HTML must contain.
    html: &'static str,
    /// Text that must appear in `docs/syntax.md`.
    documented_as: &'static str,
    /// Text that must appear in at least one deck under `examples/`.
    shown_as: &'static str,
}

const MARKS: &[Mark] = &[
    Mark {
        source: "**bold**",
        html: "<strong>bold</strong>",
        documented_as: "`**bold**`",
        shown_as: "**bold**",
    },
    Mark {
        source: "*italic*",
        html: "<em>italic</em>",
        documented_as: "`*italic*`",
        shown_as: "*italic*",
    },
    Mark {
        source: "~~struck~~",
        html: "<del>struck</del>",
        documented_as: "`~~text~~`",
        shown_as: "~~struck~~",
    },
    Mark {
        source: "==marked==",
        html: "<mark>marked</mark>",
        documented_as: "`==text==`",
        shown_as: "==highlighted==",
    },
    Mark {
        source: "++inserted++",
        html: "<ins>inserted</ins>",
        documented_as: "`++text++`",
        shown_as: "++underlined++",
    },
    Mark {
        source: "`code`",
        html: "<code>code</code>",
        documented_as: "`inline code`",
        shown_as: "`inline code`",
    },
    Mark {
        source: ":tada:",
        html: "🎉",
        documented_as: "`:tada:`",
        shown_as: ":tada:",
    },
    Mark {
        source: "Term\n: Meaning.\n",
        html: "<dt>",
        documented_as: "a term list",
        shown_as: "Term list\n:",
    },
    Mark {
        source: "- [x] done\n",
        html: "checkbox",
        documented_as: "`- [ ]`",
        shown_as: "- [x]",
    },
    Mark {
        source: "1. first\n",
        html: "<ol>",
        documented_as: "numbered",
        shown_as: "1. Write the deck",
    },
    Mark {
        source: "> quoted\n",
        html: "<blockquote>",
        documented_as: "quotation",
        shown_as: "> A quotation",
    },
    Mark {
        source: "| a | b |\n|---|---|\n| 1 | 2 |\n",
        html: "<table>",
        documented_as: "`---:` right",
        shown_as: "| Command | Writes | When |",
    },
    Mark {
        source: "[text](https://example.com)",
        html: r#"href="https://example.com""#,
        documented_as: "a link",
        shown_as: "[a link](https://example.com)",
    },
];

/// A mark that does not render is not a feature, whatever the docs say.
#[test]
fn every_listed_mark_renders() {
    for m in MARKS {
        let out = render_markdown(&preprocess(m.source));
        assert!(
            out.contains(m.html),
            "`{}` should produce `{}`, got:\n{out}",
            m.source.escape_debug(),
            m.html
        );
    }
}

/// A mark nobody can find is not a feature either. Task lists worked for two
/// releases without a line in the reference, which is the failure this catches.
#[test]
fn every_listed_mark_is_in_the_syntax_reference() {
    let doc = std::fs::read_to_string(repo_root().join("docs/syntax.md")).expect("docs/syntax.md");
    let missing: Vec<_> = MARKS
        .iter()
        .filter(|m| !doc.contains(m.documented_as))
        .map(|m| m.source.escape_debug().to_string())
        .collect();
    assert!(
        missing.is_empty(),
        "these render but docs/syntax.md never mentions them: {missing:?}"
    );
}

/// And a mark with no sample is one nobody can see the shape of. `02-writing.md`
/// is where the inline marks live; any deck counts.
#[test]
fn every_listed_mark_appears_in_a_sample_deck() {
    let decks: String = std::fs::read_dir(repo_root().join("examples"))
        .expect("examples/")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();

    let missing: Vec<_> = MARKS
        .iter()
        .filter(|m| !decks.contains(m.shown_as))
        .map(|m| m.source.escape_debug().to_string())
        .collect();
    assert!(
        missing.is_empty(),
        "these are documented but no deck under examples/ shows them: {missing:?}"
    );
}

/// The pane classes that reshape a term list, held to the same three
/// conditions as a mark: styled, documented, shown.
///
/// They are not marks — nothing new is written in the Markdown, the class rides
/// on the pane — so they cannot live in `MARKS`, whose test renders a fragment.
/// The failure they are exposed to is the same one, though: a class that only
/// the stylesheet knows about is a feature nobody can find. Three modes exist
/// because which one is right is a per-list question, and a renderer that picks
/// for you is wrong a third of the time.
const TERM_MODES: &[&str] = &["terms-aligned", "terms-stacked"];

#[test]
fn every_term_list_mode_is_styled_documented_and_shown() {
    let css = std::fs::read_to_string(repo_root().join("crates/mirzam-render/src/theme/base.css"))
        .expect("base.css");
    let doc = std::fs::read_to_string(repo_root().join("docs/syntax.md")).expect("docs/syntax.md");
    let decks: String = std::fs::read_dir(repo_root().join("examples"))
        .expect("examples/")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();

    for mode in TERM_MODES {
        assert!(
            css.contains(&format!(".{mode} dl")),
            "base.css does not style `.{mode}`"
        );
        assert!(
            doc.contains(mode),
            "docs/syntax.md never mentions `.{mode}`"
        );
        assert!(
            decks.contains(&format!("{{.{mode}}}")),
            "no deck under examples/ shows `{{.{mode}}}`"
        );
    }
}

/// The dials under the modes. A default written straight onto `dl` would beat
/// the value a pane or a theme sets, so each is read as a `var()` fallback —
/// which is the whole reason they are overridable at all.
#[test]
fn every_term_list_dial_is_read_with_its_default_as_a_fallback() {
    let css = std::fs::read_to_string(repo_root().join("crates/mirzam-render/src/theme/base.css"))
        .expect("base.css");
    for (dial, default) in [
        ("--mz-terms-hang", "2em"),
        ("--mz-terms-gap", ".6em"),
        ("--mz-terms-col", "38%"),
    ] {
        assert!(
            css.contains(&format!("var({dial}, {default})")),
            "`{dial}` should be read as `var({dial}, {default})`"
        );
        assert!(
            !css.contains(&format!("  {dial}:")),
            "`{dial}` is declared on an element; a pane or theme could not override it"
        );
    }
}
