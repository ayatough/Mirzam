//! Correctness of incremental builds:
//! **An incremental build must equal a full rebuild, exactly.**
//!
//! This invariant mechanically rules out stale-preview bugs in hot reload.

mod common;

use common::repo_root;
use std::collections::HashMap;

/// A deck written to a temporary directory.
struct TempDeck {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDeck {
    fn new(name: &str, body: &str) -> TempDeck {
        let dir = std::env::temp_dir().join(format!("mirzam-test-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("deck.md");
        std::fs::write(&path, body).expect("write deck");
        TempDeck { dir, path }
    }

    fn write(&self, body: &str) {
        std::fs::write(&self.path, body).expect("update deck");
    }
}

impl Drop for TempDeck {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn deck_source(titles: &[&str]) -> String {
    let mut s = String::from("---\ntitle: T\nvars:\n  n: 3\n---\n\n");
    for (i, t) in titles.iter().enumerate() {
        if i > 0 {
            s.push_str("\n---\n\n");
        }
        s.push_str(&format!(
            "## {t}\n\nBody {{{{ n * {} }}}} with math $x^{}$\n",
            i + 1,
            i + 1
        ));
    }
    s
}

#[test]
fn incremental_equals_full_rebuild() {
    let deck = TempDeck::new("equiv", &deck_source(&["A", "B", "C", "D"]));
    let mut warm = HashMap::new();
    mirzam_cli::pipeline::build_deck(&deck.path, &mut warm).unwrap();

    // Edit only the third slide.
    deck.write(&deck_source(&["A", "B", "C-edited", "D"]));

    let incremental = mirzam_cli::pipeline::build_deck(&deck.path, &mut warm).unwrap();
    let mut cold = HashMap::new();
    let full = mirzam_cli::pipeline::build_deck(&deck.path, &mut cold).unwrap();

    assert_eq!(
        incremental.sections, full.sections,
        "incremental output does not match a full rebuild"
    );
    assert_eq!(incremental.hashes, full.hashes);
    assert_eq!(
        incremental.rendered, 1,
        "only the edited slide should re-render (actual: {})",
        incremental.rendered
    );
}

#[test]
fn variable_change_invalidates_all_slides_that_use_it() {
    let deck = TempDeck::new("vars", &deck_source(&["A", "B"]));
    let mut cache = HashMap::new();
    let first = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    // Changing only a frontmatter variable changes every slide that uses it.
    let updated = deck_source(&["A", "B"]).replace("n: 3", "n: 5");
    deck.write(&updated);
    let second = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    assert_ne!(
        first.sections, second.sections,
        "variable change was not applied"
    );
    assert_eq!(
        second.rendered, 2,
        "both slides referencing the variable should re-render"
    );

    let mut cold = HashMap::new();
    let full = mirzam_cli::pipeline::build_deck(&deck.path, &mut cold).unwrap();
    assert_eq!(second.sections, full.sections);
}

#[test]
fn unchanged_rebuild_renders_nothing() {
    let deck = TempDeck::new("noop", &deck_source(&["A", "B", "C"]));
    let mut cache = HashMap::new();
    mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();
    let again = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();
    assert_eq!(again.rendered, 0, "no change means nothing re-renders");
}

#[test]
fn include_and_assets_are_tracked_for_watching() {
    let mut cache = HashMap::new();
    let path = repo_root().join("examples/pitch.md");
    let out = mirzam_cli::pipeline::build_deck(&path, &mut cache).unwrap();

    let names: Vec<String> = out
        .files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"pitch.md".to_string()));
    assert!(
        names.contains(&"pitch.css".to_string()),
        "custom stylesheet is missing from the watch set: {names:?}"
    );
    assert!(
        names.contains(&"adoption.csv".to_string()),
        "chart data file is missing from the watch set: {names:?}"
    );
}

/// Reordering slides still produces correct position-dependent output.
#[test]
fn reordering_slides_updates_indices() {
    let deck = TempDeck::new("reorder", &deck_source(&["A", "B"]));
    let mut cache = HashMap::new();
    mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    deck.write(&deck_source(&["B", "A"]));
    let swapped = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    let mut cold = HashMap::new();
    let full = mirzam_cli::pipeline::build_deck(&deck.path, &mut cold).unwrap();
    assert_eq!(
        swapped.sections, full.sections,
        "output after reordering does not match"
    );
    assert!(swapped.sections[0].contains("data-index=\"0\""));
}
