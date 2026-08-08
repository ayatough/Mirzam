//! The `mirzam` command line interface.
//!
//! Usage:
//!   mirzam build <input.md> [-o <out_dir>]
//!   mirzam serve <input.md> [-p <port>]

use mirzam_cli::{pipeline, serve};
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
            let mut split: Option<u8> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" | "--out" => {
                        i += 1;
                        match args.get(i) {
                            Some(dir) => out_dir = PathBuf::from(dir),
                            None => return usage("-o requires an output path"),
                        }
                    }
                    "--split" => {
                        i += 1;
                        match args.get(i).map(String::as_str) {
                            Some("h1") => split = Some(1),
                            Some("h2") => split = Some(2),
                            Some("h3") => split = Some(3),
                            _ => return usage("--split takes h1, h2 or h3"),
                        }
                    }
                    other if input.is_none() => input = Some(PathBuf::from(other)),
                    other => return usage(&format!("unknown argument: {other}")),
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("an input file is required");
            };
            run(build(&input, &out_dir, split))
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
                            None => return usage("-p requires a port number"),
                        }
                    }
                    other if input.is_none() => input = Some(PathBuf::from(other)),
                    other => return usage(&format!("unknown argument: {other}")),
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("an input file is required");
            };
            run(serve::serve(&input, port))
        }
        Some("export") => {
            if args.get(1).map(String::as_str) != Some("pdf") {
                return usage("pdf is currently the only supported export format");
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
                            None => return usage("-o requires an output path"),
                        }
                    }
                    "--chromium" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => chromium = Some(p.clone()),
                            None => return usage("--chromium requires an executable path"),
                        }
                    }
                    other if input.is_none() => input = Some(PathBuf::from(other)),
                    other => return usage(&format!("unknown argument: {other}")),
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("an input file is required");
            };
            let out_path = out_path.unwrap_or_else(|| {
                input
                    .with_extension("pdf")
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("deck.pdf"))
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
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(msg: &str) -> ExitCode {
    if !msg.is_empty() {
        eprintln!("error: {msg}\n");
    }
    eprintln!(
        "mirzam {} - a Markdown-based slide renderer\n\n\
         Usage:\n  mirzam build <input.md> [-o <out_dir>] [--split h1|h2|h3]\n  mirzam serve <input.md> [-p <port>]\n  mirzam export pdf <input.md> [-o <out.pdf>] [--chromium <bin>]\n\n\
         build : write <out_dir>/index.html, a single file with the viewer embedded\n\
                 --split starts a new slide at every heading of that level, which\n\
                 turns an ordinary document into a deck without editing it\n\
         serve : development server with hot reload (default port 4321)\n\
         export: render a PDF with headless Chromium (also honors MIRZAM_CHROMIUM)",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::FAILURE
}

fn build(input: &Path, out_dir: &Path, split: Option<u8>) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let out = pipeline::build_deck_with(input, &mut cache, split)?;
    let opts = mirzam_render::PageOptions {
        live_version: None,
        custom_css: out.custom_css.clone(),
    };
    let html = mirzam_render::assemble_page(&out.meta, &out.sections, &opts);

    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let out_path = out_dir.join("index.html");
    std::fs::write(&out_path, &html)
        .map_err(|e| format!("cannot write {}: {e}", out_path.display()))?;

    println!(
        "✓ wrote {} slides to {} ({} ms, {} KB)",
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
    std::fs::write(&tmp, &html).map_err(|e| format!("cannot write temporary file: {e}"))?;

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
        .map_err(|e| format!("cannot run {bin}: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    if !status.success() {
        return Err(format!("Chromium failed to produce the PDF ({status})"));
    }
    let size = std::fs::metadata(&out_abs).map(|m| m.len()).unwrap_or(0);
    println!(
        "✓ wrote {} slides to {} ({} ms, {} KB)",
        out.sections.len(),
        out_path.display(),
        t0.elapsed().as_millis(),
        size / 1024,
    );
    Ok(())
}

/// Locates Chromium: explicit flag, then $MIRZAM_CHROMIUM, then well-known names.
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
    Err("Chromium not found; pass --chromium or set MIRZAM_CHROMIUM".into())
}
