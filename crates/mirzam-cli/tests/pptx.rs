//! `mirzam export pptx` end to end: a deck goes in, a package comes out
//! whose slides hold the words as text, the table as a table, the chart as
//! a picture and the notes as notes.
//!
//! The export needs a browser, so these tests skip themselves when none can
//! be found — the same rule `check_json.rs` follows — and run wherever
//! `MIRZAM_CHROMIUM` points at one. The package is stored uncompressed, so
//! its XML can be read straight out of the bytes without a ZIP reader.

use std::path::PathBuf;
use std::process::Command;

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

const DECK: &str = r#"---
title: Export test
---

# The words survive

- A bullet with a [link](https://example.com/docs?a=1&b=2)
- Inline maths such as $x^2$ in a line

| Plan | Seats |
|---|---:|
| Team | 8 |

```rust
fn main() {}
```

```chart
type: bar
title: Marks
data: |
  x, y
  a, 1
  b, 2
```

<!-- note: Remember to breathe. -->
"#;

/// Runs one export and returns the bytes of the package it wrote.
fn export(extra: &[&str]) -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!(
        "mirzam-pptx-test-{}-{}",
        std::process::id(),
        extra.len()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let md: PathBuf = dir.join("deck.md");
    std::fs::write(&md, DECK).unwrap();
    let out = dir.join("deck.pptx");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mirzam"));
    cmd.arg("export").arg("pptx").arg(&md).arg("-o").arg(&out);
    cmd.args(extra);
    let status = cmd.output().expect("mirzam runs");
    assert!(
        status.status.success(),
        "export failed: {}{}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
    let bytes = std::fs::read(&out).expect("the export wrote a file");
    let _ = std::fs::remove_dir_all(&dir);
    bytes
}

/// The text of one stored part, walked to by its local header: the same
/// name also appears in `[Content_Types].xml` and the central directory,
/// so a plain search would land on the wrong bytes.
fn part(zip: &[u8], name: &str) -> String {
    let mut at = 0;
    while at + 30 <= zip.len() {
        let Some(next) = zip[at..].windows(4).position(|w| w == b"PK\x03\x04") else {
            break;
        };
        let h = at + next;
        let size =
            u32::from_le_bytes([zip[h + 18], zip[h + 19], zip[h + 20], zip[h + 21]]) as usize;
        let name_len = u16::from_le_bytes([zip[h + 26], zip[h + 27]]) as usize;
        let extra_len = u16::from_le_bytes([zip[h + 28], zip[h + 29]]) as usize;
        let start = h + 30 + name_len + extra_len;
        if &zip[h + 30..h + 30 + name_len] == name.as_bytes() {
            return String::from_utf8_lossy(&zip[start..start + size]).into_owned();
        }
        at = start + size;
    }
    panic!("no part {name}");
}

#[test]
fn the_words_come_out_as_text_and_the_rest_as_what_it_is() {
    if !chromium_available() {
        eprintln!("skipping: no Chromium; set MIRZAM_CHROMIUM to run this test");
        return;
    }
    let zip = export(&[]);
    let slide = part(&zip, "ppt/slides/slide1.xml");

    // The heading and the list are runs of text, the list bulleted.
    assert!(slide.contains("<a:t>The words survive</a:t>"), "{slide}");
    assert!(slide.contains("A bullet with a "), "{slide}");
    assert!(slide.contains("<a:buChar"), "{slide}");
    // The link is a hyperlink relationship, escaped as XML wants it.
    assert!(slide.contains("<a:hlinkClick r:id="), "{slide}");
    let rels = part(&zip, "ppt/slides/_rels/slide1.xml.rels");
    assert!(
        rels.contains("Target=\"https://example.com/docs?a=1&amp;b=2\" TargetMode=\"External\""),
        "{rels}"
    );
    // Inline maths simple enough to be words is words: a superscript run.
    assert!(slide.contains("baseline=\"30000\""), "{slide}");
    assert!(slide.contains("<a:t>2</a:t>"), "{slide}");
    // The table is a table, its number right-aligned.
    assert!(slide.contains("<a:tbl>"), "{slide}");
    assert!(slide.contains("<a:t>Team</a:t>"), "{slide}");
    assert!(slide.contains("algn=\"r\""), "{slide}");
    // The code block keeps its text — coloured, so in several runs — on
    // its own paper.
    assert!(slide.contains("<a:t>main</a:t>"), "{slide}");
    assert!(slide.contains("() {}"), "{slide}");
    assert!(slide.contains("name=\"pre\""), "{slide}");
    // The chart is a picture, embedded through a relationship.
    assert!(slide.contains("<p:pic>"), "{slide}");
    assert!(rels.contains("Target=\"../media/image1.png\""), "{rels}");
    // The slide's own background colour is the slide background.
    assert!(slide.contains("<p:bg><p:bgPr>"), "{slide}");
    // And the notes are where presenter view reads them.
    let notes = part(&zip, "ppt/notesSlides/notesSlide1.xml");
    assert!(notes.contains("Remember to breathe."), "{notes}");
}

#[test]
fn pictures_mode_is_one_photograph_per_slide() {
    if !chromium_available() {
        eprintln!("skipping: no Chromium; set MIRZAM_CHROMIUM to run this test");
        return;
    }
    let zip = export(&["--pictures"]);
    let slide = part(&zip, "ppt/slides/slide1.xml");
    assert_eq!(slide.matches("<p:pic>").count(), 1, "{slide}");
    assert!(!slide.contains("<a:t>"), "{slide}");
    // The whole slide, at its full size.
    assert!(
        slide.contains("<a:ext cx=\"12192000\" cy=\"6858000\"/>"),
        "{slide}"
    );
    let notes = part(&zip, "ppt/notesSlides/notesSlide1.xml");
    assert!(notes.contains("Remember to breathe."), "{notes}");
}

#[test]
fn pictures_belongs_to_pptx_alone() {
    let out = Command::new(env!("CARGO_BIN_EXE_mirzam"))
        .args(["export", "pdf", "deck.md", "--pictures"])
        .output()
        .expect("mirzam runs");
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr) + String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--pictures"), "{text}");
}
