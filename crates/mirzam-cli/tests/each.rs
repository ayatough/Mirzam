//! Data-driven slides: a slide holding an ```each block is a template,
//! rendered once per data row. These drive the real pipeline over decks on
//! disk, because the feature is the pipeline's — the parser only finds the
//! block, and the renderer must never see one.

mod common;

use std::collections::HashMap;

struct TempDeck {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDeck {
    fn new(name: &str, body: &str) -> TempDeck {
        let dir = std::env::temp_dir().join(format!("mirzam-each-{}-{}", name, std::process::id()));
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

fn build(deck: &TempDeck) -> mirzam_cli::pipeline::BuildOutput {
    let mut cache = HashMap::new();
    mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build")
}

#[test]
fn inline_rows_become_slides() {
    let deck = TempDeck::new(
        "inline",
        "# Cover\n\n---\n\n```each\nname, ms\nparse, 4\nrender, 11\n```\n\n\
         ## {{name}}\n\nCosts {{ms}} ms, or {{ms * 2}} for two.\n\n---\n\n# End\n",
    );
    let out = build(&deck);
    // Cover, one slide per row, End.
    assert_eq!(out.sections.len(), 4, "{:?}", out.warnings);
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert!(out.sections[1].contains("parse") && out.sections[1].contains("Costs 4 ms"));
    assert!(out.sections[2].contains("render") && out.sections[2].contains("Costs 11 ms"));
    // A number is a number: arithmetic over a field works.
    assert!(out.sections[2].contains("22 for two"));
    // The block itself reaches no slide.
    assert!(out.sections.iter().all(|s| !s.contains("```each")));
    // The source view stays authored: three slides were written.
    assert_eq!(out.slides.len(), 3);
}

#[test]
fn rows_can_live_in_a_csv_file_and_the_file_is_watched() {
    let deck = TempDeck::new(
        "file",
        "```each\ndata: rows.csv\n```\n\n## {{city}}\n\n{{count}} sites\n",
    );
    std::fs::write(
        deck.dir.join("rows.csv"),
        "city, count\nOsaka, 3\nNagoya, 2\n",
    )
    .expect("write csv");
    let out = build(&deck);
    assert_eq!(out.sections.len(), 2, "{:?}", out.warnings);
    assert!(out.sections[0].contains("Osaka") && out.sections[0].contains("3 sites"));
    assert!(
        out.files.contains(&deck.dir.join("rows.csv")),
        "the rows are part of the deck, so `serve` must watch them"
    );
}

#[test]
fn a_missing_file_keeps_the_template_and_says_why() {
    let deck = TempDeck::new(
        "missing",
        "```each\ndata: nowhere.csv\n```\n\n## {{name}}\n",
    );
    let out = build(&deck);
    // The slide renders once, placeholders visible, block gone.
    assert_eq!(out.sections.len(), 1);
    assert!(out.sections[0].contains("{{name}}"));
    assert!(!out.sections[0].contains("each"));
    let w = out
        .warnings
        .iter()
        .find(|w| w.contains("cannot read nowhere.csv"))
        .expect("a warning names the file");
    assert_eq!(mirzam_cli::pipeline::warning_kind(w), "build.each");
}

#[test]
fn a_column_shadowed_by_a_deck_var_warns() {
    let deck = TempDeck::new(
        "shadow",
        "---\nvars:\n  name: Deck\n---\n\n```each\nname\nRow\n```\n\n## {{name}}\n",
    );
    let out = build(&deck);
    // The deck's value already substituted before the template was found.
    assert!(out.sections[0].contains("Deck"));
    assert!(out
        .warnings
        .iter()
        .any(|w| w.contains("column `name`") && w.contains("rename")));
}

#[test]
fn a_generated_slide_may_still_carry_a_pane_on() {
    let deck = TempDeck::new(
        "cont",
        "```each\nname\nA\nB\n```\n\n::: pane body\n{{name}} first\n\n<!-- next -->\n\n{{name}} second\n:::\n",
    );
    let out = build(&deck);
    // Two rows, each broken once: four sections, two continuation groups.
    assert_eq!(out.sections.len(), 4, "{:?}", out.warnings);
    let group = |s: &str| {
        s.split("data-cont=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .map(str::to_string)
    };
    assert_eq!(group(&out.sections[0]), group(&out.sections[1]));
    assert_ne!(group(&out.sections[1]), group(&out.sections[2]));
}
