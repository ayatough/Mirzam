//! The `anim` extraction pass: turns the `anim` fenced blocks a slide
//! collected (see `mirzam_syntax::BlockKind::Anim`) into the [C1] JSON blob,
//! via `mirzam-anim` for the DSL parsing itself.
//!
//! Two things happen here that `mirzam-anim` cannot do on its own, because
//! they need the slide's rendered HTML rather than just the DSL text:
//!
//! - **Target validation.** A `#id` or `.class` target that matches nothing
//!   on the slide is "a line that points at nothing" per the workstream
//!   brief: it is reported as a warning, and the whole block is dropped for
//!   that slide (no `mz-anim` script), rather than failing the build.
//! - **Build-time text splitting.** `target.split` wraps the target's text in
//!   `<span class="mz-split-item">` per unit, so the runtime only ever
//!   selects existing spans. Splitting never crosses a tag boundary (so
//!   inline markup such as `<strong>` is never broken apart) and treats each
//!   HTML entity reference (`&amp;` and friends) as one indivisible unit.
//!
//! [C1]: ../../../docs/workstreams.md#c1-animation-timeline

use mirzam_syntax::BlockKind;
use regex::Regex;
use std::sync::OnceLock;

/// Processes every `anim` block collected for one slide, mutating `body` in
/// place to add build-time text splitting, and returns the `<script>` tag to
/// append to the slide (empty when there is nothing to animate, or the block
/// is dropped after a validation problem). Problems are pushed to `warnings`,
/// never treated as a render failure.
pub fn extract(
    slide_index: usize,
    reserved: &[(BlockKind, String)],
    body: &mut String,
    shapes_html: &str,
    warnings: &mut Vec<String>,
) -> String {
    let src = reserved
        .iter()
        .filter(|(kind, _)| matches!(kind, BlockKind::Anim))
        .map(|(_, s)| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if src.trim().is_empty() {
        return String::new();
    }

    let doc = mirzam_anim::parse(&src);
    let warn = |warnings: &mut Vec<String>, msg: String| {
        warnings.push(format!("slide {}: {msg}", slide_index + 1));
    };
    if !doc.errors.is_empty() {
        for e in &doc.errors {
            warn(warnings, e.clone());
        }
        return String::new();
    }

    // Validate everything before mutating anything, so a single bad line
    // drops the whole block instead of leaving a half-applied split.
    let haystack = format!("{body}{shapes_html}");
    let mut problems = Vec::new();
    let mut split_targets: Vec<(usize, usize, mirzam_anim::Split)> = Vec::new();
    for t in &doc.tracks {
        if let mirzam_anim::Trigger::After { id, .. } = &t.trigger {
            if !selector_exists(&haystack, &format!("#{id}")) {
                problems.push(format!(
                    "anim trigger references #{id}, but no element with that id exists"
                ));
            }
        }
        if !selector_exists(&haystack, &t.target.sel) {
            problems.push(format!(
                "anim target `{}` matches nothing on this slide",
                display_sel(&t.target.sel)
            ));
            continue;
        }
        if let Some(split) = t.target.split {
            if t.target.sel == ":scope" {
                problems.push("cannot split the whole slide; target an element instead".into());
                continue;
            }
            match locate(body, &t.target.sel) {
                Some((start, end)) => split_targets.push((start, end, split)),
                None => problems.push(format!(
                    "cannot split `{}`; only simple #id and .class targets on an \
                     element with a closing tag support split in v1",
                    display_sel(&t.target.sel)
                )),
            }
        }
    }
    split_targets.sort_by_key(|(start, _, _)| *start);
    for w in split_targets.windows(2) {
        if w[0].0 == w[1].0 && w[0].1 == w[1].1 {
            problems.push("a target is split by more than one track".into());
            break;
        }
    }
    if !problems.is_empty() {
        for p in problems {
            warn(warnings, p);
        }
        return String::new();
    }

    // Apply splits back to front so earlier byte ranges stay valid.
    for (start, end, split) in split_targets.into_iter().rev() {
        let wrapped = split_inner(&body[start..end], split);
        body.replace_range(start..end, &wrapped);
    }

    format!(
        "<script type=\"application/json\" class=\"mz-anim\">{}</script>\n",
        mirzam_anim::to_json(&doc)
    )
}

/// Checks every `[carry]` track against the slide it carries *to*, which is
/// the one thing [`extract`] cannot do: it sees one slide at a time, and a
/// carry is a statement about two.
///
/// A carry whose id is missing next door is not an error — the deck turns the
/// page as it always did — but it is exactly the silent degradation the
/// warnings exist for: the author wrote a line asking for movement and got
/// none, with nothing on screen to say why.
pub fn carry_warnings(sections: &[String]) -> Vec<String> {
    let mut warnings = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        for sel in carry_targets(section) {
            let id = sel.trim_start_matches('#');
            match sections.get(i + 1) {
                Some(next) if selector_exists(next, &sel) => {}
                Some(_) => warnings.push(format!(
                    "slide {}: `[carry] {sel}` has nothing to carry to; slide {} has no element with id `{id}`",
                    i + 1,
                    i + 2
                )),
                None => warnings.push(format!(
                    "slide {}: `[carry] {sel}` is on the last slide, so there is no next slide to carry it to",
                    i + 1
                )),
            }
        }
    }
    warnings
}

/// The `#id` of every carry track in a rendered slide, read back out of the
/// C1 blob the slide is carrying. Reading the emitted JSON rather than the DSL
/// keeps this pass working for a slide that came from the build cache and was
/// never re-parsed.
fn carry_targets(section: &str) -> Vec<String> {
    const OPEN: &str = "<script type=\"application/json\" class=\"mz-anim\">";
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len()..];
        let Some(end) = after.find("</script>") else {
            break;
        };
        out.extend(mirzam_anim::carry_targets_in_json(&after[..end]));
        rest = &after[end..];
    }
    out
}

/// `:scope` reads better as `slide` in a message the author wrote `slide` to
/// produce.
fn display_sel(sel: &str) -> &str {
    if sel == ":scope" {
        "slide"
    } else {
        sel
    }
}

fn class_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"class="([^"]*)""#).expect("static regex"))
}

fn simple_id(sel: &str) -> Option<&str> {
    let id = sel.strip_prefix('#')?;
    is_ident(id).then_some(id)
}

fn simple_class(sel: &str) -> Option<&str> {
    let class = sel.strip_prefix('.')?;
    is_ident(class).then_some(class)
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Whether `sel` can be shown to exist in `haystack`. Only simple `#id` and
/// `.class` selectors (and the always-present `:scope`) are checked; anything
/// more complex is assumed valid, since this is a build-time heuristic, not a
/// CSS engine.
pub(crate) fn selector_exists(haystack: &str, sel: &str) -> bool {
    if sel == ":scope" {
        return true;
    }
    if let Some(id) = simple_id(sel) {
        return haystack.contains(&format!("id=\"{id}\""));
    }
    if let Some(class) = simple_class(sel) {
        return class_regex()
            .captures_iter(haystack)
            .any(|c| c[1].split_whitespace().any(|tok| tok == class));
    }
    true
}

/// Finds the `[inner_start, inner_end)` byte range of the first element in
/// `html` matching the simple selector `sel`. `None` when the selector is not
/// simple, or the element has no separate closing tag to hold split content
/// (a self-closing SVG shape, for instance).
fn locate(html: &str, sel: &str) -> Option<(usize, usize)> {
    let attr_start = if let Some(id) = simple_id(sel) {
        html.find(&format!("id=\"{id}\""))?
    } else {
        let class = simple_class(sel)?;
        class_regex()
            .captures_iter(html)
            .find(|c| c[1].split_whitespace().any(|t| t == class))
            .map(|c| c.get(0).unwrap().start())?
    };

    let tag_start = html[..attr_start].rfind('<')?;
    let after_lt = &html[tag_start + 1..];
    let name_end = after_lt.find(|c: char| c.is_whitespace() || c == '>' || c == '/')?;
    let tag_name = &after_lt[..name_end];
    if tag_name.is_empty() {
        return None;
    }
    let open_end = html[attr_start..].find('>')? + attr_start + 1;
    if html[..open_end].ends_with("/>") {
        return None; // self-closing: no content to split
    }

    let close_tag = format!("</{tag_name}>");
    let mut depth = 1usize;
    let mut cursor = open_end;
    loop {
        let next_open = find_open_tag(html, cursor, tag_name);
        let next_close = html[cursor..].find(&close_tag).map(|i| i + cursor);
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                cursor = o + 1 + tag_name.len();
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some((open_end, c));
                }
                cursor = c + close_tag.len();
            }
            _ => return None,
        }
    }
}

/// Finds the next `<name` that opens a tag (not merely a prefix of a longer
/// tag name), from byte offset `from`.
fn find_open_tag(html: &str, from: usize, name: &str) -> Option<usize> {
    let mut search_from = from;
    while let Some(rel) = html[search_from..].find('<') {
        let pos = search_from + rel;
        let after = &html[pos + 1..];
        if let Some(rest) = after.strip_prefix(name) {
            if rest.starts_with([' ', '>', '/', '\n', '\t']) {
                return Some(pos);
            }
        }
        search_from = pos + 1;
    }
    None
}

enum Atom<'a> {
    Tag(&'a str),
    Text(&'a str),
}

/// Splits `html` into tags (passed through verbatim) and text runs (the only
/// thing split-wrapping ever touches), so a split never lands inside a tag's
/// own markup.
fn tokenize(html: &str) -> Vec<Atom<'_>> {
    let mut atoms = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            Some(pos) => {
                if pos > 0 {
                    atoms.push(Atom::Text(&rest[..pos]));
                }
                match rest[pos..].find('>') {
                    Some(end) => {
                        atoms.push(Atom::Tag(&rest[pos..pos + end + 1]));
                        rest = &rest[pos + end + 1..];
                    }
                    None => {
                        atoms.push(Atom::Text(rest));
                        rest = "";
                    }
                }
            }
            None => {
                atoms.push(Atom::Text(rest));
                rest = "";
            }
        }
    }
    atoms
}

fn is_br(tag: &str) -> bool {
    tag.trim_matches(['<', '>', '/'])
        .split_whitespace()
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case("br"))
}

fn split_inner(inner: &str, split: mirzam_anim::Split) -> String {
    match split {
        mirzam_anim::Split::Lines => wrap_lines(inner),
        mirzam_anim::Split::Chars => wrap_chars_or_words(inner, false),
        mirzam_anim::Split::Words => wrap_chars_or_words(inner, true),
    }
}

fn wrap_lines(inner: &str) -> String {
    let mut out = String::new();
    let mut seg = String::new();
    for atom in tokenize(inner) {
        match atom {
            Atom::Tag(t) if is_br(t) => {
                flush_line(&mut seg, &mut out);
                out.push_str(t);
            }
            Atom::Tag(t) => seg.push_str(t),
            Atom::Text(t) => seg.push_str(t),
        }
    }
    flush_line(&mut seg, &mut out);
    out
}

fn flush_line(seg: &mut String, out: &mut String) {
    if seg.trim().is_empty() {
        out.push_str(seg);
    } else {
        push_span(out, seg);
    }
    seg.clear();
}

fn wrap_chars_or_words(inner: &str, words: bool) -> String {
    let mut out = String::new();
    for atom in tokenize(inner) {
        match atom {
            Atom::Tag(t) => out.push_str(t),
            Atom::Text(t) => {
                if words {
                    wrap_words_into(t, &mut out);
                } else {
                    for unit in text_units(t) {
                        push_span(&mut out, unit);
                    }
                }
            }
        }
    }
    out
}

fn wrap_words_into(text: &str, out: &mut String) {
    let mut word = String::new();
    for u in text_units(text) {
        if u.chars().all(char::is_whitespace) {
            if !word.is_empty() {
                push_span(out, &word);
                word.clear();
            }
            out.push_str(u);
        } else if is_cjk(u) {
            if !word.is_empty() {
                push_span(out, &word);
                word.clear();
            }
            push_span(out, u);
        } else {
            word.push_str(u);
        }
    }
    if !word.is_empty() {
        push_span(out, &word);
    }
}

fn push_span(out: &mut String, content: &str) {
    out.push_str("<span class=\"mz-split-item\">");
    out.push_str(content);
    out.push_str("</span>");
}

/// One indivisible unit within a text run: a single character, or a whole
/// HTML entity reference (`&amp;`, `&#39;`, ...), which must never be split
/// across a span boundary or it stops being recognised as an entity.
fn text_units(text: &str) -> Vec<&str> {
    let mut units = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if text.as_bytes()[i] == b'&' {
            if let Some(rel) = text[i..].find(';') {
                if rel <= 10 {
                    let cand = &text[i..=i + rel];
                    if is_html_entity(cand) {
                        units.push(cand);
                        i += cand.len();
                        continue;
                    }
                }
            }
        }
        let len = text[i..].chars().next().map_or(1, char::len_utf8);
        units.push(&text[i..i + len]);
        i += len;
    }
    units
}

fn is_html_entity(s: &str) -> bool {
    let Some(inner) = s.strip_prefix('&').and_then(|s| s.strip_suffix(';')) else {
        return false;
    };
    if let Some(num) = inner.strip_prefix('#') {
        let digits = num.strip_prefix(['x', 'X']).unwrap_or(num);
        return !digits.is_empty() && digits.chars().all(|c| c.is_ascii_hexdigit());
    }
    !inner.is_empty() && inner.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Whether `unit` is a single CJK character, which (having no spaces to mark
/// word boundaries) is treated as its own word when splitting by `words`.
fn is_cjk(unit: &str) -> bool {
    let mut chars = unit.chars();
    let Some(c) = chars.next() else { return false };
    if chars.next().is_some() {
        return false; // an entity reference, not a single character
    }
    matches!(c as u32,
        0x3040..=0x30FF   // Hiragana + Katakana
        | 0x3400..=0x4DBF  // CJK extension A
        | 0x4E00..=0x9FFF  // CJK unified ideographs
        | 0xF900..=0xFAFF  // CJK compatibility ideographs
        | 0xAC00..=0xD7A3  // Hangul syllables
        | 0x1100..=0x11FF  // Hangul Jamo
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(src: &str) -> mirzam_anim::AnimDoc {
        mirzam_anim::parse(src)
    }

    /// One rendered section: a body plus whatever `extract` made of the DSL,
    /// which is the pair `carry_warnings` reads back.
    fn section(body: &str, anim: &str) -> String {
        let mut html = body.to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(BlockKind::Anim, anim.to_string())];
        let script = extract(0, &reserved, &mut html, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        format!("{html}{script}")
    }

    #[test]
    fn a_carry_with_the_id_on_both_slides_is_silent() {
        let sections = vec![
            section("<span id=\"chip\">a</span>", "[carry] #chip : move 400ms\n"),
            "<h2 id=\"chip\">a</h2>".to_string(),
        ];
        assert!(carry_warnings(&sections).is_empty());
    }

    #[test]
    fn a_carry_the_next_slide_cannot_receive_warns() {
        let sections = vec![
            section("<span id=\"chip\">a</span>", "[carry] #chip : move 400ms\n"),
            "<h2 id=\"other\">a</h2>".to_string(),
        ];
        let w = carry_warnings(&sections);
        assert_eq!(w.len(), 1);
        assert!(
            w[0].contains("slide 2 has no element with id `chip`"),
            "{w:?}"
        );
    }

    #[test]
    fn a_carry_on_the_last_slide_warns() {
        let sections = vec![section(
            "<span id=\"chip\">a</span>",
            "[carry] #chip : move 400ms\n",
        )];
        let w = carry_warnings(&sections);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("no next slide"), "{w:?}");
    }

    #[test]
    fn an_ordinary_track_is_not_mistaken_for_a_carry() {
        // `#chip` is animated on slide 1 and absent from slide 2, which is
        // perfectly ordinary: only a carry is a claim about the next slide.
        let sections = vec![
            section(
                "<span id=\"chip\">a</span>",
                "[enter] #chip : fade-in 400ms\n",
            ),
            "<p>nothing</p>".to_string(),
        ];
        assert!(carry_warnings(&sections).is_empty());
    }

    #[test]
    fn no_anim_blocks_produces_nothing() {
        let mut body = "<div></div>".to_string();
        let mut warnings = Vec::new();
        let script = extract(0, &[], &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn valid_block_emits_a_script_tag() {
        let mut body = "<div class=\"pane\"><h1 class=\"title\">Hello</h1></div>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .title : fade-in 400ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(script.starts_with("<script type=\"application/json\" class=\"mz-anim\">"));
        assert!(script.contains("\"fade-in\""));
    }

    #[test]
    fn target_not_found_warns_and_drops_the_whole_block() {
        let mut body = "<div class=\"pane\"></div>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .nope : fade-in 400ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("slide 1"));
        assert!(warnings[0].contains("matches nothing"));
    }

    #[test]
    fn after_trigger_references_a_missing_id() {
        let mut body = "<div></div>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[after #ghost] .b : fade-in 100ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert!(warnings[0].contains("#ghost"));
    }

    #[test]
    fn parse_errors_become_warnings_not_render_errors() {
        let mut body = "<div></div>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(BlockKind::Anim, "not a valid line\n".to_string())];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("anim line 1"));
    }

    #[test]
    fn slide_keyword_targets_the_whole_section_and_needs_no_lookup() {
        let mut body = "<div></div>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[exit] slide : iris-out 500ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!script.is_empty());
    }

    #[test]
    fn splitting_the_whole_slide_is_an_error() {
        let mut body = "<div></div>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] slide : chars fade-in 500ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert!(warnings[0].contains("cannot split the whole slide"));
    }

    #[test]
    fn chars_split_wraps_each_character_and_keeps_tags_intact() {
        let mut body = "<h1 class=\"title\">Hi</h1>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .title : chars fade-in 400ms\n".to_string(),
        )];
        extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            body,
            "<h1 class=\"title\"><span class=\"mz-split-item\">H</span><span class=\"mz-split-item\">i</span></h1>"
        );
    }

    #[test]
    fn chars_split_does_not_break_an_entity_reference() {
        let mut body = "<h1 class=\"title\">A&amp;B</h1>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .title : chars fade-in 400ms\n".to_string(),
        )];
        extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        // &amp; stays a single unit inside its own span, never split apart.
        assert!(body.contains("<span class=\"mz-split-item\">&amp;</span>"));
        assert!(!body.contains("<span class=\"mz-split-item\">&</span>"));
    }

    #[test]
    fn chars_split_does_not_break_cjk_multibyte_characters() {
        let mut body = "<h1 class=\"title\">日本語</h1>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .title : chars fade-in 400ms\n".to_string(),
        )];
        extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        for ch in ["日", "本", "語"] {
            assert!(body.contains(&format!("<span class=\"mz-split-item\">{ch}</span>")));
        }
    }

    #[test]
    fn chars_split_does_not_break_inline_markup() {
        let mut body = "<h1 class=\"title\">a <strong>b</strong> c</h1>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .title : chars fade-in 400ms\n".to_string(),
        )];
        extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(body.contains("<strong><span class=\"mz-split-item\">b</span></strong>"));
    }

    #[test]
    fn words_split_treats_each_cjk_character_as_its_own_word() {
        let out = wrap_chars_or_words("日本語 hello", true);
        assert!(out.contains("<span class=\"mz-split-item\">日</span>"));
        assert!(out.contains("<span class=\"mz-split-item\">本</span>"));
        assert!(out.contains("<span class=\"mz-split-item\">語</span>"));
        assert!(out.contains("<span class=\"mz-split-item\">hello</span>"));
    }

    #[test]
    fn words_split_keeps_latin_words_whole() {
        let out = wrap_chars_or_words("one two", true);
        assert!(out.contains("<span class=\"mz-split-item\">one</span>"));
        assert!(out.contains("<span class=\"mz-split-item\">two</span>"));
        assert!(!out.contains(">o</span>"));
    }

    #[test]
    fn lines_split_breaks_only_at_br() {
        let out = wrap_lines("first<br>second");
        assert_eq!(
            out,
            "<span class=\"mz-split-item\">first</span><br><span class=\"mz-split-item\">second</span>"
        );
    }

    #[test]
    fn split_target_that_cannot_be_located_reports_a_warning() {
        // A self-closing element (no separate closing tag) cannot hold split spans.
        let mut body = "<rect id=\"bar1\" x=\"0\"/>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] #bar1 : chars fade-in 400ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert!(warnings[0].contains("cannot split"));
    }

    #[test]
    fn duplicate_split_of_the_same_target_is_a_problem() {
        let mut body = "<h1 class=\"title\">Hi</h1>".to_string();
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] .title : chars fade-in 400ms\n\
             [click 1] .title : words pop 300ms\n"
                .to_string(),
        )];
        let script = extract(0, &reserved, &mut body, "", &mut warnings);
        assert!(script.is_empty());
        assert!(warnings[0].contains("more than one track"));
    }

    #[test]
    fn target_lookup_finds_ids_produced_by_shapes() {
        let mut body = "<div class=\"pane\"></div>".to_string();
        let shapes = "<svg><rect id=\"box1\"/></svg>";
        let mut warnings = Vec::new();
        let reserved = vec![(
            BlockKind::Anim,
            "[enter] #box1 : fade-in 400ms\n".to_string(),
        )];
        let script = extract(0, &reserved, &mut body, shapes, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!script.is_empty());
    }

    #[test]
    fn steps_and_track_helpers_are_reexported_correctly() {
        // Sanity check that this module is calling the crate API it thinks it is.
        let doc = track("[click 2] .a : fade-in 100ms\n");
        assert_eq!(mirzam_anim::steps(&doc), 2);
    }
}
