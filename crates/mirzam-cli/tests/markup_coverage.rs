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
    Mark {
        // Highlighting is not an inline mark, but it fails the same three
        // ways: a language the table forgot renders plain, a reader who
        // cannot find the list does not know which languages work, and a deck
        // with no coloured code shows nobody what it looks like.
        source: "```rust\nfn main() {}\n```\n",
        html: r#"<span class="tok-keyword">fn</span>"#,
        documented_as: "36 languages",
        shown_as: "```python",
    },
    Mark {
        source: "a claim[@vaswani2017]",
        html: r#"<span class="mz-cite">[1]</span>"#,
        documented_as: "`[@key]`",
        shown_as: "[@vaswani2017]",
    },
    Mark {
        // A hosted video is an image reference like any other — what it becomes
        // follows from what it points at — and it was the third failure in this
        // file's list exactly: it rendered, it was documented, and no deck
        // showed one. So `mirzam check` never laid a player frame out, and the
        // frame clipped its own bottom edge on every deck that had one.
        source: "![talk](https://www.youtube.com/watch?v=dQw4w9WgXcQ)",
        html: r#"src="https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ""#,
        documented_as: "`youtube-nocookie.com`",
        shown_as: "https://www.youtube.com/watch?v=",
    },
    Mark {
        // The timestamp a share link carries. It used to be read for the id and
        // dropped, so "start at 1:30" silently became "start at the beginning".
        source: "![talk](https://youtu.be/dQw4w9WgXcQ?t=90)",
        html: "start=90",
        documented_as: "`{start=1m30s}`",
        shown_as: "?t=33",
    },
    Mark {
        // A recording was the third thing in this file's own list of failures:
        // it rendered, it was documented, and no deck had one.
        source: "![Interview](media/talk.mp3)",
        html: r#"<audio src="media/talk.mp3""#,
        documented_as: "becomes a player with the alt text as its label",
        shown_as: "](media/chime.wav)",
    },
];

/// The `.bib` the citation mark is rendered against, so the row above is
/// checked the whole way through rather than up to the marker.
const REFS: &str = "@inproceedings{vaswani2017, author={Vaswani, Ashish and \
                    Shazeer, Noam}, title={Attention Is All You Need}, \
                    booktitle={NeurIPS}, year={2017}}";

/// One fragment through the path a slide takes: citations marked, Markdown
/// preprocessed and rendered, then the deck pass that numbers what was cited.
///
/// A citation cannot be checked any other way — its mark is a number nothing
/// on the slide knows — and running every mark through the same function keeps
/// this list honest about the pipeline rather than about one half of it.
fn render(source: &str) -> String {
    let meta = mirzam_core::parse_meta("bibliography: refs.bib").expect("frontmatter");
    let (bib, _) = mirzam_render::deck_bibliography(&meta, |_| Ok(REFS.to_string()));
    let mut sections = vec![render_markdown(&preprocess(
        &mirzam_render::mark_citations(source),
    ))];
    mirzam_render::resolve_citations(&mut sections, &bib, mirzam_render::CiteStyle::Numeric);
    sections.remove(0)
}

/// A mark that does not render is not a feature, whatever the docs say.
#[test]
fn every_listed_mark_renders() {
    for m in MARKS {
        let out = render(m.source);
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

/// The flags a media reference takes, held to two of the three conditions: the
/// renderer's own tests check what each one emits, but a flag no deck uses is
/// one nobody has ever watched work. `autoplay` is the case in point — it meant
/// "when the deck loads" for three releases, which a single sample slide would
/// have caught the first time anyone turned a page.
const MEDIA_FLAGS: &[&str] = &["autoplay", "loop", "muted", "cover"];

#[test]
fn every_media_flag_is_documented_and_shown() {
    let doc = std::fs::read_to_string(repo_root().join("docs/syntax.md")).expect("docs/syntax.md");
    let decks: String = std::fs::read_dir(repo_root().join("examples"))
        .expect("examples/")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();
    for flag in MEDIA_FLAGS {
        // `cover=` is written with its value, the rest as classes.
        let (in_doc, in_deck) = if *flag == "cover" {
            (format!("`{{{flag}="), format!("{{{flag}="))
        } else {
            (format!("`{{.{flag}}}`"), format!(".{flag}"))
        };
        assert!(
            doc.contains(&in_doc),
            "docs/syntax.md never mentions {in_doc}"
        );
        assert!(
            decks.contains(&in_deck),
            "no deck under examples/ uses {in_deck}"
        );
    }
}

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

/// Every presentation dial, held to the pattern that makes it a dial at all.
///
/// A default written straight onto the element would beat the value a pane or a
/// theme sets, so each is read as a `var()` fallback instead. The difference is
/// invisible until somebody tries to override one, which is exactly the kind of
/// thing that rots — the term-list dials were declared on `dl` first, and
/// nothing about the rendering said so.
#[test]
fn every_presentation_dial_is_read_with_its_default_as_a_fallback() {
    let css = std::fs::read_to_string(repo_root().join("crates/mirzam-render/src/theme/base.css"))
        .expect("base.css");
    for (dial, default) in [
        ("--mz-terms-hang", "2em"),
        ("--mz-terms-gap", ".6em"),
        ("--mz-terms-col", "38%"),
        ("--mz-bullet", "disc"),
        ("--mz-bullet-2", "circle"),
        ("--mz-bullet-3", "square"),
        ("--mz-number", "decimal"),
        ("--mz-number-2", "decimal"),
        ("--mz-number-3", "decimal"),
        ("--mz-marker", "currentColor"),
        ("--mz-bib-size", "1.05em"),
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
