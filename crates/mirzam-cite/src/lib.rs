//! Bibliography entries: BibTeX in, a citation label and a formatted
//! reference out.
//!
//! This crate knows nothing about slides. It answers two questions about one
//! reference — *what does the mark in the text say* and *what does the line in
//! the reference list read like* — and leaves where either lands to
//! `mirzam-render`.
//!
//! The parser is deliberately forgiving. A `.bib` file is usually exported by
//! a reference manager and never read by its owner, so refusing to load one
//! over a field Mirzam does not use would fail a deck for a reason its author
//! cannot see. Anything unrecognised is skipped with a warning; anything
//! recognised is kept.

use std::collections::BTreeMap;

/// How a citation is written where the claim is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CiteStyle {
    /// `[1]`, numbered in order of first citation. Narrow, so it costs a slide
    /// almost nothing.
    #[default]
    Numeric,
    /// `[Vaswani+17]`, which says whose paper it is without a trip to the back
    /// of the deck. Wider, and worth it in a seminar where the audience knows
    /// the names.
    Author,
}

impl CiteStyle {
    pub fn parse(src: &str) -> Result<Self, String> {
        match src.trim() {
            "numeric" | "number" | "numbered" => Ok(CiteStyle::Numeric),
            "author" | "author-year" | "alpha" => Ok(CiteStyle::Author),
            other => Err(format!(
                "citation-style: `{other}` is not a style; use `numeric` or `author`"
            )),
        }
    }
}

/// One reference: its citation key, its BibTeX entry type, and its fields with
/// their names lowercased.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    /// `article`, `inproceedings`, … lowercased. Empty for an entry written in
    /// frontmatter, which has no type to give.
    pub kind: String,
    pub fields: BTreeMap<String, String>,
}

/// Every reference a deck can cite, by key.
pub type Bibliography = BTreeMap<String, Entry>;

impl Entry {
    /// An entry assembled from name/value pairs rather than parsed, which is
    /// how a bibliography written straight into frontmatter arrives.
    pub fn from_fields(key: &str, fields: BTreeMap<String, String>) -> Self {
        Entry {
            key: key.to_string(),
            kind: String::new(),
            fields: fields
                .into_iter()
                .map(|(k, v)| (k.trim().to_lowercase(), clean(&v)))
                .collect(),
        }
    }

    /// The first of `names` this entry has, non-empty.
    pub fn field(&self, names: &[&str]) -> Option<&str> {
        names
            .iter()
            .find_map(|n| self.fields.get(*n))
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    /// What `[@key]` reads as under [`CiteStyle::Author`]: `Vaswani+17`.
    ///
    /// The `+` stands in for every author after the first, which is the form a
    /// slide has room for — `Vaswani, Shazeer, Parmar, Uszkoreit et al. 2017`
    /// is a reference list entry, not a mark inside a sentence. An entry with
    /// no author falls back to its own key, since that is the only name it has.
    pub fn label(&self) -> String {
        let (names, truncated) = self.surnames();
        let stem = match names.first() {
            Some(n) => n.clone(),
            None => return self.key.clone(),
        };
        let more = truncated || names.len() > 1;
        format!(
            "{stem}{}{}",
            if more { "+" } else { "" },
            self.short_year().unwrap_or_default()
        )
    }

    /// Volume, issue and pages as a reference list writes them: `29(5),
    /// 502–528`. Empty when the entry carries none, which is most of the
    /// preprints and all of the software.
    fn locator(&self) -> String {
        let mut out = match (self.field(&["volume"]), self.field(&["number", "issue"])) {
            (Some(v), Some(n)) => format!("{}({})", escape(v), escape(n)),
            (Some(v), None) => escape(v),
            // An issue with no volume is rare enough that the bare number
            // would read as one.
            (None, Some(n)) => format!("no. {}", escape(n)),
            (None, None) => String::new(),
        };
        if let Some(p) = self.field(&["pages"]) {
            if !out.is_empty() {
                out.push_str(", ");
            }
            // BibTeX spells a range `502--528`; a slide sets it as one dash.
            out.push_str(&escape(&p.replace("--", "–")));
        }
        out
    }

    /// The reference as it reads in the list: authors, title, where it
    /// appeared, and a link to it.
    ///
    /// HTML rather than Markdown because this is substituted into a slide that
    /// has already been rendered — the reference list is resolved once the
    /// whole deck exists, long after any Markdown parse.
    pub fn html(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(a) = self.author_display() {
            parts.push(escape(&a));
        }
        if let Some(t) = self.field(&["title"]) {
            parts.push(format!("<em>{}</em>", escape(t)));
        }
        let container = self.field(&[
            "journal",
            "booktitle",
            "venue",
            "publisher",
            "school",
            "institution",
            "howpublished",
            "series",
        ]);
        // Where it appeared reads as one clause: venue, locators, year.
        // "NeurIPS. 2017." is two sentences for one fact, and the locators
        // belong with the venue they locate — `IJRR 29(5), 502–528, 2010` is
        // how a reader finds the paper, where `IJRR, 2010` only lets them
        // recognise one they already know. A talk whose references are a
        // reading list needs the first.
        let mut clause = container.map(escape).unwrap_or_default();
        let locator = self.locator();
        if !locator.is_empty() {
            if !clause.is_empty() {
                clause.push(' ');
            }
            clause.push_str(&locator);
        }
        if let Some(y) = self.year() {
            if !clause.is_empty() {
                clause.push_str(", ");
            }
            clause.push_str(&escape(y));
        }
        if !clause.is_empty() {
            parts.push(clause);
        }
        // Clauses are separated by a full stop, and `et al.` already ends in
        // one — without this, four authors print `Vaswani, Shazeer, Parmar et
        // al.. <em>Attention…`.
        let mut out = String::new();
        for part in parts {
            if !out.is_empty() {
                if !out.ends_with('.') {
                    out.push('.');
                }
                out.push(' ');
            }
            out.push_str(&part);
        }
        if !out.is_empty() && !out.ends_with('.') {
            out.push('.');
        }
        if let Some((text, href)) = self.link() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape(&href),
                escape(&text)
            ));
        }
        // An entry with nothing in it at all still has to show something, or
        // the list has a blank row nobody can trace back to a key.
        if out.is_empty() {
            out = escape(&self.key);
        }
        out
    }

    /// Authors as a reference list writes them: surnames only, at most three,
    /// then `et al.`
    ///
    /// Surnames rather than the field as written, because a `.bib` exported by
    /// a reference manager spells them `Vaswani, Ashish and Shazeer, Noam and …`
    /// — correct, and unreadable at the foot of a slide.
    ///
    /// Separated by commas and never by `and`, which is an English word: a
    /// Japanese entry rendered `山田 and 鈴木`, one conjunction of the wrong
    /// language in the middle of an otherwise Japanese line. A comma is what
    /// every numeric style uses anyway, and it is the same in both.
    fn author_display(&self) -> Option<String> {
        let (names, truncated) = self.surnames();
        let (shown, _) = names.split_at(names.len().min(3));
        if shown.is_empty() {
            return None;
        }
        let cut = truncated || names.len() > 3;
        Some(format!(
            "{}{}",
            shown.join(", "),
            if cut { " et al." } else { "" }
        ))
    }

    /// Every author's surname, and whether the list was cut short — either by
    /// the field saying `et al.` itself or by this returning what it could.
    fn surnames(&self) -> (Vec<String>, bool) {
        let Some(field) = self.field(&["author", "editor"]) else {
            return (Vec::new(), false);
        };
        // `Vaswani et al.` is already a summary. Keep the names in front of it
        // and remember that it is one, rather than reading `al.` as a surname.
        let (list, truncated) = match find_et_al(field) {
            Some(head) => (head, true),
            None => (field, false),
        };
        let names: Vec<String> = split_authors(list)
            .iter()
            .map(|n| surname(n))
            .filter(|n| !n.is_empty())
            .collect();
        (names, truncated)
    }

    fn year(&self) -> Option<&str> {
        self.field(&["year", "date"])
    }

    /// The last two digits of the year, for a label.
    fn short_year(&self) -> Option<String> {
        let year = self.year()?;
        let digits: String = year
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();
        (digits.len() >= 2).then(|| digits[digits.len() - 2..].to_string())
    }

    /// Where to read it: the DOI if there is one, else the URL, else the arXiv
    /// identifier. Returns the text to show and the address to go to.
    fn link(&self) -> Option<(String, String)> {
        if let Some(doi) = self.field(&["doi"]) {
            let bare = doi
                .trim_start_matches("https://doi.org/")
                .trim_start_matches("http://doi.org/")
                .trim_start_matches("doi:")
                .trim();
            return Some((format!("doi:{bare}"), format!("https://doi.org/{bare}")));
        }
        if let Some(url) = self
            .field(&["url", "howpublished"])
            .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
        {
            return Some((url.to_string(), url.to_string()));
        }
        let eprint = self.field(&["eprint", "arxiv"])?;
        Some((
            format!("arXiv:{eprint}"),
            format!("https://arxiv.org/abs/{eprint}"),
        ))
    }
}

/// Parses a `.bib` file into entries, with a warning for anything skipped.
///
/// `@string`, `@preamble` and `@comment` are read past rather than expanded: a
/// deck citing a paper does not need string macros, and silently dropping the
/// block is better than mistaking its body for an entry.
pub fn parse_bibtex(src: &str) -> (Bibliography, Vec<String>) {
    let cs: Vec<char> = src.chars().collect();
    let mut out: Bibliography = BTreeMap::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut i = 0;

    while i < cs.len() {
        if cs[i] != '@' {
            i += 1;
            continue;
        }
        i += 1;
        let kind = take_while(&cs, &mut i, |c| c.is_alphanumeric() || c == '_').to_lowercase();
        skip_space(&cs, &mut i);
        let close = match cs.get(i) {
            Some('{') => '}',
            Some('(') => ')',
            // `@` in an email address or a `\@` in prose: not an entry at all.
            _ => continue,
        };
        i += 1;
        if matches!(kind.as_str(), "comment" | "preamble" | "string") {
            skip_to_close(&cs, &mut i, close);
            continue;
        }
        let key = take_while(&cs, &mut i, |c| c != ',' && c != close && c != '\n')
            .trim()
            .to_string();
        if key.is_empty() {
            warnings.push(format!("a `@{kind}` entry has no citation key; skipped"));
            skip_to_close(&cs, &mut i, close);
            continue;
        }
        let mut fields: BTreeMap<String, String> = BTreeMap::new();
        loop {
            skip_space(&cs, &mut i);
            match cs.get(i) {
                None => break,
                Some(c) if *c == close => {
                    i += 1;
                    break;
                }
                Some(',') => {
                    i += 1;
                    continue;
                }
                _ => {}
            }
            let name = take_while(&cs, &mut i, |c| {
                c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
            })
            .trim()
            .to_lowercase();
            skip_space(&cs, &mut i);
            if cs.get(i) != Some(&'=') {
                // Not a `name = value` pair. Skipping to the next comma keeps
                // the rest of the entry rather than losing it to one bad line.
                if name.is_empty() {
                    i += 1;
                }
                skip_to_separator(&cs, &mut i, close);
                continue;
            }
            i += 1;
            let value = read_value(&cs, &mut i, close);
            if !name.is_empty() {
                fields.insert(name, clean(&value));
            }
        }
        if out
            .insert(
                key.clone(),
                Entry {
                    key: key.clone(),
                    kind,
                    fields,
                },
            )
            .is_some()
        {
            // The second definition wins, which is what a file assembled by
            // concatenation usually means — but a key that means two papers is
            // a citation pointing at the wrong one, so say so.
            warnings.push(format!(
                "`{key}` is defined twice; the later entry is the one cited"
            ));
        }
    }
    (out, warnings)
}

/// A field's value: `{braced}`, `"quoted"`, a bare token, or several of those
/// joined with `#`.
fn read_value(cs: &[char], i: &mut usize, close: char) -> String {
    let mut out = String::new();
    loop {
        skip_space(cs, i);
        match cs.get(*i) {
            Some('{') => {
                *i += 1;
                let mut depth = 1usize;
                while *i < cs.len() {
                    match cs[*i] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                *i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    if depth > 0 {
                        out.push(cs[*i]);
                    }
                    *i += 1;
                }
            }
            Some('"') => {
                *i += 1;
                let mut depth = 0usize;
                while *i < cs.len() {
                    match cs[*i] {
                        '{' => depth += 1,
                        '}' => depth = depth.saturating_sub(1),
                        // A `"` inside braces is part of the text, not the end
                        // of the value.
                        '"' if depth == 0 => {
                            *i += 1;
                            break;
                        }
                        _ => {}
                    }
                    out.push(cs[*i]);
                    *i += 1;
                }
            }
            // A bare number, or a `@string` macro name we cannot expand: keep
            // the token, which is right for the number and readable for the
            // macro.
            Some(c) if *c != ',' && *c != close => {
                out.push_str(&take_while(cs, i, |c| {
                    c != ',' && c != close && c != '#' && !c.is_whitespace()
                }));
            }
            _ => {}
        }
        skip_space(cs, i);
        if cs.get(*i) == Some(&'#') {
            *i += 1;
            continue;
        }
        return out;
    }
}

fn take_while(cs: &[char], i: &mut usize, f: impl Fn(char) -> bool) -> String {
    let start = *i;
    while *i < cs.len() && f(cs[*i]) {
        *i += 1;
    }
    cs[start..*i].iter().collect()
}

fn skip_space(cs: &[char], i: &mut usize) {
    while *i < cs.len() && cs[*i].is_whitespace() {
        *i += 1;
    }
}

/// Past the end of the current entry, counting braces so a `}` inside a value
/// does not end it early.
fn skip_to_close(cs: &[char], i: &mut usize, close: char) {
    let mut depth = 0usize;
    while *i < cs.len() {
        match cs[*i] {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            c if c == close && depth == 0 => {
                *i += 1;
                return;
            }
            _ => {}
        }
        *i += 1;
    }
}

/// To the next field boundary, so one malformed line costs one field.
fn skip_to_separator(cs: &[char], i: &mut usize, close: char) {
    let mut depth = 0usize;
    while *i < cs.len() {
        match cs[*i] {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            ',' if depth == 0 => {
                *i += 1;
                return;
            }
            c if c == close && depth == 0 => return,
            _ => {}
        }
        *i += 1;
    }
}

/// A field value as it should read: one line, no case-protecting braces, and
/// the handful of LaTeX escapes a bibliography actually contains.
fn clean(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // `{BERT}` protects capitals for BibTeX's own case folding. Mirzam
            // never folds case, so the braces are noise.
            '{' | '}' => {}
            // A tie is a space that must not break; a slide's line breaking is
            // the browser's business either way.
            '~' => out.push(' '),
            '\\' => match chars.peek() {
                Some(&e @ ('&' | '%' | '$' | '#' | '_' | '{' | '}')) => {
                    out.push(e);
                    chars.next();
                }
                _ => out.push('\\'),
            },
            c => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The names in front of an `et al.`, when the field carries one.
fn find_et_al(field: &str) -> Option<&str> {
    let lower = field.to_lowercase();
    let at = lower.find("et al")?;
    Some(field[..at].trim().trim_end_matches(',').trim())
}

/// Splits on BibTeX's ` and ` separator, which is a word and not a character:
/// `Sanders and Sons` is one publisher, and `A and B` is two people.
fn split_authors(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = list;
    while let Some(at) = rest.find(" and ") {
        out.push(rest[..at].trim().to_string());
        rest = &rest[at + " and ".len()..];
    }
    out.push(rest.trim().to_string());
    out.retain(|n| !n.is_empty());
    out
}

/// One author's surname. `Vaswani, Ashish` and `Ashish Vaswani` are the two
/// spellings BibTeX allows; a name written as one word (which is every
/// Japanese name, and some others) is its own surname.
fn surname(name: &str) -> String {
    let name = name.trim().trim_end_matches(['.', ',']).trim();
    if let Some((last, _)) = name.split_once(',') {
        return last.trim().to_string();
    }
    name.split_whitespace().last().unwrap_or(name).to_string()
}

/// The four characters that would otherwise close a tag or an attribute.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const VASWANI: &str = r#"
@inproceedings{vaswani2017,
  author    = {Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and Uszkoreit, Jakob},
  title     = {Attention Is All You Need},
  booktitle = {NeurIPS},
  year      = {2017},
  doi       = {10.5555/3295222.3295349}
}
"#;

    fn one(src: &str) -> Entry {
        let (bib, warnings) = parse_bibtex(src);
        assert!(warnings.is_empty(), "{warnings:?}");
        bib.into_values().next().expect("one entry")
    }

    #[test]
    fn an_entry_keeps_its_key_type_and_fields() {
        let e = one(VASWANI);
        assert_eq!(e.key, "vaswani2017");
        assert_eq!(e.kind, "inproceedings");
        assert_eq!(e.field(&["title"]), Some("Attention Is All You Need"));
        assert_eq!(e.field(&["booktitle"]), Some("NeurIPS"));
    }

    #[test]
    fn a_label_is_the_first_surname_and_the_year() {
        assert_eq!(one(VASWANI).label(), "Vaswani+17");
        assert_eq!(
            one("@book{k, author={Knuth, Donald E.}, year={1984}}").label(),
            "Knuth84"
        );
    }

    /// A field that already says `et al.` is a summary, not a name list: `al.`
    /// must not become the surname.
    #[test]
    fn et_al_in_the_field_is_read_as_the_summary_it_is() {
        let e = one("@misc{x, author={Vaswani et al.}, year={2017}}");
        assert_eq!(e.label(), "Vaswani+17");
        assert_eq!(e.html_authors(), "Vaswani et al.");
    }

    #[test]
    fn a_one_word_name_is_its_own_surname() {
        let e = one("@misc{x, author={山田太郎 and 鈴木花子}, year={2024}}");
        assert_eq!(e.label(), "山田太郎+24");
    }

    #[test]
    fn an_entry_with_no_author_falls_back_to_its_key() {
        assert_eq!(one("@misc{rfc7231, title={HTTP/1.1}}").label(), "rfc7231");
    }

    #[test]
    fn a_reference_reads_as_authors_title_venue_year() {
        let html = one(VASWANI).html();
        assert!(
            html.starts_with("Vaswani, Shazeer, Parmar et al. <em>"),
            "{html}"
        );
        assert!(
            html.contains("<em>Attention Is All You Need</em>"),
            "{html}"
        );
        assert!(html.contains("NeurIPS, 2017."), "{html}");
        assert!(
            html.contains(r#"<a href="https://doi.org/10.5555/3295222.3295349">"#),
            "{html}"
        );
    }

    /// The locators are what makes an entry followable: two papers by the same
    /// group in the same journal two years apart are told apart by `29(5),
    /// 502–528`, not by the venue and the year.
    #[test]
    fn a_reference_carries_volume_issue_and_pages() {
        let e = one(
            "@article{h, author={Huang, Guoquan}, title={T}, journal={IJRR}, \
             volume={29}, number={5}, pages={502--528}, year={2010}}",
        );
        assert!(
            e.html().contains("IJRR 29(5), 502–528, 2010."),
            "{}",
            e.html()
        );
        // Each part is optional, and what is missing must not leave a comma
        // or a bracket behind.
        let vol = one("@article{a, title={T}, journal={J}, volume={7}, year={2020}}");
        assert!(vol.html().contains("J 7, 2020."), "{}", vol.html());
        let pages = one("@incollection{b, title={T}, booktitle={B}, pages={1--9}}");
        assert!(pages.html().contains("B 1–9."), "{}", pages.html());
        let issue = one("@article{c, title={T}, journal={J}, number={3}}");
        assert!(issue.html().contains("J no. 3."), "{}", issue.html());
        // An entry with none of them reads exactly as it did before.
        assert!(one(VASWANI).html().contains("NeurIPS, 2017."));
    }

    /// Under three authors the list is complete, so it must not say `et al.`
    /// — and the separator is a comma in every language, never an English
    /// `and` dropped into a Japanese line.
    #[test]
    fn a_short_author_list_is_shown_whole_and_comma_separated() {
        let e = one("@misc{x, author={Rivest, Ron and Shamir, Adi}, title={T}}");
        assert!(e.html().starts_with("Rivest, Shamir."), "{}", e.html());
        let ja = one("@misc{y, author={山田, 太郎 and 鈴木, 花子}, title={T}}");
        assert!(ja.html().starts_with("山田, 鈴木."), "{}", ja.html());
    }

    #[test]
    fn an_arxiv_entry_links_to_the_abstract() {
        let e = one("@misc{x, title={T}, eprint={1810.04805}, archiveprefix={arXiv}}");
        assert!(
            e.html()
                .contains(r#"href="https://arxiv.org/abs/1810.04805""#),
            "{}",
            e.html()
        );
        assert!(e.html().contains("arXiv:1810.04805"));
    }

    #[test]
    fn quoted_and_bare_values_parse_too() {
        let e = one(r#"@article{x, author = "Lovelace, Ada", year = 1843, title = {Notes}}"#);
        assert_eq!(e.field(&["year"]), Some("1843"));
        assert_eq!(e.label(), "Lovelace43");
    }

    #[test]
    fn case_protecting_braces_and_escapes_are_not_shown() {
        let e = one(r#"@misc{x, title={{BERT} \& friends}}"#);
        assert_eq!(e.field(&["title"]), Some("BERT & friends"));
    }

    #[test]
    fn a_value_wrapped_across_lines_becomes_one_line() {
        let e = one("@misc{x, title={A title\n    split over lines}}");
        assert_eq!(e.field(&["title"]), Some("A title split over lines"));
    }

    #[test]
    fn a_nested_brace_does_not_end_the_value_early() {
        let e = one("@misc{x, title={A {nested} brace}, year={2020}}");
        assert_eq!(e.field(&["title"]), Some("A nested brace"));
        assert_eq!(e.field(&["year"]), Some("2020"));
    }

    #[test]
    fn string_and_comment_blocks_are_read_past_not_into() {
        let (bib, warnings) =
            parse_bibtex("@string{acl = {ACL}}\n@comment{ignore me}\n@misc{real, title={T}}\n");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(bib.keys().collect::<Vec<_>>(), vec!["real"]);
    }

    /// A file whose last entry was truncated still yields the entries before
    /// it: a deck should not lose its whole bibliography to one bad tail.
    #[test]
    fn an_unterminated_entry_costs_only_itself() {
        let (bib, _) = parse_bibtex("@misc{a, title={A}}\n@misc{b, title={B}\n");
        assert!(bib.contains_key("a"));
    }

    #[test]
    fn a_key_defined_twice_is_reported() {
        let (bib, warnings) = parse_bibtex("@misc{a, title={One}}\n@misc{a, title={Two}}\n");
        assert_eq!(bib["a"].field(&["title"]), Some("Two"));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn frontmatter_fields_make_the_same_entry() {
        let e = Entry::from_fields(
            "x",
            BTreeMap::from([
                ("Author".to_string(), "Lovelace, Ada".to_string()),
                ("year".to_string(), "1843".to_string()),
            ]),
        );
        assert_eq!(e.label(), "Lovelace43");
    }

    #[test]
    fn a_style_name_is_checked_not_guessed() {
        assert_eq!(CiteStyle::parse("author"), Ok(CiteStyle::Author));
        assert_eq!(CiteStyle::parse(" numeric "), Ok(CiteStyle::Numeric));
        assert!(CiteStyle::parse("apa").is_err());
    }

    #[test]
    fn markup_in_a_field_cannot_escape_into_the_page() {
        let e = one(r#"@misc{x, title={<script>alert(1)</script>}}"#);
        assert!(!e.html().contains("<script>"), "{}", e.html());
    }

    impl Entry {
        /// The author clause alone, for the tests that are about it.
        fn html_authors(&self) -> String {
            self.author_display().unwrap_or_default()
        }
    }
}
