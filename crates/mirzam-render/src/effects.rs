//! The `effects` extraction pass: presenter-triggered flourishes bound to keys.
//!
//! ```text
//! 1 : flash
//! s : shake
//! e : burst 🎉
//! m : danmaku "this bit matters"
//! ```
//!
//! **These are not animations, and the difference is the whole design.** An
//! animation belongs to the document: ordered, deterministic, and present in
//! the PDF. An effect belongs to the *performance*: it fires when the speaker
//! presses a key, it never reaches the exported file, and nothing is lost if
//! it never fires at all. So the two share the runtime's primitives and
//! nothing else — separate block, separate script, and the script ships only
//! into a deck that declares effects.
//!
//! The registry contract is [C4]; the effects themselves live in
//! `theme/effects.js`.
//!
//! [C4]: ../../../docs/workstreams.md#c4-effect-registry

/// Effects that take a text or emoji argument; the rest ignore one.
const TAKES_ARG: &[&str] = &["burst", "danmaku"];

/// Every effect `theme/effects.js` knows how to draw.
const EFFECTS: &[&str] = &[
    "flash", "shake", "burst", "danmaku", "lines", "boom", "confetti",
];

/// Keys the viewer already owns. Binding one of these would shadow navigation,
/// which the presenter needs far more than a flourish.
const RESERVED: &[&str] = &[
    "arrowleft",
    "arrowright",
    "pageup",
    "pagedown",
    "home",
    "end",
    "escape",
    " ",
    "n",
    "f",
    "l",
    "d",
];

struct Binding {
    key: String,
    effect: String,
    arg: Option<String>,
}

/// Parses one slide's `effects` blocks, returning the `<script>` tag to append.
/// A bad line is a warning and drops the whole block, as with `anim`.
pub fn extract(slide_index: usize, blocks: &[String], warnings: &mut Vec<String>) -> String {
    if blocks.iter().all(|b| b.trim().is_empty()) {
        return String::new();
    }
    let mut bindings: Vec<Binding> = Vec::new();
    let mut problems = Vec::new();

    for block in blocks {
        for (ln, raw) in block.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            match parse_line(line) {
                Ok(b) => {
                    if bindings.iter().any(|other| other.key == b.key) {
                        problems.push(format!(
                            "effects line {}: `{}` is bound twice",
                            ln + 1,
                            b.key
                        ));
                    } else {
                        bindings.push(b);
                    }
                }
                Err(e) => problems.push(format!("effects line {}: {e}", ln + 1)),
            }
        }
    }

    if !problems.is_empty() {
        for p in problems {
            warnings.push(format!("slide {}: {p}", slide_index + 1));
        }
        return String::new();
    }
    if bindings.is_empty() {
        return String::new();
    }

    let items: Vec<String> = bindings
        .iter()
        .map(|b| {
            let arg = match &b.arg {
                Some(a) => format!(",\"arg\":{}", json_string(a)),
                None => String::new(),
            };
            format!(
                "{{\"key\":{},\"effect\":\"{}\"{arg}}}",
                json_string(&b.key),
                b.effect
            )
        })
        .collect();
    format!(
        "<script type=\"application/json\" class=\"mz-fx\">[{}]</script>\n",
        items.join(",")
    )
}

fn parse_line(line: &str) -> Result<Binding, String> {
    let (key, rest) = line
        .split_once(':')
        .ok_or("a binding is written `key : effect`")?;
    let key = key.trim();
    if key.chars().count() != 1 {
        return Err(format!(
            "`{key}` is not a single key; bind one character, such as `1` or `e`"
        ));
    }
    if RESERVED.contains(&key.to_lowercase().as_str()) {
        return Err(format!(
            "`{key}` is taken by the viewer (navigation, notes, fullscreen, layout, mode)"
        ));
    }

    let rest = rest.trim();
    let (name, arg) = match rest.split_once(char::is_whitespace) {
        Some((n, a)) => (n, Some(a.trim())),
        None => (rest, None),
    };
    if !EFFECTS.contains(&name) {
        return Err(format!(
            "unknown effect `{name}` (known: {})",
            EFFECTS.join(", ")
        ));
    }
    let takes = TAKES_ARG.contains(&name);
    let arg = match (takes, arg.filter(|a| !a.is_empty())) {
        (true, Some(a)) => Some(a.trim_matches('"').to_string()),
        (true, None) => {
            return Err(format!(
                "`{name}` needs something to show, e.g. `{name} 🎉`"
            ))
        }
        (false, Some(a)) => return Err(format!("`{name}` takes no argument (got `{a}`)")),
        (false, None) => None,
    };
    Ok(Binding {
        key: key.to_string(),
        effect: name.to_string(),
        arg,
    })
}

/// A JSON string literal. The result is embedded in a `<script>` element, so
/// `<` is escaped as well — otherwise a `</script>` inside a danmaku line
/// would close the block early.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Whether any section binds an effect, deciding if `effects.js` is inlined.
pub fn deck_has_effects(sections: &[String]) -> bool {
    sections.iter().any(|s| s.contains("class=\"mz-fx\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> (String, Vec<String>) {
        let mut w = Vec::new();
        let out = extract(0, &[src.to_string()], &mut w);
        (out, w)
    }

    #[test]
    fn no_blocks_emit_nothing() {
        let mut w = Vec::new();
        assert!(extract(0, &[], &mut w).is_empty());
        assert!(w.is_empty());
    }

    #[test]
    fn binds_keys_to_effects() {
        let (out, w) = one("1 : flash\ns : shake\ne : burst 🎉\n");
        assert!(w.is_empty(), "{w:?}");
        assert!(out.starts_with("<script type=\"application/json\" class=\"mz-fx\">["));
        assert!(out.contains(r#"{"key":"1","effect":"flash"}"#), "{out}");
        assert!(out.contains(r#"{"key":"s","effect":"shake"}"#), "{out}");
        assert!(
            out.contains(r#"{"key":"e","effect":"burst","arg":"🎉"}"#),
            "{out}"
        );
    }

    #[test]
    fn danmaku_keeps_its_quoted_text() {
        let (out, w) = one("m : danmaku \"this bit matters\"\n");
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("this bit matters"));

        // Multi-byte text survives the round trip intact.
        let (out, w) = one("m : danmaku \"そこ、大事です\"\n");
        assert!(w.is_empty(), "{w:?}");
        assert!(out.contains("そこ、大事です"));
    }

    #[test]
    fn a_reserved_key_is_refused() {
        let (out, w) = one("n : flash\n");
        assert!(out.is_empty());
        assert!(w[0].contains("taken by the viewer"), "{w:?}");
    }

    #[test]
    fn an_unknown_effect_is_refused() {
        let (out, w) = one("1 : sparkle\n");
        assert!(out.is_empty());
        assert!(w[0].contains("unknown effect"));
    }

    #[test]
    fn a_multi_character_key_is_refused() {
        let (_, w) = one("ctrl : flash\n");
        assert!(w[0].contains("single key"));
    }

    #[test]
    fn binding_the_same_key_twice_is_refused() {
        let (_, w) = one("1 : flash\n1 : shake\n");
        assert!(w[0].contains("bound twice"));
    }

    #[test]
    fn an_effect_that_needs_an_argument_says_so() {
        let (_, w) = one("e : burst\n");
        assert!(w[0].contains("needs something to show"));
    }

    #[test]
    fn an_effect_that_takes_none_refuses_one() {
        let (_, w) = one("1 : flash loudly\n");
        assert!(w[0].contains("takes no argument"));
    }

    #[test]
    fn a_script_close_tag_in_text_cannot_break_out_of_the_block() {
        let (out, w) = one("m : danmaku \"</script><img onerror=x>\"\n");
        assert!(w.is_empty(), "{w:?}");
        assert!(!out.contains("</script><img"), "{out}");
        assert!(out.contains("\\u003c/script"), "{out}");
    }
}
