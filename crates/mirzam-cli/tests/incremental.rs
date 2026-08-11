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
        names.contains(&"mirzam.css".to_string()),
        "custom stylesheet is missing from the watch set: {names:?}"
    );
    assert!(
        names.contains(&"adoption.csv".to_string()),
        "chart data file is missing from the watch set: {names:?}"
    );
}

/// Slides render the same regardless of theme/mode (they only affect page
/// assembly), so a `theme:`/`mode:`-only edit must still bump the page
/// fingerprint or `serve` would never tell the client to reload.
#[test]
fn theme_and_mode_change_bump_the_page_fingerprint() {
    let deck = TempDeck::new("theme-fp", &deck_source(&["A"]));
    let mut cache = HashMap::new();
    let first = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    deck.write(&deck_source(&["A"]).replacen("title: T", "title: T\ntheme: nord", 1));
    let themed = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();
    assert_eq!(
        first.sections, themed.sections,
        "theme does not change per-slide HTML"
    );
    assert_ne!(
        first.page_fingerprint, themed.page_fingerprint,
        "a theme-only edit must still invalidate the assembled page"
    );

    deck.write(&deck_source(&["A"]).replacen("title: T", "title: T\ntheme: nord\nmode: dark", 1));
    let dark = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();
    assert_ne!(
        themed.page_fingerprint, dark.page_fingerprint,
        "a mode-only edit must still invalidate the assembled page"
    );
}

/// An unknown theme name is a warning, not a build failure, and falls back
/// to `default`.
#[test]
fn unknown_theme_warns_but_still_builds() {
    let deck = TempDeck::new(
        "bad-theme",
        &deck_source(&["A"]).replacen("title: T", "title: T\ntheme: nope", 1),
    );
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();
    assert!(out.warnings.iter().any(|w| w.contains("nope")));
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

/// The source map, end to end through a real build: a block written in an
/// included file must resolve back to that file's own bytes. This is what
/// makes an edit made in the preview writable back to the line it came from.
#[test]
fn a_block_resolves_back_to_the_file_it_was_written_in() {
    let deck = TempDeck::new(
        "srcmap",
        "---\ntitle: T\n---\n\nintro\n\n---\n\n![[part.md]]\n",
    );
    let part = "## Figure\n\n```annotate\ntarget: fig\ncircle 40,30 20x20\n```\n";
    std::fs::write(deck.dir.join("part.md"), part).expect("write part");

    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");

    let slide = out
        .slides
        .iter()
        .find(|s| s.text.contains("annotate"))
        .expect("the slide holding the block");
    let parsed = mirzam_syntax::parse_slide(&slide.text);
    let block = parsed
        .blocks
        .iter()
        .find(|b| b.info == "annotate")
        .expect("the block was recorded");

    let (file, range) = out
        .map
        .resolve(slide.start + block.body.start..slide.start + block.body.end)
        .expect("resolves to one file");
    assert_eq!(file, deck.dir.join("part.md"));
    assert_eq!(&part[range], "target: fig\ncircle 40,30 20x20\n");
}

/// The root deck's own offsets include its frontmatter, so a range resolves
/// against the file on disk rather than against the body alone.
#[test]
fn root_offsets_are_relative_to_the_whole_file() {
    let src = "---\ntitle: T\n---\n\n## One\n\n```shape\nrect #a at(1,2) size(3,4)\n```\n";
    let deck = TempDeck::new("srcmap-root", src);
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");

    let slide = &out.slides[0];
    let parsed = mirzam_syntax::parse_slide(&slide.text);
    let block = parsed.blocks.iter().find(|b| b.info == "shape").unwrap();
    let (file, range) = out
        .map
        .resolve(slide.start + block.body.start..slide.start + block.body.end)
        .expect("resolves");
    assert_eq!(file, deck.path);
    assert_eq!(&src[range], "rect #a at(1,2) size(3,4)\n");
}

/// A line the variable substitution rewrote is not a line anyone typed, so it
/// must resolve to nothing rather than to a plausible wrong place.
#[test]
fn a_substituted_line_resolves_to_nothing() {
    let deck = TempDeck::new(
        "srcmap-vars",
        "---\ntitle: T\nvars:\n  n: 3\n---\n\nvalue {{ n }}\n",
    );
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    let slide = &out.slides[0];
    let at = slide.start + slide.text.find("value").expect("the line is there");
    assert_eq!(out.map.lookup(at), None);
}

/// A warning raised on a slide that came from an included file says so.
#[test]
fn a_warning_names_the_file_the_slide_came_from() {
    let deck = TempDeck::new("srcmap-warn", "---\ntitle: T\n---\n\n![[part.md]]\n");
    // An anim target that matches nothing is the standard warning path.
    std::fs::write(
        deck.dir.join("part.md"),
        "## S\n\n```anim\n[enter] .nope : fade-in 400ms\n```\n",
    )
    .expect("write part");
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    assert!(
        out.warnings.iter().any(|w| w.contains("part.md")),
        "{:?}",
        out.warnings
    );
}

// ---- `masters:` naming a file ----

/// A deck drawn on a shared masters file, and the file beside it.
fn deck_on_a_masters_file(dir_name: &str) -> TempDeck {
    let deck = TempDeck::new(
        dir_name,
        "---\ntitle: T\nmasters: shapes.md\nlayout: body\n---\n\n\
         ::: pane head\n## Heading\n:::\n\n::: pane main\nWords.\n:::\n",
    );
    std::fs::write(
        deck.dir.join("shapes.md"),
        "# Shapes\n\n## body\n\n```pane\n+-------+\n| head  |\n+-------+\n| main  |\n+-------+\n```\n",
    )
    .expect("write masters");
    deck
}

#[test]
fn a_deck_is_drawn_on_the_masters_file_it_names() {
    let deck = deck_on_a_masters_file("masters-file");
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert!(out.sections[0].contains(r#"grid-template-areas:"head" "main""#));
    // In the watch set, so editing the shared shapes rebuilds the decks on them.
    assert!(out.files.contains(&deck.dir.join("shapes.md")));
}

/// The masters file is part of the deck's input, so changing it has to reach
/// the slides — a cache keyed on slide text alone would serve the old shape.
#[test]
fn editing_the_masters_file_re_renders_the_slides() {
    let deck = deck_on_a_masters_file("masters-edit");
    let mut cache = HashMap::new();
    let first = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    std::fs::write(
        deck.dir.join("shapes.md"),
        "## body\n\n```pane\n+-------+-------+\n| head  | main  |\n+-------+-------+\n```\n",
    )
    .expect("rewrite masters");
    let second = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("rebuild");
    assert_ne!(first.sections[0], second.sections[0]);
    assert!(second.sections[0].contains(r#"grid-template-areas:"head main""#));
}

/// Losing the file loses every layout in the deck, which is a much louder
/// failure than losing a stylesheet — so the warning has to say so.
/// Losing the file loses every layout in the deck, which is a much louder
/// failure than losing a stylesheet — so the warning has to say so. Once:
/// every name in a deck of forty being unknown is that one fact, repeated.
#[test]
fn a_masters_file_that_is_not_there_warns_once_and_builds() {
    let deck = TempDeck::new(
        "masters-missing",
        "---\ntitle: T\nmasters: gone.md\nlayout: body\n---\n\n\
         ::: pane main\nWords.\n:::\n\n---\n\n\
         <!-- layout: two-up -->\n\n::: pane main\nMore.\n:::\n",
    );
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
    assert!(
        out.warnings[0].starts_with("masters: cannot read")
            && out.warnings[0].ends_with("render as a single pane"),
        "{:?}",
        out.warnings
    );
    assert!(out.sections[0].contains("Words."));
    assert!(out.sections[1].contains("More."));
}

// ---- `<!-- next -->`: one pane carried on to the next slide ----

/// A slide with `fig` holding still and `body` broken into `parts`.
fn continued_deck(parts: &[&str]) -> String {
    format!(
        "---\ntitle: T\n---\n\n\
         ```pane\n+-----+-----+\n| fig | body|\n+-----+-----+\n```\n\n\
         ::: pane fig\nheld still\n:::\n\n\
         ::: pane body\n{}\n:::\n",
        parts.join("\n\n<!-- next -->\n\n")
    )
}

/// The pane between the markers is the only thing that differs, and the
/// generated sections say which group they belong to so the viewer can cut
/// rather than turn the page between them.
#[test]
fn a_broken_pane_becomes_several_slides_that_share_a_group() {
    let deck = TempDeck::new("cont", &continued_deck(&["first", "second", "third"]));
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");

    assert_eq!(out.sections.len(), 3);
    // One authored slide: the source view does not gain entries.
    assert_eq!(out.slides.len(), 1);
    for (i, word) in ["first", "second", "third"].iter().enumerate() {
        assert!(out.sections[i].contains("data-cont=\"0\""), "part {i}");
        assert!(out.sections[i].contains(word), "part {i}");
    }
    assert!(!out.sections[0].contains("second"));
    assert!(!out.sections[2].contains("first"));

    // The pane that did not break renders identically on every part.
    let fig = |s: &str| {
        let at = s.find("data-pane=\"fig\"").expect("the still pane");
        let end = s.find("data-pane=\"body\"").expect("the broken pane");
        s[at..end].to_string()
    };
    assert_eq!(fig(&out.sections[0]), fig(&out.sections[1]));
    assert_eq!(fig(&out.sections[1]), fig(&out.sections[2]));
}

/// A slide with no marker is untouched, and carries no group: an ordinary
/// page turn is still an ordinary page turn.
#[test]
fn a_slide_without_a_marker_gains_nothing() {
    let deck = TempDeck::new("cont-none", &continued_deck(&["only"]));
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    assert_eq!(out.sections.len(), 1);
    assert!(!out.sections[0].contains("data-cont"));
}

/// Two panes breaking at once is a cross product no author can predict. It is
/// reported, and the slide renders whole rather than not at all.
#[test]
fn two_panes_breaking_warns_and_renders_the_slide_whole() {
    let src = "---\ntitle: T\n---\n\n\
               ```pane\n+-----+-----+\n| a   | b   |\n+-----+-----+\n```\n\n\
               ::: pane a\none\n\n<!-- next -->\n\ntwo\n:::\n\n\
               ::: pane b\nthree\n\n<!-- next -->\n\nfour\n:::\n";
    let deck = TempDeck::new("cont-two", src);
    let mut cache = HashMap::new();
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    assert_eq!(out.sections.len(), 1);
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("more than one pane")),
        "{:?}",
        out.warnings
    );
    for word in ["one", "two", "three", "four"] {
        assert!(out.sections[0].contains(word), "{word} survived");
    }
}

/// Continuation runs before the cache, so an edit to one part re-renders that
/// part and leaves its siblings alone - the invariant the whole cache rests on.
#[test]
fn editing_one_part_re_renders_only_that_part() {
    let deck = TempDeck::new("cont-edit", &continued_deck(&["first", "second"]));
    let mut cache = HashMap::new();
    let warm = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("build");
    assert_eq!(warm.rendered, 2);

    deck.write(&continued_deck(&["first", "second, revised"]));
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).expect("rebuild");
    assert_eq!(out.rendered, 1, "only the edited part re-rendered");
    assert_eq!(out.sections[0], warm.sections[0]);
    assert!(out.sections[1].contains("revised"));
}
