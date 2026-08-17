//! `mirzam check`: the layout checker `scripts/check-layout.mjs` gives
//! contributors, reachable without `cargo`, a hand-installed
//! `playwright-core`, or the repository root - the three things that keep it
//! out of a binary install's reach (see the usability report this answers).
//! Builds the deck the same way `build` does, then drives the exact same
//! check (`check.js`, embedded at compile time so there is nothing extra to
//! install) through a one-shot headless Chromium process instead of a
//! kept-open browser tab.
//!
//! Headless Chromium has no CLI flag for "run this page, wait for an async
//! script to finish, then hand me its result" on its own - `--dump-dom`
//! alone dumps as soon as the `load` event fires. `--virtual-time-budget` is
//! what makes that possible from a one-shot process: it advances a
//! simulated clock (driving `setTimeout`/`setInterval`) up to the given
//! budget, or until the page goes idle, before the requested dump runs -
//! which is enough for the script's own waits to complete. What virtual time
//! does *not* drive is anything gated on an actually rendered frame:
//! `requestAnimationFrame` never fires under it, and a freshly started
//! `Element.animate()` never advances past `currentTime: 0`. `check.js`
//! works around both - see its own header for why - so this and
//! `scripts/check-layout.mjs` (which drives a real, interactive tab and
//! needs none of that) see the same thing. Verified by running both against
//! every example deck, and a deliberately broken one, before this landed.

use crate::{apply_deck_overrides, find_chromium, DeckArgs};
use mirzam_cli::pipeline::BuildOutput;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

const CHECK_JS: &str = include_str!("check.js");

/// The version of the `--format json` contract, documented in `docs/agents.md`.
/// A field may be added without moving it; it goes up only when a field is
/// renamed, removed, or given a different meaning - which that document
/// promises will not happen quietly.
const SCHEMA_VERSION: u32 = 1;

/// Generous on purpose: real wall-clock cost tracks actual page work, not
/// this number - Chromium dumps as soon as the page goes idle, well under
/// budget, for every deck measured while this was written. The ceiling only
/// matters for a deck animating far longer than any of them.
const VIRTUAL_TIME_BUDGET_MS: &str = "60000";

/// How the result is written. `Text` is what a person reads and is the
/// default; `Json` is the same run's findings as records, for the caller that
/// is going to *act* on them - see `docs/agents.md`.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    #[default]
    Text,
    Json,
}

/// Everything `mirzam check` was asked for: the same deck-shaping flags
/// `build` takes (so a `--split` deck, or one built with a `--theme`
/// override, is checked as it would actually be published), plus `export
/// pdf`'s `--chromium`, since this also launches Chromium headless.
#[derive(Default)]
pub(crate) struct CheckArgs {
    pub(crate) deck: DeckArgs,
    pub(crate) format: Format,
    pub(crate) base_url: Option<String>,
    pub(crate) debug_layout: bool,
    pub(crate) chromium: Option<String>,
    /// `--min-slack <px>`: how much room a pane has to have left before this
    /// call it fitted. A deck measured on one machine's fonts fits by whatever
    /// margin that machine's fonts left it; asking for a margin is how a deck
    /// that has to survive a font substitution says so.
    pub(crate) min_slack: Option<u32>,
}

struct Problem {
    slide: u64,
    kind: String,
    pane: String,
    detail: String,
}

pub(crate) fn check(input: &Path, args: &CheckArgs) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = std::collections::HashMap::new();
    let mut out = mirzam_cli::pipeline::build_deck_with(
        input,
        &mut cache,
        args.deck.split,
        args.base_url.as_deref(),
    )?;
    apply_deck_overrides(&mut out, &args.deck)?;
    // The checker is the command an agent runs after every edit, so it is where
    // a skill card that has drifted from this binary is most useful to say -
    // as an ordinary warning, in the list the caller is already reading.
    mirzam_cli::skill::note_drift(input, &mut out);
    // Under `--format json` stdout carries one JSON document and nothing else,
    // so every incidental line - this one included - waits until the end and
    // goes into the document as a record instead.
    if args.format == Format::Text {
        for w in &out.warnings {
            println!("  ⚠ {w}");
        }
    }

    let opts = mirzam_render::PageOptions {
        live_version: None,
        file_themes: out.file_themes.clone(),
        debug_layout: args.debug_layout,
        // The page under test is the page `build` writes, palettes included.
        all_themes: false,
        // The one thing it does not carry: the source panel is hidden, so it
        // occupies no space on a slide and there is nothing here to check.
        source: None,
    };
    let html = mirzam_render::assemble_page(&out.meta, &out.sections, &opts);
    let html = inject_before_closing_body(&html, &check_script(args.min_slack.unwrap_or(0)));

    // A directory, not a bare temp file: the deck is otherwise built exactly
    // like `build` writes it, and nothing here needs the file to outlive the
    // check, so it is removed again once Chromium is done with it either way.
    let dir = std::env::temp_dir().join(format!("mirzam-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let tmp = dir.join("index.html");
    let result = std::fs::write(&tmp, &html)
        .map_err(|e| format!("cannot write temporary file: {e}"))
        .and_then(|()| run_chromium(&tmp, args.chromium.as_deref()));
    let _ = std::fs::remove_dir_all(&dir);
    let (count, problems, notes) = result?;

    if args.format == Format::Json {
        println!("{}", json_report(input, &out, count, &problems, &notes));
        // The verdict is the exit code, exactly as it is for the text form:
        // the error text goes to stderr, so it cannot reach the document
        // stdout just carried.
        return verdict(&problems);
    }

    if problems.is_empty() {
        println!(
            "✓ {count} slides, no layout problems ({} ms)",
            t0.elapsed().as_millis()
        );
        // After the verdict, because they qualify it rather than replace it:
        // what the layout was measured with, and how little room the tightest
        // pane had. A clean run on a machine missing the deck's fonts is a
        // statement about that machine, and until now nothing said so.
        for n in &notes {
            println!("  · {n}");
        }
        return Ok(());
    }
    println!("✗ {} problem(s) across {count} slides", problems.len());
    for p in &problems {
        println!(
            "    slide {} [{}] pane \"{}\": {}",
            p.slide, p.kind, p.pane, p.detail
        );
    }
    for n in &notes {
        println!("  · {n}");
    }
    verdict(&problems)
}

/// The exit status, kept in one place so the two output formats cannot come to
/// different conclusions about the same run.
fn verdict(problems: &[Problem]) -> Result<(), String> {
    if problems.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} problem(s). Widen the band in the pane block, shorten the text, \
         or move the content to another pane. See docs/layout.md.",
        problems.len()
    ))
}

/// The whole run as one JSON document: the build's own warnings and the
/// in-page check's findings in a single `diagnostics` array, because a caller
/// fixing a deck does not care which pass noticed - only what is wrong and
/// where. Everything the text form prints is here, notes included.
///
/// Pretty-printed rather than packed: the reader is usually a program, but the
/// person debugging that program reads the same bytes, and nothing downstream
/// pays for the newlines.
fn json_report(
    input: &Path,
    out: &BuildOutput,
    count: u64,
    problems: &[Problem],
    notes: &[String],
) -> String {
    let mut lines = LineIndex::default();
    let mut diagnostics: Vec<serde_json::Value> = Vec::new();

    for (message, site) in out.warnings.iter().zip(&out.warning_sites) {
        let mut d = record(build_kind(message), "warning", message);
        if let Some(n) = site.slide {
            d.insert("slide".into(), n.into());
        }
        if let (Some(file), Some(offset)) = (&site.file, site.offset) {
            locate(&mut d, &mut lines, file, offset);
        }
        diagnostics.push(d.into());
    }

    for p in problems {
        // The kind travels verbatim under a namespace rather than through a
        // translation table: a table would have to be updated in step with
        // `check.js`, and the one that was not is how a new failure mode
        // arrives named `unknown`.
        let mut d = record(&format!("layout.{}", p.kind), "error", &p.detail);
        d.insert("slide".into(), p.slide.into());
        // `-` is the in-page check's way of saying "no single pane", and `?`
        // its way of saying "a pane the class names did not spell out".
        let pane = (p.pane != "-" && p.pane != "?").then_some(p.pane.as_str());
        if let Some(pane) = pane {
            d.insert("pane".into(), pane.into());
        }
        // A baked-in debug overlay is a property of the build, not of the
        // slide it is reported against, so it gets no place in the source.
        if p.kind != "debug" {
            let at = match pane {
                Some(pane) => out.pane_origin(p.slide as usize, pane),
                None => out.slide_origin(p.slide as usize),
            };
            if let Some((file, offset)) = at {
                let file = file.to_path_buf();
                locate(&mut d, &mut lines, &file, offset);
            }
        }
        diagnostics.push(d.into());
    }

    let report = serde_json::json!({
        "schema": "mirzam-check",
        "version": SCHEMA_VERSION,
        // Which binary wrote this document, as opposed to which schema it
        // follows. A caller repairing a stale skill card needs to know what to
        // repair it to, and this is additive, so the schema stays at 1.
        "mirzam": mirzam_cli::skill::VERSION,
        "deck": input.display().to_string(),
        "slides": count,
        "ok": problems.is_empty(),
        "diagnostics": diagnostics,
        "notes": notes,
    });
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

fn record(kind: &str, severity: &str, message: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut d = serde_json::Map::new();
    d.insert("kind".into(), kind.into());
    d.insert("severity".into(), severity.into());
    d.insert("message".into(), message.into());
    d
}

/// Adds `file`, and `line` when the file can still be read. A field that is
/// absent means the location is not known - never that there is none - so a
/// file whose line cannot be counted carries the file alone rather than a
/// guessed number.
fn locate(
    d: &mut serde_json::Map<String, serde_json::Value>,
    lines: &mut LineIndex,
    file: &Path,
    offset: usize,
) {
    d.insert("file".into(), file.display().to_string().into());
    if let Some(line) = lines.line_of(file, offset) {
        d.insert("line".into(), line.into());
    }
}

/// Byte offset to line number, over the files a deck was written in.
///
/// The source map answers in offsets because that is what a rewrite needs; a
/// person, and every editor they might open, counts lines. Contents are cached
/// because a deck of forty slides in one file would otherwise read it forty
/// times.
#[derive(Default)]
struct LineIndex {
    files: HashMap<PathBuf, Option<String>>,
}

impl LineIndex {
    fn line_of(&mut self, file: &Path, offset: usize) -> Option<usize> {
        let text = self
            .files
            .entry(file.to_path_buf())
            .or_insert_with(|| std::fs::read_to_string(file).ok())
            .as_deref()?;
        // Counted over bytes rather than a `&str` slice: an offset taken from a
        // file that has since been edited can land mid-character, and losing
        // the line number over that would be worse than being one line out.
        let upto = &text.as_bytes()[..offset.min(text.len())];
        Some(upto.iter().filter(|b| **b == b'\n').count() + 1)
    }
}

/// A stable name for what a build warning is about.
///
/// The messages themselves are prose written for a person and are free to be
/// reworded; this is the part a program may branch on, so it is matched on the
/// one distinctive token each family of warnings carries. Order matters - the
/// first match wins - and anything unrecognised is `build.other` rather than a
/// guess, which is also what a warning added after this table gets until it is
/// added here.
fn build_kind(message: &str) -> &'static str {
    const TABLE: &[(&str, &str)] = &[
        // First, because this is the one message that quotes another program:
        // `mmdc` says "flowchart", which contains `chart`, and it is free to
        // say anything else on this list too. Classifying it before the table
        // can misread it is cheaper than teaching every other needle about a
        // tool Mirzam does not control.
        ("mermaid:", "build.mermaid"),
        // Then, and matched on two words: the message carries a filesystem
        // path, and a deck living under `charts/` must not be classified by
        // somebody's directory name.
        ("skill card", "build.skill"),
        ("shape line ", "build.shape"),
        ("shape:", "build.shape"),
        ("grid-pad", "build.layout"),
        ("grid-gap", "build.layout"),
        ("anim ", "build.anim"),
        ("cannot split", "build.anim"),
        ("a target is split", "build.anim"),
        ("annotate ", "build.annotate"),
        ("effects line ", "build.effects"),
        ("connect ", "build.connect"),
        ("chart", "build.chart"),
        ("footnote reference", "build.footnote"),
        ("toc:", "build.toc"),
        ("bibliography", "build.bibliography"),
        ("citations:", "build.bibliography"),
        ("masters:", "build.master"),
        ("master ", "build.master"),
        ("is not in the layout", "build.layout"),
        ("pane block", "build.layout"),
        ("merged region", "build.layout"),
        ("bg-light", "build.layout"),
        ("bg-dark", "build.layout"),
        ("is still on the slide as text", "build.span"),
        ("the brace over", "build.math"),
        ("math:", "build.math"),
        ("unknown theme", "build.theme"),
        // `theme: default` is an unknown name that gets its own wording, so it
        // needs its own needle or it would classify as `build.other`.
        ("no longer a theme name", "build.theme"),
        ("unknown mode", "build.theme"),
        // The stem rule, reported against the slide or pane that named a
        // theme file which cannot answer to a name.
        ("file theme is usable", "build.theme"),
        // A stylesheet the deck named and the host could not read.
        ("theme: cannot read", "build.css"),
        // Everything else a theme file has to say about itself: a stem that
        // collides with a built-in, one palette where two are needed, text
        // that cannot be read on its own background.
        ("theme: `", "build.theme"),
        ("transition:", "build.transition"),
        ("autoplay:", "build.autoplay"),
        ("no slides:", "build.deck"),
        ("<!-- next -->", "build.continuation"),
        ("file not found", "build.asset"),
        ("not inlined", "build.asset"),
    ];
    TABLE
        .iter()
        .find(|(needle, _)| message.contains(needle))
        .map_or("build.other", |(_, kind)| *kind)
}

/// The check page: `check.js` verbatim, then an invocation that runs it,
/// encodes whatever it returns (or throws) so no HTML-escaping can corrupt
/// it, and parks the result in a `<pre>` `--dump-dom` will still show once
/// the script is done - the only channel back to the process that launched
/// Chromium headless.
fn check_script(min_slack: u32) -> String {
    let tail = format!(
        r#"
(async function () {{
  try {{
    var payload = JSON.stringify(await mzRunCheck({{ minSlack: {min_slack} }}));
  }} catch (e) {{
    var payload = JSON.stringify({{ error: String((e && e.stack) || e) }});
  }}
  var marker = document.createElement('pre');
  marker.id = 'mz-check-result';
  marker.style.display = 'none';
  marker.textContent = encodeURIComponent(payload);
  document.body.appendChild(marker);
}})();
"#
    );
    format!("<script>\n{CHECK_JS}\n{tail}</script>")
}

/// Splices `script` in just before the page's closing `</body>` - found from
/// the end, not the start, so a literal `</body>` string sitting in raw HTML
/// a deck embedded earlier in the document cannot be mistaken for the real
/// one, which the print page's own template always closes with.
fn inject_before_closing_body(html: &str, script: &str) -> String {
    match html.rfind("</body>") {
        Some(pos) => format!("{}{script}{}", &html[..pos], &html[pos..]),
        None => format!("{html}{script}"),
    }
}

type CheckResult = (u64, Vec<Problem>, Vec<String>);

fn run_chromium(html_path: &Path, chromium: Option<&str>) -> Result<CheckResult, String> {
    let bin = find_chromium(chromium)?;
    let output = std::process::Command::new(&bin)
        .args([
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            &format!("--virtual-time-budget={VIRTUAL_TIME_BUDGET_MS}"),
            "--dump-dom",
            &format!("file://{}", html_path.display()),
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {bin}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Chromium failed to check the deck ({})",
            output.status
        ));
    }
    let dom = String::from_utf8_lossy(&output.stdout);
    let payload = extract_result_payload(&dom)?;
    let decoded = percent_decode(payload);
    let value: serde_json::Value = serde_json::from_str(&decoded)
        .map_err(|e| format!("could not read Chromium's check result: {e}"))?;
    if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
        return Err(format!("the in-page checker failed: {err}"));
    }
    let count = value.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    let problems = value
        .get("problems")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|p| Problem {
            slide: p.get("slide").and_then(|v| v.as_u64()).unwrap_or(0),
            kind: p
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string(),
            pane: p
                .get("pane")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            detail: p
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();
    let notes = value
        .get("notes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|n| n.as_str().map(str::to_string))
        .collect();
    Ok((count, problems, notes))
}

/// Pulls the `encodeURIComponent`-encoded payload out of `--dump-dom`'s
/// output, found by the marker's `id` rather than a full HTML parse - the
/// payload itself cannot contain `<` or `>`, so the text up to the next tag
/// is exactly it.
fn extract_result_payload(dom: &str) -> Result<&str, String> {
    let not_found =
        || "Chromium produced no check result - the injected script may not have run".to_string();
    let marker = dom.find("id=\"mz-check-result\"").ok_or_else(not_found)?;
    let tag_end = dom[marker..]
        .find('>')
        .map(|p| marker + p + 1)
        .ok_or_else(not_found)?;
    let content_end = dom[tag_end..]
        .find('<')
        .map(|p| tag_end + p)
        .ok_or_else(not_found)?;
    Ok(&dom[tag_end..content_end])
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(hex) = bytes.get(i + 1..i + 3) {
                if let Ok(hex) = std::str::from_utf8(hex) {
                    if let Ok(byte) = u8::from_str_radix(hex, 16) {
                        out.push(byte);
                        i += 3;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_round_trips_json() {
        assert_eq!(percent_decode("%7B%22a%22%3A1%7D"), r#"{"a":1}"#);
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn injects_before_the_last_closing_body_tag() {
        let html = "<html><body><div>x</div></body></html>";
        let out = inject_before_closing_body(html, "<script>Z</script>");
        assert_eq!(
            out,
            "<html><body><div>x</div><script>Z</script></body></html>"
        );
    }

    #[test]
    fn extracts_the_marker_payload_between_its_tags() {
        let dom = r#"<body><pre id="mz-check-result" style="display: none;">%7B%7D</pre></body>"#;
        assert_eq!(extract_result_payload(dom).unwrap(), "%7B%7D");
    }

    #[test]
    fn a_missing_marker_is_a_readable_error() {
        assert!(extract_result_payload("<body></body>").is_err());
    }

    /// One real message per family, copied from a build of a deliberately
    /// broken deck. A reworded message is allowed to keep its kind; a message
    /// that lands on `build.other` is one this table has not been taught.
    #[test]
    fn every_family_of_build_warning_has_its_own_kind() {
        for (message, kind) in [
            (
                "slide 1: shape line 1: unknown shape kind `boxx`",
                "build.shape",
            ),
            ("slide 1: shape: no element with id `#nope`", "build.shape"),
            (
                "grid-gap: `wide` is not a pixel length (write `64px` or `64`)",
                "build.layout",
            ),
            (
                "slide 1: footnote reference `[^gone]` has no definition on this slide",
                "build.footnote",
            ),
            (
                "slide 1: `[span broken]{.small}` is still on the slide as text",
                "build.span",
            ),
            (
                "slide 1: connect endpoint `#nowhere` matches nothing on this slide",
                "build.connect",
            ),
            (
                "slide 2: anim target `#nothing` matches nothing on this slide",
                "build.anim",
            ),
            (
                "slide 2: annotate target `nosuchpane` matches nothing on this slide",
                "build.annotate",
            ),
            (
                "slide 2: effects line 1: unknown effect `nosucheffect`",
                "build.effects",
            ),
            ("slide 2: chart: cannot parse block: type", "build.chart"),
            (
                "slide 2: mermaid: no diagram renderer found, so the block is shown as code",
                "build.mermaid",
            ),
            // The reason `mermaid:` is matched before `chart`: an external
            // tool's own words come through in this message, and Mermaid's
            // vocabulary contains half of Mirzam's.
            (
                "slide 3: mermaid: mmdc failed (exit status: 1): Parse error on line 2 \
                 of the flowchart",
                "build.mermaid",
            ),
            ("slide 1: pane `x` is not in the layout", "build.layout"),
            ("toc: unknown key `bogus`", "build.toc"),
            ("bibliography: nothing to list", "build.bibliography"),
            ("masters: cannot read masters.md", "build.master"),
            ("unknown theme `nope`; using `mirzam`", "build.theme"),
            (
                "`default` is no longer a theme name: it was a second name for \
                 the `mirzam` palette",
                "build.theme",
            ),
            (
                "unknown mode `sideways`; expected `light` or `dark`",
                "build.theme",
            ),
            ("math: unknown dialect `maple`", "build.math"),
            (
                "transition: unknown transition `wobble`",
                "build.transition",
            ),
            ("theme: cannot read missing.css", "build.css"),
            (
                "theme: `themes/acme.css` paints in one palette: 12 colour tokens",
                "build.theme",
            ),
            (
                "slide 2, pane `fig`: `acme` is loaded from `themes/acme.css`, but that file \
                 sets its tokens outside `[data-theme=\"acme\"]`. A file theme is usable by \
                 name only if it scopes its tokens to its own stem",
                "build.theme",
            ),
            ("no slides: deck.md is empty", "build.deck"),
            ("nope.png: file not found", "build.asset"),
        ] {
            assert_eq!(build_kind(message), kind, "for: {message}");
        }
    }

    #[test]
    fn an_unfamiliar_warning_is_named_rather_than_guessed() {
        assert_eq!(
            build_kind("something nobody has classified yet"),
            "build.other"
        );
    }

    #[test]
    fn a_line_number_counts_from_one() {
        let dir = std::env::temp_dir().join(format!("mirzam-lineindex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("deck.md");
        std::fs::write(&path, "one\ntwo\nthree\n").expect("write");

        let mut index = LineIndex::default();
        assert_eq!(index.line_of(&path, 0), Some(1));
        assert_eq!(index.line_of(&path, 4), Some(2));
        assert_eq!(index.line_of(&path, 8), Some(3));
        // Past the end rather than a panic: an offset is only ever as fresh as
        // the file it was taken from.
        assert_eq!(index.line_of(&path, 9_999), Some(4));
        assert_eq!(index.line_of(&dir.join("gone.md"), 0), None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
