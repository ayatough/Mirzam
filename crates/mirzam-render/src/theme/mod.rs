//! Assembles the built-in theme's CSS and the viewer runtime JS from the
//! files in this directory, and selects a named theme's tokens.
//!
//! - `themes/*.css` — one file per built-in theme, each defining the full
//!   token set for both light and dark mode ([C3] in `docs/workstreams.md`)
//! - `base.css` — layout, typography, panes; everything that reads a token
//!   rather than defining one, shared by every theme
//! - `print.css` — overrides applied for PDF export
//! - `viewer.js` — the runtime shipped inside every deck
//!
//! [C3]: ../../../docs/workstreams.md#c3-theme-tokens

pub const BASE_CSS: &str = include_str!("base.css");
pub const VIEWER_JS: &str = concat!("\n", include_str!("viewer.js"));

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
            .starts_with(":root[data-theme=\"default\"]"));
        assert!(css.contains("--mz-accent1"));
        assert!(css.contains("* { box-sizing: border-box; }"));
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

    fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
        let hex = hex.trim().trim_start_matches('#');
        assert_eq!(hex.len(), 6, "expected a 6-digit hex color, got `#{hex}`");
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
        (
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
        )
    }

    fn channel_lin(c: f64) -> f64 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn relative_luminance(hex: &str) -> f64 {
        let (r, g, b) = hex_to_rgb(hex);
        0.2126 * channel_lin(r) + 0.7152 * channel_lin(g) + 0.0722 * channel_lin(b)
    }

    /// WCAG contrast ratio between two colors; always >= 1.0.
    fn contrast_ratio(a: &str, b: &str) -> f64 {
        let la = relative_luminance(a) + 0.05;
        let lb = relative_luminance(b) + 0.05;
        if la > lb {
            la / lb
        } else {
            lb / la
        }
    }

    /// Extracts `--mz-token: value;` pairs from the first `{ ... }` block
    /// whose selector is exactly `selector` (including the trailing ` {`),
    /// found as an exact substring so it can't match a longer selector that
    /// merely starts the same way (`:not(...)`, `[data-mode="dark"]`, ...).
    fn parse_tokens(css: &str, selector: &str) -> BTreeMap<String, String> {
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
        format!(":root[data-theme=\"{name}\"] {{")
    }

    fn dark_selector(name: &str) -> String {
        format!(":root[data-theme=\"{name}\"][data-mode=\"dark\"] {{")
    }

    fn auto_dark_selector(name: &str) -> String {
        format!(":root[data-theme=\"{name}\"]:not([data-mode=\"light\"]) {{")
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
    ];

    /// Rendered as text directly against `--mz-slide-bg` (h1/h2/h3, links,
    /// `strong`, paragraphs, `.small`): WCAG 1.4.3 body-text contrast, 4.5:1.
    const BODY_TEXT_TOKENS: &[&str] = &["fg", "muted", "accent1"];

    /// Chart series colors: they only have to be perceivable as distinct
    /// marks, not read as text, so WCAG 1.4.11 graphical-object contrast,
    /// 3:1, applies instead of the stricter text threshold.
    const CHART_MARK_TOKENS: &[&str] = &["chart3", "chart4", "chart5", "chart6"];

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
}
