//! 統合テスト共通のヘルパ
//!
//! 各テストバイナリごとに取り込まれるため、使われない項目が出る
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// リポジトリルート(crates/mirzam-cli から 2 階層上)
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/mirzam-cli の 2 階層上")
        .to_path_buf()
}

pub fn example(name: &str) -> PathBuf {
    repo_root().join("examples").join(name)
}

/// リポジトリに含まれるサンプルデッキ(ゴールデンテストの対象)
pub const EXAMPLE_DECKS: &[&str] = &["demo.md", "seminar.md", "media.md"];

/// スナップショット比較用に出力を正規化する。
/// data URI(フォント・画像・動画)は長さだけを残し、内容の差分ノイズを消す。
pub fn normalize(html: &str) -> String {
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

/// 依存を増やさないための最小のパターン検索(data URI 用)。
/// 見つかった範囲 (start, end) を返す。
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
