//! 性能ベンチ(常設)。
//!
//! 設計目標「ページが増えても編集反映は一定」を数値で監視する。
//!   cargo run --release -p mirzam-cli --bin mirzam-bench
//!
//! 合成デッキを生成し、フルビルド時間と 1 枚編集時の再ビルド時間を測る。

use std::collections::HashMap;
use std::time::Instant;

fn main() {
    println!("Mirzam 性能ベンチ({} ビルド)\n", profile());
    for &slides in &[20usize, 120, 500] {
        bench(slides, Deck::Plain);
    }
    bench(100, Deck::Math);
    println!("\n設計目標: 1 枚編集の反映がスライド数に依存しないこと(O(1))");
}

fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

enum Deck {
    Plain,
    Math,
}

fn bench(slides: usize, kind: Deck) {
    let label = match kind {
        Deck::Plain => format!("{slides:>3} 枚(通常)"),
        Deck::Math => format!("{slides:>3} 枚(数式 8 個/枚)"),
    };
    let dir = std::env::temp_dir().join(format!("mirzam-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("一時ディレクトリ");
    let path = dir.join("bench.md");
    std::fs::write(&path, generate(slides, &kind)).expect("デッキ書き込み");

    // フルビルド
    let mut cache = HashMap::new();
    let t0 = Instant::now();
    let full = mirzam_cli::pipeline::build_deck(&path, &mut cache).expect("ビルド");
    let full_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(full.sections.len(), slides);

    // 中央のスライドを 1 箇所だけ編集して再ビルド
    let edited = generate(slides, &kind).replacen(
        &format!("## セクション {}", slides / 2),
        &format!("## セクション {} (編集)", slides / 2),
        1,
    );
    std::fs::write(&path, edited).expect("デッキ更新");
    let t1 = Instant::now();
    let inc = mirzam_cli::pipeline::build_deck(&path, &mut cache).expect("再ビルド");
    let inc_ms = t1.elapsed().as_secs_f64() * 1000.0;

    println!(
        "{label}: フルビルド {full_ms:7.1} ms | 1 枚編集 {inc_ms:6.1} ms(再レンダリング {} 枚)",
        inc.rendered
    );
    assert_eq!(inc.rendered, 1, "1 枚だけ再レンダリングされるはず");
    let _ = std::fs::remove_dir_all(&dir);
}

fn generate(slides: usize, kind: &Deck) -> String {
    let mut s = String::from("---\ntitle: ベンチ\n---\n\n");
    for i in 1..=slides {
        if i > 1 {
            s.push_str("\n---\n\n");
        }
        s.push_str(&format!("## セクション {i}\n\n"));
        s.push_str("```pane\n+----------+----------+\n|          |          |\n|  main    |  side    |\n|          |          |\n+----------+----------+\n```\n\n");
        s.push_str("::: pane main\n");
        match kind {
            Deck::Plain => {
                s.push_str(&format!(
                    "スライド {i} の本文。**強調**と `code`。\n\n- A\n- B\n- C\n"
                ));
            }
            Deck::Math => {
                for j in 1..=5 {
                    s.push_str(&format!(
                        "段落 {j}: $\\alpha_{{{i}}}^{{{j}}} + \\frac{{x}}{{y}}$\n\n"
                    ));
                }
                for j in 1..=3 {
                    s.push_str(&format!("$$\\int_0^{{{j}}} e^{{-x^2}} dx$$\n\n"));
                }
            }
        }
        s.push_str(":::\n\n::: pane side\n| 列 | 値 |\n|---|---:|\n| a | 1 |\n| b | 2 |\n:::\n");
    }
    s
}
