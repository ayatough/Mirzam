//! `[@key]` citations and the `bibliography` block that lists them.
//!
//! Footnotes (`[^key]`) answer a different question and stay as they are: a
//! remark that belongs to one slide, written and read on that slide. This is
//! for the other half — a reference the deck cites more than once, whose entry
//! is worth writing down exactly once, at the back.
//!
//! Same two passes as `toc`, and for the same reason. A slide is rendered on
//! its own and cached by content hash, so nothing on it can know which number
//! its citation is:
//!
//! 1. [`mark`] turns `[@key]` into a comment marker while the slide is still
//!    Markdown, and [`extract`] does the same for the `bibliography` fence.
//!    Both markers carry everything the second pass needs, so a slide served
//!    from the cache takes part without being re-rendered.
//! 2. [`resolve_deck`] runs once every slide has rendered: it numbers the
//!    references in citation order, writes the list, and links each mark to it
//!    and each entry back to the slides that cited it.
//!
//! Keeping the entry text out of the slide has a second effect worth having:
//! editing `refs.bib` rebuilds the list without invalidating one cached slide,
//! because a slide only ever recorded the key.

use mirzam_cite::{Bibliography, CiteStyle};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// `[@key]`, or several keys separated by `;` inside one pair of brackets.
///
/// The key charset is BibTeX's, minus the characters that would make the
/// bracket ambiguous. Anything else between the brackets — `[@handle said so]`,
/// or an email address in an aside — does not match and is left exactly as
/// written, which is what keeps this from claiming text that is not a citation.
fn cite_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\[@[A-Za-z0-9_][A-Za-z0-9_:.+/-]*(?:\s*;\s*@[A-Za-z0-9_][A-Za-z0-9_:.+/-]*)*\]",
        )
        .expect("static regex")
    })
}

/// Turns `[@key]` into `<!--mz-cite:key-->`, outside code fences and inline
/// code spans.
///
/// Called only when the deck declared a bibliography. Without one there is
/// nothing a key could name, and `[@name]` is then someone's prose — turning it
/// into a marker would delete it.
pub fn mark(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        match mirzam_syntax::fence_len(line.trim()) {
            Some(open) => {
                out.push_str(line);
                out.push('\n');
                for inner in lines.by_ref() {
                    out.push_str(inner);
                    out.push('\n');
                    if mirzam_syntax::closes_fence(inner.trim(), open) {
                        break;
                    }
                }
            }
            None => {
                out.push_str(&map_outside_code(line, mark_line));
                out.push('\n');
            }
        }
    }
    out
}

fn mark_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in cite_regex().find_iter(text) {
        // `\[@key]` is the author asking for the brackets themselves.
        if text[..m.start()].ends_with('\\') {
            continue;
        }
        out.push_str(&text[last..m.start()]);
        let inner = &m.as_str()[1..m.as_str().len() - 1];
        let keys: Vec<&str> = inner
            .split(';')
            .map(|k| k.trim().trim_start_matches('@'))
            .collect();
        out.push_str(&format!("<!--mz-cite:{}-->", keys.join(";")));
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

/// Applies `f` to the parts of a line that are not inside an inline code span.
///
/// Without this, ``a citation reads `[@key]` `` — the one line every reference
/// page writes — would turn into a live citation in its own explanation.
fn map_outside_code(line: &str, f: impl Fn(&str) -> String) -> String {
    if !line.contains('`') {
        return f(line);
    }
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut plain = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] != '`' {
            plain.push(cs[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < cs.len() && cs[i] == '`' {
            i += 1;
        }
        let run = i - start;
        // A span closes on a run of exactly the same length, which is how
        // ``a ` inside`` works.
        let mut j = i;
        let mut close = None;
        while j < cs.len() {
            if cs[j] != '`' {
                j += 1;
                continue;
            }
            let s = j;
            while j < cs.len() && cs[j] == '`' {
                j += 1;
            }
            if j - s == run {
                close = Some(j);
                break;
            }
        }
        match close {
            Some(end) => {
                out.push_str(&f(&plain));
                plain.clear();
                out.extend(cs[start..end].iter());
                i = end;
            }
            // Backticks that never close are literal text, not a span.
            None => plain.extend(cs[start..i].iter()),
        }
    }
    out.push_str(&f(&plain));
    out
}

/// What a `bibliography` block asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spec {
    /// List every entry in the bibliography, not only the cited ones. Off by
    /// default: a `.bib` is a library, and a talk cites a corner of it.
    all: bool,
    /// Show, on each entry, the slides that cited it.
    back: bool,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            all: false,
            back: true,
        }
    }
}

/// Turns ```bibliography fences into markers, the same shape `toc` uses.
pub fn extract(md: &str, errors: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(md.len());
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "```bibliography" {
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
                "\n<!--mz-bib:{}:{}-->\n",
                u8::from(spec.all),
                u8::from(spec.back)
            ));
        } else if let Some(open) = mirzam_syntax::fence_len(trimmed).filter(|n| *n > 3) {
            // A longer fence quotes the syntax rather than using it, which is
            // how the reference shows a `bibliography` block.
            out.push_str(line);
            out.push('\n');
            for inner in lines.by_ref() {
                out.push_str(inner);
                out.push('\n');
                if mirzam_syntax::closes_fence(inner.trim(), open) {
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

fn parse(body: &str, errors: &mut Vec<String>) -> Spec {
    let mut spec = Spec::default();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            errors.push(format!("bibliography: `{line}` is not `key: value`"));
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "show" => match value {
                "cited" => spec.all = false,
                "all" => spec.all = true,
                _ => errors.push(format!(
                    "bibliography: show `{value}` must be `cited` or `all`"
                )),
            },
            "back" => match value {
                "true" | "yes" | "on" => spec.back = true,
                "false" | "no" | "off" => spec.back = false,
                _ => errors.push(format!(
                    "bibliography: back `{value}` must be true or false"
                )),
            },
            other => errors.push(format!("bibliography: unknown key `{other}`")),
        }
    }
    spec
}

/// Resolves every citation and every `bibliography` block in the deck, and
/// returns what could not be resolved.
///
/// Call it once, after every slide has rendered and after `toc`. Idempotent in
/// the way that matters: a deck with neither marker is left byte-identical, so
/// a deck that does not cite anything pays nothing for this existing.
pub fn resolve_deck(sections: &mut [String], bib: &Bibliography, style: CiteStyle) -> Vec<String> {
    let has_cite = sections.iter().any(|s| s.contains("<!--mz-cite:"));
    let has_list = sections.iter().any(|s| s.contains("<!--mz-bib:"));
    if !has_cite && !has_list {
        return Vec::new();
    }
    let mut warnings = Vec::new();

    // Where the list goes, and what it was asked for. More than one block is
    // allowed — a long deck may repeat its references — and the marks link to
    // the first, which is the one an audience reaches by reading forward.
    let mut list_slides: Vec<(usize, Spec)> = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        for spec in specs_in(section) {
            list_slides.push((i, spec));
        }
    }
    let show_all = list_slides.iter().any(|(_, s)| s.all);

    // Citations in reading order. `cited` is the numbering order and
    // `backlinks` is its inverse: which slides sent the reader here.
    let mut cited: Vec<String> = Vec::new();
    let mut backlinks: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut unknown: BTreeSet<(usize, String)> = BTreeSet::new();
    for (i, section) in sections.iter().enumerate() {
        for keys in citations_in(section) {
            for key in keys {
                if !bib.contains_key(&key) {
                    unknown.insert((i, key));
                    continue;
                }
                if !cited.contains(&key) {
                    cited.push(key.clone());
                }
                let seen = backlinks.entry(key).or_default();
                // One slide citing a reference three times is one backlink.
                if seen.last() != Some(&i) {
                    seen.push(i);
                }
            }
        }
    }
    for (slide, key) in &unknown {
        warnings.push(format!(
            "slide {}: `[@{key}]` is in no bibliography entry; the mark is left \
             as written",
            slide + 1
        ));
    }

    // What the list holds, and what each mark says.
    let mut listed = cited.clone();
    if show_all {
        listed.extend(bib.keys().filter(|k| !cited.contains(k)).cloned());
    }
    let labels = label_table(&listed, bib, style);
    if style == CiteStyle::Author {
        listed.sort_by(|a, b| labels[a].cmp(&labels[b]).then(a.cmp(b)));
    }

    let target = list_slides.first().map(|(i, _)| *i);
    if has_cite && target.is_none() {
        warnings.push(format!(
            "citations: {} reference(s) are cited and no `bibliography` block \
             lists them; each mark shows but links to nothing",
            cited.len()
        ));
    }
    if has_list && listed.is_empty() {
        warnings.push(
            "bibliography: nothing to list; no `[@key]` on any slide cites an entry".to_string(),
        );
    }

    for section in sections.iter_mut() {
        *section = substitute_cites(section, &labels, target);
        *section = substitute_lists(section, &listed, bib, &labels, &backlinks);
    }
    warnings
}

/// The mark each cited key carries: its number, or its author-year label made
/// unique.
fn label_table(
    listed: &[String],
    bib: &Bibliography,
    style: CiteStyle,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    match style {
        CiteStyle::Numeric => {
            for (i, key) in listed.iter().enumerate() {
                out.insert(key.clone(), (i + 1).to_string());
            }
        }
        CiteStyle::Author => {
            // Two papers by the same first author in the same year would
            // otherwise print the same mark and the reader could not tell
            // which entry it meant.
            let mut used: BTreeMap<String, usize> = BTreeMap::new();
            for key in listed {
                let base = bib[key].label();
                let n = used.entry(base.clone()).or_insert(0);
                let label = match *n {
                    0 => base.clone(),
                    k => format!("{base}{}", (b'a' + (k as u8 - 1).min(24)) as char),
                };
                *n += 1;
                out.insert(key.clone(), label);
            }
        }
    }
    out
}

/// Every `<!--mz-cite:…-->` in a section, as its list of keys, in order.
fn citations_in(section: &str) -> Vec<Vec<String>> {
    markers(section, "<!--mz-cite:")
        .map(|args| args.split(';').map(str::to_string).collect())
        .collect()
}

fn specs_in(section: &str) -> Vec<Spec> {
    markers(section, "<!--mz-bib:")
        .map(|args| {
            let mut parts = args.split(':');
            Spec {
                all: parts.next() == Some("1"),
                back: parts.next() != Some("0"),
            }
        })
        .collect()
}

/// The argument text of each marker with this prefix.
fn markers<'a>(section: &'a str, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let mut rest = section;
    std::iter::from_fn(move || {
        let at = rest.find(prefix)?;
        let from = at + prefix.len();
        // A marker with no `-->` cannot happen from this crate's own output,
        // and stopping is the safe reading of one that did.
        let end = rest[from..].find("-->").map(|o| from + o)?;
        let args = &rest[from..end];
        rest = &rest[end + 3..];
        Some(args)
    })
}

fn substitute_cites(
    section: &str,
    labels: &BTreeMap<String, String>,
    target: Option<usize>,
) -> String {
    let mut out = String::with_capacity(section.len());
    let mut rest = section;
    while let Some(at) = rest.find("<!--mz-cite:") {
        out.push_str(&rest[..at]);
        let from = at + "<!--mz-cite:".len();
        let Some(end) = rest[from..].find("-->").map(|o| from + o) else {
            break;
        };
        let marks: Vec<String> = rest[from..end]
            .split(';')
            .map(|key| match labels.get(key) {
                // A key nothing defines stays visible as the author wrote it,
                // rather than becoming a number that points nowhere. The
                // warning above says which slide it is on.
                None => format!("@{}", crate::inline::html_escape(key)),
                Some(label) => match target {
                    // Two addresses for one mark: the slide number the viewer
                    // navigates by, and the entry the print page turns it into
                    // once the ids exist. See `retarget_for_print`.
                    Some(slide) => format!(
                        "<a href=\"#{}\" data-mz-bib=\"mz-bib-{}\">{}</a>",
                        slide + 1,
                        anchor(key),
                        crate::inline::html_escape(label)
                    ),
                    None => crate::inline::html_escape(label),
                },
            })
            .collect();
        out.push_str(&format!(
            "<span class=\"mz-cite\">[{}]</span>",
            marks.join(", ")
        ));
        rest = &rest[end + 3..];
    }
    out.push_str(rest);
    out
}

fn substitute_lists(
    section: &str,
    listed: &[String],
    bib: &Bibliography,
    labels: &BTreeMap<String, String>,
    backlinks: &BTreeMap<String, Vec<usize>>,
) -> String {
    let mut out = String::with_capacity(section.len());
    let mut rest = section;
    while let Some(at) = rest.find("<!--mz-bib:") {
        out.push_str(&rest[..at]);
        let from = at + "<!--mz-bib:".len();
        let Some(end) = rest[from..].find("-->").map(|o| from + o) else {
            break;
        };
        let mut parts = rest[from..end].split(':');
        let _all = parts.next();
        let back = parts.next() != Some("0");
        out.push_str(&render_list(listed, bib, labels, backlinks, back));
        rest = &rest[end + 3..];
    }
    out.push_str(rest);
    out
}

fn render_list(
    listed: &[String],
    bib: &Bibliography,
    labels: &BTreeMap<String, String>,
    backlinks: &BTreeMap<String, Vec<usize>>,
    back: bool,
) -> String {
    if listed.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul class=\"mz-bib\">");
    for key in listed {
        let Some(entry) = bib.get(key) else { continue };
        let label = labels.get(key).cloned().unwrap_or_default();
        out.push_str(&format!(
            "<li id=\"mz-bib-{}\"><span class=\"mz-bib-mark\">[{}]</span>\
             <span class=\"mz-bib-entry\">{}</span>",
            anchor(key),
            crate::inline::html_escape(&label),
            entry.html(),
        ));
        // Where the claim was made. The link text is the slide number, so it
        // still says something in a PDF, where there is nothing to click.
        if back {
            if let Some(slides) = backlinks.get(key).filter(|s| !s.is_empty()) {
                let links: Vec<String> = slides
                    .iter()
                    .map(|s| format!("<a href=\"#{}\">{}</a>", s + 1, s + 1))
                    .collect();
                out.push_str(&format!(
                    "<span class=\"mz-bib-back\" aria-label=\"cited on\">↩ {}</span>",
                    links.join(", ")
                ));
            }
        }
        out.push_str("</li>");
    }
    out.push_str("</ul>");
    out
}

/// Points every `[@key]` mark at the entry it names, rather than at the slide
/// the list is on.
///
/// On screen a mark can only address a slide: the viewer turns pages, and
/// scrolling to an entry would mean scrolling a stage that does not scroll.
/// Paper has the opposite shape — the whole deck is one document, and a reader
/// who taps `[12]` in a list of thirty expects to land on the twelfth entry,
/// not on the top of the page it happens to be on. The `<li>` already carries
/// that id, so this is a matter of preferring the address the mark was written
/// with.
pub fn retarget_for_print(section: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r##"<a href="#\d+" data-mz-bib="([\w-]+)">"##).expect("static regex")
    });
    re.replace_all(section, r##"<a href="#$1">"##).into_owned()
}

/// A citation key is nearly an HTML id already, but `:` and `/` are legal in
/// one and awkward in the other.
fn anchor(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bib() -> Bibliography {
        mirzam_cite::parse_bibtex(
            r#"
@inproceedings{vaswani2017, author={Vaswani, Ashish and Shazeer, Noam},
  title={Attention Is All You Need}, booktitle={NeurIPS}, year={2017}}
@inproceedings{devlin2019, author={Devlin, Jacob}, title={BERT},
  booktitle={NAACL}, year={2019}}
"#,
        )
        .0
    }

    /// The whole path a slide takes: mark, then resolve with the deck around it.
    fn deck(slides: &[&str], style: CiteStyle) -> (Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut sections: Vec<String> = slides
            .iter()
            .map(|s| extract(&mark(s), &mut errors))
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
        let warnings = resolve_deck(&mut sections, &bib(), style);
        (sections, warnings)
    }

    #[test]
    fn a_citation_becomes_a_marker_carrying_only_the_key() {
        assert_eq!(
            mark("Attention[@vaswani2017] replaced recurrence.\n"),
            "Attention<!--mz-cite:vaswani2017--> replaced recurrence.\n"
        );
    }

    #[test]
    fn several_keys_ride_on_one_marker() {
        assert_eq!(
            mark("Both[@vaswani2017; @devlin2019].\n"),
            "Both<!--mz-cite:vaswani2017;devlin2019-->.\n"
        );
    }

    /// Text that is not a citation must survive untouched, or the feature
    /// deletes prose the moment a deck gains a bibliography.
    #[test]
    fn brackets_that_are_not_citations_are_left_alone() {
        for src in [
            "[@handle said so]\n",
            "mail me at [a@b.com]\n",
            "an [aside] and a [link](http://x)\n",
            "escaped \\[@vaswani2017]\n",
        ] {
            assert_eq!(mark(src), src, "{src}");
        }
    }

    #[test]
    fn a_citation_inside_code_is_an_example_not_a_citation() {
        let src = "write `[@key]` for a citation\n";
        assert_eq!(mark(src), src);
        let fenced = "```markdown\n[@vaswani2017]\n```\n";
        assert_eq!(mark(fenced), fenced);
    }

    #[test]
    fn numbering_follows_the_order_of_first_citation() {
        let (out, warnings) = deck(
            &[
                "Pretraining[@devlin2019].\n",
                "Attention[@vaswani2017].\n",
                "```bibliography\n```\n",
            ],
            CiteStyle::Numeric,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(out[0].contains(">1</a>"), "{}", out[0]);
        assert!(out[1].contains(">2</a>"), "{}", out[1]);
        // The list is in number order, so BERT comes first.
        let list = &out[2];
        assert!(
            list.find("BERT").unwrap() < list.find("Attention Is All").unwrap(),
            "{list}"
        );
    }

    #[test]
    fn author_style_marks_the_claim_with_the_name() {
        let (out, _) = deck(
            &["Attention[@vaswani2017].\n", "```bibliography\n```\n"],
            CiteStyle::Author,
        );
        assert!(out[0].contains(">Vaswani+17</a>"), "{}", out[0]);
    }

    /// Under `author`, the list reads alphabetically: the mark is the address,
    /// so the list has to be ordered the way somebody would look one up.
    #[test]
    fn author_style_lists_alphabetically_not_in_citation_order() {
        let (out, _) = deck(
            &[
                "Attention[@vaswani2017].\n",
                "Pretraining[@devlin2019].\n",
                "```bibliography\n```\n",
            ],
            CiteStyle::Author,
        );
        let list = &out[2];
        assert!(
            list.find("Devlin19").unwrap() < list.find("Vaswani+17").unwrap(),
            "{list}"
        );
    }

    #[test]
    fn a_mark_links_to_the_slide_the_list_is_on() {
        let (out, _) = deck(
            &["a[@vaswani2017]\n", "b\n", "```bibliography\n```\n"],
            CiteStyle::Numeric,
        );
        assert!(out[0].contains("href=\"#3\""), "{}", out[0]);
    }

    /// The mark carries both addresses: the slide the viewer turns to, and the
    /// entry the print page sends a reader to instead.
    #[test]
    fn a_mark_also_names_the_entry_it_stands_for() {
        let (out, _) = deck(
            &["a[@vaswani2017]\n", "```bibliography\n```\n"],
            CiteStyle::Numeric,
        );
        assert!(
            out[0].contains("data-mz-bib=\"mz-bib-vaswani2017\""),
            "{}",
            out[0]
        );
        let printed = retarget_for_print(&out[0]);
        assert!(
            printed.contains("<a href=\"#mz-bib-vaswani2017\">1</a>"),
            "{printed}"
        );
        assert!(!printed.contains("data-mz-bib"), "{printed}");
    }

    /// A key nothing defines has no entry to land on, so there is nothing to
    /// retarget and the mark stays as written.
    #[test]
    fn print_leaves_a_mark_that_points_nowhere_alone() {
        let (out, _) = deck(
            &["a[@nosuchkey]\n", "```bibliography\n```\n"],
            CiteStyle::Numeric,
        );
        assert_eq!(retarget_for_print(&out[0]), out[0]);
    }

    #[test]
    fn an_entry_links_back_to_every_slide_that_cited_it() {
        let (out, _) = deck(
            &[
                "a[@vaswani2017]\n",
                "b\n",
                "c[@vaswani2017] and again[@vaswani2017]\n",
                "```bibliography\n```\n",
            ],
            CiteStyle::Numeric,
        );
        let list = &out[3];
        assert!(list.contains("mz-bib-back"), "{list}");
        assert!(list.contains(">1</a>") && list.contains(">3</a>"), "{list}");
        // Slide 3 cites it twice and is one backlink.
        assert_eq!(list.matches("href=\"#3\"").count(), 1, "{list}");
    }

    #[test]
    fn back_false_drops_the_backlinks() {
        let (out, _) = deck(
            &["a[@vaswani2017]\n", "```bibliography\nback: false\n```\n"],
            CiteStyle::Numeric,
        );
        assert!(!out[1].contains("mz-bib-back"), "{}", out[1]);
    }

    #[test]
    fn show_all_lists_what_was_never_cited() {
        let (out, _) = deck(
            &["a[@vaswani2017]\n", "```bibliography\nshow: all\n```\n"],
            CiteStyle::Numeric,
        );
        assert!(out[1].contains("BERT"), "{}", out[1]);
    }

    #[test]
    fn a_key_nothing_defines_stays_visible_and_is_reported() {
        let (out, warnings) = deck(
            &["a[@nosuchkey]\n", "```bibliography\n```\n"],
            CiteStyle::Numeric,
        );
        assert!(out[0].contains("[@nosuchkey]"), "{}", out[0]);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("slide 1"), "{warnings:?}");
    }

    #[test]
    fn citing_with_nowhere_to_land_is_reported() {
        let (out, warnings) = deck(&["a[@vaswani2017]\n"], CiteStyle::Numeric);
        // The mark still reads; it just does not go anywhere.
        assert!(
            out[0].contains("<span class=\"mz-cite\">[1]</span>"),
            "{}",
            out[0]
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("no `bibliography` block")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_list_with_nothing_cited_renders_nothing_and_says_so() {
        let (out, warnings) = deck(&["```bibliography\n```\n"], CiteStyle::Numeric);
        assert!(!out[0].contains("mz-bib"), "{}", out[0]);
        assert!(
            warnings.iter().any(|w| w.contains("nothing to list")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_deck_that_cites_nothing_is_untouched() {
        let before = vec!["<p>plain</p>".to_string()];
        let mut after = before.clone();
        assert!(resolve_deck(&mut after, &bib(), CiteStyle::Numeric).is_empty());
        assert_eq!(before, after);
    }

    #[test]
    fn a_longer_fence_quotes_the_block_rather_than_using_it() {
        let mut errors = Vec::new();
        let out = extract("````markdown\n```bibliography\n```\n````\n", &mut errors);
        assert!(out.contains("```bibliography"), "{out}");
        assert!(!out.contains("<!--mz-bib"), "{out}");
    }

    #[test]
    fn bad_options_are_reported_not_guessed() {
        let mut errors = Vec::new();
        extract("```bibliography\nshow: some\nnope: 1\n```\n", &mut errors);
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    /// Two papers by the same author in the same year cannot print the same
    /// mark, or the reader cannot tell which entry it points at.
    #[test]
    fn colliding_author_labels_are_made_distinct() {
        let bib = mirzam_cite::parse_bibtex(
            "@misc{a, author={Ito, Ken}, title={One}, year={2020}}\n\
             @misc{b, author={Ito, Ken}, title={Two}, year={2020}}\n",
        )
        .0;
        let mut sections = vec![mark("x[@a] y[@b]\n"), "<!--mz-bib:0:1-->".to_string()];
        resolve_deck(&mut sections, &bib, CiteStyle::Author);
        assert!(sections[0].contains(">Ito20</a>"), "{}", sections[0]);
        assert!(sections[0].contains(">Ito20a</a>"), "{}", sections[0]);
    }
}
