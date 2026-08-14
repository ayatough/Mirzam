//! The sample themes under `examples/themes/` are held to the standard the
//! built-in ones are — by the very check a reader's own theme is held to.
//!
//! A deck's own stylesheet overrides the built-in tokens — deliberately, since
//! the built-ins carry no specificity — which means a custom theme that sets
//! its colours once **pins the deck to one mode**, and `D` in the viewer looks
//! broken. Worse, a custom theme that sets *most* of them twice leaves the
//! stragglers at their dark values on a white slide, which is how a deck ships
//! with text the audience cannot read.
//!
//! Both used to be checked here, against a hard-coded pair of selectors. They
//! are now [`mirzam_render::file_theme_warnings`], which every build runs
//! against every theme a deck loads — so this file asks the sample themes the
//! question a reader's deck is asked, rather than a similar question written
//! twice. A gate that only the samples could fail was a gate on the wrong
//! stylesheets.

mod common;

use common::repo_root;

fn sample_themes() -> Vec<mirzam_render::FileTheme> {
    let dir = repo_root().join("examples/themes");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("examples/themes exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("css") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let css = std::fs::read_to_string(&path).expect("read theme");
            out.push(mirzam_render::FileTheme::new(&name, css));
        }
    }
    assert!(
        !out.is_empty(),
        "examples/ ships no custom theme: a repository whose point is that a theme of \
         your own is a supported artefact has to demonstrate one"
    );
    out
}

/// Every token set for one mode set for the other, every pair of colours over
/// the contrast floor, and no stem colliding with a built-in name.
#[test]
fn a_sample_theme_passes_the_check_a_readers_own_theme_gets() {
    let warnings = mirzam_render::file_theme_warnings(&sample_themes());
    assert!(warnings.is_empty(), "{}", warnings.join("\n"));
}

/// And every one of them is usable by name, which is the thing a sample is
/// for: `theme=blueprint` on a pane has to reach the tokens, and it only does
/// when the file scopes them to its own stem.
#[test]
fn a_sample_theme_scopes_its_tokens_to_its_own_stem() {
    for theme in sample_themes() {
        assert!(
            theme.scopes_to_stem(),
            "{}: a sample theme is the file people copy, so it has to show the form that \
             works in a pane's `theme=` — wrap the tokens in `[data-theme=\"{}\"]`",
            theme.path,
            theme.name
        );
    }
}
