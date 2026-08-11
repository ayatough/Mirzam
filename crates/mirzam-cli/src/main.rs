//! The `mirzam` command line interface.
//!
//! Usage:
//!   mirzam new <file.md> [--empty]
//!   mirzam build <input.md> [-o <out_dir>]
//!   mirzam serve <input.md> [-p <port>]

use mirzam_cli::{pipeline, scaffold, serve};
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
                    "--debug-layout" => opts.debug_layout = true,
                    "--strict" => opts.strict = true,
                    arg => match parse_deck_flag(&args, &mut i, &mut opts.deck) {
                        Some(Ok(())) => {}
                        Some(Err(e)) => return usage(&e),
                        None if input.is_none() => input = Some(PathBuf::from(arg)),
                        None => return usage(&format!("unknown argument: {arg}")),
                    },
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("an input file is required");
            };
            run(build(&input, &opts))
        }
        Some("new") => {
            let mut path: Option<PathBuf> = None;
            let mut empty = false;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--empty" => empty = true,
                    other if path.is_none() => path = Some(PathBuf::from(other)),
                    other => return usage(&format!("unknown argument: {other}")),
                }
                i += 1;
            }
            let Some(path) = path else {
                return usage("a file to create is required");
            };
            run(new_deck(&path, empty))
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
            let mut deck = DeckArgs::default();
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
                    arg => match parse_deck_flag(&args, &mut i, &mut deck) {
                        Some(Ok(())) => {}
                        Some(Err(e)) => return usage(&e),
                        None if input.is_none() => input = Some(PathBuf::from(arg)),
                        None => return usage(&format!("unknown argument: {arg}")),
                    },
                }
                i += 1;
            }
            let Some(input) = input else {
                return usage("an input file is required");
            };
            // `out/index.html` re-parsed as Markdown "succeeds" with a
            // title-only PDF - a silent loss of the whole deck. Refusing
            // anything but `.md` here turns that into an error that says the
            // right command, instead of a PDF nobody would think to check.
            if !is_markdown_path(&input) {
                return usage(&format!(
                    "export pdf expects a Markdown source, not {} - point it at the deck itself: \
                     `mirzam export pdf deck.md --split h2 --theme <name> ...`",
                    input.display()
                ));
            }
            let out_path = out_path.unwrap_or_else(|| {
                input
                    .with_extension("pdf")
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("deck.pdf"))
            });
            run(export_pdf(&input, &out_path, chromium.as_deref(), &deck))
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
    const COMMANDS: [&str; 4] = ["new", "build", "serve", "export"];
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
  mirzam new <file.md> [--empty]
  mirzam build <input.md> [-o <out_dir>] [--split h1|h2|h3] [--theme <name>]
               [--css <file>] [--fit shrink] [--mode light|dark]
               [--base-url <url>] [--debug-layout] [--strict]
  mirzam serve <input.md> [-p <port>]
  mirzam export pdf <input.md> [-o <out.pdf>] [--split h1|h2|h3]
               [--theme <name>] [--css <file>] [--fit shrink]
               [--mode light|dark] [--chromium <bin>]

  new     write a deck to start from - frontmatter, a title slide and a
          slide break - or, with --empty, a blank file to type into.
          An existing file is never overwritten
  build   write <out_dir>/index.html, a single file with the viewer embedded
  serve   development server with hot reload (default port 4321)
  export  render a PDF with headless Chromium (also honors MIRZAM_CHROMIUM).
          Takes a Markdown source, not a built `out/index.html` - re-parsing
          already-rendered HTML as Markdown would silently lose the deck

  --split starts a new slide at every heading of that level, which turns an
          ordinary document into a deck without editing it. `build` and
          `export pdf` take it the same way, so a deck assembled with --split
          exports to PDF with the same slide breaks in one command
  --theme and --css override the deck's frontmatter, so a document that
          carries none still gets an identity: --theme takes a built-in
          palette, --css a stylesheet with the type and furniture as well
  --fit shrink scales an overfull pane's text down instead of clipping it,
          which is what a section of prose that was never written to be a
          slide usually needs
  --mode  pins the deck to light or dark. Leave it off and the deck follows
          the reader's machine - which is wrong for a stylesheet that rests
          in one mode, because every per-mode image in the deck would then
          pick its copy by a rule the stylesheet is ignoring
  --base-url is where the input file's directory lives once published, so
          links to other documents still resolve from the deck's own path
          (build only)
  --debug-layout bakes on the pane outline overlay, for screenshotting a
          broken deck (toggle it live in the viewer with the L key instead;
          build only)
  --strict exits non-zero when the build produced any warnings - a shape
          block inside a pane, a footnote with no definition on its slide, a
          connect endpoint that matches nothing - so CI can catch a silent
          degradation instead of shipping it. The deck still builds either
          way; only the exit code changes (build only)

Examples:
  mirzam new deck.md
  mirzam build examples/01-start.md -o out
  mirzam serve examples/04-components.md
  mirzam build README.md --split h2 -o out
  mirzam export pdf README.md --split h2 -o out.pdf
  mirzam build deck.md --strict"#
    )
}

/// Flags that shape the deck itself, as opposed to where it lands. `build`
/// and `export pdf` both render a deck from the same Markdown source - one to
/// HTML, one to PDF - so these must parse, and mean, exactly the same thing
/// for both. Kept in one struct so a flag added to one command cannot drift
/// out of step with the other.
#[derive(Default)]
struct DeckArgs {
    split: Option<u8>,
    /// Overrides frontmatter `theme:`.
    theme: Option<String>,
    /// Overrides frontmatter `css:`. Resolved against the working directory,
    /// not the deck's, because it is a path the caller typed.
    css: Option<PathBuf>,
    /// Overrides frontmatter `fit:`.
    fit: Option<String>,
    /// Overrides frontmatter `mode:`.
    mode: Option<String>,
}

/// Tries to consume one of `DeckArgs`' flags at `args[*i]`. On a match,
/// advances `*i` to the flag's value and returns `Some` - `Err` holding the
/// usage message for a missing or invalid value. Returns `None` when
/// `args[*i]` is none of these flags, so the caller's own flags (`-o`,
/// `--base-url`, `--chromium`, ...) still get a turn at it.
fn parse_deck_flag(
    args: &[String],
    i: &mut usize,
    opts: &mut DeckArgs,
) -> Option<Result<(), String>> {
    match args[*i].as_str() {
        "--theme" => {
            *i += 1;
            // A flag is typed, not authored, so an unknown name is a typo to
            // report rather than something to fall back from: silently
            // rendering in `default` is exactly what the flag was reached
            // for to avoid.
            match args.get(*i) {
                Some(name) if mirzam_render::THEME_NAMES.contains(&name.as_str()) => {
                    opts.theme = Some(name.clone());
                    Some(Ok(()))
                }
                _ => Some(Err(format!(
                    "--theme takes one of: {}",
                    mirzam_render::THEME_NAMES.join(", ")
                ))),
            }
        }
        "--css" => {
            *i += 1;
            match args.get(*i) {
                Some(p) => {
                    opts.css = Some(PathBuf::from(p));
                    Some(Ok(()))
                }
                None => Some(Err("--css requires a stylesheet path".to_string())),
            }
        }
        "--fit" => {
            *i += 1;
            match args.get(*i).map(String::as_str) {
                Some("shrink") => {
                    opts.fit = Some("shrink".to_string());
                    Some(Ok(()))
                }
                _ => Some(Err("--fit takes shrink".to_string())),
            }
        }
        "--mode" => {
            *i += 1;
            match args.get(*i).map(String::as_str) {
                Some(m @ ("light" | "dark")) => {
                    opts.mode = Some(m.to_string());
                    Some(Ok(()))
                }
                _ => Some(Err("--mode takes light or dark".to_string())),
            }
        }
        "--split" => {
            *i += 1;
            match args.get(*i).map(String::as_str) {
                Some("h1") => {
                    opts.split = Some(1);
                    Some(Ok(()))
                }
                Some("h2") => {
                    opts.split = Some(2);
                    Some(Ok(()))
                }
                Some("h3") => {
                    opts.split = Some(3);
                    Some(Ok(()))
                }
                _ => Some(Err("--split takes h1, h2 or h3".to_string())),
            }
        }
        _ => None,
    }
}

/// Whether `path` looks like a Markdown source - `export pdf`'s only valid
/// input. Case-insensitive, since a filesystem that accepted `Deck.MD` to
/// write it should not refuse to read it back.
fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// Everything `mirzam build` was asked for. A struct rather than a row of
/// positional arguments, so adding a flag cannot silently pass it in the wrong
/// slot.
struct BuildArgs {
    out_dir: PathBuf,
    debug_layout: bool,
    /// `--strict`: fail the build (non-zero exit) if it produced any
    /// warnings, for a CI gate that catches a silent degradation - a
    /// shape in a pane, an unresolved footnote, a connector to nowhere -
    /// before it ships. The deck still builds; only the exit code changes.
    strict: bool,
    base_url: Option<String>,
    deck: DeckArgs,
}

impl Default for BuildArgs {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("out"),
            debug_layout: false,
            strict: false,
            base_url: None,
            deck: DeckArgs::default(),
        }
    }
}

/// Writes the file, then says what to run on it: `new` is the first command
/// anyone types, so it is also where the second one is learned.
fn new_deck(path: &Path, empty: bool) -> Result<(), String> {
    scaffold::create(path, empty)?;
    println!(
        "✓ wrote {}{}",
        path.display(),
        if empty { " (empty)" } else { "" }
    );
    println!("  next: mirzam serve {}", path.display());
    Ok(())
}

/// Applies `--theme`/`--css`/`--fit`/`--mode` on top of the deck's own
/// frontmatter. Shared by `build` and `export pdf`, which render the same
/// deck to different formats and so must resolve these identically.
fn apply_deck_overrides(out: &mut pipeline::BuildOutput, deck: &DeckArgs) -> Result<(), String> {
    // `--theme` and `--css` override the frontmatter, which is what lets a
    // document carrying none - a README published as a deck - still be given
    // an identity without editing the document to get one.
    if let Some(name) = &deck.theme {
        out.meta.theme = Some(name.clone());
    }
    if let Some(fit) = &deck.fit {
        out.meta.fit = Some(fit.clone());
    }
    // `--mode` matters most for the deck that cannot say it any other way. A
    // stylesheet may rest in either mode - `examples/themes/mirzam.css` is dark
    // by default and says so - but nothing in the CSS tells the renderer which,
    // and an unset mode means "follow the reader's machine". So a dark-resting
    // deck left unset paints dark while every per-mode asset in it, a
    // `<picture>` or a `bg-light=`/`bg-dark=` pane, shows its light copy.
    if let Some(mode) = &deck.mode {
        out.meta.mode = Some(mode.clone());
    }
    if let Some(path) = &deck.css {
        // Unreadable frontmatter `css:` is a warning, because the deck is still
        // a deck without it. An unreadable `--css` is an error: it is the whole
        // reason this invocation exists, and a wrong path would otherwise
        // publish a deck that looks nothing like the one that was asked for.
        out.custom_css = Some(
            std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        );
    }
    Ok(())
}

fn build(input: &Path, args: &BuildArgs) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let mut out =
        pipeline::build_deck_with(input, &mut cache, args.deck.split, args.base_url.as_deref())?;
    apply_deck_overrides(&mut out, &args.deck)?;

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
    if args.strict && !out.warnings.is_empty() {
        return Err(format!(
            "--strict: {} warning{} reported",
            out.warnings.len(),
            if out.warnings.len() == 1 { "" } else { "s" }
        ));
    }
    Ok(())
}

fn export_pdf(
    input: &Path,
    out_path: &Path,
    chromium: Option<&str>,
    deck: &DeckArgs,
) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let mut out = pipeline::build_deck_with(input, &mut cache, deck.split, None)?;
    apply_deck_overrides(&mut out, deck)?;
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
        assert!(unknown_command("nwe").contains("did you mean `new`?"));
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

    #[test]
    fn only_dot_md_is_a_valid_export_input() {
        assert!(is_markdown_path(Path::new("deck.md")));
        assert!(is_markdown_path(Path::new("deck.MD")));
        assert!(!is_markdown_path(Path::new("out/index.html")));
        assert!(!is_markdown_path(Path::new("deck")));
    }

    #[test]
    fn parse_deck_flag_shares_split_theme_css_fit_mode() {
        let args: Vec<String> = [
            "--split", "h2", "--theme", "nord", "--css", "x.css", "--fit", "shrink", "--mode",
            "dark",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let mut opts = DeckArgs::default();
        let mut i = 0;
        while i < args.len() {
            assert!(matches!(
                parse_deck_flag(&args, &mut i, &mut opts),
                Some(Ok(()))
            ));
            i += 1;
        }
        assert_eq!(opts.split, Some(2));
        assert_eq!(opts.theme.as_deref(), Some("nord"));
        assert_eq!(opts.css.as_deref(), Some(Path::new("x.css")));
        assert_eq!(opts.fit.as_deref(), Some("shrink"));
        assert_eq!(opts.mode.as_deref(), Some("dark"));
    }

    #[test]
    fn parse_deck_flag_ignores_flags_it_does_not_own() {
        let args: Vec<String> = ["-o", "out"].into_iter().map(String::from).collect();
        let mut opts = DeckArgs::default();
        let mut i = 0;
        assert!(parse_deck_flag(&args, &mut i, &mut opts).is_none());
    }

    #[test]
    fn parse_deck_flag_reports_a_bad_value() {
        let args: Vec<String> = ["--split", "h9"].into_iter().map(String::from).collect();
        let mut opts = DeckArgs::default();
        let mut i = 0;
        let err = parse_deck_flag(&args, &mut i, &mut opts);
        assert!(matches!(err, Some(Err(e)) if e.contains("--split takes")));
    }
}
