// Webview 側のプレビュークライアント。
// WASM コアでレンダリングし、変更されたスライドだけ iframe 内の DOM を差し替える。

const vscode = acquireVsCodeApi();
const script = document.querySelector("script[data-wasm-js]");
const frame = document.getElementById("frame");
const statEl = document.getElementById("stat");
const warnEl = document.getElementById("warn");

let renderer = null;
let lastText = "";
let wantIndex = 0;

async function boot() {
  try {
    // dynamic import はこのモジュール自身の URL 基準で解決されるため、
    // 相対パスを渡された場合に備えてドキュメント基準へ正規化する
    const resolve = (u) => new URL(u, document.baseURI).href;
    const mod = await import(resolve(script.dataset.wasmJs));
    await mod.default({ module_or_path: resolve(script.dataset.wasmBin) });
    renderer = new mod.Renderer();
    statEl.textContent = "準備完了";
    vscode.postMessage({ type: "ready" });
  } catch (e) {
    statEl.textContent = "WASM の読み込みに失敗しました";
    vscode.postMessage({ type: "error", message: String(e) });
  }
}

function fullRender(text) {
  const t0 = performance.now();
  const out = renderer.render_page(text);
  frame.srcdoc = out.html;
  // 差分計算の基準を、いま表示している内容に合わせる
  renderer.reset();
  renderer.render_changed(text);
  report(t0, out.slide_count, "フル", JSON.parse(out.warnings));
  frame.addEventListener("load", () => reveal(wantIndex), { once: true });
}

function update(text) {
  if (!renderer) return;
  if (!lastText) {
    lastText = text;
    fullRender(text);
    return;
  }
  lastText = text;
  const t0 = performance.now();
  const res = JSON.parse(renderer.render_changed(text));
  const doc = frame.contentDocument;
  if (res.structural || !doc || !doc.querySelector("section.slide")) {
    fullRender(text);
    return;
  }
  for (const [i, html] of res.changes) {
    const sec = doc.querySelector(`section.slide[data-index="${i}"]`);
    if (sec) sec.outerHTML = html;
  }
  if (res.changes.length && doc.defaultView.__mirzamRefresh) {
    doc.defaultView.__mirzamRefresh();
  }
  report(t0, res.count, `差分 ${res.changes.length} 枚`, res.warnings);
}

/** エディタのカーソル位置に対応するスライドを表示する */
function reveal(index) {
  wantIndex = index;
  const win = frame.contentWindow;
  if (!win || !win.__mirzamGoto) return;
  win.__mirzamGoto(index);
}

function report(t0, count, kind, warnings) {
  const ms = (performance.now() - t0).toFixed(1);
  statEl.innerHTML = `${count} スライド / ${kind}: <b>${ms} ms</b>`;
  warnEl.textContent = (warnings || []).join(" / ");
}

window.addEventListener("message", (event) => {
  const msg = event.data;
  if (msg.type === "render") {
    if (msg.files !== undefined) renderer.set_files(msg.files);
    if (msg.assets !== undefined) {
      renderer.set_assets(msg.assets);
      // アセットが変わると全スライドの出力が変わりうるため作り直す
      lastText = "";
    }
    update(msg.text);
  } else if (msg.type === "reveal") {
    reveal(msg.index);
  } else if (msg.type === "export") {
    const out = renderer.render_page(lastText);
    vscode.postMessage({ type: "exported", html: out.html });
  }
});

boot();
