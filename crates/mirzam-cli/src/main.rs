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
        Some("export") => {
            if args.get(1).map(String::as_str) != Some("pdf") {
                return usage("現在サポートするエクスポート形式は pdf のみです");
            }
            let mut input: Option<PathBuf> = None;
            let mut out_path: Option<PathBuf> = None;
            let mut chromium: Option<String> = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" | "--out" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => out_path = Some(PathBuf::from(p)),
                            None => return usage("-o には出力先を指定してください"),
                        }
                    }
                    "--chromium" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => chromium = Some(p.clone()),
                            None => return usage("--chromium には実行ファイルを指定してください"),
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
            let out_path = out_path.unwrap_or_else(|| {
                input.with_extension("pdf").file_name().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("deck.pdf"))
            });
            run(export_pdf(&input, &out_path, chromium.as_deref()))
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
         使い方:\n  mirzam build <input.md> [-o <out_dir>]\n  mirzam serve <input.md> [-p <port>]\n  mirzam export pdf <input.md> [-o <out.pdf>] [--chromium <bin>]\n\n\
         build : <out_dir>/index.html(ビューア内蔵・単一ファイル)を出力\n\
         serve : ホットリロード付き開発サーバ(既定ポート 4321)\n\
         export: ヘッドレス Chromium で PDF を生成(MIRZAM_CHROMIUM でも指定可)",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::FAILURE
}

fn build(input: &Path, out_dir: &Path) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let out = pipeline::build_deck(input, &mut cache)?;
    let opts = mirzam_render::PageOptions {
        live_version: None,
        custom_css: out.custom_css.clone(),
    };
    let html = mirzam_render::assemble_page(&out.meta, &out.sections, &opts);

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

fn export_pdf(input: &Path, out_path: &Path, chromium: Option<&str>) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let out = pipeline::build_deck(input, &mut cache)?;
    let html =
        mirzam_render::assemble_print_page(&out.meta, &out.sections, out.custom_css.as_deref());
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }

    let tmp = std::env::temp_dir().join(format!("mirzam-print-{}.html", std::process::id()));
    std::fs::write(&tmp, &html).map_err(|e| format!("一時ファイルを書けません: {e}"))?;

    let bin = find_chromium(chromium)?;
    let out_abs = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(out_path);
    let status = std::process::Command::new(&bin)
        .args([
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            "--no-pdf-header-footer",
            &format!("--print-to-pdf={}", out_abs.display()),
            &format!("file://{}", tmp.display()),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("{bin} を起動できません: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    if !status.success() {
        return Err(format!("Chromium の PDF 生成が失敗しました({status})"));
    }
    let size = std::fs::metadata(&out_abs).map(|m| m.len()).unwrap_or(0);
    println!(
        "✓ {} スライドを {} に出力({} ms, {} KB)",
        out.sections.len(),
        out_path.display(),
        t0.elapsed().as_millis(),
        size / 1024,
    );
    Ok(())
}

/// Chromium 実行ファイルを探す: 明示指定 → $MIRZAM_CHROMIUM → 既知の名前
fn find_chromium(explicit: Option<&str>) -> Result<String, String> {
    if let Some(c) = explicit {
        return Ok(c.to_string());
    }
    if let Ok(c) = std::env::var("MIRZAM_CHROMIUM") {
        return Ok(c);
    }
    for cand in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        let found = std::process::Command::new(cand)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if found {
            return Ok(cand.to_string());
        }
    }
    Err("Chromium が見つかりません。--chromium か環境変数 MIRZAM_CHROMIUM で指定してください".into())
}
