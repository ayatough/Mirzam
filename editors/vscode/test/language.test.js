// The client, driven against the real `mirzam lsp`.
//
// Everything here could be read and believed instead: the wiring is short, and
// each step is obviously right. But the failure mode of a language client is
// not a crash — it is an editor that quietly shows nothing, which reads
// exactly like a deck with no problems in it. So this starts the actual
// server, hands it a broken deck through the same code path VS Code uses, and
// checks that the marks come back on the right words.
//
// `vscode` is not a package; it is an object the editor injects at runtime. So
// the module loader is taught to answer with a stub, which is also the only
// way to run any of this outside an editor.
//
// Skipped when there is no binary to run: `MIRZAM_BIN`, or a `target/` build.

const test = require("node:test");
const assert = require("node:assert");
const Module = require("node:module");
const path = require("node:path");
const fs = require("node:fs");

const REPO = path.resolve(__dirname, "../../..");

function binary() {
  const named = process.env.MIRZAM_BIN;
  if (named && fs.existsSync(named)) return named;
  for (const profile of ["release", "debug"]) {
    const built = path.join(REPO, "target", profile, "mirzam");
    if (fs.existsSync(built)) return built;
  }
  return null;
}

/** Just enough of the editor for the client to talk to. */
function stubEditor(serverPath) {
  const handlers = {};
  const on = (name) => (fn) => {
    handlers[name] = fn;
    return { dispose() {} };
  };
  const collection = new Map();

  const Position = class {
    constructor(line, character) {
      this.line = line;
      this.character = character;
    }
  };
  const Range = class {
    constructor(start, end) {
      this.start = start;
      this.end = end;
    }
  };

  const vscode = {
    Position,
    Range,
    Uri: { parse: (s) => ({ toString: () => s, scheme: "file", fsPath: s }) },
    DiagnosticSeverity: { Warning: 1 },
    SymbolKind: { String: 14 },
    Diagnostic: class {
      constructor(range, message, severity) {
        Object.assign(this, { range, message, severity });
      }
    },
    CompletionItem: class {
      constructor(label) {
        this.label = label;
      }
    },
    MarkdownString: class {
      constructor(value) {
        this.value = value;
      }
    },
    Hover: class {
      constructor(contents) {
        this.contents = contents;
      }
    },
    Location: class {
      constructor(uri, range) {
        Object.assign(this, { uri, range });
      }
    },
    DocumentSymbol: class {
      constructor(name, detail, kind, range, selectionRange) {
        Object.assign(this, { name, detail, kind, range, selectionRange });
      }
    },
    window: {
      createOutputChannel: () => ({
        lines: [],
        append(s) {
          this.lines.push(s);
        },
        appendLine(s) {
          this.lines.push(s);
        },
        dispose() {},
      }),
    },
    languages: {
      createDiagnosticCollection: () => ({
        set: (uri, found) => collection.set(uri.toString(), found),
        delete: (uri) => collection.delete(uri.toString()),
        dispose() {},
      }),
      registerCompletionItemProvider: (_s, provider) => {
        handlers.completion = provider;
        return { dispose() {} };
      },
      registerHoverProvider: (_s, provider) => {
        handlers.hover = provider;
        return { dispose() {} };
      },
      registerDefinitionProvider: () => ({ dispose() {} }),
      registerDocumentSymbolProvider: (_s, provider) => {
        handlers.symbols = provider;
        return { dispose() {} };
      },
    },
    workspace: {
      textDocuments: [],
      getConfiguration: () => ({
        get: (key, fallback) => (key === "serverPath" ? serverPath : fallback),
      }),
      onDidOpenTextDocument: on("open"),
      onDidChangeTextDocument: on("change"),
      onDidCloseTextDocument: on("close"),
    },
  };
  return { vscode, handlers, collection };
}

function document(file, text) {
  return {
    languageId: "markdown",
    version: 1,
    uri: { toString: () => `file://${file}`, scheme: "file", fsPath: file },
    getText: () => text,
  };
}

const settle = (ms) => new Promise((ok) => setTimeout(ok, ms));

const BROKEN = `---
title: A deck with mistakes
theme: nosuchtheme
---

\`\`\`pane
+--------+
|        |
| text   |
|        |
+--------+
\`\`\`

::: pane figure
This pane is not in the grid.
:::
`;

test("the client starts the server and puts its marks where the mistakes are", async (t) => {
  const bin = binary();
  if (!bin) {
    // On a laptop with nothing built, skipping is right. In CI the job builds
    // the binary first, so a skip there is a check that did not happen.
    assert.ok(!process.env.CI, "CI ran the client test with no mirzam binary built");
    return t.skip("no mirzam binary built; run `cargo build` first");
  }

  const { vscode, handlers, collection } = stubEditor(bin);
  const load = Module._load;
  Module._load = (request, ...rest) =>
    request === "vscode" ? vscode : load(request, ...rest);

  let session;
  try {
    // Required *after* the loader is taught, since the module takes `vscode`
    // at the top like every extension does.
    delete require.cache[require.resolve("../src/language")];
    const language = require("../src/language");
    const context = { subscriptions: [] };
    session = language.activate(context);

    const deck = document("/tmp/mirzam-client-test.md", BROKEN);
    vscode.workspace.textDocuments.push(deck);

    // The server is spawned and handshaken asynchronously; the client syncs
    // every open document once that finishes.
    for (let waited = 0; waited < 60 && collection.size === 0; waited++) {
      await settle(100);
    }

    const found = collection.get("file:///tmp/mirzam-client-test.md");
    assert.ok(found, "no diagnostics arrived for the open deck");
    const marks = found.map((d) => `${d.range.start.line}:${d.range.start.character} ${d.message}`);
    assert.strictEqual(marks.length, 2, marks.join("\n"));

    // `nosuchtheme` is on line 2 (zero-based), after `theme: `.
    assert.match(marks[0], /^2:7 unknown theme `nosuchtheme`/);
    // `figure` is on the `::: pane figure` line, after `::: pane `.
    assert.match(marks[1], /^13:9 slide 1: pane `figure` is not in the layout/);

    // And an ordinary README gets nothing at all, which is the other half of
    // being useful in an editor.
    const readme = document("/tmp/mirzam-client-readme.md", "# Project\n\nProse.\n");
    await handlers.open(readme);
    await settle(300);
    assert.strictEqual(collection.get("file:///tmp/mirzam-client-readme.md"), undefined);
  } finally {
    Module._load = load;
    session?.stop();
    delete require.cache[require.resolve("../src/language")];
  }
});
