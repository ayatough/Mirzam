// Mirzam live preview extension.
//
// Rendering happens inside the webview, using the WASM build of the same core
// the CLI uses. The extension host's job is to read transcluded .md files and
// referenced assets from disk and hand them to the webview, which has no
// filesystem of its own.

const vscode = require("vscode");
const path = require("path");
const fs = require("fs");

/** Open previews, keyed by document URI. */
const panels = new Map();

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("mirzam.showPreview", () =>
      showPreview(context)
    ),
    vscode.commands.registerCommand("mirzam.exportHtml", () =>
      exportHtml(context)
    )
  );

  // Follow edits; debouncing happens here rather than in the webview.
  let timer;
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      const entry = panels.get(e.document.uri.toString());
      if (!entry) return;
      clearTimeout(timer);
      const delay = vscode.workspace
        .getConfiguration("mirzam")
        .get("previewDelay", 120);
      timer = setTimeout(() => update(entry, e.document), delay);
    }),
    // Reveal the slide matching the cursor position.
    vscode.window.onDidChangeTextEditorSelection((e) => {
      const entry = panels.get(e.textEditor.document.uri.toString());
      if (!entry) return;
      const index = slideIndexAtLine(
        e.textEditor.document.getText(),
        e.selections[0].active.line
      );
      entry.panel.webview.postMessage({ type: "reveal", index });
    })
  );
}

function showPreview(context) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "markdown") {
    vscode.window.showWarningMessage("Open a Markdown file first");
    return;
  }
  const key = editor.document.uri.toString();
  const existing = panels.get(key);
  if (existing) {
    existing.panel.reveal(vscode.ViewColumn.Beside);
    return;
  }

  const panel = vscode.window.createWebviewPanel(
    "mirzamPreview",
    `Mirzam: ${path.basename(editor.document.fileName)}`,
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [vscode.Uri.file(context.extensionPath)],
    }
  );

  const entry = { panel, ready: false, pending: null, lastAssets: "" };
  panels.set(key, entry);

  panel.webview.html = webviewHtml(panel.webview, context);
  panel.webview.onDidReceiveMessage((msg) => {
    if (msg.type === "ready") {
      entry.ready = true;
      update(entry, editor.document);
    } else if (msg.type === "error") {
      vscode.window.showErrorMessage(`Mirzam preview: ${msg.message}`);
    }
  });
  panel.onDidDispose(() => panels.delete(key));
}

function update(entry, document) {
  if (!entry.ready) return;
  const text = document.getText();
  const baseDir = path.dirname(document.uri.fsPath);
  const { files, assets } = collectResources(text, baseDir);

  // Assets are heavy, so only resend them when they actually change.
  const assetsJson = JSON.stringify(assets);
  const payload = { type: "render", text, files: JSON.stringify(files) };
  if (assetsJson !== entry.lastAssets) {
    payload.assets = assetsJson;
    entry.lastAssets = assetsJson;
  }
  entry.panel.webview.postMessage(payload);
}

async function exportHtml(context) {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const entry = panels.get(editor.document.uri.toString());
  if (!entry || !entry.ready) {
    vscode.window.showWarningMessage(
      "Open the preview first; rendering runs inside it"
    );
    return;
  }
  const target = await vscode.window.showSaveDialog({
    filters: { HTML: ["html"] },
    defaultUri: vscode.Uri.file(
      editor.document.uri.fsPath.replace(/\.md$/, ".html")
    ),
  });
  if (!target) return;
  entry.exportTo = target;
  entry.panel.webview.postMessage({ type: "export" });
  const disposable = entry.panel.webview.onDidReceiveMessage(async (msg) => {
    if (msg.type !== "exported") return;
    disposable.dispose();
    await vscode.workspace.fs.writeFile(target, Buffer.from(msg.html, "utf8"));
    vscode.window.showInformationMessage(
      `Mirzam: exported ${path.basename(target.fsPath)}`
    );
  });
}

/**
 * Collects transcluded files (`![[...]]`) and referenced assets from the source
 * into tables the webview can consume, following references recursively.
 */
function collectResources(text, baseDir) {
  const maxSize = vscode.workspace
    .getConfiguration("mirzam")
    .get("maxAssetSize", 20 * 1024 * 1024);
  const files = {};
  const assets = {};
  const seen = new Set();

  const walk = (source, dir) => {
    for (const rel of references(source)) {
      const abs = path.resolve(dir, rel);
      const key = normalize(rel, dir, baseDir);
      if (seen.has(abs)) continue;
      seen.add(abs);
      let stat;
      try {
        stat = fs.statSync(abs);
      } catch {
        continue; // Missing files fall back to the core's placeholder handling.
      }
      if (rel.endsWith(".md")) {
        const content = fs.readFileSync(abs, "utf8");
        files[key] = content;
        walk(content, path.dirname(abs));
      } else if (stat.size <= maxSize) {
        assets[key] = dataUri(abs);
      }
    }
  };
  walk(text, baseDir);
  return { files, assets };
}

/** Targets of `![[a.md]]` and `![alt](path)`, excluding URLs and data URIs. */
function references(source) {
  const out = [];
  const include = /^!\[\[([^\]]+)\]\]\s*$/gm;
  const media = /!\[[^\]]*\]\(([^()\s"]+)\)/g;
  for (const re of [include, media]) {
    let m;
    while ((m = re.exec(source))) {
      const p = m[1].trim();
      if (!p || p.startsWith("data:") || p.includes("://")) continue;
      out.push(p);
    }
  }
  return out;
}

/** Matches the core's key format: a path relative to the deck. */
function normalize(rel, dir, baseDir) {
  const abs = path.resolve(dir, rel);
  return path.relative(baseDir, abs).split(path.sep).join("/");
}

function dataUri(file) {
  const ext = path.extname(file).toLowerCase();
  const mime =
    {
      ".svg": "image/svg+xml",
      ".png": "image/png",
      ".jpg": "image/jpeg",
      ".jpeg": "image/jpeg",
      ".gif": "image/gif",
      ".webp": "image/webp",
      ".mp4": "video/mp4",
      ".webm": "video/webm",
    }[ext] || "application/octet-stream";
  return `data:${mime};base64,${fs.readFileSync(file).toString("base64")}`;
}

/** Which slide (0-based) a line belongs to; `---` inside code fences is ignored. */
function slideIndexAtLine(text, line) {
  const lines = text.split(/\r?\n/);
  let index = 0;
  let inCode = false;
  // Skip frontmatter.
  let start = 0;
  if (lines[0] && lines[0].trim() === "---") {
    for (let i = 1; i < lines.length; i++) {
      if (lines[i].trim() === "---") {
        start = i + 1;
        break;
      }
    }
  }
  for (let i = start; i < Math.min(line, lines.length); i++) {
    const t = lines[i].trim();
    if (t.startsWith("```")) inCode = !inCode;
    else if (!inCode && t.length >= 3 && /^-+$/.test(t)) index++;
  }
  return index;
}

function webviewHtml(webview, context) {
  const uri = (...p) =>
    webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, ...p)));
  const wasmJs = uri("media", "mirzam_wasm.js");
  const wasmBin = uri("media", "mirzam_wasm_bg.wasm");
  const client = uri("media", "preview.js");
  const csp = [
    `default-src 'none'`,
    `img-src ${webview.cspSource} data: blob:`,
    `media-src ${webview.cspSource} data: blob:`,
    `style-src ${webview.cspSource} 'unsafe-inline'`,
    // Required for running WASM and for the srcdoc preview iframe.
    `script-src ${webview.cspSource} 'wasm-unsafe-eval' 'unsafe-inline'`,
    `frame-src 'self' data:`,
    `connect-src ${webview.cspSource}`,
  ].join("; ");

  return `<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<style>
  html, body { margin: 0; height: 100%; background: var(--vscode-editor-background); }
  body { display: flex; flex-direction: column; font-family: var(--vscode-font-family); }
  #bar {
    padding: 4px 10px; font-size: 12px; display: flex; gap: 12px; align-items: center;
    color: var(--vscode-descriptionForeground);
    border-bottom: 1px solid var(--vscode-panel-border);
  }
  #bar b { color: var(--vscode-textLink-foreground); }
  #frame { flex: 1; border: 0; width: 100%; }
  #warn {
    max-height: 25%; overflow: auto; padding: 4px 10px; font-size: 12px;
    color: var(--vscode-errorForeground);
    border-top: 1px solid var(--vscode-panel-border);
  }
  #warn:empty { display: none; }
</style>
</head>
<body>
<div id="bar"><span id="stat">Loading WASM…</span></div>
<iframe id="frame" sandbox="allow-scripts allow-same-origin"></iframe>
<div id="warn"></div>
<script type="module" src="${client}" data-wasm-js="${wasmJs}" data-wasm-bin="${wasmBin}"></script>
</body>
</html>`;
}

function deactivate() {}

module.exports = { activate, deactivate };
