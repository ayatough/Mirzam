//! `mirzam new` writes the first file, so two things have to hold:
//! it never destroys a deck that is already there, and what it writes is a
//! deck the renderer accepts - a starter file that builds to a blank page
//! would be a worse start than no command at all.

use mirzam_cli::{pipeline, scaffold};
use std::collections::HashMap;
use std::path::PathBuf;

/// A directory that removes itself, so a failed assertion cannot leak one.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("mirzam-new-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        TempDir(dir)
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn writes_a_deck_that_renders() {
    let dir = TempDir::new("template");
    let path = dir.join("deck.md");
    scaffold::create(&path, false).expect("create");

    let mut cache = HashMap::new();
    let out = pipeline::build_deck(&path, &mut cache).expect("build the starter deck");
    assert_eq!(out.sections.len(), 2, "the starter deck is two slides");
    assert!(
        out.warnings.is_empty(),
        "the starter deck must build clean: {:?}",
        out.warnings
    );
}

#[test]
fn empty_writes_an_empty_file() {
    let dir = TempDir::new("empty");
    let path = dir.join("deck.md");
    scaffold::create(&path, true).expect("create");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
}

/// An empty deck builds - `serve` polls this file while the first slide is
/// being typed - but it warns, because a blank page otherwise looks broken.
#[test]
fn an_empty_deck_builds_with_a_warning() {
    let dir = TempDir::new("empty-build");
    let path = dir.join("deck.md");
    scaffold::create(&path, true).expect("create");

    let mut cache = HashMap::new();
    let out = pipeline::build_deck(&path, &mut cache).expect("an empty deck is not an error");
    assert!(out.sections.is_empty());
    assert!(
        out.warnings.iter().any(|w| w.contains("no slides")),
        "expected a no-slides warning, got {:?}",
        out.warnings
    );
}

/// Frontmatter and nothing else is the other way to end up with a blank deck,
/// and it is not the same message: the file is not empty.
#[test]
fn frontmatter_alone_warns_that_nothing_follows_it() {
    let dir = TempDir::new("frontmatter-only");
    let path = dir.join("deck.md");
    std::fs::write(&path, "---\ntitle: T\n---\n").expect("write");

    let mut cache = HashMap::new();
    let out = pipeline::build_deck(&path, &mut cache).expect("build");
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("nothing outside its frontmatter")),
        "got {:?}",
        out.warnings
    );
}

#[test]
fn refuses_to_overwrite() {
    let dir = TempDir::new("overwrite");
    let path = dir.join("deck.md");
    std::fs::write(&path, "# Mine\n").expect("write");

    let err = scaffold::create(&path, false).expect_err("must not overwrite");
    assert!(err.contains("already exists"), "got {err}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# Mine\n",
        "the existing deck must be untouched"
    );
}

#[test]
fn creates_missing_parent_directories() {
    let dir = TempDir::new("parents");
    let path = dir.join("talks/2026/deck.md");
    scaffold::create(&path, false).expect("create");
    assert!(path.exists());
}

/// Pointing `build` at a file nobody has created yet is the case `new` exists
/// for, so the error names it rather than only reporting the failed read.
#[test]
fn a_missing_input_points_at_new() {
    let dir = TempDir::new("missing");
    let path = dir.join("nothing-here.md");
    let mut cache = HashMap::new();
    let Err(err) = pipeline::build_deck(&path, &mut cache) else {
        panic!("a missing file must not build");
    };
    assert!(err.contains("mirzam new"), "got {err}");
}
