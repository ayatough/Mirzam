//! The frontmatter `math:` switch. It is a per-deck choice with LaTeX as the
//! default, so every existing deck renders byte-for-byte as it always did —
//! and it is part of the render cache key, so flipping it actually re-renders.

use std::collections::HashMap;

/// A deck written to a temporary directory.
struct TempDeck {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDeck {
    fn new(name: &str, body: &str) -> TempDeck {
        let dir = std::env::temp_dir().join(format!("mirzam-math-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("deck.md");
        std::fs::write(&path, body).expect("write deck");
        TempDeck { dir, path }
    }
}

impl Drop for TempDeck {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The same formula under both dialects: `alpha/2` is a Greek fraction in
/// Typst and four italic letters over a slash in LaTeX.
const BODY: &str = "# Slide\n\nSome $alpha/2$ math\n";

#[test]
fn typst_frontmatter_switches_the_dialect() {
    let deck = TempDeck::new("switch", &format!("---\nmath: typst\n---\n\n{BODY}"));
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut HashMap::new()).unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let html = out.sections.concat();
    assert!(html.contains("mfrac"), "no fraction rendered: {html}");
    assert!(html.contains('α'), "no alpha rendered: {html}");
}

#[test]
fn without_frontmatter_the_dialect_is_latex() {
    let deck = TempDeck::new("default", BODY);
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut HashMap::new()).unwrap();
    let html = out.sections.concat();
    assert!(
        !html.contains("mfrac"),
        "LaTeX `alpha/2` has no fraction: {html}"
    );
    assert!(
        !html.contains('α'),
        "LaTeX `alpha` is letters, not Greek: {html}"
    );
}

#[test]
fn an_unknown_dialect_warns_and_renders_as_latex() {
    let deck = TempDeck::new("unknown", &format!("---\nmath: maple\n---\n\n{BODY}"));
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut HashMap::new()).unwrap();
    assert!(
        out.warnings.iter().any(|w| w.contains("maple")),
        "the typo should be named: {:?}",
        out.warnings
    );
    assert!(!out.sections.concat().contains("mfrac"));
}

/// Flipping `math:` changes how every formula renders while every slide's
/// source text stays identical — exactly what a source-only cache key misses.
#[test]
fn changing_the_dialect_defeats_the_render_cache() {
    let deck = TempDeck::new("cache", BODY);
    let mut cache = HashMap::new();
    let latex = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    std::fs::write(&deck.path, format!("---\nmath: typst\n---\n\n{BODY}")).expect("update deck");
    let warm = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();
    let cold = mirzam_cli::pipeline::build_deck(&deck.path, &mut HashMap::new()).unwrap();

    assert_ne!(
        latex.sections, warm.sections,
        "the dialect change was not applied"
    );
    assert_eq!(
        warm.sections, cold.sections,
        "a warm build after a dialect change must equal a cold one"
    );
}
