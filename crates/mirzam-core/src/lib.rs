//! Deck metadata (frontmatter) and the evaluator for `{{ variable/expression }}`.

mod expr;

pub use expr::{eval_expr, Value};

use serde::Deserialize;
use std::collections::BTreeMap;

/// Deck settings declared in frontmatter.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeckMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub theme: Option<String>,
    /// Forces light or dark mode. Unset defers to the viewer's
    /// `prefers-color-scheme`, overridable there with `?mode=` or `D`.
    pub mode: Option<String>,
    /// Aspect ratio, e.g. "16:9" or "4:3".
    pub aspect: Option<String>,
    /// Path to a custom stylesheet, relative to the input file.
    pub css: Option<String>,
    /// Start a new slide at every heading of this level: "h1", "h2", "h3".
    /// Slides always break on `---` as well.
    pub split: Option<String>,
    /// How pages turn, e.g. "fade" or "slide-left 400ms". A slide that
    /// declares its own whole-slide `[enter]`/`[exit]` track overrides the
    /// matching half. Parsed by `mirzam_anim::parse_transition`.
    pub transition: Option<String>,
    /// `fit: shrink` asks every pane to shrink its text rather than clip it.
    /// Panes opt in individually with `{fit=shrink}`; this is the same thing
    /// said once for the whole deck.
    pub fit: Option<String>,
    /// Which syntax `$...$` holds: `latex` (the default) or `typst`.
    /// Per deck, not per formula — a deck reads as one language.
    pub math: Option<String>,
    /// Named layouts a slide can be drawn on instead of carrying a `pane`
    /// block of its own. A slide picks one with `<!-- layout: name -->`.
    pub masters: Masters,
    /// The master every slide takes when it neither draws a grid nor names
    /// one. A slide opts out of it with `<!-- layout: none -->`.
    pub layout: Option<String>,
    /// Text drawn along the bottom of every slide. `{n}` and `{total}` are
    /// substituted; a slide drops it with `<!-- chrome: none -->`.
    pub footer: Option<String>,
    /// The slide's own number, drawn opposite the footer. Same substitutions,
    /// so `"{n} / {total}"` is the usual value.
    #[serde(rename = "slide-number", alias = "slide_number")]
    pub slide_number: Option<String>,
    pub vars: BTreeMap<String, serde_yaml::Value>,
}

/// Where a deck's named slide shapes come from.
///
/// Two forms because the drawings are big. A set worth sharing between decks —
/// or one long enough that it pushes the first slide off the screen — belongs
/// in a file of its own, where the ASCII sits in `pane` fences at column zero
/// rather than indented inside a YAML block scalar. A deck with one shape it
/// reuses can keep it in frontmatter and skip the second file.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Masters {
    /// `masters: masters.md`, resolved relative to the deck like `css:` is.
    File(String),
    /// `masters: {two-up: |…}`, written in the deck's own frontmatter.
    Inline(BTreeMap<String, String>),
}

impl Default for Masters {
    fn default() -> Self {
        Masters::Inline(BTreeMap::new())
    }
}

/// The syntax math is written in. Every dialect renders through the same
/// LaTeX -> MathML path; this only chooses the front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MathDialect {
    #[default]
    Latex,
    Typst,
}

impl DeckMeta {
    /// Logical slide size (width, height) for the aspect ratio. Defaults to 16:9.
    pub fn slide_size(&self) -> (u32, u32) {
        match self.aspect.as_deref() {
            Some("4:3") => (1024, 768),
            _ => (1280, 720),
        }
    }

    /// The math front end `math:` asks for. `Err` carries a warning for an
    /// unrecognised value; the deck still renders, with the default.
    pub fn math_dialect(&self) -> Result<MathDialect, String> {
        match self.math.as_deref().map(str::trim) {
            None | Some("latex") => Ok(MathDialect::Latex),
            Some("typst") => Ok(MathDialect::Typst),
            Some(other) => Err(format!(
                "math: unknown dialect `{other}`; `latex` and `typst` are supported, \
                 rendering as latex"
            )),
        }
    }

    /// The masters file this deck names, if it names one rather than writing
    /// its shapes inline. Reading it is the caller's job: the core has no
    /// filesystem, and both hosts already carry a `FileProvider`.
    pub fn masters_file(&self) -> Option<&str> {
        match &self.masters {
            Masters::File(path) => Some(path.as_str()),
            Masters::Inline(_) => None,
        }
    }

    /// Shapes written in the deck's own frontmatter; `None` when it names a
    /// file instead.
    pub fn inline_masters(&self) -> Option<&BTreeMap<String, String>> {
        match &self.masters {
            Masters::Inline(m) => Some(m),
            Masters::File(_) => None,
        }
    }

    /// Heading level that starts a new slide, if `split:` asks for one.
    pub fn split_level(&self) -> Option<u8> {
        match self.split.as_deref()?.trim().to_ascii_lowercase().as_str() {
            "h1" | "1" => Some(1),
            "h2" | "2" => Some(2),
            "h3" | "3" => Some(3),
            _ => None,
        }
    }

    /// Variable table used by the expression evaluator.
    pub fn var_table(&self) -> BTreeMap<String, Value> {
        self.vars
            .iter()
            .map(|(k, v)| {
                let val = match v {
                    serde_yaml::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
                    serde_yaml::Value::Bool(b) => Value::Str(b.to_string()),
                    serde_yaml::Value::String(s) => {
                        // Treat numeric-looking strings as numbers so they can be used in arithmetic.
                        match s.parse::<f64>() {
                            Ok(n) => Value::Num(n),
                            Err(_) => Value::Str(s.clone()),
                        }
                    }
                    other => Value::Str(
                        serde_yaml::to_string(other)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                    ),
                };
                (k.clone(), val)
            })
            .collect()
    }
}

/// Parses frontmatter as YAML.
pub fn parse_meta(yaml: &str) -> Result<DeckMeta, String> {
    serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse frontmatter: {e}"))
}

/// Evaluates and substitutes `{{ ... }}` occurrences in `text`.
/// Anything that fails to evaluate is left verbatim rather than dropped.
pub fn substitute_vars(text: &str, vars: &BTreeMap<String, Value>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let inner = &after[..end];
                match eval_expr(inner, vars) {
                    Ok(v) => out.push_str(&v.to_display()),
                    Err(_) => {
                        out.push_str("{{");
                        out.push_str(inner);
                        out.push_str("}}");
                    }
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> BTreeMap<String, Value> {
        let mut m = BTreeMap::new();
        m.insert("price".to_string(), Value::Num(1200.0));
        m.insert("product".to_string(), Value::Str("Mirzam".to_string()));
        m
    }

    #[test]
    fn substitute_simple_and_expr() {
        let v = vars();
        assert_eq!(
            substitute_vars("{{product}} costs {{price * 12}} per year", &v),
            "Mirzam costs 14400 per year"
        );
    }

    #[test]
    fn theme_and_mode_are_parsed_from_frontmatter() {
        let meta = parse_meta("theme: nord\nmode: dark\n").unwrap();
        assert_eq!(meta.theme.as_deref(), Some("nord"));
        assert_eq!(meta.mode.as_deref(), Some("dark"));
    }

    #[test]
    fn split_level_parses_forms() {
        let meta = |v: &str| parse_meta(&format!("split: {v}\n")).unwrap();
        assert_eq!(meta("h2").split_level(), Some(2));
        assert_eq!(meta("3").split_level(), Some(3));
        assert_eq!(meta("none").split_level(), None);
        assert_eq!(DeckMeta::default().split_level(), None);
    }

    /// One key, two forms: a string names a file, a mapping is the shapes
    /// themselves. A deck that says neither has no masters and no complaint.
    #[test]
    fn masters_is_either_a_path_or_the_shapes_themselves() {
        let file = parse_meta("masters: masters/cookbook.md\n").unwrap();
        assert_eq!(file.masters_file(), Some("masters/cookbook.md"));
        assert_eq!(file.inline_masters(), None);

        let inline =
            parse_meta("masters:\n  two-up: |\n    +---+\n    | a |\n    +---+\n").unwrap();
        assert_eq!(inline.masters_file(), None);
        assert!(inline.inline_masters().unwrap()["two-up"].contains("| a |"));

        let none = DeckMeta::default();
        assert_eq!(none.masters_file(), None);
        assert!(none.inline_masters().unwrap().is_empty());
    }

    #[test]
    fn unknown_var_left_as_is() {
        let v = vars();
        assert_eq!(substitute_vars("{{unknown}}", &v), "{{unknown}}");
    }

    #[test]
    fn unterminated_braces_kept() {
        let v = vars();
        assert_eq!(substitute_vars("a {{price", &v), "a {{price");
    }
}
