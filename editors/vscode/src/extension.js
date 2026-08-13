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
    // Reveal the slide matching the cursor position. Which slide that is, only
    // the core knows: a deck split across files has slides this document never
    // wrote a `---` for, so the offset goes to the webview and the WASM core
    // answers there, from the same expanded document it renders.
    vscode.window.onDidChangeTextEditorSelection((e) => {
      const entry = panels.get(e.textEditor.document.uri.toString());
      if (!entry) return;
      entry.panel.webview.postMessage({
        type: "reveal",
        offset: byteOffset(e.textEditor.document, e.selections[0].active),
      });
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

/**
 * Targets of `![[a.md]]`, `![alt](path)` and frontmatter `masters:`, excluding
 * URLs and data URIs.
 *
 * The masters file is here because the core reads it through the same
 * `FileProvider` a transclusion uses, and in this host that provider is the
 * table below. Miss it and the preview draws every slide as a single pane
 * while the CLI draws the deck correctly — the kind of disagreement between
 * the two that is worse than either being wrong.
 */
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
  const masters = mastersFile(source);
  if (masters) out.push(masters);
  return out;
}

/**
 * The path in frontmatter `masters:`, or null when the deck writes its shapes
 * inline (a mapping, so the value is empty and the drawings are indented
 * under it) or names none.
 */
function mastersFile(source) {
  const front = /^---\r?\n([\s\S]*?)\r?\n---\s*$/m.exec(source);
  if (!front || front.index !== 0) return null;
  const m = /^masters:[ \t]*(\S.*)$/m.exec(front[1]);
  if (!m) return null;
  const value = m[1].trim().replace(/^["']|["']$/g, "");
  return value && !value.startsWith("{") ? value : null;
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

/**
 * A position in the document as a byte offset into its text — what the core
 * counts in. VS Code counts UTF-16 code units, which is the same number only
 * until the deck contains a non-ASCII character.
 */
function byteOffset(document, position) {
  const upto = document.getText(
    new vscode.Range(new vscode.Position(0, 0), position)
  );
  return Buffer.byteLength(upto, "utf8");
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
