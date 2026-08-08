//! Standing performance benchmark.
//!
//! Tracks the design goal that edit latency stays flat as decks grow.
//!   cargo run --release -p mirzam-cli --bin mirzam-bench
//!
//! Generates synthetic decks and measures full builds and single-slide edits.

use std::collections::HashMap;
use std::time::Instant;

fn main() {
    println!("Mirzam performance benchmark ({} build)\n", profile());
    for &slides in &[20usize, 120, 500] {
        bench(slides, Deck::Plain);
    }
    bench(100, Deck::Math);
    println!("\nGoal: single-slide edit latency should not grow with deck size");
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
        Deck::Plain => format!("{slides:>3} slides (plain)"),
        Deck::Math => format!("{slides:>3} slides (8 formulas each)"),
    };
    let dir = std::env::temp_dir().join(format!("mirzam-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("bench.md");
    std::fs::write(&path, generate(slides, &kind)).expect("write deck");

    // Full build.
    let mut cache = HashMap::new();
    let t0 = Instant::now();
    let full = mirzam_cli::pipeline::build_deck(&path, &mut cache).expect("build");
    let full_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(full.sections.len(), slides);

    // Edit one slide in the middle and rebuild.
    let edited = generate(slides, &kind).replacen(
        &format!("## Section {}", slides / 2),
        &format!("## Section {} (edited)", slides / 2),
        1,
    );
    std::fs::write(&path, edited).expect("update deck");
    let t1 = Instant::now();
    let inc = mirzam_cli::pipeline::build_deck(&path, &mut cache).expect("rebuild");
    let inc_ms = t1.elapsed().as_secs_f64() * 1000.0;

    println!(
        "{label}: full {full_ms:7.1} ms | single edit {inc_ms:6.1} ms ({} slide re-rendered)",
        inc.rendered
    );
    assert_eq!(inc.rendered, 1, "exactly one slide should re-render");
    let _ = std::fs::remove_dir_all(&dir);
}

fn generate(slides: usize, kind: &Deck) -> String {
    let mut s = String::from("---\ntitle: Benchmark\n---\n\n");
    for i in 1..=slides {
        if i > 1 {
            s.push_str("\n---\n\n");
        }
        s.push_str(&format!("## Section {i}\n\n"));
        s.push_str("```pane\n+----------+----------+\n|          |          |\n|  main    |  side    |\n|          |          |\n+----------+----------+\n```\n\n");
        s.push_str("::: pane main\n");
        match kind {
            Deck::Plain => {
                s.push_str(&format!(
                    "Body text for slide {i} with **emphasis** and `code`.\n\n- A\n- B\n- C\n"
                ));
            }
            Deck::Math => {
                for j in 1..=5 {
                    s.push_str(&format!(
                        "Paragraph {j}: $\\alpha_{{{i}}}^{{{j}}} + \\frac{{x}}{{y}}$\n\n"
                    ));
                }
                for j in 1..=3 {
                    s.push_str(&format!("$$\\int_0^{{{j}}} e^{{-x^2}} dx$$\n\n"));
                }
            }
        }
        s.push_str(
            ":::\n\n::: pane side\n| Key | Value |\n|---|---:|\n| a | 1 |\n| b | 2 |\n:::\n",
        );
    }
    s
}
