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
use std::path::Path;
use std::time::Instant;

const CHECK_JS: &str = include_str!("check.js");

/// Generous on purpose: real wall-clock cost tracks actual page work, not
/// this number - Chromium dumps as soon as the page goes idle, well under
/// budget, for every deck measured while this was written. The ceiling only
/// matters for a deck animating far longer than any of them.
const VIRTUAL_TIME_BUDGET_MS: &str = "60000";

/// Everything `mirzam check` was asked for: the same deck-shaping flags
/// `build` takes (so a `--split` deck, or one built with a `--theme`
/// override, is checked as it would actually be published), plus `export
/// pdf`'s `--chromium`, since this also launches Chromium headless.
#[derive(Default)]
pub(crate) struct CheckArgs {
    pub(crate) deck: DeckArgs,
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
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }

    let opts = mirzam_render::PageOptions {
        live_version: None,
        custom_css: out.custom_css.clone(),
        debug_layout: args.debug_layout,
        // The page under test is the page `build` writes, palettes included.
        all_themes: false,
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
    Err(format!(
        "{} problem(s). Widen the band in the pane block, shorten the text, \
         or move the content to another pane. See docs/layout.md.",
        problems.len()
    ))
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
}
