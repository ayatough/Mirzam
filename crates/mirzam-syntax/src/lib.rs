//! ソーステキストの構造分解。
//!
//! - frontmatter の切り出し
//! - `![[file.md]]` のトランスクルージョン(循環検出付き)
//! - `---` によるスライド分割(コードフェンス内は無視)
//! - スライド内の構造抽出: `pane` レイアウトブロック、`::: pane` div、
//!   `<!-- note: -->`、予約 fenced block(shape/connect/anim)

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// ファイル読み込みの抽象(WASM 環境では別実装を注入する)
pub trait FileProvider {
    fn read(&self, path: &Path) -> Result<String, String>;
}

/// std::fs ベースの既定実装
pub struct FsProvider;

impl FileProvider for FsProvider {
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("{} を読めません: {e}", path.display()))
    }
}

/// frontmatter(YAML)と本文に分離する
pub fn split_frontmatter(src: &str) -> (Option<&str>, &str) {
    let src_norm = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lines = src_norm.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, src_norm);
    }
    // 2 本目の --- を探す
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

/// `![[path]]` を再帰的に展開する。循環参照はエラーテキストに置換。
pub fn expand_includes(
    body: &str,
    base_dir: &Path,
    provider: &dyn FileProvider,
) -> String {
    let mut visited = BTreeSet::new();
    expand_includes_inner(body, base_dir, provider, &mut visited)
}

fn expand_includes_inner(
    body: &str,
    base_dir: &Path,
    provider: &dyn FileProvider,
    visited: &mut BTreeSet<PathBuf>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_code = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !in_code {
            if let Some(target) = parse_include_line(trimmed) {
                let path = base_dir.join(target);
                let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
                if visited.contains(&canon) {
                    out.push_str(&format!(
                        "> ⚠ 循環参照のため展開できません: `{}`\n",
                        target
                    ));
                    continue;
                }
                match provider.read(&path) {
                    Ok(content) => {
                        visited.insert(canon.clone());
                        // 子ファイルの frontmatter は無視する
                        let (_, child_body) = split_frontmatter(&content);
                        let child_dir = path.parent().unwrap_or(base_dir).to_path_buf();
                        out.push_str(&expand_includes_inner(
                            child_body, &child_dir, provider, visited,
                        ));
                        visited.remove(&canon);
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    Err(e) => {
                        out.push_str(&format!("> ⚠ include 失敗: {e}\n"));
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

/// 行全体が `![[...]]` のとき、その中身を返す
fn parse_include_line(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("![[")?.strip_suffix("]]")?;
    if inner.is_empty() || inner.contains("[[") {
        return None;
    }
    Some(inner.trim())
}

/// 本文をスライド単位に分割する(コードフェンス外の `---` 行)
pub fn split_slides(body: &str) -> Vec<String> {
    let mut slides = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
        }
        if !in_code && is_slide_break(trimmed) {
            slides.push(std::mem::take(&mut current));
            continue;
        }
        current.push_str(line);
        current.push('\n');
    }
    slides.push(current);
    // 空白のみのスライドは除外
    slides
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect()
}

fn is_slide_break(trimmed: &str) -> bool {
    trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-')
}

/// 予約 fenced block の種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Shape,
    Connect,
    Anim,
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Shape => "shape",
            BlockKind::Connect => "connect",
            BlockKind::Anim => "anim",
        }
    }

    fn from_info(info: &str) -> Option<Self> {
        match info {
            "shape" => Some(BlockKind::Shape),
            "connect" => Some(BlockKind::Connect),
            "anim" => Some(BlockKind::Anim),
            _ => None,
        }
    }
}

/// スライド 1 枚の構造分解結果
#[derive(Debug, Clone, Default)]
pub struct SlideSource {
    /// ```pane ブロックの中身(ASCII グリッド)
    pub layout: Option<String>,
    /// (ペイン名, Markdown 内容)。`::: pane X` で割り当てられたもの
    pub panes: Vec<(String, String)>,
    /// どのペインにも割り当てられていない Markdown
    pub loose: String,
    /// スピーカーノート
    pub notes: Vec<String>,
    /// 予約ブロック(未実装フェーズのもの)
    pub reserved: Vec<(BlockKind, String)>,
}

/// スライドのソースを構造分解する
pub fn parse_slide(src: &str) -> SlideSource {
    let mut slide = SlideSource::default();
    let mut lines = src.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // fenced code block
        if let Some(info) = trimmed.strip_prefix("```") {
            let info = info.trim();
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.trim() == "```" {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            if info == "pane" || info.starts_with("pane ") {
                slide.layout = Some(body);
            } else if let Some(kind) = BlockKind::from_info(info) {
                slide.reserved.push((kind, body));
            } else {
                // 通常のコードブロックとして本文に戻す
                slide.loose.push_str(&format!("```{info}\n{body}```\n"));
            }
            continue;
        }

        // fenced div: ::: pane NAME [{attrs}]
        if let Some(rest) = trimmed.strip_prefix(":::") {
            let rest = rest.trim();
            if let Some(pane_name) = parse_pane_open(rest) {
                let mut body = String::new();
                let mut in_code = false;
                for inner in lines.by_ref() {
                    let t = inner.trim();
                    if t.starts_with("```") {
                        in_code = !in_code;
                    }
                    if !in_code && t == ":::" {
                        break;
                    }
                    body.push_str(inner);
                    body.push('\n');
                }
                slide.panes.push((pane_name, body));
                continue;
            }
            // pane 以外の div(::: note など)はそのまま本文へ(将来対応)
            slide.loose.push_str(line);
            slide.loose.push('\n');
            continue;
        }

        // HTML コメント(note 収集、slide 設定は将来)
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
            // note 以外のコメントは本文に残す(HTML として非表示)
            slide.loose.push_str(&comment);
            slide.loose.push('\n');
            continue;
        }

        slide.loose.push_str(line);
        slide.loose.push('\n');
    }

    slide
}

/// `pane NAME {attrs}` 形式の開始行からペイン名を取り出す
fn parse_pane_open(rest: &str) -> Option<String> {
    let rest = rest.strip_prefix("pane")?.trim();
    if rest.is_empty() {
        return None;
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
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
        assert_eq!(s.panes[0].0, "a");
        assert!(s.panes[0].1.contains("**world**"));
        assert!(s.loose.contains("## Title"));
        assert!(s.loose.contains("loose text"));
        assert_eq!(s.reserved.len(), 1);
        assert_eq!(s.reserved[0].0, BlockKind::Connect);
        assert_eq!(s.notes, vec!["remember this"]);
    }

    #[test]
    fn code_fence_inside_pane_div() {
        let src = "::: pane main\n```rust\nlet x = 1;\n```\n:::\n";
        let s = parse_slide(src);
        assert_eq!(s.panes.len(), 1);
        assert!(s.panes[0].1.contains("let x = 1;"));
    }

    #[test]
    fn include_line_parse() {
        assert_eq!(parse_include_line("![[a/b.md]]"), Some("a/b.md"));
        assert_eq!(parse_include_line("![](x.png)"), None);
        assert_eq!(parse_include_line("text ![[a.md]]"), None);
    }
}
