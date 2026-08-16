//! `mermaid` fences through the real build pipeline.
//!
//! This machine, and CI, have no `mmdc`, so what is testable here is the path
//! that matters most: **a deck whose diagram cannot be drawn still builds, the
//! fence is a readable code block, and the build says so.** The other half —
//! an SVG arriving and being re-coloured — is unit-tested in
//! `mirzam-render/src/mermaid.rs` against a fake renderer, since a test may
//! not depend on a Node tool being installed.

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
}

impl Drop for TempDeck {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const DECK: &str = "\
---
title: Pipelines
---

## How it flows

```mermaid
flowchart LR
  ingest --> store --> serve
```
";

/// Without a renderer the deck is still a deck: it builds, the diagram's
/// source is on the slide as code, and nothing is drawn in red over it.
#[test]
fn a_deck_with_no_renderer_builds_and_keeps_the_diagram_as_code() {
    let deck = TempDeck::new("mermaid-degrade", DECK);
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut HashMap::new()).expect("build");
    let html = out.sections.concat();

    // `mmdc` is not installed here; if a machine running these tests has one,
    // the deck renders a diagram instead and there is nothing to assert about
    // the degradation. Skipping beats a suite that fails on a better machine.
    if !html.contains("language-mermaid") {
        assert!(
            html.contains("mz-mermaid"),
            "neither a diagram nor code: {html}"
        );
        return;
    }

    assert!(html.contains("flowchart LR"), "{html}");
    assert!(
        !html.contains("mz-error"),
        "a warning must not draw a box: {html}"
    );
    let mermaid: Vec<&String> = out
        .warnings
        .iter()
        .filter(|w| w.contains("mermaid:"))
        .collect();
    assert_eq!(mermaid.len(), 1, "{:?}", out.warnings);
    assert!(mermaid[0].contains("shown as code"), "{mermaid:?}");
    assert!(
        mermaid[0].contains("MIRZAM_MMDC"),
        "the warning has to say how to fix it: {mermaid:?}"
    );
}

/// The whole of non-negotiable 1 for this block form, checked on real output
/// rather than on a fence in isolation: the degraded slide is exactly what a
/// plain CommonMark parser makes of the same source — which is also what
/// GitHub draws as a diagram.
#[test]
fn the_degraded_block_is_the_one_a_plain_parser_would_have_made() {
    let deck = TempDeck::new("mermaid-plain", DECK);
    let out = mirzam_cli::pipeline::build_deck(&deck.path, &mut HashMap::new()).expect("build");
    let html = out.sections.concat();
    if !html.contains("language-mermaid") {
        return; // this machine has a renderer; see the test above
    }
    assert!(
        html.contains("<pre><code class=\"language-mermaid\">"),
        "{html}"
    );
    // Uncoloured: `mermaid` is not a language the highlighter knows, and a
    // half-highlighted diagram source would be worse than none.
    assert!(!html.contains("tok-"), "{html}");
}
