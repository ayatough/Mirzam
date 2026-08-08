//! `mirzam serve` — ホットリロード付き開発サーバ。
//!
//! - ソースファイル(入力 + include)の mtime を 200ms 間隔で監視
//! - 変更があればキャッシュ付き再ビルド(変更スライドのみ再レンダリング)
//! - クライアントは `/events?v=N` のロングポーリングで差分を受け取り、
//!   変更された `<section>` だけ DOM を差し替える

use crate::pipeline::{build_deck, BuildOutput, RenderCache};
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
    custom_css: Option<String>,
    /// 過去バージョンのハッシュ列(差分計算用)
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
    let shared = Arc::new(Shared {
        snap: Mutex::new(Snapshot {
            version: 1,
            meta: first.meta,
            sections: first.sections,
            hashes: first.hashes.clone(),
            page_fingerprint: first.page_fingerprint,
            custom_css: first.custom_css,
            history: VecDeque::from([(1, first.hashes)]),
        }),
        changed: Condvar::new(),
    });

    // ファイル監視スレッド
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
                        publish(&shared, out, t0);
                    }
                    Err(e) => {
                        // 編集途中の一時的なエラーは表示のみ。直前の状態を保持する
                        eprintln!("  ✗ 再ビルド失敗: {e}");
                    }
                }
            }
        });
    }

    let server = tiny_http::Server::http(("127.0.0.1", port))
        .map_err(|e| format!("ポート {port} で待ち受けできません: {e}"))?;
    println!("▶ http://localhost:{port} で配信中(ホットリロード有効、Ctrl-C で終了)");

    for request in server.incoming_requests() {
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || handle(request, shared));
    }
    Ok(())
}

fn publish(shared: &Shared, out: BuildOutput, t0: Instant) {
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }
    let mut snap = shared.snap.lock().unwrap();
    if snap.hashes == out.hashes && snap.page_fingerprint == out.page_fingerprint {
        // 出力に変化なし(mtime のみの更新、保存のみ等)
        if out.rendered > 0 {
            println!(
                "↻ 出力変化なし({} 枚を再レンダリングしたが同一内容)",
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
    snap.meta = out.meta;
    snap.sections = out.sections;
    snap.hashes = out.hashes.clone();
    snap.page_fingerprint = out.page_fingerprint;
    snap.custom_css = out.custom_css;
    let v = snap.version;
    snap.history.push_back((v, out.hashes));
    while snap.history.len() > HISTORY_LIMIT {
        snap.history.pop_front();
    }
    println!(
        "↻ v{v}: {}/{} 枚を更新、再レンダリング {} 枚({} ms)",
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
                custom_css: snap.custom_css.clone(),
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

/// バージョンが進むまで(またはタイムアウトまで)待ち、差分 JSON を返す
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
        // タイムアウト: 変化なし
        return serde_json::json!({ "v": client_v, "changes": [], "full": false }).to_string();
    }
    // クライアントのバージョンのハッシュ列が履歴にあれば差分、なければ全リロード
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
                // スライドは同一だがバージョンが進んだ = ページレベルの変更
                //(カスタム CSS 等)なので全体リロード
                serde_json::json!({ "v": snap.version, "changes": [], "full": true }).to_string()
            } else {
                serde_json::json!({ "v": snap.version, "changes": changes, "full": false })
                    .to_string()
            }
        }
        // スライド枚数が変わった、または履歴切れ → ページ全体をリロード
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
