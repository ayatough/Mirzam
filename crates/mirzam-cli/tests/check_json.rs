//! `mirzam check --format json`: the machine-readable half of the checker.
//!
//! An agent editing a deck will run the checker after every edit if - and only
//! if - it can read the answer. So what is asserted here is the contract, not
//! the prose: the document parses, it carries its schema version, and a deck
//! with known defects produces the kinds those defects are supposed to have,
//! each pointing back at the line that caused it.
//!
//! Rendering needs a browser, so the browser-driven tests skip themselves when
//! there is none - the same way `scripts/release.sh` does, and for the same
//! reason: a contributor without Chromium should still be able to run the
//! suite. Point `MIRZAM_CHROMIUM` at one to have them run.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A directory that removes itself, so a failed assertion cannot leak one.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir =
            std::env::temp_dir().join(format!("mirzam-check-json-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        TempDir(dir)
    }

    /// Writes `body` into the directory and returns its path.
    fn deck(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, body).expect("write the deck");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Whether a browser can be found, by the same route `mirzam check` takes.
fn chromium_available() -> bool {
    if std::env::var_os("MIRZAM_CHROMIUM").is_some() {
        return true;
    }
    ["chromium", "chromium-browser", "google-chrome", "chrome"]
        .iter()
        .any(|c| {
            Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
}

/// Runs `mirzam check --format json` and returns its stdout parsed, plus
/// whether the command succeeded. stdout must be the document and nothing
/// else - that is half of what this format promises.
fn check_json(deck: &Path) -> (serde_json::Value, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_mirzam"))
        .args(["check", &deck.display().to_string(), "--format", "json"])
        .output()
        .expect("run mirzam check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout was not valid JSON ({e}):\n{stdout}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (value, out.status.success())
}

fn kinds(report: &serde_json::Value) -> Vec<String> {
    report["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .map(|d| d["kind"].as_str().expect("a kind").to_string())
        .collect()
}

/// A deck carrying one of each failure the two passes can see: a shape block
/// inside a pane and a connector to nothing (the build notices both), and a
/// body pane with far more text than its one-line band can hold (only a
/// browser notices that).
const BROKEN: &str = r#"---
title: Known problems
---

# Findings

```pane
+------------------+
|  head            |
+------------------+
|  body            |
+------------------+
```

::: pane body
This pane is one line tall in the drawing above and holds ten paragraphs, so
its content is clipped. That is the failure no HTML diff can see.

Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.
Latency fell in every region we measured, and the cache hit rate rose with it.

```shape
rect #box at(50%, 50%) size(10%, 10%)
```
:::

```connect
#nowhere -> #alsonowhere
```
"#;

/// A deck with nothing wrong with it. Zero diagnostics is the case a caller
/// hits most often, and an empty run that wrote nothing - or wrote prose -
/// would break every one of them.
const CLEAN: &str = r#"---
title: Nothing wrong here
---

# A clean deck

One short line.
"#;

#[test]
fn a_broken_deck_reports_both_passes_with_source_locations() {
    if !chromium_available() {
        eprintln!("skipping: no Chromium; set MIRZAM_CHROMIUM to run this test");
        return;
    }
    let dir = TempDir::new("broken");
    let deck = dir.deck("broken.md", BROKEN);
    let (report, ok) = check_json(&deck);

    assert_eq!(report["schema"], "mirzam-check");
    assert_eq!(report["version"], 1);
    assert_eq!(report["ok"], false, "the deck has layout problems");
    assert!(!ok, "the exit code says so too");
    assert_eq!(report["slides"], 1);

    let kinds = kinds(&report);
    for expected in ["build.shape", "build.connect", "layout.clipped"] {
        assert!(
            kinds.iter().any(|k| k == expected),
            "expected a {expected} diagnostic, got {kinds:?}"
        );
    }

    // A build warning and an in-page finding must both point back at the
    // source, which is the whole reason a machine-readable form exists.
    for d in report["diagnostics"].as_array().expect("diagnostics") {
        let kind = d["kind"].as_str().expect("a kind");
        if kind == "build.shape" || kind == "layout.clipped" {
            assert_eq!(
                d["file"].as_str(),
                Some(deck.display().to_string().as_str()),
                "{kind} should name the file it came from: {d}"
            );
            assert!(d["line"].as_u64().is_some_and(|l| l > 0), "{kind}: {d}");
            assert_eq!(d["slide"].as_u64(), Some(1), "{kind}: {d}");
        }
    }

    // Severities separate what the build noticed from what only rendering can.
    for d in report["diagnostics"].as_array().expect("diagnostics") {
        let (kind, severity) = (
            d["kind"].as_str().expect("a kind"),
            d["severity"].as_str().expect("a severity"),
        );
        let expected = if kind.starts_with("build.") {
            "warning"
        } else {
            "error"
        };
        assert_eq!(severity, expected, "{kind} carries the wrong severity");
    }

    // The pane the clipping happened in is named, so a fix can be aimed.
    let clipped = report["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|d| d["kind"] == "layout.clipped")
        .expect("a clipped diagnostic");
    assert_eq!(clipped["pane"], "body");
}

#[test]
fn a_clean_deck_still_writes_a_document() {
    if !chromium_available() {
        eprintln!("skipping: no Chromium; set MIRZAM_CHROMIUM to run this test");
        return;
    }
    let dir = TempDir::new("clean");
    let deck = dir.deck("clean.md", CLEAN);
    let (report, ok) = check_json(&deck);

    assert!(ok, "a clean deck exits zero");
    assert_eq!(report["ok"], true);
    assert_eq!(report["version"], 1);
    assert_eq!(
        report["diagnostics"].as_array().map(Vec::len),
        Some(0),
        "an empty array, not a missing field: {report}"
    );
    assert!(report["notes"].is_array(), "notes is always an array");
}

/// The text form is the default and is unchanged: a caller that never asks for
/// JSON must not start receiving it.
#[test]
fn text_stays_the_default() {
    if !chromium_available() {
        eprintln!("skipping: no Chromium; set MIRZAM_CHROMIUM to run this test");
        return;
    }
    let dir = TempDir::new("text");
    let deck = dir.deck("clean.md", CLEAN);
    let out = Command::new(env!("CARGO_BIN_EXE_mirzam"))
        .args(["check", &deck.display().to_string()])
        .output()
        .expect("run mirzam check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("no layout problems"),
        "the text verdict, not JSON: {stdout}"
    );
}

/// An unknown format is a typo to report, not something to fall back from -
/// silently printing prose to a caller that asked for records is exactly the
/// failure a parseable format exists to avoid.
#[test]
fn an_unknown_format_is_refused() {
    let out = Command::new(env!("CARGO_BIN_EXE_mirzam"))
        .args(["check", "deck.md", "--format", "yaml"])
        .output()
        .expect("run mirzam check");
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--format takes text or json"),
        "the error should name the valid values"
    );
}
