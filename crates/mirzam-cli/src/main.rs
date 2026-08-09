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
            let mut opts = BuildArgs::default();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" | "--out" => {
                        i += 1;
                        match args.get(i) {
                            Some(dir) => opts.out_dir = PathBuf::from(dir),
                            None => return usage("-o requires an output path"),
                        }
                    }
                    "--base-url" => {
                        i += 1;
                        match args.get(i) {
                            Some(u) => opts.base_url = Some(u.clone()),
                            None => return usage("--base-url requires a URL"),
                        }
                    }
                    "--theme" => {
                        i += 1;
                        // A flag is typed, not authored, so an unknown name is
                        // a typo to report rather than something to fall back
                        // from: silently rendering in `default` is exactly what
                        // the flag was reached for to avoid.
                        match args.get(i) {
                            Some(name) if mirzam_render::THEME_NAMES.contains(&name.as_str()) => {
                                opts.theme = Some(name.clone());
                            }
                            _ => {
                                return usage(&format!(
                                    "--theme takes one of: {}",
                                    mirzam_render::THEME_NAMES.join(", ")
                                ));
                            }
                        }
                    }
                    "--css" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => opts.css = Some(PathBuf::from(p)),
                            None => return usage("--css requires a stylesheet path"),
                        }
                    }
                    "--split" => {
                        i += 1;
                        match args.get(i).map(String::as_str) {
                            Some("h1") => opts.split = Some(1),
                            Some("h2") => opts.split = Some(2),
                            Some("h3") => opts.split = Some(3),
                            _ => return usage("--split takes h1, h2 or h3"),
                        }
                    }
                    "--debug-layout" => opts.debug_layout = true,
                    other if input.is_none() => input = Some(PathBuf::from(other)),
                    other => return usage(&format!("unknown argument: {other}")),
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("an input file is required");
            };
            run(build(&input, &opts))
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
        // Asking for help is not an error: it goes to stdout and succeeds.
        Some("--help" | "-h" | "help") => {
            println!("{}", help_text());
            ExitCode::SUCCESS
        }
        None => usage(""),
        Some(other) => usage(&unknown_command(other)),
    }
}

/// Message for an unrecognised subcommand, with a suggestion when one is close.
/// `mirzam server` instead of `mirzam serve` should not read as "your file is
/// wrong"; it should name the mistake.
fn unknown_command(given: &str) -> String {
    const COMMANDS: [&str; 3] = ["build", "serve", "export"];
    let close = COMMANDS
        .iter()
        .map(|c| (edit_distance(given, c), *c))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d);
    match close {
        Some((_, c)) => format!("unknown command `{given}` - did you mean `{c}`?"),
        None => format!("unknown command `{given}`"),
    }
}

/// Levenshtein distance, over chars so non-ASCII input cannot panic.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
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
    eprintln!("{}", help_text());
    ExitCode::FAILURE
}

fn help_text() -> String {
    // A raw string, not `\`-continued lines: the continuation strips the leading
    // whitespace, which flattened the indentation this text relies on.
    format!(
        "mirzam {} - a Markdown-based slide renderer\n{}",
        env!("CARGO_PKG_VERSION"),
        r#"
Usage:
  mirzam build <input.md> [-o <out_dir>] [--split h1|h2|h3] [--theme <name>]
               [--css <file>] [--base-url <url>] [--debug-layout]
  mirzam serve <input.md> [-p <port>]
  mirzam export pdf <input.md> [-o <out.pdf>] [--chromium <bin>]

  build   write <out_dir>/index.html, a single file with the viewer embedded
          --split starts a new slide at every heading of that level, which
          turns an ordinary document into a deck without editing it
          --theme and --css override the deck's frontmatter, so a document
          that carries none still gets an identity: --theme takes a built-in
          palette, --css a stylesheet with the type and furniture as well
          --base-url is where the input file's directory lives once published,
          so links to other documents still resolve from the deck's own path
          --debug-layout bakes on the pane outline overlay, for screenshotting
          a broken deck (toggle it live in the viewer with the L key instead)
  serve   development server with hot reload (default port 4321)
  export  render a PDF with headless Chromium (also honors MIRZAM_CHROMIUM)

Examples:
  mirzam build examples/showcase.md -o out
  mirzam serve examples/showcase.md
  mirzam build README.md --split h2 -o out"#
    )
}

/// Everything `mirzam build` was asked for. A struct rather than a row of
/// positional arguments, so adding a flag cannot silently pass it in the wrong
/// slot.
struct BuildArgs {
    out_dir: PathBuf,
    split: Option<u8>,
    debug_layout: bool,
    base_url: Option<String>,
    /// Overrides frontmatter `theme:`.
    theme: Option<String>,
    /// Overrides frontmatter `css:`. Resolved against the working directory,
    /// not the deck's, because it is a path the caller typed.
    css: Option<PathBuf>,
}

impl Default for BuildArgs {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("out"),
            split: None,
            debug_layout: false,
            base_url: None,
            theme: None,
            css: None,
        }
    }
}

fn build(input: &Path, args: &BuildArgs) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let mut out =
        pipeline::build_deck_with(input, &mut cache, args.split, args.base_url.as_deref())?;

    // `--theme` and `--css` override the frontmatter, which is what lets a
    // document carrying none - a README published as a deck - still be given
    // an identity without editing the document to get one.
    if let Some(name) = &args.theme {
        out.meta.theme = Some(name.clone());
    }
    if let Some(path) = &args.css {
        // Unreadable frontmatter `css:` is a warning, because the deck is still
        // a deck without it. An unreadable `--css` is an error: it is the whole
        // reason this invocation exists, and a wrong path would otherwise
        // publish a deck that looks nothing like the one that was asked for.
        out.custom_css = Some(
            std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        );
    }

    let opts = mirzam_render::PageOptions {
        live_version: None,
        custom_css: out.custom_css.clone(),
        debug_layout: args.debug_layout,
    };
    let html = mirzam_render::assemble_page(&out.meta, &out.sections, &opts);

    std::fs::create_dir_all(&args.out_dir)
        .map_err(|e| format!("cannot create {}: {e}", args.out_dir.display()))?;
    let out_path = args.out_dir.join("index.html");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_the_nearest_command() {
        assert!(unknown_command("server").contains("did you mean `serve`?"));
        assert!(unknown_command("buidl").contains("did you mean `build`?"));
        assert!(unknown_command("exprot").contains("did you mean `export`?"));
    }

    #[test]
    fn does_not_guess_when_nothing_is_close() {
        let msg = unknown_command("render");
        assert!(msg.contains("unknown command `render`"));
        assert!(!msg.contains("did you mean"));
    }

    #[test]
    fn edit_distance_handles_non_ascii() {
        assert_eq!(edit_distance("ビルド", "build"), 5);
        assert_eq!(edit_distance("serve", "serve"), 0);
    }
}
