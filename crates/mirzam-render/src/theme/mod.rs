//! Assembles the built-in theme's CSS and the viewer runtime JS from the
//! files in this directory, and selects a named theme's tokens.
//!
//! - `themes/*.css` — one file per built-in theme, each defining the full
//!   token set for both light and dark mode ([C3] in `docs/workstreams.md`).
//!   Every selector is wrapped in `:where()`, which contributes no
//!   specificity: a deck's own `css:` overrides tokens with a plain `:root`
//!   block, and `:root[data-theme="x"]` would otherwise outrank it no matter
//!   what order the stylesheets appear in.
//! - `base.css` — layout, typography, panes; everything that reads a token
//!   rather than defining one, shared by every theme
//! - `print.css` — overrides applied for PDF export
//! - `viewer.js` — the runtime shipped inside every deck
//! - `anim.js` — the animation runtime, shipped only when a deck animates
//! - `presenter.js` — the presenter window, and the link between two windows
//!
//! [C3]: ../../../docs/workstreams.md#c3-theme-tokens

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

/// Built-in themes. Each file defines the complete token set for both modes;
/// see `themes/CREDITS.md` for where each palette comes from.
const THEMES: &[(&str, &str)] = &[
    ("default", include_str!("themes/default.css")),
    ("nord", include_str!("themes/nord.css")),
    ("solarized", include_str!("themes/solarized.css")),
    ("vscode", include_str!("themes/vscode.css")),
];

pub const THEME_NAMES: &[&str] = &["default", "nord", "solarized", "vscode"];

/// Token CSS for a named theme. Unknown names fall back to `default`; call
/// [`theme_warning`] first if the name came from frontmatter and an unknown
/// name should be reported rather than silently substituted.
pub fn theme_tokens(name: &str) -> &'static str {
    THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(THEMES[0].1, |(_, css)| css)
}

/// Full CSS for a named theme: its tokens, then the shared layout rules.
pub fn theme_css(name: &str) -> String {
    format!("{}{}", theme_tokens(name), BASE_CSS)
}

/// `None` when there is nothing to report (no theme requested, or a known
/// one); `Some(message)` when frontmatter named a theme that is not a
/// built-in, which falls back to `default`.
pub fn theme_warning(requested: Option<&str>) -> Option<String> {
    let name = requested?;
    if THEME_NAMES.contains(&name) {
        None
    } else {
        Some(format!(
            "unknown theme `{name}`; using `default`. Built-in themes: {}",
            THEME_NAMES.join(", ")
        ))
    }
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
        let css = theme_css("default");
        assert!(css
            .trim_start()
            .starts_with(":where(:root[data-theme=\"default\"])"));
        assert!(css.contains("--mz-accent1"));
        assert!(css.contains("* { box-sizing: border-box; }"));
    }

    /// A deck's own `css:` overrides tokens with a plain `:root { }` block.
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
                     css: can no longer override the palette",
                    line.trim()
                );
            }
        }
    }

    #[test]
    fn unknown_theme_falls_back_to_default_css() {
        assert_eq!(theme_tokens("nope"), theme_tokens("default"));
    }

    #[test]
    fn theme_warning_flags_unknown_names_only() {
        assert!(theme_warning(None).is_none());
        assert!(theme_warning(Some("nord")).is_none());
        let w = theme_warning(Some("nope")).unwrap();
        assert!(w.contains("nope"));
        assert!(w.contains("default"));
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
        for member in ["presenting", "state", "html", "onChange", "refit", "sync"] {
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

    /// A phone has no keyboard, so every way of driving the deck has a touch
    /// equivalent - and the sheet says so on the devices that need it.
    #[test]
    fn every_control_has_a_touch_equivalent() {
        for gesture in ["touchstart", "touchmove", "touchend", "pointer: coarse"] {
            assert!(
                VIEWER_JS.contains(gesture),
                "viewer.js is missing {gesture}"
            );
        }
    }

    /// A stray `*/` turns the prose after it into CSS, and the parser's error
    /// recovery then swallows the *next* rule whole. That is not a typo you
    /// see: it shipped transparent slides, and page turns showed two slides
    /// through each other until someone noticed.
    #[test]
    fn stylesheets_have_no_stray_comment_markers() {
        for (name, css) in [
            ("base.css", BASE_CSS),
            ("print.css", PRINT_CSS),
            ("themes/default.css", theme_tokens("default")),
            ("themes/nord.css", theme_tokens("nord")),
            ("themes/solarized.css", theme_tokens("solarized")),
            ("themes/vscode.css", theme_tokens("vscode")),
        ] {
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
    use std::collections::BTreeMap;

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
        format!(":where(:root[data-theme=\"{name}\"]) {{")
    }

    fn dark_selector(name: &str) -> String {
        format!(":where(:root[data-theme=\"{name}\"][data-mode=\"dark\"]) {{")
    }

    fn auto_dark_selector(name: &str) -> String {
        format!(":where(:root[data-theme=\"{name}\"]:not([data-mode=\"light\"])) {{")
    }

    const ALL_TOKENS: &[&str] = &[
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
    const SURFACE_TEXT_PAIRS: &[(&str, &str)] = &[("fg", "surface"), ("danger-fg", "danger-bg")];

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
            check_contrast(
                name,
                "light",
                &parse_tokens(css, &light_selector(name)),
                &mut failures,
            );
            check_contrast(
                name,
                "dark",
                &parse_tokens(css, &dark_selector(name)),
                &mut failures,
            );
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
