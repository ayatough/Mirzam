//! Syntax highlighting for fenced code blocks, at build time.
//!
//! Highlighting happens while the deck is assembled, so the finished HTML
//! carries `<span class="tok-…">` runs and no highlighter ships to the
//! browser. That is the whole point of doing it here: a deck stays one
//! self-contained file with no client-side JavaScript, and the same output
//! goes to the PDF exporter, which never runs a script at all.
//!
//! Two rules shape everything below, both from [W20]:
//!
//! - **A language nobody recognises stays exactly as it was.** The fallback is
//!   not "highlight badly", it is byte-identical to the output before this
//!   module existed — which is also what the CommonMark-compat rule demands of
//!   a fence carrying a Mirzam block kind such as `shape` or `chart`.
//! - **Colors are theme tokens, never the highlighter's palette.** This module
//!   emits class names and nothing else; `--mz-code-*` in the theme decides
//!   what they look like, so Nord code reads Nord.
//!
//! [W20]: ../../../docs/workstreams.md#w20--syntax-highlighting-at-build-time

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

/// Synoptic expands a tab to this many spaces when it hands a line back. Any
/// block where that changes the text is not highlighted at all (see
/// [`highlight`]), so the number only decides how a tab is *measured*, never
/// what reaches the page.
const TAB_WIDTH: usize = 4;

/// The class every highlighted run carries, without the `tok-` prefix.
///
/// Six is the whole set on purpose. Synoptic names about thirty token kinds
/// across its grammars, but a slide is read from the back of a room: a palette
/// with thirty entries is a palette nobody can tell apart at that distance,
/// and every entry is another color a theme has to define and keep legible.
/// [`class_for`] folds the thirty onto these six.
pub const TOKEN_CLASSES: &[&str] = &[
    "keyword", "string", "comment", "function", "number", "operator",
];

/// Info-string language names Mirzam highlights, and the file extension
/// synoptic knows them by.
///
/// This table is the *only* thing that decides whether a block is highlighted.
/// `synoptic::from_extension` never returns `None` — an extension it does not
/// know yields an empty highlighter that silently colors nothing — so asking
/// it "do you know this language?" cannot be done by calling it. Listing the
/// names here also lets a deck write `c++` or `golang` or `zsh` and be
/// understood, which an extension table alone would not.
///
/// Names are matched case-insensitively. A name that is not here renders
/// plain, which is the documented behaviour rather than a gap.
const LANGUAGES: &[(&str, &str)] = &[
    ("asm", "asm"),
    ("assembly", "asm"),
    ("nasm", "asm"),
    ("bash", "sh"),
    ("c", "c"),
    ("c#", "cs"),
    ("c++", "cpp"),
    ("cc", "cpp"),
    ("console", "sh"),
    ("cpp", "cpp"),
    ("cs", "cs"),
    ("csharp", "cs"),
    ("css", "css"),
    ("csv", "csv"),
    ("cxx", "cpp"),
    ("dart", "dart"),
    ("diff", "diff"),
    ("go", "go"),
    ("golang", "go"),
    ("haskell", "hs"),
    ("hs", "hs"),
    ("htm", "html"),
    ("html", "html"),
    ("java", "java"),
    ("javascript", "js"),
    ("js", "js"),
    ("json", "json"),
    ("jsx", "js"),
    ("kotlin", "kt"),
    ("kt", "kt"),
    ("lua", "lua"),
    ("m", "m"),
    ("markdown", "md"),
    ("matlab", "m"),
    ("md", "md"),
    ("nu", "nu"),
    ("nushell", "nu"),
    ("objective-c", "m"),
    ("octave", "m"),
    ("patch", "diff"),
    ("perl", "pm"),
    ("php", "php"),
    ("prolog", "prolog"),
    ("py", "py"),
    ("python", "py"),
    ("python3", "py"),
    ("r", "r"),
    ("rb", "rb"),
    ("ruby", "rb"),
    ("rs", "rs"),
    ("rust", "rs"),
    ("scala", "scala"),
    ("sh", "sh"),
    ("shell", "sh"),
    ("sql", "sql"),
    ("swift", "swift"),
    ("tex", "tex"),
    ("latex", "tex"),
    ("toml", "toml"),
    ("ts", "ts"),
    ("tsx", "ts"),
    ("typescript", "ts"),
    ("vb", "vb"),
    ("vbnet", "vb"),
    ("xml", "xml"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("zsh", "sh"),
];

/// How many distinct grammars the table above can reach — the number
/// `docs/syntax.md` quotes, computed rather than remembered.
#[cfg(test)]
fn grammar_count() -> usize {
    let mut exts: Vec<&str> = LANGUAGES.iter().map(|(_, ext)| *ext).collect();
    exts.sort_unstable();
    exts.dedup();
    exts.len()
}

/// Folds synoptic's token kind onto one of [`TOKEN_CLASSES`].
///
/// `None` means "emit this run as plain text": a kind nobody mapped is a kind
/// nobody chose a color for, and inventing a class here would put an unstyled
/// span on the slide instead of leaving the text alone.
fn class_for(kind: &str) -> Option<&'static str> {
    Some(match kind {
        // Reserved words and the things that read like them: a macro call, an
        // HTML tag name, a `#[derive]`, a Markdown fenced block marker.
        "keyword" | "macro" | "tag" | "attribute" | "block" => "keyword",
        // Anything quoted, plus the character and URL literals that behave
        // like short strings.
        "string" | "character" | "link" | "image" => "string",
        // Comments, and the block quote that plays the same role in Markdown.
        "comment" | "quote" => "comment",
        // Names that a reader scans for: calls, types, namespaces, a YAML or
        // TOML key, a Markdown heading.
        "function" | "struct" | "namespace" | "key" | "header" | "heading" => "function",
        // Literal values. `boolean` and `reference` sit here rather than with
        // keywords because they are constants, not control flow.
        "digit" | "boolean" | "reference" | "math" => "number",
        // Punctuation, and the markup characters that play its part.
        "operator" | "table" | "bold" | "italic" | "strikethrough" | "insertion" | "deletion" => {
            "operator"
        }
        // Deliberately unmapped, though synoptic offers them: `list` and
        // `linebreak` match a leading `-`, `+` or `---` in Markdown, and these
        // decks are full of ```markdown fences holding ASCII pane drawings.
        // Colouring those turns every corner of a `+------+` grid into a
        // speckle. Anything not named above falls here too and renders as
        // plain text, which is the right default: a kind nobody mapped is a
        // kind nobody picked a colour for.
        _ => return None,
    })
}

/// Highlights `code` as `lang`, or `None` if the language is not one we
/// highlight.
///
/// The returned string is escaped HTML ready to sit inside `<code>`. Escaping
/// happens here and only here: synoptic tokenizes the *raw* text, so a `<` in
/// a string literal is one character to the grammar and `&lt;` on the page,
/// and nothing is ever escaped twice.
pub fn highlight(lang: &str, code: &str) -> Option<String> {
    let name = lang.trim().to_ascii_lowercase();
    let ext = LANGUAGES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, ext)| *ext)?;
    // `split('\n')` and a `'\n'` between the pieces is an exact round trip,
    // including the empty last piece a trailing newline leaves behind.
    let lines: Vec<String> = code.split('\n').map(str::to_string).collect();
    HIGHLIGHTERS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let hl = match cache.entry(ext) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(synoptic::from_extension(ext, TAB_WIDTH)?),
        };
        hl.run(&lines);
        Some(render(hl, &lines, code))
    })?
}

thread_local! {
    /// One highlighter per language, kept for the life of the process.
    ///
    /// Building one per fence is what the obvious code does, and it costs about
    /// **0.29 ms every time** — six times what the highlighting itself costs. The
    /// clone `from_extension` hands back is cheap; what is not is the first
    /// `run` through it, because each fresh copy of a grammar's regular
    /// expressions starts with a cold engine cache and pays to warm it again. A
    /// hundred fences paid it a hundred times.
    ///
    /// Reuse is exact rather than nearly exact: `run` rebuilds the atom table from
    /// the lines it is given, and the tokenizer resets the token list, the line
    /// index and its own state before it starts. `highlighting_is_the_same_cached_or_not`
    /// holds that to a sequence of unlike fences through one highlighter, because
    /// this is the change that turns a pure function into one carrying state for
    /// as long as `mirzam serve` or the browser editor is open — where a reset
    /// somebody stopped doing would show up as colours that drift while you type,
    /// and never in a one-shot build.
    ///
    /// Thread-local rather than shared: a highlighter has to be `&mut` to run, and
    /// a lock around it would serialise every fence in a deck for no gain.
    static HIGHLIGHTERS: RefCell<HashMap<&'static str, synoptic::Highlighter>> =
        RefCell::new(HashMap::new());
}

/// The HTML for `lines`, from a highlighter that has just run over them, or
/// `None`-worthy plain text if what it says the code was is not what it is.
fn render(hl: &synoptic::Highlighter, lines: &[String], code: &str) -> Option<String> {
    let mut out = String::with_capacity(code.len() * 2);
    // What the highlighter says the text was. Synoptic replaces a tab with
    // spaces before it hands a line back, and a grammar with a greedy regex
    // could in principle drop or reorder a run; either way the slide would
    // show code the author did not write. Rebuilding the plain text as we go
    // and refusing to use the result unless it matches makes that a fallback
    // to today's rendering rather than a corrupted listing.
    let mut plain = String::with_capacity(code.len());
    for (y, line) in lines.iter().enumerate() {
        if y > 0 {
            out.push('\n');
            plain.push('\n');
        }
        for tok in hl.line(y, line) {
            let (text, class) = match &tok {
                synoptic::TokOpt::Some(text, kind) => (text, class_for(kind)),
                synoptic::TokOpt::None(text) => (text, None),
            };
            plain.push_str(text);
            match class {
                Some(class) => {
                    out.push_str("<span class=\"tok-");
                    out.push_str(class);
                    out.push_str("\">");
                    escape(&mut out, text);
                    out.push_str("</span>");
                }
                None => escape(&mut out, text),
            }
        }
    }
    (plain == *code).then_some(out)
}

/// Comrak's own escaping, used directly so an unhighlighted block is
/// byte-identical to what comrak would have written itself.
fn escape(out: &mut String, text: &str) {
    // Writing to a `String` cannot fail.
    let _ = comrak::html::escape(out, text);
}

/// The comrak plugin that puts [`highlight`] in the code-fence path.
///
/// Comrak hands the adapter the `<pre>` and `<code>` attributes it had already
/// worked out, so `language-rust` still lands on the `<code>` exactly as
/// before and only the contents change.
pub struct Highlighter;

impl comrak::adapters::SyntaxHighlighterAdapter for Highlighter {
    fn write_highlighted(
        &self,
        output: &mut dyn fmt::Write,
        lang: Option<&str>,
        code: &str,
    ) -> fmt::Result {
        match lang.and_then(|lang| highlight(lang, code)) {
            Some(html) => output.write_str(&html),
            None => comrak::html::escape(output, code),
        }
    }

    fn write_pre_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        comrak::html::write_opening_tag(output, "pre", attributes)
    }

    fn write_code_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        comrak::html::write_opening_tag(output, "code", attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_markdown;

    /// A fence highlighted the way it was before the highlighters were kept:
    /// a new one, used once, dropped. The reference the cache is held to.
    fn highlight_uncached(lang: &str, code: &str) -> Option<String> {
        let name = lang.trim().to_ascii_lowercase();
        let ext = LANGUAGES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, ext)| *ext)?;
        let mut hl = synoptic::from_extension(ext, TAB_WIDTH)?;
        let lines: Vec<String> = code.split('\n').map(str::to_string).collect();
        hl.run(&lines);
        render(&hl, &lines, code)
    }

    /// The test that pays for the highlighter cache.
    ///
    /// Keeping one highlighter per language for the life of the process is
    /// worth 0.29 ms a fence, and it is the one change here that lets a deck's
    /// output depend on what was rendered before it. Today it cannot: `run`
    /// rebuilds the atom table and the tokenizer resets everything it reads.
    /// This holds that to a run of unlike fences through one cache -
    /// interleaved languages, a comment-only fence, an empty one, a string
    /// carrying comment markers - against a highlighter built fresh for each.
    /// If a future synoptic stops resetting something, it fails here rather
    /// than as colours that drift while somebody types.
    #[test]
    fn highlighting_is_the_same_cached_or_not() {
        let fences = [
            ("rust", "fn main() {\n    let s = \"a // b /* c */\";\n}\n"),
            ("python", "def f(n):\n    return n * 2  # doubled\n"),
            ("rust", "// just a comment\n"),
            ("javascript", "const t = `x ${ y } z`;\n"),
            ("rust", ""),
            ("python", "s = 'text with a # inside'\nt = \"another\"\n"),
            ("rust", "fn main() {\n    let s = \"a // b /* c */\";\n}\n"),
            ("javascript", "// gone\nfoo(/re[/]gex/);\n"),
            (
                "rust",
                "struct S { a: u32 }\n\nimpl S {\n    fn a(&self) -> u32 { self.a }\n}\n",
            ),
        ];
        // Twice through, so every fence is also seen by a cache that has
        // already been used for something longer and something shorter.
        for round in 0..2 {
            for (lang, code) in fences {
                assert_eq!(
                    highlight(lang, code),
                    highlight_uncached(lang, code),
                    "round {round}: `{lang}` highlighted differently from a \
                     highlighter kept than from a fresh one"
                );
            }
        }
    }

    #[test]
    fn a_known_language_is_coloured() {
        let html = highlight("rust", "fn main() {}\n").expect("rust is known");
        assert!(
            html.contains("<span class=\"tok-keyword\">fn</span>"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"tok-function\">main</span>"),
            "{html}"
        );
    }

    #[test]
    fn language_names_are_case_insensitive_and_aliased() {
        for name in ["Rust", "RS", "rust"] {
            assert!(highlight(name, "let x = 1;\n").is_some(), "{name}");
        }
    }

    #[test]
    fn an_unknown_language_is_left_alone() {
        assert!(highlight("brainfuck", "+++\n").is_none());
        // The Mirzam block kinds that reach comrak as ordinary fences must
        // stay ordinary fences, or the CommonMark-compat promise breaks.
        // `mermaid` is on this list for a second reason: on a machine with no
        // renderer the fence *is* the code block, and a half-coloured diagram
        // source would read worse than the plain one GitHub draws.
        for kind in [
            "shape", "chart", "connect", "anim", "pane", "toc", "mermaid",
        ] {
            assert!(highlight(kind, "anything\n").is_none(), "{kind}");
        }
    }

    /// The whole fallback contract in one assertion: with no language, and
    /// with a language nobody knows, the bytes are what they were before
    /// highlighting existed.
    #[test]
    fn an_unhighlighted_block_renders_exactly_as_before() {
        for md in [
            "```\n<b> & \"quoted\"\n```\n",
            "```shape\nrect a & b <c>\n```\n",
            "    indented & <raw>\n",
        ] {
            let html = render_markdown(md);
            assert!(!html.contains("tok-"), "{html}");
            assert!(html.contains("&amp;"), "{html}");
        }
    }

    #[test]
    fn html_is_escaped_once_and_only_once() {
        let html = highlight("rust", "let s = \"a & b <c>\";\n").expect("rust is known");
        assert!(html.contains("&amp;"), "{html}");
        assert!(!html.contains("&amp;amp;"), "{html}");
        assert!(!html.contains("&amp;lt;"), "{html}");
        // The quotes bounding the string literal are part of its token, so
        // they are escaped inside the span rather than breaking out of it.
        assert!(html.contains("tok-string"), "{html}");
    }

    /// Whatever the grammar decides, the text on the slide is the text the
    /// author wrote. Tabs are the case that actually bites: synoptic expands
    /// them, so those blocks fall back instead of silently reflowing.
    #[test]
    fn a_tab_indented_block_falls_back_rather_than_being_rewritten() {
        assert!(highlight("rust", "fn main() {\n\tlet x = 1;\n}\n").is_none());
    }

    /// A fragment picked so that every grammar in the table finds *something*
    /// in it — a comment marker in three dialects, a quoted string, a comma
    /// for `csv`, a hunk header for `diff`, a tag for the markup languages.
    const PROBE: &str = concat!(
        "x = 1, 2 # a comment\n",
        "% another comment\n",
        "\"a string\" and 'c'\n",
        "+added line\n",
        "@@ -1 +1 @@\n",
        "<tag attr=\"v\">text</tag>\n",
    );

    #[test]
    fn every_language_maps_to_a_grammar_synoptic_has() {
        // `synoptic::from_extension` answers `Some` for anything, handing back
        // an empty highlighter for an extension it does not know — so a typo
        // in the table would silently produce a block that renders plain
        // while this module claims to have highlighted it.
        for (name, ext) in LANGUAGES {
            let coloured = highlight(name, PROBE);
            assert!(coloured.is_some(), "{name} produced nothing");
            assert!(
                coloured.unwrap().contains("tok-"),
                "`{name}` -> `{ext}` is not a grammar synoptic knows"
            );
        }
    }

    #[test]
    fn the_documented_language_count_is_the_real_one() {
        let doc = include_str!("../../../docs/syntax.md");
        assert!(
            doc.contains(&format!("{} languages", grammar_count())),
            "docs/syntax.md does not quote {} languages",
            grammar_count()
        );
    }

    #[test]
    fn the_classes_it_emits_are_the_ones_the_theme_styles() {
        let css = crate::theme::BASE_CSS;
        for class in TOKEN_CLASSES {
            assert!(
                css.contains(&format!(".tok-{class} {{")),
                "base.css does not style `.tok-{class}`"
            );
        }
    }

    #[test]
    fn a_fence_inside_a_slide_keeps_its_language_class() {
        let html = render_markdown("```python\ndef f():\n    return 1\n```\n");
        assert!(html.contains("<code class=\"language-python\">"), "{html}");
        assert!(html.contains("tok-keyword"), "{html}");
    }
}
