//! What a build warning is *about*, and *where* it is.
//!
//! A warning is a sentence written for a person, and three places now need
//! more than the sentence: `check --format json` gives an agent a stable kind
//! to branch on, `mirzam lsp` gives an editor a range to underline, and the
//! browser editor gives a line to jump to. All three have to agree, so the
//! rules live here rather than in whichever of them was written first.
//!
//! Neither answer is derived from a span the warning carries, because warnings
//! do not carry one: they are `String`s, built in half a dozen crates. Both
//! are read back out of the message itself. That is a real approximation and
//! is documented as one wherever it surfaces; threading spans through every
//! warning is the exact fix and a stream of its own.

/// A stable name for what a build warning is about.
///
/// The messages themselves are prose and are free to be reworded; this is the
/// part a program may branch on, so it is matched on the one distinctive token
/// each family of warnings carries. Order matters - the first match wins - and
/// anything unrecognised is `build.other` rather than a guess, which is also
/// what a warning added after this table gets until it is added here.
pub fn warning_kind(message: &str) -> &'static str {
    const TABLE: &[(&str, &str)] = &[
        // First, because this is the one message that quotes another program:
        // `mmdc` says "flowchart", which contains `chart`, and it is free to
        // say anything else on this list too. Classifying it before the table
        // can misread it is cheaper than teaching every other needle about a
        // tool Mirzam does not control.
        ("mermaid:", "build.mermaid"),
        // Then, and matched on two words: the message carries a filesystem
        // path, and a deck living under `charts/` must not be classified by
        // somebody's directory name.
        ("skill card", "build.skill"),
        ("shape line ", "build.shape"),
        ("shape:", "build.shape"),
        ("grid-pad", "build.layout"),
        ("grid-gap", "build.layout"),
        ("anim ", "build.anim"),
        ("cannot split", "build.anim"),
        ("a target is split", "build.anim"),
        ("annotate ", "build.annotate"),
        ("effects line ", "build.effects"),
        ("connect ", "build.connect"),
        ("each:", "build.each"),
        ("chart", "build.chart"),
        ("footnote reference", "build.footnote"),
        ("toc:", "build.toc"),
        ("bibliography", "build.bibliography"),
        ("citations:", "build.bibliography"),
        ("masters:", "build.master"),
        ("master ", "build.master"),
        ("is not in the layout", "build.layout"),
        ("pane block", "build.layout"),
        ("merged region", "build.layout"),
        ("bg-light", "build.layout"),
        ("bg-dark", "build.layout"),
        ("is still on the slide as text", "build.span"),
        ("the brace over", "build.math"),
        ("math:", "build.math"),
        ("unknown theme", "build.theme"),
        // `theme: default` is an unknown name that gets its own wording, so it
        // needs its own needle or it would classify as `build.other`.
        ("no longer a theme name", "build.theme"),
        ("unknown mode", "build.theme"),
        // The stem rule, reported against the slide or pane that named a
        // theme file which cannot answer to a name.
        ("file theme is usable", "build.theme"),
        // A stylesheet the deck named and the host could not read.
        ("theme: cannot read", "build.css"),
        // Everything else a theme file has to say about itself: a stem that
        // collides with a built-in, one palette where two are needed, text
        // that cannot be read on its own background.
        ("theme: `", "build.theme"),
        ("transition:", "build.transition"),
        ("autoplay:", "build.autoplay"),
        ("no slides:", "build.deck"),
        ("<!-- next -->", "build.continuation"),
        ("file not found", "build.asset"),
        ("not inlined", "build.asset"),
    ];
    TABLE
        .iter()
        .find(|(needle, _)| message.contains(needle))
        .map_or("build.other", |(_, kind)| *kind)
}

/// The byte range a warning is about, within `text`.
///
/// A warning knows its slide, not its token — but it *quotes* what is wrong:
/// "`[@wilson2021]` is in no bibliography entry", "pane `fig` is not in the
/// layout", "unknown theme `nord2`". So the first backticked word in the
/// message is looked for between `from` and `to`, and that is the range. The
/// first occurrence wins, and a message that quotes nothing falls back to the
/// first line of its slide, which is still better than underlining the slide.
pub fn locate(text: &str, from: usize, to: usize, message: &str) -> (usize, usize) {
    let from = from.min(text.len());
    let to = to.clamp(from, text.len());
    if let Some(token) = quoted(message) {
        if let Some(at) = text.get(from..to).and_then(|window| window.find(token)) {
            return (from + at, from + at + token.len());
        }
        // A warning that belongs to no slide — the frontmatter's — searches the
        // whole document, which is the same search over a wider window:
        // `theme:` names its bad value, and the value is up there in the YAML.
        if from == 0 {
            if let Some(at) = text.find(token) {
                return (at, at + token.len());
            }
        }
    }
    let line = text[..from].rfind('\n').map_or(0, |nl| nl + 1);
    let start = line + text[line..].len() - text[line..].trim_start().len();
    (start, line_end(text, start))
}

/// The first backticked run in a warning message, when it holds one worth
/// looking for. A single character is not: `` `D` `` is a key, and finding a
/// `D` somewhere on the slide points at nothing.
pub fn quoted(message: &str) -> Option<&str> {
    let after = message.split_once('`')?.1;
    let token = after.split_once('`')?.0;
    (token.len() > 1 && !token.contains('\n')).then_some(token)
}

/// The rendered slide number a message opens with, as `slide 4: …` or
/// `slide 4, pane \`x\`: …`. `None` for a warning that belongs to the deck
/// rather than to one of its slides.
pub fn slide_number(message: &str) -> Option<usize> {
    let rest = message.strip_prefix("slide ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Where the line containing `from` ends.
pub fn line_end(text: &str, from: usize) -> usize {
    text[from.min(text.len())..]
        .find('\n')
        .map_or(text.len(), |nl| from + nl)
}

/// A byte offset as an editor counts one: a zero-based line, and a column in
/// UTF-16 code units — which is what both the Language Server Protocol and a
/// browser's `textarea` count, and what makes a mark on a line of Japanese
/// land on the word rather than to the left of it.
pub fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let upto = &text[..offset.min(text.len())];
    let line = upto.matches('\n').count();
    let column = upto
        .rfind('\n')
        .map_or(upto, |nl| &upto[nl + 1..])
        .chars()
        .map(char::len_utf16)
        .sum();
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_of_build_warning_has_its_own_kind() {
        for (message, kind) in [
            ("mermaid: no diagram renderer found", "build.mermaid"),
            ("slide 2: pane `fig` is not in the layout", "build.layout"),
            ("unknown theme `nord2`; using `mirzam`", "build.theme"),
            (
                "slide 1: `[@a2020]` is in no bibliography entry",
                "build.bibliography",
            ),
            ("math: unknown dialect `klingon`", "build.math"),
            ("each: cannot read rows.csv", "build.each"),
        ] {
            assert_eq!(warning_kind(message), kind, "for: {message}");
        }
    }

    #[test]
    fn an_unfamiliar_warning_is_named_rather_than_guessed() {
        assert_eq!(
            warning_kind("something nobody has classified yet"),
            "build.other"
        );
    }

    #[test]
    fn a_single_character_is_not_a_token_worth_hunting() {
        assert_eq!(quoted("press `D` to flip the deck"), None);
        assert_eq!(quoted("pane `fig` is not in the layout"), Some("fig"));
        assert_eq!(quoted("nothing here"), None);
    }

    #[test]
    fn the_range_is_the_word_the_message_quotes() {
        let text = "---\ntheme: nosuchtheme\n---\n\n# One\n";
        let (start, end) = locate(text, 0, text.len(), "unknown theme `nosuchtheme`");
        assert_eq!(&text[start..end], "nosuchtheme");
    }

    #[test]
    fn a_message_quoting_nothing_falls_back_to_the_line_it_starts_on() {
        let text = "one\n  two\nthree\n";
        assert_eq!(locate(text, 6, text.len(), "nothing quoted here"), (6, 9));
    }

    /// The window matters: the same name on two slides is two different
    /// mistakes, and the mark belongs on the one the warning is about.
    #[test]
    fn the_search_stays_inside_the_slide_it_was_given() {
        let text = "::: pane fig\nfirst\n:::\n---\n::: pane fig\nsecond\n:::\n";
        let second = text.find("---").expect("a rule") + 4;
        let (start, _) = locate(
            text,
            second,
            text.len(),
            "slide 2: pane `fig` is not in the layout",
        );
        assert!(start > second, "the mark landed on the first slide");
    }

    #[test]
    fn a_slide_number_is_read_from_either_way_a_message_opens() {
        assert_eq!(
            slide_number("slide 4: pane `x` is not in the layout"),
            Some(4)
        );
        assert_eq!(slide_number("slide 12, pane `x`: something"), Some(12));
        assert_eq!(slide_number("unknown theme `nord2`"), None);
        assert_eq!(slide_number("slides are not numbered here"), None);
    }

    #[test]
    fn a_column_is_utf16_code_units_not_bytes() {
        let text = "日本語のスライド `nosuchpane` です\n";
        assert_eq!(line_column(text, 0), (0, 0));
        assert_eq!(line_column(text, "日本語のスライド ".len()), (0, 9));
    }
}
