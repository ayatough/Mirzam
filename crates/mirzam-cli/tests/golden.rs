//! Golden (snapshot) tests.
//!
//! Compares rendered sample decks against `tests/snapshots/` to catch
//! unintended output changes.
//!
//! After an intentional change, review the diff and then update:
//!   MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden

mod common;

use common::{example, normalize, EXAMPLE_DECKS};
use std::collections::HashMap;

#[test]
fn examples_match_snapshots() {
    let update = std::env::var("MIRZAM_UPDATE_SNAPSHOTS").is_ok();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    std::fs::create_dir_all(&dir).expect("snapshot directory");

    let mut failures = Vec::new();
    for deck in EXAMPLE_DECKS {
        let mut cache = HashMap::new();
        let out = mirzam_cli::pipeline::build_deck(&example(deck), &mut cache)
            .unwrap_or_else(|e| panic!("failed to build {deck}: {e}"));

        // One exception to warning-free: the gallery's `mermaid` fence tells a
        // machine with no mermaid-cli that it stayed a code block. That is a
        // fact about the machine, not about the deck - and the snapshot
        // normalizes the diagram away for the same reason - so a laptop
        // without the renderer still runs this suite.
        let warnings: Vec<&String> = out
            .warnings
            .iter()
            .filter(|w| !w.contains("no diagram renderer found"))
            .collect();
        assert!(
            warnings.is_empty(),
            "{deck} produced warnings; samples are expected to be warning-free: {warnings:?}"
        );

        let actual = normalize(&out.sections.concat());
        let snap_path = dir.join(format!("{}.html", deck.trim_end_matches(".md")));

        if update || !snap_path.exists() {
            std::fs::write(&snap_path, &actual).expect("write snapshot");
            continue;
        }
        let expected = std::fs::read_to_string(&snap_path).expect("read snapshot");
        if expected != actual {
            let at = expected
                .chars()
                .zip(actual.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(actual.len()));
            let ctx = |s: &str| -> String {
                s.chars()
                    .skip(at.saturating_sub(60))
                    .take(160)
                    .collect::<String>()
            };
            failures.push(format!(
                "{deck}: output differs from the snapshot (around offset {at})\n  expected: …{}…\n  actual:   …{}…",
                ctx(&expected),
                ctx(&actual)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n\nIf the change is intentional, update with MIRZAM_UPDATE_SNAPSHOTS=1",
        failures.join("\n\n")
    );
}

/// Slide counts per deck, catching structural regressions.
#[test]
fn example_slide_counts() {
    for (deck, expected) in [
        ("01-start.md", 6),
        ("02-writing.md", 11),
        ("03-layout.md", 14),
        // 25 rendered from 24 authored: the `each` example is one slide
        // written, two rows rendered.
        ("04-components.md", 25),
        ("05-motion.md", 11),
        ("06-theming.md", 16),
        ("07-charts.md", 10),
        ("pitch.md", 9),
        ("research.md", 9),
        ("seminar.md", 12),
        ("slideshow.md", 5),
    ] {
        let mut cache = HashMap::new();
        let out = mirzam_cli::pipeline::build_deck(&example(deck), &mut cache).unwrap();
        assert_eq!(out.sections.len(), expected, "slide count for {deck}");
    }
}
