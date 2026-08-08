//! ゴールデン(スナップショット)テスト。
//!
//! サンプルデッキのレンダリング結果を `tests/snapshots/` と比較し、
//! 意図しない出力変化を検出する。
//!
//! 出力を意図的に変えた場合は、差分を確認してから更新する:
//!   MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test golden

mod common;

use common::{example, normalize, EXAMPLE_DECKS};
use std::collections::HashMap;

#[test]
fn examples_match_snapshots() {
    let update = std::env::var("MIRZAM_UPDATE_SNAPSHOTS").is_ok();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    std::fs::create_dir_all(&dir).expect("スナップショット用ディレクトリ");

    let mut failures = Vec::new();
    for deck in EXAMPLE_DECKS {
        let mut cache = HashMap::new();
        let out = mirzam_cli::pipeline::build_deck(&example(deck), &mut cache)
            .unwrap_or_else(|e| panic!("{deck} のビルドに失敗: {e}"));

        assert!(
            out.warnings.is_empty(),
            "{deck} に警告があります(サンプルは警告ゼロを保つ): {:?}",
            out.warnings
        );

        let actual = normalize(&out.sections.concat());
        let snap_path = dir.join(format!("{}.html", deck.trim_end_matches(".md")));

        if update || !snap_path.exists() {
            std::fs::write(&snap_path, &actual).expect("スナップショット書き込み");
            continue;
        }
        let expected = std::fs::read_to_string(&snap_path).expect("スナップショット読み込み");
        if expected != actual {
            let at = expected
                .chars()
                .zip(actual.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(actual.len()));
            let ctx = |s: &str| -> String {
                s.chars()
                    .skip(at.saturating_sub(60))
                    .take(160)
                    .collect::<String>()
            };
            failures.push(format!(
                "{deck}: 出力がスナップショットと異なります(位置 {at} 付近)\n  期待: …{}…\n  実際: …{}…",
                ctx(&expected),
                ctx(&actual)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n\n意図した変更なら MIRZAM_UPDATE_SNAPSHOTS=1 で更新してください",
        failures.join("\n\n")
    );
}

/// デッキごとのスライド枚数が想定どおりか(構造の退行検知)
#[test]
fn example_slide_counts() {
    for (deck, expected) in [("demo.md", 6), ("seminar.md", 10), ("media.md", 2)] {
        let mut cache = HashMap::new();
        let out = mirzam_cli::pipeline::build_deck(&example(deck), &mut cache).unwrap();
        assert_eq!(out.sections.len(), expected, "{deck} のスライド枚数");
    }
}
