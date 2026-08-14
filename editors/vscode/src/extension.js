// Mirzam live preview extension.
//
// Rendering happens inside the webview, using the WASM build of the same core
// the CLI uses. The extension host's job is to read transcluded .md files and
// referenced assets from disk and hand them to the webview, which has no
// filesystem of its own.

const vscode = require("vscode");
const path = require("path");
const fs = require("fs");
const { references, isTextFile } = require("./references");

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

  // Follow edits, in the deck or in any file it is assembled from; debouncing
  // happens here rather than in the webview, per preview so two open decks do
  // not cancel each other's updates.
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      const delay = vscode.workspace
        .getConfiguration("mirzam")
        .get("previewDelay", 120);
      for (const entry of previewsOf(e.document)) {
        clearTimeout(entry.timer);
        entry.timer = setTimeout(() => update(entry), delay);
      }
    }),
    // Reveal the slide matching the cursor position. Which slide that is, only
    // the core knows: a deck split across files has slides this document never
    // wrote a `---` for, so the offset goes to the webview and the WASM core
    // answers there, from the same expanded document it renders — which is
    // also what lets a cursor in a section file name a slide of the deck.
    vscode.window.onDidChangeTextEditorSelection((e) => {
      const document = e.textEditor.document;
      for (const entry of previewsOf(document)) {
        entry.panel.webview.postMessage({
          type: "reveal",
          offset: byteOffset(document, e.selections[0].active),
          file: entry.files.get(document.uri.fsPath) || "",
        });
      }
    })
  );
}

/**
 * The previews `document` takes part in: the deck it *is*, and every deck that
 * transcludes it.
 *
 * A deck split across files is written in its sections, and the preview is
 * open on the file that transcludes them. Watching only that file left the
 * whole workflow with a preview that never moved.
 */
function* previewsOf(document) {
  const self = panels.get(document.uri.toString());
  if (self) yield self;
  for (const entry of panels.values()) {
    if (entry !== self && entry.files.has(document.uri.fsPath)) yield entry;
  }
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

  const entry = {
    panel,
    document: editor.document,
    ready: false,
    lastAssets: "",
    // Every file this deck was assembled from last time it was rendered, as
    // absolute path -> the key the core knows it by. This is what turns an
    // edit somewhere in the workspace into "that deck has to be re-rendered",
    // and a cursor in a section file into a place in the deck.
    files: new Map(),
    timer: undefined,
  };
  panels.set(key, entry);

  panel.webview.html = webviewHtml(panel.webview, context);
  panel.webview.onDidReceiveMessage((msg) => {
    if (msg.type === "ready") {
      entry.ready = true;
      update(entry);
    } else if (msg.type === "error") {
      vscode.window.showErrorMessage(`Mirzam preview: ${msg.message}`);
    }
  });
  panel.onDidDispose(() => {
    clearTimeout(entry.timer);
    panels.delete(key);
  });
}

function update(entry) {
  if (!entry.ready) return;
  const document = entry.document;
  const text = document.getText();
  const baseDir = path.dirname(document.uri.fsPath);
  const { files, assets, sources } = collectResources(text, baseDir);
  entry.files = sources;

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
  // The deck being exported is the one the preview is open on, which is not
  // necessarily the file in front of you: a section of it is the deck as much
  // as the file that transcludes it.
  const [entry] = previewsOf(editor.document);
  if (!entry || !entry.ready) {
    vscode.window.showWarningMessage(
      "Open the preview first; rendering runs inside it"
    );
    return;
  }
  const target = await vscode.window.showSaveDialog({
    filters: { HTML: ["html"] },
    defaultUri: vscode.Uri.file(
      entry.document.uri.fsPath.replace(/\.md$/, ".html")
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
 * Collects the files a deck reads and the assets it references into tables the
 * webview can consume, following references recursively.
 *
 * Which paths those are is `references()`; this decides which table each one
 * goes in. Text the core reads through `FileProvider` — a transcluded section,
 * a masters file, a bibliography, the `css:` stylesheet — goes in `files` as
 * itself. Everything else is an asset and goes in `assets` as a data URI,
 * a chart's CSV included: the core reads chart data through the same
 * `AssetSource` it reads images through.
 *
 * `sources` maps each file's absolute path to the key the core knows it by,
 * which is how an edit anywhere in the workspace finds the previews it belongs
 * to.
 */
function collectResources(text, baseDir) {
  const maxSize = vscode.workspace
    .getConfiguration("mirzam")
    .get("maxAssetSize", 20 * 1024 * 1024);
  const files = {};
  const assets = {};
  const sources = new Map();
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
        const content = readText(abs);
        files[key] = content;
        sources.set(abs, key);
        walk(content, path.dirname(abs));
      } else if (isTextFile(rel)) {
        // Text, not an asset: the core reads a bibliography and a stylesheet
        // through the same `FileProvider` as a transclusion, so they belong in
        // the file table and not in the data-URI table beside the images.
        // Nothing inside either references anything, so there is nothing to
        // walk.
        files[key] = readText(abs);
        sources.set(abs, key);
      } else if (stat.size <= maxSize) {
        assets[key] = dataUri(abs);
        sources.set(abs, key);
      }
    }
  };
  walk(text, baseDir);
  return { files, assets, sources };
}

/**
 * A file as it currently reads: the editor's buffer while it is open, unsaved
 * edits included, and the file on disk otherwise.
 *
 * A section of a deck is a file like any other, and a preview that waited for
 * it to be saved would spend the whole editing session showing the draft
 * before the one being typed.
 */
function readText(file) {
  const open = vscode.workspace.textDocuments.find(
    (d) => !d.isClosed && d.uri.scheme === "file" && d.uri.fsPath === file
  );
  return open ? open.getText() : fs.readFileSync(file, "utf8");
}

/** Matches the core's key format: a path relative to the deck. */
function normalize(rel, dir, baseDir) {
  const abs = path.resolve(dir, rel);
  return path.relative(baseDir, abs).split(path.sep).join("/");
}

/**
 * An asset as the core wants it: a base64 data URI. The types mirror
 * `mirzam-render`'s own table, because the browser plays a recording served as
 * octet-stream no more readily here than it does in an exported deck.
 */
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
      ".avif": "image/avif",
      ".mp4": "video/mp4",
      ".m4v": "video/mp4",
      ".webm": "video/webm",
      ".ogv": "video/ogg",
      ".mov": "video/quicktime",
      ".mp3": "audio/mpeg",
      ".m4a": "audio/mp4",
      ".aac": "audio/mp4",
      ".wav": "audio/wav",
      ".oga": "audio/ogg",
      ".ogg": "audio/ogg",
      ".opus": "audio/ogg",
      ".flac": "audio/flac",
      // Chart data. Not media, but it travels the same way: the core decodes
      // it back out of the data URI.
      ".csv": "text/csv",
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
