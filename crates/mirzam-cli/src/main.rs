//! The `mirzam` command line interface.
//!
//! Usage:
//!   mirzam new <file.md> [--empty]
//!   mirzam build <input.md> [-o <out_dir>]
//!   mirzam serve <input.md> [-p <port>]

mod check;

use mirzam_cli::{pipeline, scaffold, serve, skill};
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
                    "--embed-source" => opts.embed_source = true,
                    "--editor-url" => {
                        i += 1;
                        match args.get(i) {
                            Some(u) => opts.editor_url = Some(u.clone()),
                            None => return usage("--editor-url requires a URL"),
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
        Some("check") => {
            let mut input: Option<PathBuf> = None;
            let mut opts = check::CheckArgs::default();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--base-url" => {
                        i += 1;
                        match args.get(i) {
                            Some(u) => opts.base_url = Some(u.clone()),
                            None => return usage("--base-url requires a URL"),
                        }
                    }
                    "--debug-layout" => opts.debug_layout = true,
                    "--chromium" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => opts.chromium = Some(p.clone()),
                            None => return usage("--chromium requires an executable path"),
                        }
                    }
                    "--min-slack" => {
                        i += 1;
                        match args.get(i).and_then(|v| v.parse().ok()) {
                            Some(px) => opts.min_slack = Some(px),
                            None => return usage("--min-slack requires a number of pixels"),
                        }
                    }
                    "--format" => {
                        i += 1;
                        match args.get(i).map(String::as_str) {
                            Some("text") => opts.format = check::Format::Text,
                            Some("json") => opts.format = check::Format::Json,
                            _ => return usage("--format takes text or json"),
                        }
                    }
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
            run(check::check(&input, &opts))
        }
        Some("skill") => {
            if args.get(1).map(String::as_str) != Some("install") {
                return usage("install is currently the only skill subcommand");
            }
            let mut opts = SkillArgs::default();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--user" => opts.user = true,
                    "--force" => opts.force = true,
                    "--zip" => {
                        // The path is optional, so the next argument is only
                        // consumed when it looks like one rather than like the
                        // next flag.
                        let path = args.get(i + 1).filter(|a| !a.starts_with('-'));
                        opts.zip = Some(match path {
                            Some(p) => {
                                i += 1;
                                PathBuf::from(p)
                            }
                            None => PathBuf::from(skill::ZIP_NAME),
                        });
                    }
                    other => return usage(&format!("unknown argument: {other}")),
                }
                i += 1;
            }
            run(install_skill(&opts))
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
    const COMMANDS: [&str; 6] = ["new", "build", "serve", "export", "check", "skill"];
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
  mirzam build <input.md> [-o <out_dir>] [--split h1|h2|h3]
               [--theme <name|file.css>]... [--fit shrink] [--mode light|dark]
               [--base-url <url>] [--embed-source] [--editor-url <url>]
               [--debug-layout] [--strict]
  mirzam serve <input.md> [-p <port>]
  mirzam export pdf <input.md> [-o <out.pdf>] [--split h1|h2|h3]
               [--theme <name|file.css>]... [--fit shrink]
               [--mode light|dark] [--chromium <bin>]
  mirzam check <input.md> [--split h1|h2|h3] [--theme <name|file.css>]...
               [--fit shrink] [--mode light|dark] [--base-url <url>]
               [--debug-layout] [--chromium <bin>] [--min-slack <px>]
               [--format text|json]
  mirzam skill install [--user] [--zip [<path>]] [--force]

  new     write a deck to start from - frontmatter, a title slide and a
          slide break - or, with --empty, a blank file to type into.
          An existing file is never overwritten
  build   write <out_dir>/index.html, a single file with the viewer embedded
  serve   development server with hot reload (default port 4321)
  export  render a PDF with headless Chromium (also honors MIRZAM_CHROMIUM).
          Takes a Markdown source, not a built `out/index.html` - re-parsing
          already-rendered HTML as Markdown would silently lose the deck
  check   build the deck, then render it with headless Chromium (also honors
          MIRZAM_CHROMIUM) and report every slide with content clipped by its
          pane, panes overflowing into a neighbour, a nested list sized wrong,
          an unresolved connector, an unplayed animation, or the layout debug
          overlay baked in. Exits non-zero on any of them, so CI - or a
          binary install with no cargo, no playwright-core, no repository -
          can catch what a build's own warnings cannot: a slide that renders,
          just wrong. It also says what it measured with: the fonts this
          machine actually had, and how little room the tightest pane had
          left, because a deck embeds no text font and a clean run is
          therefore a statement about one machine

  skill   install the Claude Code skill for writing decks - SKILL.md and the
          syntax card, both embedded in this binary, into
          .claude/skills/mirzam/ in this repository (or ~/.claude/skills/ with
          --user). The card is therefore always the one this binary
          implements, and the copy it writes is stamped with this version, so
          `build` and `check` can report a card that has drifted. An edited
          skill is never overwritten without --force. --zip writes the smaller
          skill instead, as the archive claude.ai, the desktop app and phones
          upload: no binary runs there, so it writes Mirzam markdown and hands
          the .md back for the browser editor to render

  --split starts a new slide at every heading of that level, which turns an
          ordinary document into a deck without editing it. `build` and
          `export pdf` take it the same way, so a deck assembled with --split
          exports to PDF with the same slide breaks in one command
  --theme overrides the deck's frontmatter, so a document that carries none
          still gets an identity. It takes a built-in palette or a path
          ending in .css, and repeating it is a cascade:
          `--theme mirzam --theme house.css`. A file named here re-themes
          the deck; naming it in the deck's own `theme:` is what also
          registers its stem for a slide's or a pane's `theme=`.
          (--css is the old spelling of --theme <file.css>. It still works
          for this release and says what to write instead.)
  --fit shrink scales an overfull pane's text down instead of clipping it,
          which is what a section of prose that was never written to be a
          slide usually needs
  --mode  pins the deck to light or dark. Leave it off and the deck follows
          the reader's machine - which is wrong for a stylesheet that rests
          in one mode, because every per-mode image in the deck would then
          pick its copy by a rule the stylesheet is ignoring
  --base-url is where the input file's directory lives once published, so
          links to other documents still resolve from the deck's own path
          (build and check)
  --embed-source carries the deck's own Markdown inside the deck: the viewer's
          V key then shows the text this slide was written as, beside the
          slide rather than over it, and a phone gets the same panel from the
          </> control. A published deck otherwise shows what the markup does
          and never what it says (build only)
  --editor-url is where the browser editor lives, absolute or relative to the
          deck. It puts a link in that panel that hands the whole deck over
          for editing, opened at the slide you were on - with the files it
          reads by name: the stylesheets `theme:` points at, the bibliography,
          the masters. It rides in the URL's fragment, so nothing is uploaded
          anywhere. Implies --embed-source (build only)
  --debug-layout bakes on the pane outline overlay, for screenshotting a
          broken deck (toggle it live in the viewer with the L key instead).
          `check` reports it baked on, since it is meant for screenshotting a
          broken deck, not for publishing (build and check)
  --min-slack reports any pane with less than that many pixels of room left
          under its content, even though it fits here. A deck is measured in
          whatever fonts the checking machine has; asking for a margin is how
          a deck that will be shown elsewhere says it needs one (check only)
  --format json writes the same run as one JSON document on stdout instead of
          prose - every build warning and every in-page finding as a record
          carrying a stable kind, a severity, the slide and pane, and the
          source file and line it came from, through transclusion. The exit
          code is unchanged, and errors still go to stderr, so the document is
          safe to pipe. The schema is versioned in docs/agents.md (check only)
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
  mirzam build deck.md --strict
  mirzam check deck.md --split h2
  mirzam check deck.md --format json
  mirzam skill install
  mirzam skill install --zip"#
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
    /// Overrides frontmatter `theme:`, in cascade order: a built-in name, or a
    /// path ending in `.css`. Repeating `--theme` appends, so
    /// `--theme mirzam --theme house.css` is the frontmatter list on the
    /// command line. A path is resolved against the working directory, not the
    /// deck's, because it is a path the caller typed.
    theme: Vec<String>,
    /// Whether any of it arrived as the retired `--css`, which is `--theme`
    /// with a path. Kept only so the deck can be told what to write instead;
    /// it goes when the alias does.
    css_alias: bool,
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
            // rendering in the fallback theme is exactly what the flag was
            // reached for to avoid. A path is not checked here — whether the
            // file reads is a question for the filesystem, and the answer is
            // an error there.
            match args.get(*i) {
                Some(entry)
                    if mirzam_core::is_theme_path(entry)
                        || mirzam_render::THEME_NAMES.contains(&entry.as_str()) =>
                {
                    opts.theme.push(entry.clone());
                    Some(Ok(()))
                }
                _ => Some(Err(format!(
                    "--theme takes a stylesheet path ending in .css, or one of: {}",
                    mirzam_render::THEME_NAMES.join(", ")
                ))),
            }
        }
        // The retired half of the same flag, accepted for one release: `--css
        // x.css` is `--theme x.css`, and says so when it is used.
        "--css" => {
            *i += 1;
            match args.get(*i) {
                Some(p) => {
                    opts.theme.push(p.clone());
                    opts.css_alias = true;
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
    /// `--embed-source`: carry each slide's Markdown inside the deck, so the
    /// viewer's `V` key can show the text the slide was written as. Off by
    /// default: it is the documentation site's need, not every deck's, and a
    /// deck that carries no source is a smaller file.
    embed_source: bool,
    /// `--editor-url`: where the browser editor lives, which turns the source
    /// panel into a way out — one click hands the slide over for editing.
    /// Implies `--embed-source`, since a link with nothing to hand over is
    /// a link to an empty editor.
    editor_url: Option<String>,
    deck: DeckArgs,
}

impl Default for BuildArgs {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("out"),
            debug_layout: false,
            strict: false,
            base_url: None,
            embed_source: false,
            editor_url: None,
            deck: DeckArgs::default(),
        }
    }
}

/// What `mirzam skill install` was asked for.
#[derive(Default)]
struct SkillArgs {
    /// `--user`: `~/.claude/skills/` instead of this repository's.
    user: bool,
    /// `--zip [path]`: write the sandbox skill as an archive instead of
    /// installing a folder. A different skill and a different destination, so
    /// it does not combine with `--user`.
    zip: Option<PathBuf>,
    force: bool,
}

/// Writes the skill, then says where it went and what to do with it. The
/// command is run once, usually by somebody who has just heard it exists, so
/// the output has to answer "and now what?" without a second page of docs.
fn install_skill(args: &SkillArgs) -> Result<(), String> {
    if let Some(path) = &args.zip {
        if args.user {
            return Err(
                "--zip writes an archive to upload, and --user installs a folder on this \
                 machine; run them separately"
                    .into(),
            );
        }
        skill::write_zip(path)?;
        println!("✓ wrote {} (mirzam {})", path.display(), skill::VERSION);
        println!("  the skill for claude.ai, the desktop app and phones, where no binary runs:");
        println!("  it writes Mirzam markdown from the bundled syntax card and hands back a .md");
        println!("  next: Settings -> Capabilities -> Skills -> upload this file");
        return Ok(());
    }

    let dest = if args.user {
        skill::Destination::User
    } else {
        skill::Destination::Project
    };
    let out = skill::install(dest, args.force)?;
    println!(
        "✓ wrote {} (mirzam {})",
        shorten(&out.skill).display(),
        skill::VERSION
    );
    println!(
        "  and {}, the syntax card it reads",
        shorten(&out.card).display()
    );
    println!(
        "  next: open Claude Code in {} and ask it for a deck - the skill loads itself",
        if args.user {
            "any directory".to_string()
        } else {
            "this directory".to_string()
        }
    );
    Ok(())
}

/// A path as short as it can be said from here: the absolute path a skill
/// install resolves to is mostly somebody's home directory, and the part that
/// matters is the tail.
fn shorten(path: &Path) -> &Path {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .unwrap_or(path)
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

/// Applies `--theme`/`--fit`/`--mode` on top of the deck's own frontmatter.
/// Shared by `build` and `export pdf`, which render the same deck to different
/// formats and so must resolve these identically.
fn apply_deck_overrides(out: &mut pipeline::BuildOutput, deck: &DeckArgs) -> Result<(), String> {
    // `--theme` overrides the frontmatter, which is what lets a document
    // carrying none - a README published as a deck - still be given an
    // identity without editing the document to get one. It replaces the list
    // rather than adding to it: a flag that half-overrode would leave the
    // caller unable to say "not that one".
    if !deck.theme.is_empty() {
        out.meta.theme = mirzam_core::ThemeSpec::Many(deck.theme.clone());
        out.meta.css = None;
        // An unreadable frontmatter path is a warning, because the deck is
        // still a deck without it. An unreadable one here is an error: it is
        // the whole reason this invocation exists, and a wrong path would
        // otherwise publish a deck that looks nothing like the one asked for.
        out.file_themes = Vec::new();
        for entry in deck.theme.iter().filter(|e| mirzam_core::is_theme_path(e)) {
            let css = std::fs::read_to_string(entry)
                .map_err(|e| format!("--theme: cannot read {entry}: {e}"))?;
            out.file_themes
                .push(mirzam_render::FileTheme::new(entry, css));
        }
        for warning in mirzam_render::file_theme_warnings(&out.file_themes) {
            out.warnings.push(warning);
            out.warning_sites.push(pipeline::WarningSite::default());
        }
    }
    if deck.css_alias {
        // No slide and no file: this is a property of the command line, not of
        // a line anybody wrote in the deck.
        out.warnings.push(
            "`--css` is retired and goes away in the next release: `--theme` takes a \
             stylesheet path as well as a built-in name. Write `--theme <file.css>` instead."
                .to_string(),
        );
        out.warning_sites.push(pipeline::WarningSite::default());
    }
    if let Some(fit) = &deck.fit {
        out.meta.fit = Some(fit.clone());
    }
    // `--mode` matters most for the deck that cannot say it any other way. A
    // stylesheet may rest in either mode - a theme of your own may define one
    // palette and mean it - but nothing in the CSS tells the renderer which,
    // and an unset mode means "follow the reader's machine". So a dark-resting
    // deck left unset paints dark while every per-mode asset in it, a
    // `<picture>` or a `bg-light=`/`bg-dark=` pane, shows its light copy.
    if let Some(mode) = &deck.mode {
        out.meta.mode = Some(mode.clone());
    }
    Ok(())
}

/// The text files a handed-over slide needs and cannot get anywhere else.
///
/// Only the bibliography today. The stylesheet is already inlined in the page
/// and the viewer reads it back from there; images and chart data go through
/// the asset table as data URIs, which would put a megabyte in a URL, so a
/// handed-over slide that uses one arrives with the reference intact and the
/// file missing — reported by the editor the way any missing asset is.
///
/// A file that cannot be read is left out rather than failing the build: the
/// deck itself has already reported it, and a second copy of the same warning
/// helps nobody.
fn embedded_files(
    input: &Path,
    meta: &mirzam_core::DeckMeta,
    file_themes: &[mirzam_render::FileTheme],
) -> Vec<(String, String)> {
    let base = input.parent().unwrap_or(Path::new("."));
    // The stylesheets have already been read, once, by whoever resolved
    // `theme:` — the pipeline, or `--theme` on top of it. Reading them again
    // here could disagree with the deck this page is.
    let mut out: Vec<(String, String)> = file_themes
        .iter()
        .map(|t| (t.path.clone(), t.css.clone()))
        .collect();
    // The other two keys naming a file the core reads through a provider. A
    // slide drawn on a master and handed over without one is not the slide:
    // it has no grid at all, and renders as a single pane.
    for rel in [meta.bibliography_file(), meta.masters_file()]
        .into_iter()
        .flatten()
    {
        if let Ok(text) = std::fs::read_to_string(base.join(rel)) {
            out.push((rel.to_string(), text));
        }
    }
    out
}

/// The frontmatter a handed-over slide carries, saying what the deck was
/// actually built in rather than only what its own text says.
///
/// A deck given `--theme`, `--mode`, `--fit` or `--split` on the command line
/// is a deck its own text does not describe — that is the point of the flags,
/// and it is how the README is published as a deck at all. Handing over the
/// text alone would send it to the editor dressed as the document rather than
/// as the deck, so the effective values replace whatever the file said about
/// those keys.
/// A deck that took no overrides hands over its frontmatter untouched.
fn handover_frontmatter(out: &pipeline::BuildOutput, deck: &DeckArgs) -> Option<String> {
    let mut overrides: Vec<(&str, String)> = Vec::new();
    if !deck.theme.is_empty() {
        let entries = out.meta.theme.entries();
        if !entries.is_empty() {
            // Quoted, because an entry is a built-in name or a path, and a
            // path may contain anything a filesystem allows.
            let list: Vec<String> = entries.iter().map(|e| format!("{e:?}")).collect();
            overrides.push(("theme", format!("[{}]", list.join(", "))));
        }
    }
    if deck.mode.is_some() {
        if let Some(mode) = &out.meta.mode {
            overrides.push(("mode", mode.clone()));
        }
    }
    if deck.fit.is_some() {
        if let Some(fit) = &out.meta.fit {
            overrides.push(("fit", fit.clone()));
        }
    }
    // `--split` decides what a slide *is*, so a deck handed over without it
    // arrives as one long slide - which is what the README was before this
    // flag, and the deck exists to show the difference.
    if let Some(level) = deck.split {
        overrides.push(("split", format!("h{level}")));
    }
    if overrides.is_empty() {
        return out.frontmatter.clone();
    }

    // Every line of the authored frontmatter except the ones being replaced
    // and whatever was indented under them: `theme:` is a scalar or a list,
    // and the list form owns the lines below it. `css:` goes too — it is the
    // retired spelling of the key `theme:` replaces.
    let replaced = |line: &str| {
        overrides
            .iter()
            .any(|(key, _)| line.starts_with(&format!("{key}:")))
            || line.starts_with("css:")
    };
    let mut lines: Vec<String> = Vec::new();
    let mut dropping = false;
    for line in out.frontmatter.as_deref().unwrap_or("").lines() {
        let indented = line.starts_with(' ') || line.starts_with('\t') || line.starts_with('-');
        if dropping && indented {
            continue;
        }
        dropping = !indented && replaced(line);
        if !dropping {
            lines.push(line.to_string());
        }
    }
    lines.extend(overrides.into_iter().map(|(k, v)| format!("{k}: {v}")));
    let text = lines.join("\n").trim_matches('\n').to_string();
    (!text.is_empty()).then_some(text)
}

fn build(input: &Path, args: &BuildArgs) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let mut out =
        pipeline::build_deck_with(input, &mut cache, args.deck.split, args.base_url.as_deref())?;
    apply_deck_overrides(&mut out, &args.deck)?;
    // An installed skill card older or newer than this binary describes markup
    // this binary does not have. It is reported here, with the deck's own
    // warnings, because the agent that would repair it is already reading them.
    skill::note_drift(input, &mut out);

    let opts = mirzam_render::PageOptions {
        live_version: None,
        file_themes: out.file_themes.clone(),
        debug_layout: args.debug_layout,
        // A built deck is assembled in one pass, so it carries the palettes it
        // actually uses and no more.
        all_themes: false,
        source: (args.embed_source || args.editor_url.is_some()).then(|| {
            // The document the renderer read, frontmatter included: one text,
            // which the panel and the handover both take slices of.
            let head = match handover_frontmatter(&out, &args.deck) {
                Some(fm) => format!("---\n{fm}\n---\n"),
                None => String::new(),
            };
            mirzam_render::DeckSource {
                starts: out.slides.iter().map(|s| head.len() + s.start).collect(),
                doc: head + &out.body,
                section_slides: out.section_slides.clone(),
                files: embedded_files(input, &out.meta, &out.file_themes),
                editor_url: args.editor_url.clone(),
            }
        }),
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
    let html = mirzam_render::assemble_print_page(&out.meta, &out.sections, &out.file_themes);
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
        assert!(unknown_command("chek").contains("did you mean `check`?"));
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
        // One list, in the order the flags were typed: `--css` is `--theme`
        // with a path, for the release the old spelling is still accepted.
        assert_eq!(opts.theme, ["nord", "x.css"]);
        assert!(opts.css_alias);
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
