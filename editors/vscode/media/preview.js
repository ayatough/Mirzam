// Preview client running inside the webview.
// Renders with the WASM core and patches only the changed slides in the iframe.

const vscode = acquireVsCodeApi();
const script = document.querySelector("script[data-wasm-js]");
const frame = document.getElementById("frame");
const statEl = document.getElementById("stat");
const warnEl = document.getElementById("warn");

let renderer = null;
let lastText = "";
/**
 * Where the editor's cursor is: a byte offset, and which of the deck's files it
 * is an offset into — the empty string for the deck's own source, a transclusion
 * key such as `sections/two.md` for one of its sections.
 */
let want = { offset: 0, file: "" };

async function boot() {
  try {
    // Dynamic import resolves against this module's own URL, so normalize
    // against the document base in case a relative path is supplied.
    const resolve = (u) => new URL(u, document.baseURI).href;
    const mod = await import(resolve(script.dataset.wasmJs));
    await mod.default({ module_or_path: resolve(script.dataset.wasmBin) });
    renderer = new mod.Renderer();
    statEl.textContent = "Ready";
    vscode.postMessage({ type: "ready" });
  } catch (e) {
    statEl.textContent = "Failed to load WASM";
    vscode.postMessage({ type: "error", message: String(e) });
  }
}

function fullRender(text) {
  const t0 = performance.now();
  const out = renderer.render_page(text);
  frame.srcdoc = out.html;
  // Align the diff baseline with what is currently displayed.
  renderer.reset();
  renderer.render_changed(text);
  report(t0, out.slide_count, "full", JSON.parse(out.warnings));
  frame.addEventListener("load", () => reveal(want), { once: true });
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
  report(t0, res.count, `${res.changes.length} changed`, res.warnings);
  // The cursor has not moved, but the slide under it may have: typing a `---`
  // above it makes the page it belongs to a different one.
  reveal(want);
}

/**
 * Shows the slide the editor's cursor sits on, given as a byte offset into one
 * of the deck's files. The core turns it into a slide number, since only it
 * knows how many slides the files before this one contribute.
 */
function reveal(at) {
  want = { offset: at.offset || 0, file: at.file || "" };
  const win = frame.contentWindow;
  if (!win || !win.__mirzamGoto || !renderer || !lastText) return;
  win.__mirzamGoto(renderer.slide_at_offset(lastText, want.offset, want.file));
}

function report(t0, count, kind, warnings) {
  const ms = (performance.now() - t0).toFixed(1);
  statEl.innerHTML = `${count} slides / ${kind}: <b>${ms} ms</b>`;
  warnEl.textContent = (warnings || []).join(" / ");
}

window.addEventListener("message", (event) => {
  const msg = event.data;
  if (msg.type === "render") {
    if (msg.files !== undefined) renderer.set_files(msg.files);
    if (msg.assets !== undefined) {
      renderer.set_assets(msg.assets);
      // Changed assets can affect every slide, so rebuild from scratch.
      lastText = "";
    }
    update(msg.text);
  } else if (msg.type === "reveal") {
    reveal(msg);
  } else if (msg.type === "export") {
    const out = renderer.render_page(lastText);
    vscode.postMessage({ type: "exported", html: out.html });
  }
});

boot();
