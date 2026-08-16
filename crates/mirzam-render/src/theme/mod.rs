//! Assembles the built-in theme's CSS and the viewer runtime JS from the
//! files in this directory, and selects a named theme's tokens.
//!
//! - `themes/*.css` — one file per built-in theme, each defining the full
//!   token set for both light and dark mode ([C3] in `docs/workstreams.md`).
//!   Every selector is wrapped in `:where()`, which contributes no
//!   specificity: a theme of the deck's own overrides tokens with a plain
//!   `:root` block, and `[data-theme="x"]` would otherwise outrank it no
//!   matter what order the stylesheets appear in.
//!
//!   The selectors are written against *any* element rather than `:root`,
//!   because a theme is not only a property of the deck: a slide or a single
//!   pane can carry `data-theme` of its own, and custom properties inherit, so
//!   setting the token block on that element re-themes everything inside it.
//!   The dark blocks therefore also have to answer "which mode is this element
//!   in", which is the nearest `data-mode` above it — hence the
//!   `:not([data-mode="light"] *)` guard, which keeps a pane pinned to light
//!   from being pulled dark by the deck around it.
//! - `base.css` — layout, typography, panes; everything that reads a token
//!   rather than defining one, shared by every theme
//! - a deck's own `.css` entries, loaded *after* `base.css` — see
//!   [`file_theme`] for what a theme somebody wrote can and cannot do
//! - `print.css` — overrides applied for PDF export
//! - `viewer.js` — the runtime shipped inside every deck
//! - `anim.js` — the animation runtime, shipped only when a deck animates
//! - `presenter.js` — the presenter window, and the link between two windows
//!
//! [C3]: ../../../docs/workstreams.md#c3-theme-tokens

pub mod file_theme;

pub use file_theme::{file_theme_warnings, FileTheme};

pub const BASE_CSS: &str = include_str!("base.css");
pub const VIEWER_JS: &str = concat!("\n", include_str!("viewer.js"));

/// The animation runtime. Inlined before `VIEWER_JS`, and only into decks that
/// actually animate something, so an unanimated deck carries none of it.
pub const ANIM_JS: &str = concat!("\n", include_str!("anim.js"));

/// Presentation effects. Inlined only into decks that bind a key to one, and
/// never into the print page: an effect is part of the performance rather than
/// the document.
pub const EFFECTS_JS: &str = concat!("\n", include_str!("effects.js"));

/// The annotation overlay. Inlined only into decks that annotate something —
/// and, unlike every other script here, into the print page as well: an
/// annotation is additive, so drawing it cannot hide content, and the PDF
/// would otherwise lose the marks the deck exists to point at.
pub const ANNOT_JS: &str = concat!("\n", include_str!("annot.js"));

/// Shrink-to-fit for panes that ask for it. Inlined into the print page too:
/// it only ever makes content smaller than a box it is already overflowing, so
/// a page that runs it shows strictly more than one that does not.
pub const FIT_JS: &str = concat!("\n", include_str!("fit.js"));

/// The presenter window and the link between two windows. Viewer-only: the
/// print page has no second window and no keys to press.
pub const PRESENTER_JS: &str = concat!("\n", include_str!("presenter.js"));

/// Print overrides applied after a theme's CSS.
/// Slide dimensions and the `@page` size are appended by `assemble_print_page`.
pub const PRINT_CSS: &str = concat!("\n", include_str!("print.css"));

/// The theme a deck renders in when it names none, and what an unrecognized
/// name falls back to. There is no separate `default` theme: a deck that
/// chooses nothing is already in the project's colours, so `theme:` is a
/// choice to look like something *else* rather than a choice to look like
/// anything at all.
pub const FALLBACK_THEME: &str = "mirzam";

/// Its tokens, held next to the name rather than looked up by it, so
/// [`theme_tokens`] always has something to return without searching for
/// itself. Dropping the fallback out of `THEMES` would then be a theme
/// nobody can name, not a function that never returns.
const FALLBACK_TOKENS: &str = include_str!("themes/mirzam.css");

/// Built-in themes, the fallback first. Each file defines the complete token
/// set for both modes; see `themes/CREDITS.md` for where each palette comes
/// from.
const THEMES: &[(&str, &str)] = &[
    (FALLBACK_THEME, FALLBACK_TOKENS),
    ("nord", include_str!("themes/nord.css")),
    ("solarized", include_str!("themes/solarized.css")),
    ("vscode", include_str!("themes/vscode.css")),
    ("wuwei", include_str!("themes/wuwei.css")),
];

pub const THEME_NAMES: &[&str] = &["mirzam", "nord", "solarized", "vscode", "wuwei"];

/// The name as a built-in theme, or `None`. The one place a theme name is
/// checked, so a name reaching the markup is always one there are tokens for.
pub fn known_theme(name: &str) -> Option<&'static str> {
    THEME_NAMES.iter().find(|n| **n == name).copied()
}

/// Token CSS for a named theme. Unknown names fall back to
/// [`FALLBACK_THEME`]'s tokens directly, so this is total for every string
/// and cannot call itself; call [`theme_warnings`] first if the name came from
/// frontmatter and an unknown name should be reported rather than silently
/// substituted.
pub fn theme_tokens(name: &str) -> &'static str {
    THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(FALLBACK_TOKENS, |(_, css)| css)
}

/// Every dial `base.css` reads through a fallback — `var(--mz-h3-color,
/// var(--mz-accent1))` contributes `h3-color`.
///
/// Read out of the stylesheet rather than kept as a list beside it, because a
/// list would be a second place to remember: a dial added to `base.css` and
/// forgotten here would be a token that leaks again, and the leak is invisible
/// in a diff. Comments are skipped so a dial merely *described* in prose is not
/// counted, the same care [`file_theme`] takes for the same reason.
///
/// The palette tokens are not in here and do not need to be: every theme
/// defines the whole of [`contrast_tests::ALL_TOKENS`] in both modes, so there
/// is nothing for an outer theme to supply. What is in here is exactly the half
/// a theme may leave unset.
fn derived_tokens() -> &'static [&'static str] {
    static TOKENS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    TOKENS.get_or_init(|| {
        let mut out: Vec<&'static str> = Vec::new();
        let mut rest = BASE_CSS;
        while !rest.is_empty() {
            let (code, after) = rest.split_at(rest.find("/*").unwrap_or(rest.len()));
            for read in code.split("var(--mz-").skip(1) {
                let len = read
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                    .unwrap_or(read.len());
                // A read with no fallback is a palette token, and a theme that
                // left it out has broken the contract rather than chosen a
                // default; resetting it would paint nothing at all.
                if len > 0 && read[len..].starts_with(',') {
                    out.push(&read[..len]);
                }
            }
            rest = match after.strip_prefix("/*").and_then(|t| t.split_once("*/")) {
                Some((_, tail)) => tail,
                None => "",
            };
        }
        out.sort_unstable();
        out.dedup();
        out
    })
}

/// The block that makes a theme a *scope*: every derived token set back to
/// `initial`, written for `name` and emitted immediately before that theme's
/// own declarations.
///
/// Custom properties inherit, and `base.css` writes its defaults as the
/// fallback half of a `var()` — so a pane wearing a theme that sets no
/// `--mz-h3-color` used to resolve the deck's, in the deck's mode: a heading
/// coloured for a dark slide, drawn on a light pane. The fallback could never
/// fire, because the token was still *defined*, just defined by somebody else.
///
/// `initial` is the one value that undefines a custom property: it is the
/// guaranteed-invalid value, so `var(--mz-h3-color, var(--mz-accent1))` falls
/// through to the fallback again — resolved on the element that carries the
/// scope, and therefore in that scope's own palette and its own mode. That is
/// also why this is a list of names and not a list of values: the defaults stay
/// written once, in `base.css`, and no theme file has to repeat them.
pub fn scope_defaults(name: &str) -> String {
    let mut out = format!(
        "/* Every derived token, undefined for `{name}` so it cannot be \
         inherited from the theme around it; `base.css`'s own fallback \
         answers instead, in this scope's palette and mode. */\n\
         :where([data-theme=\"{name}\"]) {{\n "
    );
    let mut col = 1;
    for token in derived_tokens() {
        let decl = format!(" --mz-{token}: initial;");
        if col + decl.len() > 78 {
            out.push_str("\n ");
            col = 1;
        }
        col += decl.len();
        out.push_str(&decl);
    }
    out.push_str("\n}\n");
    out
}

/// Full CSS for a page: the token set of every theme it uses, then the shared
/// layout rules.
///
/// A deck carries the tokens of the themes it actually mentions, in the order
/// given, so a deck that names none is no larger than it was before slides and
/// panes could re-theme themselves. Repeats and unknown names are dropped;
/// an empty list still yields `mirzam`, because `base.css` reads tokens that
/// have to come from somewhere.
///
/// Each theme is its reset block and then its own declarations, in that order:
/// both carry no specificity, so source order is what makes the theme's own
/// values win over the defaults it starts from.
pub fn theme_css_for(names: &[&str]) -> String {
    let mut out = String::new();
    let mut seen: Vec<&str> = Vec::new();
    for name in names {
        let Some(name) = known_theme(name) else {
            continue;
        };
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        out.push_str(&scope_defaults(name));
        out.push_str(theme_tokens(name));
    }
    if seen.is_empty() {
        out.push_str(&scope_defaults(FALLBACK_THEME));
        out.push_str(FALLBACK_TOKENS);
    }
    out.push_str(BASE_CSS);
    out
}

/// A name a slide or a pane may write in `theme=`: a built-in, or the stem of
/// a stylesheet this deck loaded (`themes/acme.css` → `acme`).
///
/// A built-in wins a collision. The alternative lets a file sitting in the
/// deck's directory silently redefine what `theme: nord` means, and a name
/// that means different things in different directories is worse than a name
/// that is taken; [`file_theme_warnings`] reports the clash.
fn scope_name<'a>(name: &str, files: &'a [FileTheme]) -> Option<&'a str> {
    if let Some(built_in) = known_theme(name) {
        return Some(built_in);
    }
    files
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.name.as_str())
}

/// The `data-theme`/`data-mode` attributes for an element *inside* the deck —
/// a slide or a pane that asks for a palette of its own.
///
/// Silently drops anything that is not a theme this deck has or a known mode,
/// the same way [`theme_attrs`](crate::assemble_page) does for the deck: an
/// element renders in the palette it inherits rather than failing the build.
/// [`scope_warnings`] is what reports the dropped name, and is called where
/// the slide is parsed rather than here.
///
/// `files` is the deck's own themes. The attribute is written for one of those
/// even when the file does not scope its tokens to its own stem — the name is
/// registered either way, and the selector that would answer it is one line in
/// a file the author can edit. Writing nothing would make the fix invisible.
pub fn scope_attrs(theme: Option<&str>, mode: Option<&str>, files: &[FileTheme]) -> String {
    let mut out = String::new();
    if let Some(name) = theme.and_then(|t| scope_name(t, files)) {
        out.push_str(&format!(" data-theme=\"{name}\""));
    }
    if let Some(m) = normalize_mode(mode) {
        out.push_str(&format!(" data-mode=\"{m}\""));
    }
    out
}

/// `default` used to be a second name for the [`FALLBACK_THEME`] palette, and
/// decks in the wild still write it. It is an unknown name now like any other,
/// but "unknown theme `default`" would send an author looking for a theme they
/// spelled correctly — so that one name gets told what to write instead.
///
/// `retired` is what the deck wrote and `write` what it should write, spelled
/// the way the place it appears spells it: `theme: x` in frontmatter,
/// `theme=x` in a slide or pane attribute.
fn retired_name_note(retired: &str, write: &str) -> Option<String> {
    (retired == "default").then(|| {
        format!(
            "`default` is no longer a theme name: it was a second name for the \
             `{FALLBACK_THEME}` palette, and one palette now has one name. \
             Write `{write}` — the colours are the same either way."
        )
    })
}

/// Warnings for a `theme=`/`mode=` pair that named something unknown — or
/// something this deck loaded but wrote in a way that cannot answer to a name.
/// Prefixed with `where` so the author is told which pane or slide to look at.
pub fn scope_warnings(
    where_: &str,
    theme: Option<&str>,
    mode: Option<&str>,
    files: &[FileTheme],
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(name) = theme {
        let own = files.iter().find(|f| f.name == name);
        if let Some(file) = own.filter(|f| known_theme(name).is_none() && !f.scopes_to_stem()) {
            // The trap this exists for: the stem is registered, the attribute
            // is written, and the stylesheet answers to nobody — so the pane
            // renders in the deck's palette and nothing says why.
            out.push(format!(
                "{where_}: `{name}` is loaded from `{}`, but that file sets its tokens outside \
                 `[data-theme=\"{name}\"]`, so this `theme={name}` picks up nothing. A file \
                 theme is usable by name only if it scopes its tokens to its own stem: wrap \
                 the token block in `[data-theme=\"{name}\"] {{ … }}`.",
                file.path
            ));
        } else if scope_name(name, files).is_none() {
            out.push(
                match retired_name_note(name, &format!("theme={FALLBACK_THEME}")) {
                    Some(note) => format!("{where_}: {note} Keeping the surrounding theme."),
                    None => format!(
                        "{where_}: unknown theme `{name}`; keeping the surrounding theme. \
                     Built-in themes: {}{}",
                        THEME_NAMES.join(", "),
                        match files.is_empty() {
                            true => String::new(),
                            false => format!(
                                ". This deck also loads: {}",
                                files
                                    .iter()
                                    .map(|f| f.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        }
                    ),
                },
            );
        }
    }
    if let Some(m) = mode {
        if normalize_mode(Some(m)).is_none() {
            out.push(format!(
                "{where_}: unknown mode `{m}`; expected `light` or `dark`"
            ));
        }
    }
    out
}

/// What frontmatter's `theme:` has to say for itself: an unknown built-in
/// name.
///
/// The stylesheets themselves are read by the host, so what a file theme has
/// to say arrives separately through [`file_theme_warnings`].
pub fn theme_warnings(meta: &mirzam_core::DeckMeta) -> Vec<String> {
    meta.theme_names()
        .into_iter()
        .filter_map(|name| theme_warning(Some(name)))
        .collect()
}

/// `None` when there is nothing to report (no theme requested, or a known
/// one); `Some(message)` when frontmatter named a theme that is not a
/// built-in, which falls back to [`FALLBACK_THEME`].
fn theme_warning(requested: Option<&str>) -> Option<String> {
    let name = requested?;
    if THEME_NAMES.contains(&name) {
        return None;
    }
    if let Some(note) = retired_name_note(name, &format!("theme: {FALLBACK_THEME}")) {
        return Some(format!(
            "{note} Or remove the key: a deck that names no theme already \
             renders in `{FALLBACK_THEME}`."
        ));
    }
    Some(format!(
        "unknown theme `{name}`; using `{FALLBACK_THEME}`. Built-in themes: {}",
        THEME_NAMES.join(", ")
    ))
}

/// Resolves frontmatter `mode:` to `"light"` or `"dark"`; `None` when unset
/// or unrecognized, which defers to `prefers-color-scheme` in the viewer.
pub fn normalize_mode(requested: Option<&str>) -> Option<&'static str> {
    match requested?.trim().to_ascii_lowercase().as_str() {
        "light" => Some("light"),
        "dark" => Some("dark"),
        _ => None,
    }
}

/// `Some(message)` when frontmatter's `mode:` was neither `light` nor `dark`.
pub fn mode_warning(requested: Option<&str>) -> Option<String> {
    let name = requested?;
    if normalize_mode(Some(name)).is_some() {
        None
    } else {
        Some(format!("unknown mode `{name}`; expected `light` or `dark`"))
    }
}

/// `@font-face` CSS embedding STIX Two Math (OFL, see assets/STIX-LICENSE.txt)
/// as a data URI. Added only to pages containing math (~540KB), so decks
/// render at TeX quality even on machines without a math font installed.
pub fn math_font_css() -> &'static str {
    use base64::Engine as _;
    use std::sync::OnceLock;
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let woff2 = include_bytes!("../../assets/stix-two-math.woff2");
        let b64 = base64::engine::general_purpose::STANDARD.encode(woff2);
        format!(
            "@font-face {{ font-family: 'STIX Two Math'; \
             src: url(data:font/woff2;base64,{b64}) format('woff2'); font-display: swap; }}\n\
             math {{ font-family: 'STIX Two Math', math; }}"
        )
    })
}

/// WCAG 2.1 contrast ratio between two `#rrggbb` colours, from 1.0 (identical)
/// to 21.0 (black on white). `None` for anything that is not a six-digit hex
/// colour — a token holding `var(...)` or a named colour cannot be checked.
///
/// Public because the sample themes under `examples/themes/` are held to the
/// same standard as the built-in ones, and the test that does so lives in
/// another crate. A theme whose dark mode was made by inverting its light mode
/// is the failure this exists to catch.
pub fn contrast_ratio(a: &str, b: &str) -> Option<f64> {
    fn luminance(hex: &str) -> Option<f64> {
        let hex = hex.trim().strip_prefix('#')?;
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let chan = |i: usize| {
            let v = f64::from(u8::from_str_radix(&hex[i..i + 2], 16).ok()?) / 255.0;
            Some(if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            })
        };
        Some(0.2126 * chan(0)? + 0.7152 * chan(2)? + 0.0722 * chan(4)?)
    }
    let (la, lb) = (luminance(a)? + 0.05, luminance(b)? + 0.05);
    Some(if la > lb { la / lb } else { lb / la })
}

/// Hot-reload client injected in `serve` mode.
/// Long-polls for changed `<section>` HTML and patches the DOM.
pub const LIVE_JS: &str = r#"
(async () => {
  let v = window.__MIRZAM_V__;
  while (true) {
    try {
      const res = await fetch('/events?v=' + v);
      const j = await res.json();
      if (j.v === v) continue;
      v = j.v;
      if (j.full) { location.reload(); return; }
      for (const [i, html] of j.changes) {
        const sec = document.querySelector(`section.slide[data-index="${i}"]`);
        if (sec) sec.outerHTML = html;
      }
      if (window.__mirzamRefresh) window.__mirzamRefresh();
    } catch (e) {
      await new Promise(r => setTimeout(r, 1000));
    }
  }
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_css_is_tokens_then_base() {
        let css = theme_css_for(&["mirzam"]);
        // Order, not position: a sheet may open with a comment, and asserting
        // on the first characters made adding one to a theme look like a
        // regression in how the CSS is assembled.
        let tokens = css.find(":where([data-theme=\"mirzam\"])");
        let base = css.find("* { box-sizing: border-box; }");
        assert!(tokens.is_some() && base.is_some());
        assert!(
            tokens < base,
            "the theme's tokens must come before base.css, or base cannot read them"
        );
        assert!(css.contains("--mz-accent1"));
        // And the scope's reset opens it: both blocks carry no specificity, so
        // source order is the whole of why the theme's own values win.
        let reset = css.find("--mz-h3-color: initial;").expect("a reset block");
        let own = css
            .find("--mz-h3-color: var(--mz-fg);")
            .expect("mirzam's own");
        assert!(reset < own && reset < base.unwrap());
    }

    /// A deck's own theme file overrides tokens with a plain `:root { }`
    /// block.
    /// `:root[data-theme="x"]` outranks that on specificity, so wrapping the
    /// built-in selectors in the zero-specificity `:where()` is the only thing
    /// keeping custom themes working - a bare selector here silently reverts
    /// every deck that ships its own palette back to the built-in one.
    #[test]
    fn built_in_theme_tokens_carry_no_specificity() {
        for (name, css) in THEMES {
            for line in css.lines().filter(|l| l.contains("data-theme=")) {
                assert!(
                    line.trim_start().starts_with(":where("),
                    "{name}: `{}` must be wrapped in :where(), or a deck's own \
                     theme file can no longer override the palette",
                    line.trim()
                );
            }
        }
    }

    #[test]
    fn unknown_theme_falls_back_to_the_fallback_palette() {
        assert_eq!(theme_tokens("nope"), theme_tokens(FALLBACK_THEME));
        // Total, not merely terminating: the fallback is a constant rather
        // than a name looked up in the table, so no input can send this
        // function looking for itself.
        assert_eq!(theme_tokens(""), FALLBACK_TOKENS);
    }

    /// Two lists of the same set, and the one that decides whether a name is
    /// accepted is not the one that supplies its tokens — so a theme in only
    /// one of them renders under its own name in somebody else's palette.
    #[test]
    fn the_name_list_and_the_token_table_hold_the_same_themes() {
        let table: Vec<&str> = THEMES.iter().map(|(n, _)| *n).collect();
        assert_eq!(table, THEME_NAMES);
        assert!(THEME_NAMES.contains(&FALLBACK_THEME));
    }

    /// A page carries the tokens of every theme it uses and no others: that is
    /// what lets a pane switch palette, and what keeps a deck that switches
    /// nothing the size it always was.
    #[test]
    fn a_page_carries_the_tokens_of_each_theme_it_uses() {
        let css = theme_css_for(&["nord", "wuwei"]);
        assert!(css.contains(":where([data-theme=\"nord\"])"));
        assert!(css.contains(":where([data-theme=\"wuwei\"])"));
        assert!(!css.contains(":where([data-theme=\"solarized\"])"));
        // base.css once, after the tokens.
        assert_eq!(css.matches("* { box-sizing: border-box; }").count(), 1);
        assert!(
            css.find(":where([data-theme=\"wuwei\"])") < css.find("* { box-sizing: border-box; }")
        );
    }

    /// Twice, not once, because a theme is now two blocks under one selector:
    /// the reset that opens the scope and the theme's own declarations. Naming
    /// it twice in the list still emits one of each.
    #[test]
    fn theme_css_for_drops_repeats_and_unknown_names_and_never_ends_up_empty() {
        let css = theme_css_for(&["nord", "nord", "nope"]);
        assert_eq!(css.matches(":where([data-theme=\"nord\"]) {").count(), 2);
        assert!(!css.contains("data-theme=\"nope\""));
        // Nothing usable named: base.css still needs tokens to read.
        let fallback = format!(":where([data-theme=\"{FALLBACK_THEME}\"])");
        assert!(theme_css_for(&["nope"]).contains(&fallback));
        assert!(theme_css_for(&[]).contains(&fallback));
    }

    #[test]
    fn scope_attrs_emits_only_what_it_recognises() {
        assert_eq!(
            scope_attrs(Some("nord"), Some("Dark"), &[]),
            " data-theme=\"nord\" data-mode=\"dark\""
        );
        assert_eq!(
            scope_attrs(Some("wuwei"), None, &[]),
            " data-theme=\"wuwei\""
        );
        // An unknown name leaves the element inheriting what surrounds it,
        // rather than dropping it to the fallback the way the deck's own
        // theme does: a pane has something to inherit and a page does not.
        assert_eq!(scope_attrs(Some("nope"), Some("sideways"), &[]), "");
        assert_eq!(scope_attrs(None, None, &[]), "");
    }

    #[test]
    fn scope_warnings_name_the_place_and_the_alternatives() {
        assert!(scope_warnings("slide 2, pane `fig`", Some("nord"), Some("dark"), &[]).is_empty());
        let w = scope_warnings("slide 2, pane `fig`", Some("nope"), None, &[]);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("pane `fig`"));
        assert!(w[0].contains("nope"));
        assert!(w[0].contains("wuwei"));
        let w = scope_warnings("slide 2", None, Some("sideways"), &[]);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("light` or `dark"));
    }

    #[test]
    fn theme_warning_flags_unknown_names_only() {
        assert!(theme_warning(None).is_none());
        assert!(theme_warning(Some("nord")).is_none());
        let w = theme_warning(Some("nope")).unwrap();
        assert!(w.contains("nope"));
        assert!(w.contains("mirzam"));
    }

    /// `default` was a second name for this palette until the duplicate sheet
    /// was deleted, and decks in the wild still write it. It takes the
    /// unknown-name path like any other typo, but a message reading "unknown
    /// theme `default`" would send an author hunting for a name they spelled
    /// correctly — so it says what to write instead, and that the slides do
    /// not change when they do.
    #[test]
    fn the_retired_default_name_says_what_to_write_instead() {
        assert!(known_theme("default").is_none());
        let w = theme_warning(Some("default")).unwrap();
        assert!(!w.contains("unknown theme"), "{w}");
        assert!(w.contains("theme: mirzam"), "{w}");
        assert!(w.contains("remove the key"), "{w}");

        let w = scope_warnings("slide 3, pane `fig`", Some("default"), None, &[]);
        assert_eq!(w.len(), 1);
        assert!(w[0].starts_with("slide 3, pane `fig`: "), "{}", w[0]);
        assert!(!w[0].contains("unknown theme"), "{}", w[0]);
        assert!(w[0].contains("theme=mirzam"), "{}", w[0]);

        // And it really is only that one name that gets the long answer.
        assert!(theme_warning(Some("defaults"))
            .unwrap()
            .contains("unknown theme"));
    }

    /// The stem rule, from the two sides a slide sees it from: a file theme
    /// that scopes its tokens to its own stem is a name a pane can use, and
    /// one that does not is the failure the diagnostics exist for — the
    /// attribute is written either way, so fixing the file is all it takes.
    #[test]
    fn a_file_theme_is_usable_by_name_only_when_it_scopes_to_its_stem() {
        let scoped = FileTheme::new(
            "themes/acme.css",
            "[data-theme=\"acme\"] { --mz-accent1: #6557d9; }",
        );
        let loose = FileTheme::new("themes/loose.css", ":root { --mz-accent1: #6557d9; }");
        let files = vec![scoped, loose];

        assert_eq!(
            scope_attrs(Some("acme"), None, &files),
            " data-theme=\"acme\""
        );
        assert!(scope_warnings("slide 1, pane `a`", Some("acme"), None, &files).is_empty());

        assert_eq!(
            scope_attrs(Some("loose"), None, &files),
            " data-theme=\"loose\"",
            "the name is registered, so the attribute is written and the fix is one line \
             in the file"
        );
        let w = scope_warnings("slide 1, pane `a`", Some("loose"), None, &files);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("themes/loose.css"), "{}", w[0]);
        assert!(w[0].contains("[data-theme=\"loose\"]"), "{}", w[0]);

        // A name that is neither built-in nor loaded is still dropped, and the
        // deck's own themes join the list of what it could have meant.
        let w = scope_warnings("slide 1", Some("nope"), None, &files);
        assert_eq!(scope_attrs(Some("nope"), None, &files), "");
        assert!(w[0].contains("acme, loose"), "{}", w[0]);
    }

    /// A file whose stem is a built-in's name does not get to redefine it.
    #[test]
    fn a_built_in_wins_a_name_collision() {
        let files = vec![FileTheme::new(
            "themes/nord.css",
            "[data-theme=\"nord\"] { --mz-accent1: #f00; }",
        )];
        assert_eq!(
            scope_attrs(Some("nord"), None, &files),
            " data-theme=\"nord\""
        );
        assert!(scope_warnings("slide 1", Some("nord"), None, &files).is_empty());
        assert!(file_theme_warnings(&files)[0].contains("built-in theme"));
    }

    #[test]
    fn mode_normalizes_case_and_rejects_junk() {
        assert_eq!(normalize_mode(Some("Dark")), Some("dark"));
        assert_eq!(normalize_mode(Some(" light ")), Some("light"));
        assert_eq!(normalize_mode(Some("solarized")), None);
        assert_eq!(normalize_mode(None), None);
        assert!(mode_warning(Some("solarized")).is_some());
        assert!(mode_warning(Some("dark")).is_none());
        assert!(mode_warning(None).is_none());
    }

    /// The promise that makes the token vocabulary safe to grow: a deck whose
    /// theme sets none of it renders exactly as it did before the dial
    /// existed. A `var(--mz-h1-size)` with no fallback is not a dial, it is a
    /// rule that evaporates in every deck that does not set it — and the
    /// damage is invisible in a diff, because the CSS is still valid and the
    /// property simply goes missing at computed-value time.
    ///
    /// Reading a token without a fallback is fine for the palette, which every
    /// built-in theme defines in both modes, and for the handful `base.css`
    /// declares itself.
    #[test]
    fn every_dial_base_css_reads_has_a_fallback() {
        // What base.css declares for itself - the viewer chrome's palette, the
        // effect colours - is always there to be read.
        let declared: Vec<&str> = BASE_CSS
            .lines()
            .filter_map(|l| l.trim().strip_prefix("--mz-"))
            .filter_map(|d| d.split(':').next())
            .collect();
        let mut offenders = Vec::new();
        for (i, line) in BASE_CSS.lines().enumerate() {
            for use_ in line.split("var(--mz-").skip(1) {
                let name = use_
                    .split([',', ')', ' '])
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
                let has_fallback = use_[name.len()..].starts_with(',');
                if has_fallback
                    || super::contrast_tests::ALL_TOKENS.contains(&name)
                    || declared.contains(&name)
                {
                    continue;
                }
                offenders.push(format!("base.css:{}: --mz-{name}", i + 1));
            }
        }
        assert!(
            offenders.is_empty(),
            "these reads have neither a fallback nor a theme obliged to define \
             them, so they render as nothing in a deck that sets no tokens:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn base_css_carries_the_debug_overlay_rules() {
        assert!(BASE_CSS.contains("html.mz-debug"));
        assert!(BASE_CSS.contains("attr(data-pane)"));
    }

    #[test]
    fn viewer_js_handles_the_debug_toggle() {
        assert!(VIEWER_JS.contains("'l'"));
        assert!(VIEWER_JS.contains("mz-debug"));
    }

    #[test]
    fn viewer_js_handles_the_mode_toggle_and_query_override() {
        assert!(VIEWER_JS.contains("'d'"));
        assert!(VIEWER_JS.contains("prefers-color-scheme"));
        assert!(VIEWER_JS.contains("get('mode')"));
        assert!(VIEWER_JS.contains("html.dataset.mode"));
    }

    #[test]
    fn print_css_neutralizes_the_debug_overlay() {
        assert!(PRINT_CSS.contains("html.mz-debug"));
    }

    /// The chrome is the viewer, not the document: nothing here may reach a
    /// printed page or a PDF.
    #[test]
    fn the_chrome_is_never_printed() {
        assert!(BASE_CSS.contains("@media print { #chrome, #keys, #notes-panel"));
    }

    /// Swiping right on a phone is a page turn. Unless the deck claims the
    /// gesture, the browser reads it as *back* and the presenter loses the deck.
    #[test]
    fn a_horizontal_swipe_belongs_to_the_deck() {
        assert!(BASE_CSS.contains("touch-action: pan-y;"));
        assert!(BASE_CSS.contains("overscroll-behavior: none;"));
    }

    /// The cheat sheet's whole reason for existing is the keys nobody can
    /// guess, which are the ones a deck binds itself. It reads the same
    /// per-slide tag `effects.js` does, so a deck that binds none - and
    /// therefore never inlines that file - still gets a working sheet.
    #[test]
    fn the_cheat_sheet_reads_this_decks_effect_bindings() {
        assert!(VIEWER_JS.contains("script.mz-fx"));
        assert!(EFFECTS_JS.contains("script.mz-fx"));
        assert!(VIEWER_JS.contains("'/'"));
    }

    /// The presenter window is another viewer, not a privileged one, and the
    /// two halves of that arrangement live in two files. `MZDeck` is the seam;
    /// this is what stops one side from being renamed without the other.
    #[test]
    fn the_presenter_window_and_the_viewer_agree_on_one_interface() {
        for member in [
            "presenting",
            "state",
            "html",
            "onChange",
            "refit",
            "sync",
            "view",
            "setView",
        ] {
            assert!(
                VIEWER_JS.contains(member),
                "viewer.js drops MZDeck.{member}"
            );
            assert!(
                PRESENTER_JS.contains(member),
                "presenter.js drops MZDeck.{member}"
            );
        }
        assert!(VIEWER_JS.contains("window.MZDeck"));
        assert!(PRESENTER_JS.contains("window.MZDeck"));
        // Absolute state, not commands: a window that opened late still lands
        // in the right place.
        assert!(PRESENTER_JS.contains("deck.sync(msg.slide, msg.step)"));
    }

    /// The long press is how a reader selects text on a phone. Binding it to
    /// the cheat sheet took that away, and a reader could no longer copy a
    /// line off a slide - so the deck must not hold that gesture, and must not
    /// read a selection drag as a page turn either.
    #[test]
    fn a_touch_deck_leaves_text_selection_alone() {
        assert!(
            !VIEWER_JS.contains("HOLD"),
            "viewer.js has taken the long press back"
        );
        assert!(VIEWER_JS.contains("getSelection()"));
    }

    /// A phone has no keyboard, so every way of driving the deck has a touch
    /// equivalent - and the sheet says so on the devices that need it.
    #[test]
    fn every_control_has_a_touch_equivalent() {
        for gesture in ["touchstart", "touchend", "pointer: coarse"] {
            assert!(
                VIEWER_JS.contains(gesture),
                "viewer.js is missing {gesture}"
            );
        }
        // The gestures above were the whole test, which let the colour mode
        // ship as a `D` binding and nothing else: on a phone there is no
        // keyboard, so a deck baked `mode: dark` could not be read in
        // sunlight. Every button in the cluster is checked for a handler now,
        // so a control reachable only by key fails here rather than on
        // somebody's phone.
        let page = crate::assemble_page(
            &mirzam_core::DeckMeta::default(),
            &[],
            &crate::PageOptions::default(),
        );
        for id in ["mz-prev", "mz-next", "mz-mode", "mz-help"] {
            assert!(
                page.contains(&format!("id=\"{id}\"")),
                "the control cluster is missing {id}"
            );
            assert!(
                VIEWER_JS.contains(&format!("getElementById('{id}')")),
                "{id} is in the markup but nothing in viewer.js binds it"
            );
        }
    }

    /// A stray `*/` turns the prose after it into CSS, and the parser's error
    /// recovery then swallows the *next* rule whole. That is not a typo you
    /// see: it shipped transparent slides, and page turns showed two slides
    /// through each other until someone noticed.
    #[test]
    fn stylesheets_have_no_stray_comment_markers() {
        // Built from THEMES rather than listed by hand, so a theme added later
        // is covered without anyone remembering to add it here.
        let sheets = [("base.css", BASE_CSS), ("print.css", PRINT_CSS)]
            .into_iter()
            .map(|(n, css)| (n.to_string(), css))
            .chain(
                THEMES
                    .iter()
                    .map(|(n, css)| (format!("themes/{n}.css"), *css)),
            );
        for (name, css) in sheets {
            // CSS comments do not nest: `/*` inside one is ordinary text, and
            // the first `*/` ends it. So the scan alternates strictly between
            // code and comment, and a `*/` found in *code* is the mistake.
            let mut rest = css;
            loop {
                let Some(open) = rest.find("/*") else {
                    assert!(
                        !rest.contains("*/"),
                        "{name}: `*/` with no comment open before it — \
                         everything after it is being parsed as CSS"
                    );
                    break;
                };
                assert!(
                    !rest[..open].contains("*/"),
                    "{name}: `*/` with no comment open before it — \
                     everything after it is being parsed as CSS"
                );
                let body = &rest[open + 2..];
                let close = body
                    .find("*/")
                    .unwrap_or_else(|| panic!("{name}: unterminated comment"));
                rest = &body[close + 2..];
            }
        }
    }

    /// The one property a page turn depends on: a slide that is not opaque
    /// lets the slide it is replacing show through it.
    #[test]
    fn slides_are_painted_opaque() {
        assert!(
            BASE_CSS.contains(
                "section.slide { background: var(--mz-slide-bg); border-radius: inherit; }"
            ),
            "the rule that makes a slide opaque is missing or reworded"
        );
    }

    #[test]
    fn the_animation_runtime_is_separate_from_the_viewer() {
        assert!(ANIM_JS.contains("window.MZAnim"));
        // The viewer must degrade when the runtime is not inlined, so it may
        // only reach for MZAnim through a guarded reference.
        assert!(!VIEWER_JS.contains("MZAnim."));
    }

    /// A carried element flies in a layer belonging to neither slide, so the
    /// layer's name is shared between the runtime that creates it and the
    /// stylesheet that positions it. It also has to sit above both slides:
    /// they are at z-index 0 and 1, and a copy behind the slide it is crossing
    /// is a copy nobody sees.
    #[test]
    fn a_carried_element_flies_above_both_slides() {
        assert!(ANIM_JS.contains("'mz-carry-layer'"));
        assert!(BASE_CSS.contains(".mz-carry-layer {"));
        let rule = BASE_CSS
            .split(".mz-carry-layer {")
            .nth(1)
            .and_then(|s| s.split('}').next())
            .unwrap_or_default();
        assert!(rule.contains("position: absolute"), "{rule}");
        assert!(rule.contains("z-index: 5"), "{rule}");
        assert!(rule.contains("pointer-events: none"), "{rule}");
    }

    /// The viewer measures both boxes at the two moments each slide is
    /// standing still, which is what keeps the flight path between resting
    /// positions. Both halves have to be called, and in that order, or a carry
    /// aims at a slide that is already sliding.
    #[test]
    fn the_viewer_carries_in_two_halves() {
        assert!(ANIM_JS.contains("carryStart(from, to, backwards)"));
        assert!(ANIM_JS.contains("carryPlay(plan)"));
        let start = VIEWER_JS
            .find("anim.carryStart(")
            .expect("viewer never starts a carry");
        let play = VIEWER_JS
            .find("anim.carryPlay(")
            .expect("viewer never finishes a carry");
        assert!(
            start < play,
            "the destination is measured before the departure"
        );
        // Measuring the departure has to come before its exit animation runs.
        let leave = VIEWER_JS.find("leave(from, backwards)").unwrap();
        assert!(
            start < leave,
            "the departing slide is measured after it starts moving"
        );
    }

    /// An effect is part of the performance, and the print page must never be
    /// able to draw one even if the script somehow reached it.
    #[test]
    fn effects_are_neutralised_in_print() {
        assert!(EFFECTS_JS.contains("mz-fx-layer"));
        assert!(PRINT_CSS.contains(".mz-fx-layer { display: none; }"));
    }

    /// The overlay ships into the print page, so it may not depend on the
    /// viewer being there — no page counter, no active slide, no MZAnim.
    #[test]
    fn the_annotation_overlay_stands_alone() {
        assert!(ANNOT_JS.contains("mz-annot-layer"));
        for viewer_only in ["MZAnim", "__mirzamGoto", "getElementById('hud')"] {
            assert!(
                !ANNOT_JS.contains(viewer_only),
                "annot.js reaches for `{viewer_only}`, which the print page does not have"
            );
        }
    }

    /// A live-reload patch replaces a whole `<section>`, so an overlay mounted
    /// once at load is an overlay drawing into a detached node afterwards —
    /// which showed up as every mark on a slide vanishing the moment its file
    /// was saved. The fix is that mounting happens inside `refresh` rather
    /// than only at startup; if `init` ever goes back to filling `layers`
    /// itself, the same bug comes back silently.
    #[test]
    fn the_overlay_remounts_itself_after_a_live_reload_patch() {
        assert!(ANNOT_JS.contains("function sync()"));
        let refresh = ANNOT_JS
            .split("function refresh(only) {")
            .nth(1)
            .expect("annot.js no longer has a refresh entry point");
        assert!(
            refresh.trim_start().starts_with("sync();"),
            "refresh must reconcile its layers before drawing them"
        );
        let init = ANNOT_JS
            .split("function init() {")
            .nth(1)
            .and_then(|s| s.split("\n  }").next())
            .unwrap_or_default();
        assert!(
            !init.contains("layers.push"),
            "init mounts layers of its own again, so a patched slide keeps the stale ones"
        );
    }

    /// `crates/mirzam-cli/src/check.js` is the only thing that can see a mark
    /// that was not drawn or an element left in its entrance state, and it can
    /// only see them by asking the runtime. Both `scripts/check-layout.mjs`
    /// (Playwright, for CI) and `mirzam check` (a headless Chromium process)
    /// load that one file rather than keeping their own copy, so it alone is
    /// the contract to check here. Rename either of these and the checker
    /// goes quiet — passing every deck, catching nothing — so the names are
    /// part of the contract rather than an implementation detail.
    #[test]
    fn the_layout_checker_can_still_ask_what_went_undrawn() {
        assert!(
            ANNOT_JS.contains("missing(sec)"),
            "annot.js no longer reports undrawn marks"
        );
        assert!(
            ANIM_JS.contains("armed(sec, step)"),
            "anim.js no longer reports elements left in their initial state"
        );
        let checker = include_str!("../../../mirzam-cli/src/check.js");
        assert!(
            checker.contains("MZAnnot.missing("),
            "the checker stopped asking"
        );
        assert!(
            checker.contains("MZAnim.armed("),
            "the checker stopped asking"
        );
    }
}

/// WCAG contrast checks over the actual shipped theme CSS text (not a
/// parallel data table), so the test can never drift from what a deck
/// renders. This is the deliverable [W3] asks for as much as the palettes
/// are: proof that a theme's dark mode was not made by inverting light mode,
/// which loses contrast against the new background.
///
/// [W3]: ../../../docs/workstreams.md#w3--named-themes-and-dark-mode
#[cfg(test)]
mod contrast_tests {
    use super::THEMES;
    use std::collections::{BTreeMap, BTreeSet};

    /// The shared implementation, so this test and the one guarding the sample
    /// themes in `examples/themes/` measure the same thing.
    fn contrast_ratio(a: &str, b: &str) -> f64 {
        super::contrast_ratio(a, b)
            .unwrap_or_else(|| panic!("expected six-digit hex colors, got `{a}` and `{b}`"))
    }

    /// Comments removed, so a `:` inside one is not mistaken for a
    /// declaration's separator — which would silently drop the token after it
    /// and quietly shrink what this test covers.
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

    /// Extracts `--mz-token: value;` pairs from the first `{ ... }` block
    /// whose selector is exactly `selector` (including the trailing ` {`),
    /// found as an exact substring so it can't match a longer selector that
    /// merely starts the same way (`:not(...)`, `[data-mode="dark"]`, ...).
    fn parse_tokens(css: &str, selector: &str) -> BTreeMap<String, String> {
        let css = strip_comments(css);
        let css = css.as_str();
        let start = css
            .find(selector)
            .unwrap_or_else(|| panic!("selector `{selector}` not found in theme CSS"));
        let body_start = css[start..].find('{').unwrap() + start + 1;
        let body_end = css[body_start..].find('}').unwrap() + body_start;
        let mut tokens = BTreeMap::new();
        for decl in css[body_start..body_end].split(';') {
            if let Some((name, value)) = decl.trim().split_once(':') {
                if let Some(token) = name.trim().strip_prefix("--mz-") {
                    tokens.insert(token.to_string(), value.trim().to_string());
                }
            }
        }
        tokens
    }

    fn light_selector(name: &str) -> String {
        format!(":where([data-theme=\"{name}\"]) {{")
    }

    /// The *last* selector of the explicit-dark block, which is the one that
    /// ends in ` {`; the block's other selector matches an element inside
    /// something dark, and is checked by `dark_tokens_reach_a_nested_element`.
    fn dark_selector(name: &str) -> String {
        format!(":where([data-theme=\"{name}\"][data-mode=\"dark\"]) {{")
    }

    fn auto_dark_selector(name: &str) -> String {
        format!(":where([data-theme=\"{name}\"]:not([data-mode=\"light\"]):not([data-mode=\"light\"] *)) {{")
    }

    /// The palette contract: what every built-in theme defines in both modes,
    /// and therefore what `base.css` may read without a fallback.
    /// `pub(super)` because `every_dial_base_css_reads_has_a_fallback` is the
    /// other half of that sentence and lives in the module next door.
    pub(super) const ALL_TOKENS: &[&str] = &[
        "bg",
        "slide-bg",
        "fg",
        "muted",
        "accent1",
        "accent2",
        "border",
        "chart3",
        "chart4",
        "chart5",
        "chart6",
        "shape-fill",
        "surface",
        "danger-bg",
        "danger-fg",
        "danger-border",
        "code-keyword",
        "code-string",
        "code-comment",
        "code-function",
        "code-number",
        "code-operator",
    ];

    /// Rendered as text directly against `--mz-slide-bg` (h1/h2/h3, links,
    /// `strong`, paragraphs, `.small`): WCAG 1.4.3 body-text contrast, 4.5:1.
    const BODY_TEXT_TOKENS: &[&str] = &["fg", "muted", "accent1"];

    /// Chart series colors: they only have to be perceivable as distinct
    /// marks, not read as text, so WCAG 1.4.11 graphical-object contrast,
    /// 3:1, applies instead of the stricter text threshold.
    const CHART_MARK_TOKENS: &[&str] = &["chart3", "chart4", "chart5", "chart6"];

    /// Text that is *not* drawn on `--mz-slide-bg`: code spans, `pre` blocks
    /// and table headers sit on `--mz-surface`, and the parse-error box on
    /// `--mz-danger-bg`. Checking every token against the slide background
    /// alone missed exactly this, and shipped a dark mode where inline code
    /// was light-on-light and unreadable.
    /// `muted` on `surface` joined the list when the viewer chrome stopped
    /// using fixed colours: the page counter is `--mz-muted` on a
    /// `--mz-surface` pill, so a theme that never intended those two to meet
    /// would ship an unreadable counter.
    const SURFACE_TEXT_PAIRS: &[(&str, &str)] = &[
        ("fg", "surface"),
        ("muted", "surface"),
        ("danger-fg", "danger-bg"),
    ];

    /// Colour dials outside the palette contract: a theme may set none of
    /// them and `base.css` supplies the value, but a theme that sets one has
    /// put text on a surface and owes the same ratio for it. These are what
    /// makes a theme an identity rather than a palette, so leaving them
    /// unmeasured would mean the contrast guarantee shrank as the vocabulary
    /// grew.
    ///
    /// `(foreground, background, minimum)`. The background is looked up
    /// through [`background`], which knows what `base.css` falls back to, so a
    /// theme that colours its inline code without moving the paper under it is
    /// still measured against the paper it will actually be drawn on.
    const IDENTITY_TEXT_PAIRS: &[(&str, &str, f64)] = &[
        ("h3-color", "slide-bg", 4.5),
        ("strong-color", "slide-bg", 4.5),
        ("quote-fg", "slide-bg", 4.5),
        ("th-fg", "surface", 4.5),
        ("code-fg", "code-bg", 4.5),
        ("metric-color", "card-bg", 4.5),
        // Not a dial itself, but the colour a code block's unhighlighted text
        // is drawn in - so moving the code background alone can still make a
        // block unreadable.
        ("fg", "code-bg", 4.5),
    ];

    /// A dial that names another token — `--mz-h3-color: var(--mz-fg)` — read
    /// back as the colour it will paint with. An identity token is usually
    /// written in terms of the palette rather than as a literal, precisely so
    /// that it has both modes for free; without following the reference every
    /// one of them would be unmeasurable and this test's coverage would be a
    /// claim about nothing.
    ///
    /// Anything that is not a bare `var()` is returned as it stands, and
    /// `contrast_ratio` declines to measure it: `4px solid var(--mz-accent1)`
    /// is a border, not a colour.
    fn resolve(tokens: &BTreeMap<String, String>, value: &str) -> Option<String> {
        let mut value = value.trim().to_string();
        for _ in 0..4 {
            let Some(rest) = value.strip_prefix("var(--mz-") else {
                return Some(value);
            };
            let name = rest.split([',', ')']).next()?.trim().to_string();
            value = tokens.get(&name)?.trim().to_string();
        }
        None
    }

    /// The surface a pair is measured against, following `base.css`'s own
    /// fallback when the theme leaves the dial unset.
    fn background(tokens: &BTreeMap<String, String>, name: &str) -> Option<String> {
        let value = match tokens.get(name) {
            Some(v) => v.clone(),
            // `.card`'s fill and a code block's paper both default to the
            // raised surface every theme defines.
            None if matches!(name, "code-bg" | "card-bg") => tokens.get("surface")?.clone(),
            None => return None,
        };
        resolve(tokens, &value)
    }

    /// Highlighted code is text, and it is drawn on `--mz-surface` like the
    /// rest of a `pre` block — so every token kind is held to the 4.5:1 body
    /// threshold there, not to the looser one a chart mark gets. Without this
    /// a theme could pick a pretty comment grey that vanishes into its own
    /// code background, which is the exact failure a highlighter invites:
    /// the colours that look best in an editor at arm's length are the ones
    /// that disappear from the back of a room.
    const CODE_TOKENS: &[&str] = &[
        "code-keyword",
        "code-string",
        "code-comment",
        "code-function",
        "code-number",
        "code-operator",
    ];

    /// Collects every failing pair rather than stopping at the first, so a
    /// single run shows the whole picture when tuning a palette.
    fn check_contrast(
        theme: &str,
        mode: &str,
        tokens: &BTreeMap<String, String>,
        failures: &mut Vec<String>,
    ) {
        let bg = &tokens["slide-bg"];
        for token in BODY_TEXT_TOKENS {
            let fg = &tokens[*token];
            let ratio = contrast_ratio(fg, bg);
            if ratio < 4.5 {
                failures.push(format!(
                    "{theme}/{mode}: --mz-{token} ({fg}) on --mz-slide-bg ({bg}) is only \
                     {ratio:.2}:1, need >= 4.5:1 for body text"
                ));
            }
        }
        for token in CHART_MARK_TOKENS {
            let fg = &tokens[*token];
            let ratio = contrast_ratio(fg, bg);
            if ratio < 3.0 {
                failures.push(format!(
                    "{theme}/{mode}: --mz-{token} ({fg}) on --mz-slide-bg ({bg}) is only \
                     {ratio:.2}:1, need >= 3.0:1 for chart marks"
                ));
            }
        }
        // Whatever a code block is actually drawn on. A theme that gives code
        // paper of its own moves every highlighted token onto it, so measuring
        // against `--mz-surface` regardless would be measuring a pair that
        // never meets.
        let code_bg = background(tokens, "code-bg").unwrap_or_else(|| tokens["surface"].clone());
        for token in CODE_TOKENS {
            let fg = &tokens[*token];
            let ratio = contrast_ratio(fg, &code_bg);
            if ratio < 4.5 {
                failures.push(format!(
                    "{theme}/{mode}: --mz-{token} ({fg}) on the code background ({code_bg}) is \
                     only {ratio:.2}:1, need >= 4.5:1 for code"
                ));
            }
        }
        for (fg_token, bg_token) in SURFACE_TEXT_PAIRS {
            let (fg, on) = (&tokens[*fg_token], &tokens[*bg_token]);
            let ratio = contrast_ratio(fg, on);
            if ratio < 4.5 {
                failures.push(format!(
                    "{theme}/{mode}: --mz-{fg_token} ({fg}) on --mz-{bg_token} ({on}) is only \
                     {ratio:.2}:1, need >= 4.5:1 for body text"
                ));
            }
        }
        for (fg_token, bg_token, need) in IDENTITY_TEXT_PAIRS {
            // A dial the theme never set is `base.css`'s answer, not this
            // theme's, and is covered by the pairs above.
            let (Some(fg), Some(on)) = (
                tokens.get(*fg_token).and_then(|v| resolve(tokens, v)),
                background(tokens, bg_token),
            ) else {
                continue;
            };
            // Not a plain colour - a border shorthand, a keyword - so there is
            // nothing to measure and nothing to complain about.
            let Some(ratio) = super::contrast_ratio(&fg, &on) else {
                continue;
            };
            if ratio < *need {
                failures.push(format!(
                    "{theme}/{mode}: --mz-{fg_token} ({fg}) on --mz-{bg_token} ({on}) is only \
                     {ratio:.2}:1, need >= {need}:1"
                ));
            }
        }
    }

    #[test]
    fn every_theme_defines_the_full_token_set_in_both_modes() {
        for (name, css) in THEMES {
            for selector in [light_selector(name), dark_selector(name)] {
                let tokens = parse_tokens(css, &selector);
                for t in ALL_TOKENS {
                    assert!(
                        tokens.contains_key(*t),
                        "{name}: `{selector}` is missing --mz-{t}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_theme_and_mode_meets_wcag_contrast() {
        let mut failures = Vec::new();
        for (name, css) in THEMES {
            let light = parse_tokens(css, &light_selector(name));
            // The cascade, not the block: the dark selector redefines the
            // palette and leaves everything mode-independent - the faces, the
            // weight ladder, a dial written as `var(--mz-fg)` - where it was
            // declared. Reading the dark block alone would measure a theme
            // half of which it could not see.
            let mut dark = light.clone();
            dark.extend(parse_tokens(css, &dark_selector(name)));
            check_contrast(name, "light", &light, &mut failures);
            check_contrast(name, "dark", &dark, &mut failures);
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }

    /// The `@media (prefers-color-scheme: dark)` block exists only so a
    /// reader's OS preference is honored without a page reload; it must be
    /// the same palette as the explicit `[data-mode="dark"]` block, or the
    /// two would silently drift out of sync over time.
    #[test]
    fn auto_dark_media_query_matches_the_explicit_dark_block() {
        for (name, css) in THEMES {
            let auto = parse_tokens(css, &auto_dark_selector(name));
            let explicit = parse_tokens(css, &dark_selector(name));
            assert_eq!(
                auto, explicit,
                "{name}: the @media (prefers-color-scheme: dark) block must match \
                 the explicit [data-mode=\"dark\"] block exactly"
            );
        }
    }

    /// Every `--mz-*` a theme's scope declares: the reset block that opens it,
    /// plus everything the theme's own file says in either mode.
    fn declared_in_scope(name: &str, css: &str) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = super::scope_defaults(name)
            .lines()
            .flat_map(|l| l.split(';'))
            .filter_map(|d| d.trim().split_once(':'))
            .filter_map(|(n, _)| n.trim().strip_prefix("--mz-").map(str::to_string))
            .collect();
        out.extend(declared_anywhere(css));
        out
    }

    /// Every `--mz-*` a stylesheet declares, whichever block it is in.
    fn declared_anywhere(css: &str) -> BTreeSet<String> {
        strip_comments(css)
            .split(';')
            .filter_map(|d| d.trim().split_once(':'))
            .filter_map(|(n, _)| n.trim().strip_prefix("--mz-").map(str::to_string))
            .collect()
    }

    /// **The leak this stream had to answer.** Custom properties inherit, so a
    /// pane wearing theme A inside a deck of theme B saw B's value for every
    /// dial A left unset — a subheading colour, a face, a weight, a margin —
    /// and saw it in B's *mode*, which is how `### Day` in a light `wuwei` pane
    /// came out in a violet mixed for a dark slide. `base.css` writing its
    /// defaults as `var(--mz-h3-color, var(--mz-accent1))` could not help: the
    /// fallback only fires when the token is undefined, and the token was
    /// defined — by the deck.
    ///
    /// So every scope has to declare every dial, and this is that promise as
    /// one assertion: for any pair of built-ins, nothing theme B can set is
    /// left for theme A's scope to inherit. It fails on the token the author
    /// found — `mirzam` sets `--mz-h3-color`, `wuwei` does not — for as long as
    /// the reset block is not there.
    #[test]
    fn no_theme_scope_can_inherit_a_dial_from_the_theme_around_it() {
        let mut leaks = Vec::new();
        for (a, css_a) in THEMES {
            let scope = declared_in_scope(a, css_a);
            for (b, css_b) in THEMES {
                if a == b {
                    continue;
                }
                for token in declared_anywhere(css_b).difference(&scope) {
                    leaks.push(format!(
                        "a pane of `{a}` inside a deck of `{b}` resolves --mz-{token} from \
                         `{b}`, in `{b}`'s mode"
                    ));
                }
            }
        }
        assert!(
            leaks.is_empty(),
            "a theme scope must start from the same defaults as every other, or it \
             wears the deck's type where its own theme is silent:\n{}",
            leaks.join("\n")
        );
    }

    /// The reset undefines rather than restates. A block that pasted
    /// `base.css`'s defaults in as values would be a second copy to keep in
    /// step — and a colour written out as a literal would be a colour for one
    /// mode, which is the half of the bug that made a light pane wear a dark
    /// deck's ink. `initial` is the guaranteed-invalid value, so the fallback
    /// in `base.css` fires again and resolves on the element carrying the
    /// scope: that scope's palette, in that scope's mode.
    #[test]
    fn the_reset_undefines_a_dial_rather_than_restating_its_value() {
        let block = super::scope_defaults("wuwei");
        assert!(block.contains(":where([data-theme=\"wuwei\"]) {"));
        for decl in block.split(';').filter(|d| d.contains("--mz-")) {
            let (_, value) = decl.rsplit_once(':').expect("a declaration");
            assert_eq!(value.trim(), "initial", "in `{}`", decl.trim());
        }
        // And it is the whole vocabulary, read out of `base.css` rather than
        // listed beside it: the dials the author counted are all in here.
        for token in [
            "h3-color",
            "strong-color",
            "strong-weight",
            "th-fg",
            "quote-fg",
            "code-bg",
            "code-fg",
            "font",
            "font-display",
            "h1-size",
            "h2-rule-w",
            "title-weight",
            "metric-color",
            "body-leading",
            "grid-pad-x",
            "grid-pad-y",
            "grid-gap",
        ] {
            assert!(
                block.contains(&format!("--mz-{token}: initial;")),
                "the reset block leaves --mz-{token} to be inherited"
            );
        }
        // The palette is not reset: every theme defines all of it in both
        // modes, so there is nothing to inherit and undefining it would paint
        // nothing at all.
        for token in ["bg", "slide-bg", "fg", "muted", "accent1", "accent2"] {
            assert!(
                !block.contains(&format!("--mz-{token}: initial;")),
                "--mz-{token} is a palette token and must not be undefined"
            );
        }
    }

    /// A slide or a pane carries `data-theme` while the deck's `data-mode`
    /// stays on `<html>`, so every theme needs a dark block that matches an
    /// element *inside* something dark. Without it, a pane that switches
    /// palette inside a dark deck comes out in that palette's light mode —
    /// white paper in the middle of a dark slide.
    #[test]
    fn dark_tokens_reach_a_nested_element() {
        for (name, css) in THEMES {
            let nested = format!(
                ":where([data-mode=\"dark\"] [data-theme=\"{name}\"]\
                 :not([data-mode=\"light\"]):not([data-mode=\"light\"] *))"
            );
            assert!(
                css.contains(&nested),
                "{name}: no dark block matches a nested element"
            );
        }
    }

    /// The mirror of the above: a pane that pins itself to `mode=light` inside
    /// a dark deck must not be dragged dark by the deck around it, which is
    /// what the `:not([data-mode="light"] *)` guard on every dark selector is
    /// for.
    #[test]
    fn a_theme_never_binds_its_tokens_to_the_root_alone() {
        for (name, css) in THEMES {
            assert!(
                !css.contains(":root[data-theme="),
                "{name}: tokens are pinned to :root, so a slide or pane cannot carry this theme"
            );
            for line in css.lines().filter(|l| l.contains("[data-mode=\"dark\"]")) {
                assert!(
                    line.contains(":not([data-mode=\"light\"] *)")
                        || line.contains("[data-theme=\"{name}\"][data-mode=\"dark\"]")
                        || line.trim().ends_with("[data-mode=\"dark\"]) {"),
                    "{name}: `{}` can pull a pane pinned to light into dark",
                    line.trim()
                );
            }
        }
    }

    /// Every dark mode must differ from its own light mode - the failure
    /// this stream exists to prevent is a theme that never actually
    /// implemented dark mode and just repeats the light tokens.
    #[test]
    fn dark_mode_is_not_a_copy_of_light_mode() {
        for (name, css) in THEMES {
            let light = parse_tokens(css, &light_selector(name));
            let dark = parse_tokens(css, &dark_selector(name));
            assert_ne!(
                light["slide-bg"], dark["slide-bg"],
                "{name}: dark mode has the same --mz-slide-bg as light mode"
            );
        }
    }

    /// A literal color in `base.css` cannot follow the theme, so in dark mode
    /// it becomes a light surface under light text - which is how inline code
    /// and table headers shipped unreadable. Some literals genuinely do not
    /// belong to a theme (text over a photograph, the debug overlay, the
    /// letterbox behind a video), so this asks for the reason to be written
    /// down next to the color rather than banning literals outright.
    #[test]
    fn base_css_takes_its_colors_from_tokens() {
        // `#deck {` is a selector, not a color, and `dec` happens to be valid
        // hex - so a run of hex digits only counts when nothing word-like
        // follows it.
        let is_hex_color = |rest: &str| {
            let n = rest.chars().take_while(char::is_ascii_hexdigit).count();
            matches!(n, 3 | 4 | 6 | 8)
                && !rest[n..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric())
        };
        let has_literal = |line: &str| {
            line.contains("rgba(")
                || line.contains("rgb(")
                || line.split('#').skip(1).any(is_hex_color)
        };
        let lines: Vec<&str> = super::BASE_CSS.lines().collect();
        let mut offenders = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            // A selector such as `#deck {` is not a color.
            let code = line.split("/*").next().unwrap_or(line);
            if !has_literal(code) {
                continue;
            }
            let excused = line.contains("theme-independent")
                || (i > 0 && lines[i - 1].contains("theme-independent"));
            if !excused {
                offenders.push(format!("base.css:{}: {}", i + 1, line.trim()));
            }
        }
        assert!(
            offenders.is_empty(),
            "hard-coded colors in base.css must use a --mz- token, or carry a \
             `theme-independent:` comment saying why they do not:\n{}",
            offenders.join("\n")
        );
    }
}
