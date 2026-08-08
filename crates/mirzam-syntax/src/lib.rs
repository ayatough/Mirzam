//! Structural decomposition of the source text.
//!
//! - Splitting off frontmatter
//! - `![[file.md]]` transclusion, with cycle detection
//! - Slide splitting on `---` (ignored inside code fences)
//! - Per-slide extraction: the `pane` layout block, `::: pane` divs,
//!   `<!-- note: -->` comments, and the shape/connect/anim fenced blocks

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Abstraction over file reads; WASM hosts inject their own implementation.
pub trait FileProvider {
    fn read(&self, path: &Path) -> Result<String, String>;
}

/// Default implementation backed by `std::fs`.
pub struct FsProvider;

impl FileProvider for FsProvider {
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    }
}

/// Splits YAML frontmatter from the body.
pub fn split_frontmatter(src: &str) -> (Option<&str>, &str) {
    let src_norm = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lines = src_norm.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, src_norm);
    }
    // Find the closing `---`.
    let after_first = &src_norm[src_norm.find('\n').map(|i| i + 1).unwrap_or(src_norm.len())..];
    let mut offset = 0usize;
    for line in after_first.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let yaml = &after_first[..offset];
            let body = &after_first[offset + line.len()..];
            return (Some(yaml), body);
        }
        offset += line.len();
    }
    (None, src_norm)
}

/// Recursively expands `![[path]]`. Cycles are replaced with an error note.
pub fn expand_includes(body: &str, base_dir: &Path, provider: &dyn FileProvider) -> String {
    let mut files = BTreeSet::new();
    expand_includes_tracked(body, base_dir, provider, &mut files)
}

/// Same as `expand_includes`, but records every file that was read into
/// `files` so `serve` knows what to watch.
pub fn expand_includes_tracked(
    body: &str,
    base_dir: &Path,
    provider: &dyn FileProvider,
    files: &mut BTreeSet<PathBuf>,
) -> String {
    let mut visited = BTreeSet::new();
    expand_includes_inner(body, base_dir, provider, &mut visited, files)
}

fn expand_includes_inner(
    body: &str,
    base_dir: &Path,
    provider: &dyn FileProvider,
    visited: &mut BTreeSet<PathBuf>,
    files: &mut BTreeSet<PathBuf>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut fence: Option<usize> = None;
    for line in body.lines() {
        let trimmed = line.trim();
        match fence {
            Some(open) if closes_fence(trimmed, open) => fence = None,
            None => {
                if let Some(n) = fence_len(trimmed) {
                    fence = Some(n);
                }
            }
            _ => {}
        }
        if fence.is_some() || closes_fence(trimmed, 3) {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        {
            if let Some(target) = parse_include_line(trimmed) {
                let path = base_dir.join(target);
                let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
                if visited.contains(&canon) {
                    out.push_str(&format!(
                        "> ⚠ circular include, not expanded: `{}`\n",
                        target
                    ));
                    continue;
                }
                match provider.read(&path) {
                    Ok(content) => {
                        visited.insert(canon.clone());
                        files.insert(path.clone());
                        // Frontmatter in included files is ignored.
                        let (_, child_body) = split_frontmatter(&content);
                        let child_dir = path.parent().unwrap_or(base_dir).to_path_buf();
                        out.push_str(&expand_includes_inner(
                            child_body, &child_dir, provider, visited, files,
                        ));
                        visited.remove(&canon);
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    Err(e) => {
                        out.push_str(&format!("> ⚠ include failed: {e}\n"));
                    }
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Number of leading backticks when a line opens or closes a fence.
/// CommonMark lets a longer fence contain shorter ones, which is how a document
/// shows Mirzam syntax without the inner blocks being taken literally.
pub fn fence_len(trimmed: &str) -> Option<usize> {
    let n = trimmed.chars().take_while(|c| *c == '`').count();
    (n >= 3).then_some(n)
}

/// Whether `trimmed` closes a fence opened with `open` backticks.
fn closes_fence(trimmed: &str, open: usize) -> bool {
    match fence_len(trimmed) {
        Some(n) => n >= open && trimmed.chars().all(|c| c == '`'),
        None => false,
    }
}

/// Returns the target when the whole line is `![[...]]`.
fn parse_include_line(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("![[")?.strip_suffix("]]")?;
    if inner.is_empty() || inner.contains("[[") {
        return None;
    }
    Some(inner.trim())
}

/// Splits the body into slides on `---` lines outside code fences.
pub fn split_slides(body: &str) -> Vec<String> {
    split_slides_at(body, None)
}

/// Splits into slides on `---`, and additionally before every heading of
/// `level` when given. Heading splitting is what lets an ordinary document -
/// a README, a set of notes - become a deck without editing it.
pub fn split_slides_at(body: &str, level: Option<u8>) -> Vec<String> {
    let mut slides = Vec::new();
    let mut current = String::new();
    let mut fence: Option<usize> = None;
    for line in body.lines() {
        let trimmed = line.trim();
        match fence {
            Some(open) if closes_fence(trimmed, open) => fence = None,
            None => {
                if let Some(n) = fence_len(trimmed) {
                    fence = Some(n);
                }
            }
            _ => {}
        }
        let in_code = fence.is_some();
        if !in_code && is_slide_break(trimmed) {
            slides.push(std::mem::take(&mut current));
            continue;
        }
        if !in_code && level.is_some_and(|l| heading_level(trimmed) == Some(l)) {
            // The first heading opens the deck rather than an empty slide.
            if !current.trim().is_empty() {
                slides.push(std::mem::take(&mut current));
            }
        }
        current.push_str(line);
        current.push('\n');
    }
    slides.push(current);
    // Drop slides that are only whitespace.
    slides
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// The ATX heading level of a line, if it is one.
fn heading_level(trimmed: &str) -> Option<u8> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    (1..=6)
        .contains(&hashes)
        .then(|| trimmed[hashes..].starts_with(' ').then_some(hashes as u8))
        .flatten()
}

fn is_slide_break(trimmed: &str) -> bool {
    trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-')
}

/// Fenced blocks reserved for a later phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Anim,
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Anim => "anim",
        }
    }

    fn from_info(info: &str) -> Option<Self> {
        match info {
            "anim" => Some(BlockKind::Anim),
            _ => None,
        }
    }
}

/// A content block assigned to a pane via `::: pane`.
#[derive(Debug, Clone, Default)]
pub struct PaneBlock {
    pub name: String,
    /// Contents of `{align=center valign=middle .cls}`; empty when omitted.
    pub attrs: String,
    pub body: String,
}

/// The decomposed structure of a single slide.
#[derive(Debug, Clone, Default)]
pub struct SlideSource {
    /// Body of the ```pane block (the ASCII grid).
    pub layout: Option<String>,
    /// Content assigned through `::: pane X`.
    pub panes: Vec<PaneBlock>,
    /// Markdown not assigned to any pane.
    pub loose: String,
    /// Speaker notes.
    pub notes: Vec<String>,
    /// ```shape blocks; multiple blocks are concatenated.
    pub shapes: Vec<String>,
    /// ```connect blocks.
    pub connects: Vec<String>,
    /// Blocks reserved for a later phase.
    pub reserved: Vec<(BlockKind, String)>,
}

/// Decomposes a slide's source into its parts.
pub fn parse_slide(src: &str) -> SlideSource {
    let mut slide = SlideSource::default();
    let mut lines = src.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // fenced code block
        if let Some(open) = fence_len(trimmed) {
            let info = trimmed[open..].trim();
            let mut body = String::new();
            for inner in lines.by_ref() {
                if closes_fence(inner.trim(), open) {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            // A longer fence quotes Mirzam syntax rather than using it.
            if open > 3 {
                slide.loose.push_str(&format!(
                    "{}{info}\n{body}{}\n",
                    "`".repeat(open),
                    "`".repeat(open)
                ));
                continue;
            }
            if info == "pane" || info.starts_with("pane ") {
                slide.layout = Some(body);
            } else if info == "shape" {
                slide.shapes.push(body);
            } else if info == "connect" {
                slide.connects.push(body);
            } else if let Some(kind) = BlockKind::from_info(info) {
                slide.reserved.push((kind, body));
            } else {
                // Any other fence is an ordinary code block.
                slide.loose.push_str(&format!("```{info}\n{body}```\n"));
            }
            continue;
        }

        // fenced div: ::: pane NAME [{attrs}]
        if let Some(rest) = trimmed.strip_prefix(":::") {
            let rest = rest.trim();
            if let Some((pane_name, attrs)) = parse_pane_open(rest) {
                let mut body = String::new();
                let mut inner_fence: Option<usize> = None;
                for inner in lines.by_ref() {
                    let t = inner.trim();
                    match inner_fence {
                        Some(open) if closes_fence(t, open) => inner_fence = None,
                        None => {
                            if let Some(n) = fence_len(t) {
                                inner_fence = Some(n);
                            }
                        }
                        _ => {}
                    }
                    if inner_fence.is_none() && t == ":::" {
                        break;
                    }
                    body.push_str(inner);
                    body.push('\n');
                }
                slide.panes.push(PaneBlock {
                    name: pane_name,
                    attrs,
                    body,
                });
                continue;
            }
            // Other fenced divs (`::: note` etc.) stay in the body for now.
            slide.loose.push_str(line);
            slide.loose.push('\n');
            continue;
        }

        // HTML comments: speaker notes today, slide settings later.
        if trimmed.starts_with("<!--") {
            let mut comment = String::from(trimmed);
            while !comment.contains("-->") {
                match lines.next() {
                    Some(l) => {
                        comment.push('\n');
                        comment.push_str(l);
                    }
                    None => break,
                }
            }
            if let Some(note) = parse_note_comment(&comment) {
                slide.notes.push(note);
                continue;
            }
            // Non-note comments stay in the body, hidden as HTML comments.
            slide.loose.push_str(&comment);
            slide.loose.push('\n');
            continue;
        }

        slide.loose.push_str(line);
        slide.loose.push('\n');
    }

    slide
}

/// Extracts the pane name and attributes from a `pane NAME {attrs}` opener.
fn parse_pane_open(rest: &str) -> Option<(String, String)> {
    let rest = rest.strip_prefix("pane")?.trim();
    if rest.is_empty() {
        return None;
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if name.is_empty() {
        return None;
    }
    let after = rest[name.len()..].trim();
    let attrs = after
        .strip_prefix('{')
        .and_then(|a| a.strip_suffix('}'))
        .unwrap_or("")
        .to_string();
    Some((name, attrs))
}

fn parse_note_comment(comment: &str) -> Option<String> {
    let inner = comment.strip_prefix("<!--")?;
    let inner = inner.strip_suffix("-->").unwrap_or(inner);
    let inner = inner.trim();
    let note = inner.strip_prefix("note:")?;
    Some(note.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_split() {
        let (fm, body) = split_frontmatter("---\ntitle: x\n---\nbody\n");
        assert_eq!(fm, Some("title: x\n"));
        assert_eq!(body, "body\n");
    }

    #[test]
    fn no_frontmatter() {
        let (fm, body) = split_frontmatter("# hello\n");
        assert_eq!(fm, None);
        assert_eq!(body, "# hello\n");
    }

    #[test]
    fn heading_split_starts_a_slide_per_heading() {
        let body = "# Title\n\nintro\n\n## A\n\nbody a\n\n## B\n\nbody b\n";
        let slides = split_slides_at(body, Some(2));
        assert_eq!(slides.len(), 3);
        assert!(slides[0].contains("# Title") && slides[0].contains("intro"));
        assert!(slides[1].starts_with("## A"));
        assert!(slides[2].starts_with("## B"));
    }

    #[test]
    fn heading_split_ignores_other_levels_and_fences() {
        let body = "## A\n\n### sub\n\n```\n## not a heading\n```\n\n## B\n";
        let slides = split_slides_at(body, Some(2));
        assert_eq!(slides.len(), 2);
        assert!(slides[0].contains("### sub"));
        assert!(slides[0].contains("## not a heading"));
    }

    #[test]
    fn heading_level_recognises_atx_only() {
        assert_eq!(heading_level("## x"), Some(2));
        assert_eq!(heading_level("######## x"), None);
        assert_eq!(heading_level("##x"), None);
        assert_eq!(heading_level("text"), None);
    }

    #[test]
    fn slide_split_ignores_fences() {
        let body = "a\n---\nb\n```\n---\n```\nc\n";
        let slides = split_slides(body);
        assert_eq!(slides.len(), 2);
        assert!(slides[1].contains("```\n---\n```"));
    }

    #[test]
    fn parse_slide_structure() {
        let src = "\
## Title

```pane
+---+---+
| a | b |
+---+---+
```

::: pane a
hello **world**
:::

loose text

```connect
#x -> #y
```

<!-- note: remember this -->
";
        let s = parse_slide(src);
        assert!(s.layout.is_some());
        assert_eq!(s.panes.len(), 1);
        assert_eq!(s.panes[0].name, "a");
        assert!(s.panes[0].body.contains("**world**"));
        assert!(s.loose.contains("## Title"));
        assert!(s.loose.contains("loose text"));
        assert_eq!(s.connects.len(), 1);
        assert!(s.connects[0].contains("#x -> #y"));
        assert_eq!(s.notes, vec!["remember this"]);
    }

    #[test]
    fn longer_fence_quotes_mirzam_syntax() {
        // A document that *shows* Mirzam syntax wraps it in a longer fence; the
        // inner blocks must stay text rather than being executed.
        let src =
            "````markdown\n```pane\n+---+\n| a |\n+---+\n```\n\n```connect\n#a -> #b\n```\n````\n";
        let s = parse_slide(src);
        assert!(s.layout.is_none(), "inner pane block must not be applied");
        assert!(
            s.connects.is_empty(),
            "inner connect block must not be applied"
        );
        assert!(s.loose.contains("```pane"));
        assert!(s.loose.contains("#a -> #b"));
    }

    #[test]
    fn longer_fence_hides_slide_breaks() {
        let body = "a\n\n````\n---\n````\n\nb\n";
        assert_eq!(split_slides(body).len(), 1);
    }

    #[test]
    fn code_fence_inside_pane_div() {
        let src = "::: pane main\n```rust\nlet x = 1;\n```\n:::\n";
        let s = parse_slide(src);
        assert_eq!(s.panes.len(), 1);
        assert!(s.panes[0].body.contains("let x = 1;"));
    }

    #[test]
    fn include_line_parse() {
        assert_eq!(parse_include_line("![[a/b.md]]"), Some("a/b.md"));
        assert_eq!(parse_include_line("![](x.png)"), None);
        assert_eq!(parse_include_line("text ![[a.md]]"), None);
    }
}
