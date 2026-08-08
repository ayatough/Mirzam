//! mirzam CLI(スパイク版)
//!
//! 使い方:
//!   mirzam build <input.md> [-o <out_dir>]
//!   mirzam serve <input.md> [-p <port>]

mod pipeline;
mod serve;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build") => {
            let mut input: Option<PathBuf> = None;
            let mut out_dir = PathBuf::from("out");
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" | "--out" => {
                        i += 1;
                        match args.get(i) {
                            Some(dir) => out_dir = PathBuf::from(dir),
                            None => return usage("-o には出力先を指定してください"),
                        }
                    }
                    other if input.is_none() => input = Some(PathBuf::from(other)),
                    other => return usage(&format!("不明な引数: {other}")),
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("入力ファイルを指定してください");
            };
            run(build(&input, &out_dir))
        }
        Some("serve") => {
            let mut input: Option<PathBuf> = None;
            let mut port: u16 = 4321;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-p" | "--port" => {
                        i += 1;
                        match args.get(i).and_then(|p| p.parse().ok()) {
                            Some(p) => port = p,
                            None => return usage("-p にはポート番号を指定してください"),
                        }
                    }
                    other if input.is_none() => input = Some(PathBuf::from(other)),
                    other => return usage(&format!("不明な引数: {other}")),
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("入力ファイルを指定してください");
            };
            run(serve::serve(&input, port))
        }
        Some("--version" | "-V") => {
            println!("mirzam {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => usage(""),
    }
}

fn run(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(msg: &str) -> ExitCode {
    if !msg.is_empty() {
        eprintln!("エラー: {msg}\n");
    }
    eprintln!(
        "mirzam {} — Markdown ベースのスライドレンダラ(スパイク版)\n\n\
         使い方:\n  mirzam build <input.md> [-o <out_dir>]\n  mirzam serve <input.md> [-p <port>]\n\n\
         build: <out_dir>/index.html(ビューア内蔵・単一ファイル)を出力\n\
         serve: ホットリロード付き開発サーバ(既定ポート 4321)",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::FAILURE
}

fn build(input: &Path, out_dir: &Path) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let out = pipeline::build_deck(input, &mut cache)?;
    let html = mirzam_render::assemble_page(&out.meta, &out.sections, None);

    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("{} を作成できません: {e}", out_dir.display()))?;
    let out_path = out_dir.join("index.html");
    std::fs::write(&out_path, &html)
        .map_err(|e| format!("{} に書き込めません: {e}", out_path.display()))?;

    println!(
        "✓ {} スライドを {} に出力({} ms, {} KB)",
        out.sections.len(),
        out_path.display(),
        t0.elapsed().as_millis(),
        html.len() / 1024,
    );
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }
    Ok(())
}
