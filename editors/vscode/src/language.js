// Starts `mirzam lsp` and turns what it says into what VS Code shows.
//
// The preview and this are independent on purpose. The preview runs the WASM
// core inside the webview and needs nothing installed; this needs the `mirzam`
// binary, because the server is a subcommand of it. So a machine without the
// binary keeps every feature it had before and simply gets no diagnostics —
// never an error dialog about a tool the author may have chosen not to install.
//
// The protocol layer is `lsp-frame.js`; everything here is the translation
// between the server's JSON and the editor's objects.

const vscode = require("vscode");
const { spawn } = require("child_process");
const { encode, decode, looksLikeADeck } = require("./lsp-frame");

/** How long to wait for a reply before giving the editor an empty answer. */
const REPLY_TIMEOUT_MS = 5000;

class Session {
  constructor(binary, log) {
    this.binary = binary;
    this.log = log;
    this.child = null;
    this.buffered = Buffer.alloc(0);
    this.pending = new Map();
    this.nextId = 0;
    this.onDiagnostics = () => {};
  }

  start() {
    // `stderr` is inherited into the output channel rather than dropped: when
    // the server refuses a stream, its one line of explanation is the only
    // thing that tells the author why the marks stopped.
    this.child = spawn(this.binary, ["lsp"], { stdio: ["pipe", "pipe", "pipe"] });
    this.child.on("error", (e) => {
      this.log.appendLine(`mirzam lsp could not start (${e.message}).`);
      this.stop();
    });
    this.child.on("exit", (code) => {
      if (this.child) this.log.appendLine(`mirzam lsp exited (${code}).`);
      this.child = null;
      for (const { fail } of this.pending.values()) fail(new Error("the server exited"));
      this.pending.clear();
    });
    this.child.stderr.on("data", (d) => this.log.append(String(d)));
    this.child.stdout.on("data", (chunk) => {
      const { messages, rest } = decode(Buffer.concat([this.buffered, chunk]));
      this.buffered = rest;
      for (const message of messages) this.receive(message);
    });

    return this.request("initialize", {
      processId: process.pid,
      rootUri: null,
      capabilities: {},
    }).then((result) => {
      this.notify("initialized", {});
      return result;
    });
  }

  stop() {
    const child = this.child;
    this.child = null;
    if (!child) return;
    // Ask, then insist: `shutdown` and `exit` are how the server is meant to
    // end, and killing it is what happens if it does not.
    try {
      child.stdin.write(encode({ jsonrpc: "2.0", id: ++this.nextId, method: "shutdown" }));
      child.stdin.write(encode({ jsonrpc: "2.0", method: "exit" }));
      child.stdin.end();
    } catch {
      // The pipe is already gone, which is the state this wanted anyway.
    }
    setTimeout(() => child.kill(), 500);
  }

  get running() {
    return this.child !== null;
  }

  receive(message) {
    if (message.method === "textDocument/publishDiagnostics") {
      this.onDiagnostics(message.params);
      return;
    }
    const waiting = this.pending.get(message.id);
    if (!waiting) return;
    this.pending.delete(message.id);
    waiting.ok(message.error ? null : message.result);
  }

  notify(method, params) {
    if (!this.child) return;
    try {
      this.child.stdin.write(encode({ jsonrpc: "2.0", method, params }));
    } catch (e) {
      this.log.appendLine(`could not write to mirzam lsp: ${e.message}`);
    }
  }

  request(method, params) {
    if (!this.child) return Promise.resolve(null);
    const id = ++this.nextId;
    return new Promise((ok, fail) => {
      this.pending.set(id, { ok, fail });
      // A reply that never comes must not leave the editor's completion list
      // spinning: after the timeout the request answers empty and the feature
      // simply does not fire this once.
      setTimeout(() => {
        if (this.pending.delete(id)) ok(null);
      }, REPLY_TIMEOUT_MS);
      try {
        this.child.stdin.write(encode({ jsonrpc: "2.0", id, method, params }));
      } catch (e) {
        this.pending.delete(id);
        fail(e);
      }
    });
  }
}

/** The binary to run: the setting if there is one, otherwise `PATH`. */
function serverBinary() {
  const configured = vscode.workspace.getConfiguration("mirzam").get("serverPath", "");
  return configured.trim() || "mirzam";
}

const position = (p) => new vscode.Position(p.line, p.character);
const range = (r) => new vscode.Range(position(r.start), position(r.end));

function diagnostic(d) {
  // Everything a build reports is a warning: a deck with a problem still
  // renders. Painting them red would say otherwise.
  const found = new vscode.Diagnostic(range(d.range), d.message, vscode.DiagnosticSeverity.Warning);
  found.source = d.source || "mirzam";
  if (d.code) found.code = d.code;
  return found;
}

function activate(context) {
  const settings = vscode.workspace.getConfiguration("mirzam");
  if (!settings.get("languageServer", true)) return null;

  const log = vscode.window.createOutputChannel("Mirzam");
  const diagnostics = vscode.languages.createDiagnosticCollection("mirzam");
  const session = new Session(serverBinary(), log);
  const open = new Set();

  session.onDiagnostics = ({ uri, diagnostics: found }) => {
    diagnostics.set(vscode.Uri.parse(uri), (found || []).map(diagnostic));
  };

  const isDeck = (document) =>
    document.languageId === "markdown" &&
    document.uri.scheme === "file" &&
    looksLikeADeck(document.getText());

  const sync = (document) => {
    if (!session.running) return;
    const uri = document.uri.toString();
    if (!isDeck(document)) {
      // A document that stopped looking like a deck — the frontmatter was
      // deleted — has its marks taken away rather than frozen where they were.
      if (open.delete(uri)) {
        session.notify("textDocument/didClose", { textDocument: { uri } });
        diagnostics.delete(document.uri);
      }
      return;
    }
    if (open.has(uri)) {
      session.notify("textDocument/didChange", {
        textDocument: { uri, version: document.version },
        contentChanges: [{ text: document.getText() }],
      });
    } else {
      open.add(uri);
      session.notify("textDocument/didOpen", {
        textDocument: {
          uri,
          languageId: "markdown",
          version: document.version,
          text: document.getText(),
        },
      });
    }
  };

  // One analysis per settled keystroke rather than per character. The server
  // answers a 500-slide deck in about four milliseconds, so this is politeness
  // to the pipe rather than a performance measure.
  const timers = new Map();
  const syncSoon = (document) => {
    const key = document.uri.toString();
    clearTimeout(timers.get(key));
    timers.set(key, setTimeout(() => sync(document), 150));
  };

  const ask = async (method, document, position, extra = {}) => {
    if (!session.running || !isDeck(document)) return null;
    sync(document);
    return session.request(method, {
      textDocument: { uri: document.uri.toString() },
      position: { line: position.line, character: position.character },
      ...extra,
    });
  };

  const selector = { language: "markdown", scheme: "file" };
  context.subscriptions.push(
    log,
    diagnostics,
    { dispose: () => session.stop() },
    vscode.workspace.onDidOpenTextDocument(sync),
    vscode.workspace.onDidChangeTextDocument((e) => syncSoon(e.document)),
    vscode.workspace.onDidCloseTextDocument((document) => {
      const uri = document.uri.toString();
      if (!open.delete(uri)) return;
      session.notify("textDocument/didClose", { textDocument: { uri } });
      diagnostics.delete(document.uri);
    }),

    vscode.languages.registerCompletionItemProvider(
      selector,
      {
        async provideCompletionItems(document, where) {
          const items = await ask("textDocument/completion", document, where);
          return (items || []).map((i) => {
            const item = new vscode.CompletionItem(i.label);
            if (typeof i.kind === "number") item.kind = i.kind - 1; // LSP is 1-based
            if (i.detail) item.detail = i.detail;
            return item;
          });
        },
      },
      // The characters that begin a name the server knows: a pane after
      // `::: pane `, a key after `[@`, a value after `theme: `.
      " ",
      "@",
      "#",
      ":"
    ),
    vscode.languages.registerHoverProvider(selector, {
      async provideHover(document, where) {
        const hover = await ask("textDocument/hover", document, where);
        const value = hover?.contents?.value;
        if (!value) return null;
        return new vscode.Hover(new vscode.MarkdownString(value));
      },
    }),
    vscode.languages.registerDefinitionProvider(selector, {
      async provideDefinition(document, where) {
        const found = await ask("textDocument/definition", document, where);
        if (!found) return null;
        const one = Array.isArray(found) ? found[0] : found;
        if (!one) return null;
        return new vscode.Location(vscode.Uri.parse(one.uri), range(one.range));
      },
    }),
    vscode.languages.registerDocumentSymbolProvider(selector, {
      async provideDocumentSymbols(document) {
        if (!session.running || !isDeck(document)) return null;
        sync(document);
        const symbols = await session.request("textDocument/documentSymbol", {
          textDocument: { uri: document.uri.toString() },
        });
        return (symbols || []).map(
          (s) =>
            new vscode.DocumentSymbol(
              s.name,
              "",
              vscode.SymbolKind.String,
              range(s.range),
              range(s.selectionRange || s.range)
            )
        );
      },
    })
  );

  session
    .start()
    .then(() => {
      log.appendLine(`mirzam lsp started (${session.binary}).`);
      for (const document of vscode.workspace.textDocuments) sync(document);
    })
    .catch((e) => {
      // Not an error dialog: the binary is optional, and the preview — which
      // is what this extension is for — has not stopped working.
      log.appendLine(
        `mirzam lsp is not running (${e.message}). Diagnostics are off; ` +
          `install the CLI, or set mirzam.serverPath, to turn them on.`
      );
    });

  return session;
}

module.exports = { activate };
