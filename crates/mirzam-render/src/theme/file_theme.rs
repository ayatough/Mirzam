//! A theme somebody wrote, in a file of their own.
//!
//! `theme: themes/acme.css` names a stylesheet the way `theme: nord` names a
//! built-in, and this is what the renderer knows about one: its stem, its
//! text, and whether it is written in a way that makes the stem mean anything.
//!
//! **The asymmetry, which is documented rather than hidden.** A built-in theme
//! is a token set loaded *before* `base.css`; a file theme may write any rule
//! and is loaded *after* it. That is what lets a file theme override type at
//! all — the shared stylesheet is what it has to sit on top of. The cost is
//! that plain rules cascade by specificity and source order, so they apply to
//! the deck and to nothing smaller, while custom properties inherit and so
//! resolve inwards to a slide or a pane for free:
//!
//! | Written as | Applies to | Works with a pane's `theme=` |
//! |---|---|---|
//! | tokens (`--mz-*`) | deck, slide, pane | yes |
//! | rules (`h1 { }`, `.foo { }`) | the deck | no |
//!
//! **Registering a stem does not make a stylesheet scopable.** A file that
//! writes `:root { --mz-accent1: … }` sets its tokens on the document, and a
//! pane carrying `data-theme="acme"` picks up nothing at all. So the rule is:
//! a file theme is usable in a pane's or a slide's `theme=` **if, and only if,
//! it scopes its tokens to its own stem** — `[data-theme="acme"] { … }`, the
//! selector the built-ins use, minus the `:where()` they only need so a deck's
//! own stylesheet can outrank them. [`FileTheme::scopes_to_stem`] is that
//! test, and [`super::scope_warnings`] is where an author is told.

use std::collections::{BTreeMap, BTreeSet};

/// A stylesheet named by `theme:`, once the host has read it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FileTheme {
    /// The filename stem — `themes/acme.css` registers as `acme`. This is the
    /// name a slide or a pane writes in `theme=`.
    pub name: String,
    /// The path as the deck wrote it, so a diagnostic can name the file the
    /// author has open rather than the stem this crate invented.
    pub path: String,
    /// The stylesheet itself, inlined after `base.css`.
    pub css: String,
}

impl FileTheme {
    /// The theme a path and its contents make. The stem is the file name
    /// without its extension, lowercased: `theme=` names are compared
    /// literally, and a deck that writes `theme=acme` for `Acme.css` has made
    /// the kind of mistake nothing later can diagnose.
    pub fn new(path: &str, css: impl Into<String>) -> Self {
        let stem = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .trim_end_matches(".css")
            .trim_end_matches(".CSS")
            .to_ascii_lowercase();
        Self {
            name: stem,
            path: path.to_string(),
            css: css.into(),
        }
    }

    /// Whether the file scopes its tokens to its own stem, which is what makes
    /// the stem usable in a `theme=`.
    ///
    /// Comments are stripped first: a theme's header comment is the most
    /// likely place in the file to *mention* the selector without writing it,
    /// and answering "yes" because of a sentence would make this check worse
    /// than not having one.
    pub fn scopes_to_stem(&self) -> bool {
        let css = strip_comments(&self.css);
        let name = &self.name;
        [
            format!("[data-theme=\"{name}\"]"),
            format!("[data-theme='{name}']"),
            format!("[data-theme={name}]"),
        ]
        .iter()
        .any(|needle| css.contains(needle.as_str()))
    }
}

/// Comments removed, so a `:` inside one cannot be read as a declaration's
/// separator and swallow the token that follows it.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        match rest[open + 2..].find("*/") {
            Some(close) => rest = &rest[open + 2 + close + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Which palette a block of declarations belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Palette {
    /// Neither mode is named, so these values paint both.
    Both,
    Light,
    Dark,
}

/// Custom properties declared in one rule, and which palette that rule is for.
struct Block {
    palette: Palette,
    decls: BTreeMap<String, String>,
}

/// Every custom property a stylesheet declares, grouped by the palette its
/// rule belongs to.
///
/// A hand-rolled scan rather than a CSS parser: the question is only "which
/// `--x: value` pairs are written under a selector that names a mode", and a
/// parser would be a dependency shipped into every deck for one diagnostic.
/// What it understands is a selector, a declaration, and `@media` around
/// either — which is the whole of how a theme file is written.
fn blocks(css: &str) -> Vec<Block> {
    let css = strip_comments(css);
    let mut out = Vec::new();
    scan(&css, "", &mut out);
    out
}

fn scan(css: &str, media: &str, out: &mut Vec<Block>) {
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let prelude = rest[..open].trim().to_string();
        let body_start = open + 1;
        let mut depth = 1usize;
        let mut i = body_start;
        for (at, c) in rest[body_start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        i = body_start + at;
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return; // unbalanced; nothing further can be trusted
        }
        let body = &rest[body_start..i];
        if prelude.starts_with('@') {
            // An at-rule holding rules of its own: `@media`, `@supports`.
            // Anything else (`@font-face`, `@keyframes`) declares no theme
            // tokens and is skipped rather than misread as a selector.
            if prelude.starts_with("@media") || prelude.starts_with("@supports") {
                let inner = format!("{media} {prelude}");
                scan(body, &inner, out);
            }
        } else {
            let context = without_not(&format!("{media} {prelude}").to_ascii_lowercase());
            let palette = if context.contains("data-mode=\"light\"")
                || context.contains("data-mode='light'")
                || context.contains("prefers-color-scheme: light")
                || context.contains("prefers-color-scheme:light")
            {
                Palette::Light
            } else if context.contains("data-mode=\"dark\"")
                || context.contains("data-mode='dark'")
                || context.contains("prefers-color-scheme: dark")
                || context.contains("prefers-color-scheme:dark")
            {
                Palette::Dark
            } else {
                Palette::Both
            };
            let mut decls = BTreeMap::new();
            for decl in body.split(';') {
                if let Some((name, value)) = decl.split_once(':') {
                    let name = name.trim();
                    if name.starts_with("--") && !name.contains('{') {
                        decls.insert(name.to_string(), value.trim().to_string());
                    }
                }
            }
            out.push(Block { palette, decls });
        }
        rest = &rest[i + 1..];
    }
}

/// A selector with its `:not(…)` groups removed, so a mode named inside an
/// exclusion is not read as the mode the block is for. The built-in themes
/// guard their dark blocks with `:not([data-mode="light"])`, and a theme
/// copied from one of them carries that guard — without this, every such block
/// would be classified as the mode it exists to avoid.
fn without_not(selector: &str) -> String {
    let mut out = String::with_capacity(selector.len());
    let mut rest = selector;
    while let Some(at) = rest.find(":not(") {
        out.push_str(&rest[..at]);
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in rest[at + 4..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(at + 4 + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(end) => rest = &rest[end..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// One palette's declarations laid over the ones that apply in both modes.
fn merged<'a>(
    base: &BTreeMap<&'a str, &'a str>,
    side: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, &'a str> {
    let mut out = base.clone();
    out.extend(side.iter());
    out
}

/// Whether a declared value is a colour this crate can compare — which is also
/// the test for "this token has to be said twice, once per mode".
fn is_colour(value: &str) -> bool {
    let v = value.trim();
    (v.starts_with('#') && v.len() >= 4 && v[1..].chars().all(|c| c.is_ascii_hexdigit()))
        || v.starts_with("rgb(")
        || v.starts_with("rgba(")
        || v.starts_with("hsl(")
        || v.starts_with("hsla(")
        || v.starts_with("oklch(")
}

/// The contrast floors the built-in themes are held to, applied to a theme
/// somebody wrote. Body text is WCAG 1.4.3 at 4.5:1; a chart mark only has to
/// be distinguishable, 1.4.11 at 3:1.
const PAIRS: &[(&str, &str, f64)] = &[
    ("--mz-fg", "--mz-slide-bg", 4.5),
    ("--mz-muted", "--mz-slide-bg", 4.5),
    ("--mz-accent1", "--mz-slide-bg", 4.5),
    ("--mz-accent2", "--mz-slide-bg", 4.5),
    ("--mz-chart3", "--mz-slide-bg", 3.0),
    ("--mz-chart4", "--mz-slide-bg", 3.0),
    ("--mz-chart5", "--mz-slide-bg", 3.0),
    ("--mz-chart6", "--mz-slide-bg", 3.0),
    ("--mz-fg", "--mz-surface", 4.5),
    ("--mz-muted", "--mz-surface", 4.5),
    ("--mz-danger-fg", "--mz-danger-bg", 4.5),
    ("--mz-h3-color", "--mz-slide-bg", 4.5),
    ("--mz-strong-color", "--mz-slide-bg", 4.5),
    ("--mz-quote-fg", "--mz-slide-bg", 4.5),
    ("--mz-th-fg", "--mz-surface", 4.5),
    ("--mz-code-fg", "--mz-code-bg", 4.5),
    ("--mz-fg", "--mz-code-bg", 4.5),
    ("--mz-fg", "--mz-card-bg", 4.5),
];

/// What `check` says about the themes a deck loaded from files.
///
/// These are the gates the built-in themes have always been held to, pointed
/// at a theme somebody wrote — because the trap they catch is one only a
/// custom theme can fall into. A one-palette stylesheet overrides the built-in
/// tokens in *both* modes (they are wrapped in `:where()` and carry no
/// specificity), so `D` in the viewer moves `data-mode` and nothing on screen
/// changes. That is not a theme with a missing feature; it is a deck that
/// looks broken, and the author of the file is the only person who can fix it.
///
/// Ordered by file, then by kind, so a deck loading two themes reads as two
/// paragraphs rather than as interleaved noise.
pub fn file_theme_warnings(themes: &[FileTheme]) -> Vec<String> {
    let mut out = Vec::new();
    for theme in themes {
        let name = &theme.name;
        let path = &theme.path;
        if super::known_theme(name).is_some() {
            out.push(format!(
                "theme: `{path}` registers as `{name}`, which is a built-in theme, so \
                 `theme={name}` keeps meaning the built-in. Rename the file if you meant a \
                 theme of your own — its rules still load either way."
            ));
        }

        let blocks = blocks(&theme.css);
        let of = |want: Palette| -> BTreeMap<&str, &str> {
            blocks
                .iter()
                .filter(|b| b.palette == want)
                .flat_map(|b| b.decls.iter())
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect()
        };
        let (both, light, dark) = (of(Palette::Both), of(Palette::Light), of(Palette::Dark));
        let colours = |m: &BTreeMap<&str, &str>| -> BTreeSet<String> {
            m.iter()
                .filter(|(_, v)| is_colour(v))
                .map(|(k, _)| (*k).to_string())
                .collect()
        };

        // One palette, and so no second mode: the diagnostic this whole family
        // exists for.
        let unmoded = colours(&both);
        if light.is_empty() && dark.is_empty() && !unmoded.is_empty() {
            // Which half is missing is a question the file already answers: a
            // theme painting a dark slide is missing its light mode. Read off
            // the slide's own background, so the block the message names is
            // the block the author has to write.
            let missing = match both
                .get("--mz-slide-bg")
                .and_then(|bg| super::contrast_ratio(bg, "#ffffff"))
            {
                Some(against_white) if against_white > 2.0 => "light",
                _ => "dark",
            };
            let scope = if theme.scopes_to_stem() {
                format!("[data-theme=\"{name}\"][data-mode=\"{missing}\"]")
            } else {
                format!(":root[data-mode=\"{missing}\"]")
            };
            out.push(format!(
                "theme: `{path}` paints in one palette: {} colour tokens, and no second mode \
                 anywhere in the file. Your values outrank the built-in tokens in *both* \
                 modes, so `D` in the viewer changes nothing this theme paints. Give it the \
                 other half — `{scope} {{ … }}`, which outranks your own block — or set only \
                 what is the same in both.",
                unmoded.len()
            ));
        } else if !light.is_empty() || !dark.is_empty() {
            // A block for one mode and not the other means the tokens outside
            // it are the other mode, which is how the sample themes are
            // written; either way, a colour named in one and not the other
            // keeps that value where it does not belong.
            let (light_side, dark_side) = match (light.is_empty(), dark.is_empty()) {
                (false, true) => (colours(&light), colours(&both)),
                (true, false) => (colours(&both), colours(&dark)),
                _ => (colours(&light), colours(&dark)),
            };
            for (mine, theirs, set, unset) in [
                (&dark_side, &light_side, "dark", "light"),
                (&light_side, &dark_side, "light", "dark"),
            ] {
                let missing: Vec<&String> = mine.difference(theirs).collect();
                if let Some(first) = missing.first() {
                    let rest = match missing.len() {
                        1 => String::new(),
                        n => format!(" (and {} more)", n - 1),
                    };
                    out.push(format!(
                        "theme: `{path}` sets `{first}` for {set} but not for {unset}{rest}, so \
                         it keeps its {set} value on a {unset} slide."
                    ));
                }
            }
        }

        // Legibility, on the values that actually apply in each mode: a block
        // for one mode overrides the unmoded block rather than replacing it,
        // and a theme with only one mode-specific block is saying that the
        // unmoded one is the other mode.
        let per_mode = match (light.is_empty(), dark.is_empty()) {
            (true, true) => vec![("both modes", both.clone())],
            (false, true) => vec![("light", merged(&both, &light)), ("dark", both.clone())],
            (true, false) => vec![("dark", merged(&both, &dark)), ("light", both.clone())],
            (false, false) => vec![
                ("light", merged(&both, &light)),
                ("dark", merged(&both, &dark)),
            ],
        };
        for (mode, effective) in per_mode {
            for (fg, bg, need) in PAIRS {
                let (Some(f), Some(b)) = (effective.get(fg), effective.get(bg)) else {
                    continue;
                };
                let Some(ratio) = super::contrast_ratio(f, b) else {
                    continue; // not a plain hex colour; nothing to measure
                };
                if ratio < *need {
                    out.push(format!(
                        "theme: `{path}` in {mode}: `{fg}` ({f}) on `{bg}` ({b}) is {ratio:.2}:1, \
                         under the {need}:1 floor — that text is not legible on that background."
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stem_is_the_file_name_without_its_extension() {
        assert_eq!(FileTheme::new("themes/acme.css", "").name, "acme");
        assert_eq!(FileTheme::new("acme.css", "").name, "acme");
        assert_eq!(FileTheme::new("../shared/Acme.CSS", "").name, "acme");
    }

    /// The whole of the stem rule: tokens on `:root` set the document, and a
    /// pane asking for the theme by name gets nothing.
    #[test]
    fn scoping_to_the_stem_is_what_makes_the_name_mean_something() {
        let scoped = FileTheme::new("acme.css", "[data-theme=\"acme\"] { --mz-fg: #111; }");
        assert!(scoped.scopes_to_stem());
        let unscoped = FileTheme::new("acme.css", ":root { --mz-fg: #111; }");
        assert!(!unscoped.scopes_to_stem());
        // A header comment that talks about the selector is not the selector.
        let talked_about = FileTheme::new(
            "acme.css",
            "/* wrap this in [data-theme=\"acme\"] one day */\n:root { --mz-fg: #111; }",
        );
        assert!(!talked_about.scopes_to_stem());
    }

    #[test]
    fn one_palette_is_reported_with_the_block_to_add() {
        let theme = FileTheme::new(
            "themes/acme.css",
            ":root { --mz-slide-bg: #0d1117; --mz-fg: #e9edf5; }",
        );
        let warnings = file_theme_warnings(&[theme]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("one palette"), "{}", warnings[0]);
        // The theme paints a dark slide, so the half it is missing is light.
        assert!(
            warnings[0].contains(":root[data-mode=\"light\"]"),
            "{}",
            warnings[0]
        );
    }

    /// The same file with a second mode says nothing — and the suggestion a
    /// scoped theme gets is the scoped selector, not `:root`.
    #[test]
    fn two_palettes_pass_and_a_scoped_theme_is_told_its_own_selector() {
        let ok = FileTheme::new(
            "acme.css",
            "[data-theme=\"acme\"] { --mz-slide-bg: #ffffff; --mz-fg: #101010; }\n\
             [data-theme=\"acme\"][data-mode=\"dark\"] { --mz-slide-bg: #101010; --mz-fg: #f4f4f4; }",
        );
        assert!(file_theme_warnings(&[ok]).is_empty());

        // And a theme painting a white slide is told to write the dark half,
        // not the one it already has.
        let one = FileTheme::new(
            "acme.css",
            "[data-theme=\"acme\"] { --mz-slide-bg: #ffffff; --mz-fg: #101010; }",
        );
        assert!(
            file_theme_warnings(&[one])[0].contains("[data-theme=\"acme\"][data-mode=\"dark\"]")
        );
    }

    /// A token named in one mode and not the other keeps the wrong value,
    /// which is how a dark panel ends up on a white slide.
    #[test]
    fn a_token_set_in_one_mode_only_is_reported() {
        let theme = FileTheme::new(
            "acme.css",
            ":root { --mz-slide-bg: #101010; --mz-fg: #f4f4f4; --mz-surface: #202020; }\n\
             :root[data-mode=\"light\"] { --mz-slide-bg: #ffffff; --mz-fg: #101010; }",
        );
        let warnings = file_theme_warnings(&[theme]);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("`--mz-surface` for dark but not for light")),
            "{warnings:?}"
        );
    }

    /// The contrast floor, on the values that actually apply in each mode:
    /// the light block here overrides the background and inherits the text.
    #[test]
    fn illegible_text_is_reported_per_mode() {
        let theme = FileTheme::new(
            "acme.css",
            ":root { --mz-slide-bg: #101010; --mz-fg: #cccccc; }\n\
             :root[data-mode=\"light\"] { --mz-slide-bg: #ffffff; --mz-fg: #cccccc; }",
        );
        let warnings = file_theme_warnings(&[theme]);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("in light") && w.contains("--mz-fg")),
            "{warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("in dark")),
            "the dark pair is legible: {warnings:?}"
        );
    }

    /// A media query is a mode too — the shape the built-in themes are written
    /// in, and one a copied theme will be written in.
    #[test]
    fn prefers_color_scheme_counts_as_the_other_mode() {
        let theme = FileTheme::new(
            "acme.css",
            "[data-theme=\"acme\"] { --mz-slide-bg: #ffffff; --mz-fg: #101010; }\n\
             @media (prefers-color-scheme: dark) {\n\
             [data-theme=\"acme\"] { --mz-slide-bg: #101010; --mz-fg: #f4f4f4; }\n}",
        );
        assert!(file_theme_warnings(&[theme]).is_empty());
    }

    /// The guard the built-in themes write on their dark blocks —
    /// `:not([data-mode="light"])` — names the *other* mode inside an
    /// exclusion. A theme copied from one of them carries it, and reading it
    /// as the block's own mode turned every such theme into a theme with two
    /// light palettes and no dark one.
    #[test]
    fn a_mode_named_inside_a_not_is_not_the_blocks_mode() {
        let theme = FileTheme::new(
            "acme.css",
            "[data-theme=\"acme\"] { --mz-slide-bg: #ffffff; --mz-fg: #101010; }\n\
             [data-mode=\"dark\"] [data-theme=\"acme\"]:not([data-mode=\"light\"] *) {\n\
             --mz-slide-bg: #101010; --mz-fg: #f4f4f4; }",
        );
        assert!(
            file_theme_warnings(&[theme]).is_empty(),
            "a dark block guarded against light is a dark block"
        );
    }

    /// A stem that collides with a built-in does not silently redefine what
    /// that name means.
    #[test]
    fn a_stem_colliding_with_a_built_in_warns() {
        let theme = FileTheme::new("themes/nord.css", "[data-theme=\"nord\"] { --x: 1px; }");
        let warnings = file_theme_warnings(&[theme]);
        assert!(warnings[0].contains("built-in theme"), "{warnings:?}");
    }

    /// Type is not a palette: a theme that only names faces and sizes is not a
    /// theme with one palette, and telling it to add a second would be noise.
    #[test]
    fn a_theme_that_sets_no_colour_is_not_a_one_palette_theme() {
        let theme = FileTheme::new(
            "acme.css",
            "[data-theme=\"acme\"] { --mz-font: Inter, sans-serif; --mz-h1-weight: 300; }",
        );
        assert!(file_theme_warnings(&[theme]).is_empty());
    }
}
