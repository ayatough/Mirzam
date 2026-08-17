//! `mirzam serve` - development server with hot reload.
//!
//! - Polls the mtimes of the input and its includes every 200ms
//! - Rebuilds through the cache on change, re-rendering only changed slides
//! - Clients long-poll `/events?v=N` for the diff and patch only the
//!   `<section>` elements that changed

use crate::pipeline::{build_deck, deck_source, BuildOutput, RenderCache};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const LONG_POLL_TIMEOUT: Duration = Duration::from_secs(25);
const HISTORY_LIMIT: usize = 64;

struct Snapshot {
    version: u64,
    meta: mirzam_core::DeckMeta,
    sections: Vec<String>,
    hashes: Vec<u64>,
    page_fingerprint: u64,
    file_themes: Vec<mirzam_render::FileTheme>,
    /// The deck's own Markdown, for the viewer's `V` panel. Always carried
    /// here, unlike `build`, where it is a flag: the person watching a live
    /// preview is the person writing the deck, and the panel costs a local
    /// page nothing.
    source: mirzam_render::DeckSource,
    /// Hash lists from previous versions, used to compute diffs.
    history: VecDeque<(u64, Vec<u64>)>,
}

struct Shared {
    snap: Mutex<Snapshot>,
    changed: Condvar,
}

pub fn serve(input: &Path, port: u16) -> Result<(), String> {
    let mut cache: RenderCache = HashMap::new();
    let first = build_deck(input, &mut cache)?;
    for w in &first.warnings {
        println!("  ⚠ {w}");
    }
    let mut watch_files = first.files.clone();
    // Built before the fields it reads are moved into the snapshot.
    let first_source = deck_source(&first, input, first.frontmatter.clone(), None);
    let shared = Arc::new(Shared {
        snap: Mutex::new(Snapshot {
            version: 1,
            meta: first.meta,
            sections: first.sections,
            hashes: first.hashes.clone(),
            page_fingerprint: first.page_fingerprint,
            file_themes: first.file_themes,
            source: first_source,
            history: VecDeque::from([(1, first.hashes)]),
        }),
        changed: Condvar::new(),
    });

    // File-watching thread.
    {
        let shared = Arc::clone(&shared);
        let input = input.to_path_buf();
        std::thread::spawn(move || {
            let mut mtimes = collect_mtimes(&watch_files);
            loop {
                std::thread::sleep(POLL_INTERVAL);
                let now = collect_mtimes(&watch_files);
                if now == mtimes {
                    continue;
                }
                mtimes = now;
                let t0 = Instant::now();
                match build_deck(&input, &mut cache) {
                    Ok(out) => {
                        watch_files = out.files.clone();
                        mtimes = collect_mtimes(&watch_files);
                        publish(&shared, out, &input, t0);
                    }
                    Err(e) => {
                        // Transient errors while editing are reported but keep the last good state.
                        eprintln!("  ✗ rebuild failed: {e}");
                    }
                }
            }
        });
    }

    let server = tiny_http::Server::http(("127.0.0.1", port))
        .map_err(|e| format!("cannot listen on port {port}: {e}"))?;
    println!("▶ serving http://localhost:{port} with hot reload (Ctrl-C to stop)");

    for request in server.incoming_requests() {
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || handle(request, shared));
    }
    Ok(())
}

fn publish(shared: &Shared, out: BuildOutput, input: &Path, t0: Instant) {
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }
    let mut snap = shared.snap.lock().unwrap();
    if snap.hashes == out.hashes && snap.page_fingerprint == out.page_fingerprint {
        // Output is unchanged (an mtime-only touch, a no-op save, ...).
        if out.rendered > 0 {
            println!(
                "↻ no output change ({} slides re-rendered, identical result)",
                out.rendered
            );
        }
        return;
    }
    snap.version += 1;
    let changed = out
        .hashes
        .iter()
        .zip(snap.hashes.iter())
        .filter(|(a, b)| a != b)
        .count()
        + out.hashes.len().abs_diff(snap.hashes.len());
    let source = deck_source(&out, input, out.frontmatter.clone(), None);
    snap.meta = out.meta;
    snap.sections = out.sections;
    snap.hashes = out.hashes.clone();
    snap.page_fingerprint = out.page_fingerprint;
    snap.file_themes = out.file_themes;
    snap.source = source;
    let v = snap.version;
    snap.history.push_back((v, out.hashes));
    while snap.history.len() > HISTORY_LIMIT {
        snap.history.pop_front();
    }
    println!(
        "↻ v{v}: {}/{} slides updated, {} re-rendered ({} ms)",
        changed,
        snap.hashes.len(),
        out.rendered,
        t0.elapsed().as_millis()
    );
    drop(snap);
    shared.changed.notify_all();
}

fn handle(request: tiny_http::Request, shared: Arc<Shared>) {
    let url = request.url().to_string();
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => (url.as_str(), ""),
    };
    let response = match path {
        "/" | "/index.html" => {
            let snap = shared.snap.lock().unwrap();
            let opts = mirzam_render::PageOptions {
                live_version: Some(snap.version),
                file_themes: snap.file_themes.clone(),
                // The preview patches single slides into a page whose <head>
                // was assembled before the edit, so it carries every palette:
                // adding `theme=` to a pane is otherwise a change the page
                // cannot show until the next full reload.
                all_themes: true,
                source: Some(snap.source.clone()),
                ..Default::default()
            };
            let html = mirzam_render::assemble_page(&snap.meta, &snap.sections, &opts);
            html_response(html)
        }
        "/events" => {
            let client_v: u64 = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("v="))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            json_response(wait_for_change(&shared, client_v))
        }
        _ => tiny_http::Response::from_string("not found").with_status_code(404),
    };
    let _ = request.respond(response);
}

/// Waits for a new version (or a timeout) and returns the diff as JSON.
fn wait_for_change(shared: &Shared, client_v: u64) -> String {
    let deadline = Instant::now() + LONG_POLL_TIMEOUT;
    let mut snap = shared.snap.lock().unwrap();
    while snap.version == client_v {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let (guard, timeout) = shared.changed.wait_timeout(snap, left).unwrap();
        snap = guard;
        if timeout.timed_out() {
            break;
        }
    }
    if snap.version == client_v {
        // Timed out with no change.
        return serde_json::json!({ "v": client_v, "changes": [], "full": false }).to_string();
    }
    // Diff against the client's version if it is still in history; otherwise reload.
    let old_hashes = snap
        .history
        .iter()
        .find(|(v, _)| *v == client_v)
        .map(|(_, h)| h.clone());
    match old_hashes {
        Some(old) if old.len() == snap.hashes.len() => {
            let changes: Vec<serde_json::Value> = snap
                .hashes
                .iter()
                .enumerate()
                .filter(|(i, h)| old.get(*i) != Some(h))
                .map(|(i, _)| serde_json::json!([i, snap.sections[i]]))
                .collect();
            if changes.is_empty() {
                // Slides are identical but the version advanced, so a page-level
                // setting changed (custom CSS, ...) and the page must reload.
                serde_json::json!({ "v": snap.version, "changes": [], "full": true }).to_string()
            } else {
                // The source panel's payload rides along: the slides the reader
                // can see are patched, and so is the text behind them.
                serde_json::json!({
                    "v": snap.version,
                    "changes": changes,
                    "full": false,
                    "source": snap.source.payload(),
                })
                .to_string()
            }
        }
        // Slide count changed, or history expired: reload the whole page.
        _ => serde_json::json!({ "v": snap.version, "changes": [], "full": true }).to_string(),
    }
}

fn collect_mtimes(files: &BTreeSet<PathBuf>) -> Vec<(PathBuf, Option<SystemTime>)> {
    files
        .iter()
        .map(|f| {
            (
                f.clone(),
                std::fs::metadata(f).and_then(|m| m.modified()).ok(),
            )
        })
        .collect()
}

fn html_response(body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
            .expect("static header"),
    )
}

fn json_response(body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
            .expect("static header"),
    )
}
