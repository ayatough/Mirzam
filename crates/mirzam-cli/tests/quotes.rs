//! The quote check, end to end: a deck whose `annotate` block says a paper
//! prints certain words, and the paper - written here, byte by byte, like the
//! `import pdf` fixture - either does or does not.
//!
//! No browser is involved: `mirzam_cli::quotes::verify` is the half of
//! `mirzam check` that reads a PDF, and it is driven directly.

use mirzam_cli::quotes::{verify, Finding};
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("mirzam-quotes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("papers")).expect("temp dir");
        TempDir(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).expect("write");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Builds the deck the way `mirzam check` does and runs the quote check on it.
fn check(deck: &std::path::Path) -> Vec<Finding> {
    let mut cache = std::collections::HashMap::new();
    let out = mirzam_cli::pipeline::build_deck_with(deck, &mut cache, None, None)
        .expect("the deck builds");
    verify(deck, &out)
}

const BIB: &str = "@article{fox2020,\n  author = {Fox, Quick},\n  title = {On jumping},\n  \
                   year = {2020},\n  file = {Full Text PDF:papers/fox.pdf:application/pdf}\n}\n";

fn deck_source(quote_lines: &str) -> String {
    format!(
        "---\ntitle: t\nbibliography: refs.bib\n---\n\n# One\n\n---\n\n\
         ```pane\n+--------+\n| src    |\n+--------+\n```\n\n\
         ::: pane src\n![p. 1](fox.svg){{#fox-p1 fit=contain credit=\"p. 1 of [@fox2020]\"}}\n:::\n\n\
         ```annotate\ntarget: #fox-p1\nsource: @fox2020\n{quote_lines}```\n"
    )
}

#[test]
fn a_quote_the_page_prints_passes_and_one_it_does_not_fails() {
    let dir = TempDir::new("verify");
    dir.write("papers/fox.pdf", &paper());
    dir.write("refs.bib", BIB.as_bytes());
    dir.write("fox.svg", b"<svg viewBox=\"0 0 1 1\"/>");
    let deck = dir.write(
        "deck.md",
        deck_source(
            "highlight 50,20 90x8 : step=1 quote=\"jumps over the lazy dog\" page=1\n\
             highlight 50,30 90x8 : step=1\n\
             highlight 50,40 90x8 : step=2 quote=\"the quick brown fox flies\" page=1\n\
             highlight 50,50 90x8 : step=3 quote=\"the quikc brown fox\" page=1\n",
        )
        .as_bytes(),
    );

    let findings = check(&deck);
    assert_eq!(findings.len(), 2, "{findings:#?}");
    let missing = &findings[0];
    assert!(missing.error, "{missing:?}");
    assert_eq!(missing.slide, 2);
    assert!(
        missing.message.contains("not on p. 1 of fox.pdf")
            && missing.message.contains("the quick brown fox flies"),
        "{missing:?}"
    );
    let near = &findings[1];
    assert!(!near.error, "a misspelling is a warning: {near:?}");
    assert!(
        near.message.contains("differs") && near.message.contains("the quick brown fox"),
        "what the page prints is shown: {near:?}"
    );
}

#[test]
fn a_source_that_cannot_be_reached_is_a_warning_not_a_verdict() {
    let dir = TempDir::new("unreachable");
    dir.write("refs.bib", BIB.as_bytes());
    dir.write("fox.svg", b"<svg viewBox=\"0 0 1 1\"/>");
    // No papers/fox.pdf: the bibliography points at a file that is not there.
    let deck = dir.write(
        "deck.md",
        deck_source("highlight 50,20 90x8 : quote=\"jumps over the lazy dog\" page=1\n").as_bytes(),
    );
    let findings = check(&deck);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert!(!findings[0].error);
    assert!(
        findings[0].message.contains("cannot read"),
        "{:?}",
        findings[0]
    );

    // A block with quotes and no source at all.
    let deck = dir.write(
        "deck2.md",
        deck_source("highlight 50,20 90x8 : quote=\"jumps over the lazy dog\" page=1\n")
            .replace("source: @fox2020\n", "")
            .as_bytes(),
    );
    let findings = check(&deck);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert!(findings[0].message.contains("source:"), "{:?}", findings[0]);
}

#[test]
fn a_path_source_is_relative_to_the_deck() {
    let dir = TempDir::new("path");
    dir.write("papers/fox.pdf", &paper());
    dir.write("fox.svg", b"<svg viewBox=\"0 0 1 1\"/>");
    let deck = dir.write(
        "deck.md",
        deck_source("highlight 50,20 90x8 : quote=\"over the lazy dog\" page=1\n")
            .replace("source: @fox2020", "source: papers/fox.pdf")
            .replace("bibliography: refs.bib\n", "")
            .as_bytes(),
    );
    assert!(check(&deck).is_empty());
}

/// One page of prose in a standard font, no figures.
fn paper() -> Vec<u8> {
    let body = "the quick brown fox jumps over the lazy dog";
    let content = format!(
        "BT /F1 10 Tf 40 300 Td ({body}) Tj ET\n\
         BT /F1 10 Tf 40 286 Td ({body}) Tj ET\n"
    );
    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] \
           /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            .to_vec(),
        {
            let mut object = format!("<<  /Length {} >>\nstream\n", content.len()).into_bytes();
            object.extend_from_slice(content.as_bytes());
            object.extend_from_slice(b"\nendstream");
            object
        },
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}
