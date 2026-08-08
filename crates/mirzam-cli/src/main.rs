//! mirzam CLI(スパイク版)
//!
//! 使い方:
//!   mirzam build <input.md> [-o <out_dir>]

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
            match build(&input, &out_dir) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("エラー: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--version" | "-V") => {
            println!("mirzam {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => usage(""),
    }
}

fn usage(msg: &str) -> ExitCode {
    if !msg.is_empty() {
        eprintln!("エラー: {msg}\n");
    }
    eprintln!(
        "mirzam {} — Markdown ベースのスライドレンダラ(スパイク版)\n\n\
         使い方:\n  mirzam build <input.md> [-o <out_dir>]\n\n\
         出力: <out_dir>/index.html(ビューア内蔵・単一ファイル)",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::FAILURE
}

fn build(input: &Path, out_dir: &Path) -> Result<(), String> {
    let t0 = Instant::now();
    let src = std::fs::read_to_string(input)
        .map_err(|e| format!("{} を読めません: {e}", input.display()))?;
    let base_dir = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // 1. frontmatter
    let (fm, body) = mirzam_syntax::split_frontmatter(&src);
    let meta = match fm {
        Some(yaml) => mirzam_core::parse_meta(yaml).map_err(|e| e.to_string())?,
        None => mirzam_core::DeckMeta::default(),
    };

    // 2. include 展開
    let body = mirzam_syntax::expand_includes(body, &base_dir, &mirzam_syntax::FsProvider);

    // 3. 変数置換(コードフェンス内は対象外)
    let vars = meta.var_table();
    let body = substitute_outside_fences(&body, &vars);

    // 4. スライド分割 + 構造分解
    let slides: Vec<_> = mirzam_syntax::split_slides(&body)
        .iter()
        .map(|s| mirzam_syntax::parse_slide(s))
        .collect();

    // 5. レンダリング(レイアウト解決含む)
    let result = mirzam_render::render_deck(&meta, &slides, &base_dir);

    // 6. 出力
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("{} を作成できません: {e}", out_dir.display()))?;
    let out_path = out_dir.join("index.html");
    std::fs::write(&out_path, &result.html)
        .map_err(|e| format!("{} に書き込めません: {e}", out_path.display()))?;

    let elapsed = t0.elapsed();
    println!(
        "✓ {} スライドを {} に出力({} ms, {} KB)",
        slides.len(),
        out_path.display(),
        elapsed.as_millis(),
        result.html.len() / 1024,
    );
    for w in &result.warnings {
        println!("  ⚠ {w}");
    }
    Ok(())
}

/// コードフェンス外の行にのみ変数置換を適用する
fn substitute_outside_fences(
    body: &str,
    vars: &std::collections::BTreeMap<String, mirzam_core::Value>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_code = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line);
        } else {
            out.push_str(&mirzam_core::substitute_vars(line, vars));
        }
        out.push('\n');
    }
    out
}
