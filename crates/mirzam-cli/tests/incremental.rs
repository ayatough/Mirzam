//! 差分ビルドの正しさ:
//! **インクリメンタルビルドの結果は、フルビルドの結果と完全に一致する。**
//!
//! ホットリロードで古い表示が残る類のバグを機械的に防ぐための不変条件。

mod common;

use common::repo_root;
use std::collections::HashMap;

/// 一時ディレクトリに書き出したデッキ
struct TempDeck {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDeck {
    fn new(name: &str, body: &str) -> TempDeck {
        let dir = std::env::temp_dir().join(format!("mirzam-test-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).expect("一時ディレクトリ");
        let path = dir.join("deck.md");
        std::fs::write(&path, body).expect("デッキ書き込み");
        TempDeck { dir, path }
    }

    fn write(&self, body: &str) {
        std::fs::write(&self.path, body).expect("デッキ更新");
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
            "## {t}\n\n本文 {{{{ n * {} }}}} と数式 $x^{}$\n",
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

    // 3 枚目だけを編集
    deck.write(&deck_source(&["A", "B", "C-edited", "D"]));

    let incremental = mirzam_cli::pipeline::build_deck(&deck.path, &mut warm).unwrap();
    let mut cold = HashMap::new();
    let full = mirzam_cli::pipeline::build_deck(&deck.path, &mut cold).unwrap();

    assert_eq!(
        incremental.sections, full.sections,
        "差分ビルドの出力がフルビルドと一致しない"
    );
    assert_eq!(incremental.hashes, full.hashes);
    assert_eq!(
        incremental.rendered, 1,
        "変更した 1 枚だけを再レンダリングするはず(実際: {})",
        incremental.rendered
    );
}

#[test]
fn variable_change_invalidates_all_slides_that_use_it() {
    let deck = TempDeck::new("vars", &deck_source(&["A", "B"]));
    let mut cache = HashMap::new();
    let first = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    // frontmatter の変数だけを変える → 参照している全スライドが変わる
    let updated = deck_source(&["A", "B"]).replace("n: 3", "n: 5");
    deck.write(&updated);
    let second = mirzam_cli::pipeline::build_deck(&deck.path, &mut cache).unwrap();

    assert_ne!(
        first.sections, second.sections,
        "変数変更が反映されていない"
    );
    assert_eq!(
        second.rendered, 2,
        "変数を参照する 2 枚とも再レンダリングされるはず"
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
    assert_eq!(again.rendered, 0, "変更が無ければ再レンダリングは 0 枚");
}

#[test]
fn include_and_assets_are_tracked_for_watching() {
    let mut cache = HashMap::new();
    let path = repo_root().join("examples/demo.md");
    let out = mirzam_cli::pipeline::build_deck(&path, &mut cache).unwrap();

    let names: Vec<String> = out
        .files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"demo.md".to_string()));
    assert!(
        names.contains(&"architecture.md".to_string()),
        "include したファイルが監視対象に入っていない: {names:?}"
    );
    assert!(
        names.contains(&"bench.svg".to_string()),
        "参照した画像が監視対象に入っていない: {names:?}"
    );
}

/// スライドを跨いだ入れ替えでも、位置に応じた出力が正しく作られる
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
        "並べ替え後の出力が一致しない"
    );
    assert!(swapped.sections[0].contains("data-index=\"0\""));
}
