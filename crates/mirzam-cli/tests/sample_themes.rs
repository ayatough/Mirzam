//! The sample themes under `examples/themes/` are held to the standard the
//! built-in ones are.
//!
//! A deck's own stylesheet overrides the built-in tokens — deliberately, since
//! the built-ins carry no specificity — which means a custom theme that sets
//! its colours once **pins the deck to one mode**, and `D` in the viewer looks
//! broken. Worse, a custom theme that sets *most* of them twice leaves the
//! stragglers at their dark values on a white slide, which is how a deck ships
//! with text the audience cannot read.
//!
//! Both are mechanical, so both are checked here rather than remembered.

mod common;

use common::repo_root;
use std::collections::BTreeMap;

/// Comments removed, so a `:` inside one cannot be mistaken for a
/// declaration's separator and swallow the token that follows it.
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

/// Token declarations inside the first block whose selector is exactly
/// `selector`. `None` when the theme has no such block.
fn tokens(css: &str, selector: &str) -> Option<BTreeMap<String, String>> {
    let css = strip_comments(css);
    let css = css.as_str();
    let start = css.find(&format!("{selector} {{"))?;
    let body_start = css[start..].find('{')? + start + 1;
    let body_end = css[body_start..].find('}')? + body_start;
    let mut out = BTreeMap::new();
    for decl in css[body_start..body_end].split(';') {
        if let Some((name, value)) = decl.trim().split_once(':') {
            let name = name.trim();
            if name.starts_with("--") {
                out.insert(name.to_string(), value.trim().to_string());
            }
        }
    }
    Some(out)
}

fn sample_themes() -> Vec<(String, String)> {
    let dir = repo_root().join("examples/themes");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("examples/themes exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("css") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            out.push((name, std::fs::read_to_string(&path).expect("read theme")));
        }
    }
    assert!(!out.is_empty(), "no sample themes found");
    out
}

/// Every token the theme sets for one mode it must set for the other, or that
/// token keeps its other-mode value — a dark panel on a white slide.
#[test]
fn a_sample_theme_defines_every_token_it_uses_in_both_modes() {
    for (name, css) in sample_themes() {
        let dark = tokens(&css, ":root").unwrap_or_else(|| panic!("{name}: no `:root` block"));
        let light = tokens(&css, ":root[data-mode=\"light\"]").unwrap_or_else(|| {
            panic!(
                "{name}: no `:root[data-mode=\"light\"]` block, so `D` in the viewer \
                 cannot change anything this theme paints"
            )
        });
        for token in dark.keys() {
            assert!(
                light.contains_key(token),
                "{name}: `{token}` is set for dark but not for light, so it keeps its \
                 dark value on a light slide"
            );
        }
        for token in light.keys() {
            assert!(
                dark.contains_key(token),
                "{name}: `{token}` is set for light but not for dark"
            );
        }
    }
}

/// Text on the slide background, text on a raised surface, and chart marks —
/// the same pairs the built-in themes are checked on.
#[test]
fn a_sample_theme_is_legible_in_both_modes() {
    // (foreground, background, minimum ratio). Body text is WCAG 1.4.3 at
    // 4.5:1; a chart mark only has to be distinguishable, 1.4.11 at 3:1.
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
        // The viewer chrome is a piece of the deck's own paper: the page
        // counter is `--mz-muted` on a `--mz-surface` pill.
        ("--mz-muted", "--mz-surface", 4.5),
        ("--mz-danger-fg", "--mz-danger-bg", 4.5),
        // The identity dials. A theme sets none of these and the renderer
        // supplies the value; a theme that sets one has put text on a surface
        // and owes the same ratio for it. Each pair is skipped unless the
        // theme defines both halves, so this is coverage for the day a sample
        // theme writes its identity in tokens rather than in rules.
        ("--mz-h3-color", "--mz-slide-bg", 4.5),
        ("--mz-strong-color", "--mz-slide-bg", 4.5),
        ("--mz-quote-fg", "--mz-slide-bg", 4.5),
        ("--mz-th-fg", "--mz-surface", 4.5),
        ("--mz-code-fg", "--mz-code-bg", 4.5),
        ("--mz-fg", "--mz-code-bg", 4.5),
        ("--mz-fg", "--mz-card-bg", 4.5),
    ];

    let mut failures = Vec::new();
    for (name, css) in sample_themes() {
        for (mode, selector) in [("dark", ":root"), ("light", ":root[data-mode=\"light\"]")] {
            let Some(t) = tokens(&css, selector) else {
                continue; // the test above already reports a missing mode
            };
            for (fg, bg, need) in PAIRS {
                // A theme may leave a token to the built-in palette; only what
                // it defines itself is its responsibility.
                let (Some(f), Some(b)) = (t.get(*fg), t.get(*bg)) else {
                    continue;
                };
                let Some(ratio) = mirzam_render::contrast_ratio(f, b) else {
                    continue; // not a plain hex colour; nothing to measure
                };
                if ratio < *need {
                    failures.push(format!(
                        "{name}/{mode}: {fg} ({f}) on {bg} ({b}) is only {ratio:.2}:1, \
                         need >= {need}:1"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
