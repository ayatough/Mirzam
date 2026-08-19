//! Helpers shared by the integration tests.
//!
//! Each test binary includes this module, so some items go unused.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Repository root, two levels above crates/mirzam-cli.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("two levels above crates/mirzam-cli")
        .to_path_buf()
}

pub fn example(name: &str) -> PathBuf {
    repo_root().join("examples").join(name)
}

/// Sample decks in the repository, covered by the golden tests.
pub const EXAMPLE_DECKS: &[&str] = &[
    "01-start.md",
    "02-writing.md",
    "03-layout.md",
    "04-components.md",
    "05-motion.md",
    "06-theming.md",
    "pitch.md",
    "research.md",
    "seminar.md",
    "slideshow.md",
];

/// Normalizes output for snapshot comparison.
/// Data URIs (fonts, images, video) are reduced to their length to keep diffs readable.
/// A Mermaid diagram is reduced to a marker: see [`without_diagrams`].
pub fn normalize(html: &str) -> String {
    without_diagrams(&without_data_uris(html))
}

/// Replaces each `mermaid` fence's output with a fixed marker.
///
/// The gallery deck holds one, and what it renders to depends on the machine:
/// the SVG `mmdc` drew where mermaid-cli is installed, the fence as a code
/// block where it is not. Neither belongs in a snapshot - the first is a
/// picture this project did not draw and that changes with every mermaid
/// release, the second is the absence of it - so both collapse to the same
/// marker and the snapshot describes the deck instead of the machine.
pub fn without_diagrams(html: &str) -> String {
    const FORMS: [(&str, &str); 2] = [
        ("<div class=\"mz-mermaid\">", "</svg></div>"),
        ("<pre><code class=\"language-mermaid\">", "</code></pre>"),
    ];
    let mut out = html.to_string();
    for (open, close) in FORMS {
        while let Some(start) = out.find(open) {
            let Some(end) = out[start..].find(close).map(|i| start + i + close.len()) else {
                break;
            };
            out.replace_range(start..end, "<mermaid-diagram>");
        }
    }
    out
}

fn without_data_uris(html: &str) -> String {
    let re = regex_lite(r"data:[a-z/+.-]+;base64,[A-Za-z0-9+/=]+");
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(m) = re(rest) {
        out.push_str(&rest[..m.0]);
        let payload = &rest[m.0..m.1];
        out.push_str(&format!("<data-uri len={}>", payload.len()));
        rest = &rest[m.1..];
    }
    out.push_str(rest);
    out
}

/// Minimal scanner for data URIs, avoiding an extra dependency.
/// Returns the matched range as (start, end).
fn regex_lite(_pattern: &str) -> impl Fn(&str) -> Option<(usize, usize)> {
    |s: &str| {
        let start = s.find("data:")?;
        let after = &s[start..];
        let b64 = after.find(";base64,")? + ";base64,".len();
        let tail = &after[b64..];
        let len = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='))
            .unwrap_or(tail.len());
        Some((start, start + b64 + len))
    }
}
