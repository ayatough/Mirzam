// Drives `mirzam lsp` the way an editor would, and prints what it says.
//
//   node scripts/lsp-probe.mjs examples/04-components.md
//   node scripts/lsp-probe.mjs --outline examples/pitch.md
//   node scripts/lsp-probe.mjs --bin ./target/release/mirzam deck.md
//
// Why this exists: a language server has no output of its own. Started by
// hand it sits there reading a protocol nobody types, so "does it work" is
// otherwise a question you can only answer by configuring an editor — which
// is a lot of setup to discover that a path was wrong. This runs one complete
// session (initialize, open the file, collect the diagnostics, shut down) and
// prints the result as `file:line:col  kind  message`, the shape every
// compiler prints.
//
// It is also the end-to-end test: `--expect-clean` exits non-zero if the deck
// draws any diagnostic at all, which is what CI runs over the sample decks.
//
// No dependencies, deliberately: it speaks the same `Content-Length` framing
// the server does, in about thirty lines.

import { spawn } from "child_process";
import { readFileSync } from "fs";
import { resolve } from "path";
import { pathToFileURL } from "url";

const argv = process.argv.slice(2);
const flag = (name) => argv.includes(name);
const opt = (name, fallback) => {
  const i = argv.indexOf(name);
  return i === -1 ? fallback : argv[i + 1];
};
const takesValue = new Set(["--bin"]);
const files = argv.filter((a, i) => !a.startsWith("--") && !takesValue.has(argv[i - 1]));

if (files.length === 0) {
  console.error("usage: node scripts/lsp-probe.mjs [--outline] [--expect-clean] [--bin <path>] <deck.md>...");
  process.exit(2);
}

const bin = opt("--bin", null);
const outline = flag("--outline");
const expectClean = flag("--expect-clean");

// `cargo run` by default so the probe works in a fresh checkout with nothing
// built or installed; `--bin` is for the binary a release actually ships.
const [cmd, base] = bin
  ? [bin, ["lsp"]]
  : ["cargo", ["run", "-q", "--bin", "mirzam", "--", "lsp"]];

const server = spawn(cmd, base, { stdio: ["pipe", "pipe", "inherit"] });

let id = 0;
const send = (message) => {
  const body = Buffer.from(JSON.stringify({ jsonrpc: "2.0", ...message }), "utf8");
  server.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
  server.stdin.write(body);
};
const request = (method, params) => send({ id: ++id, method, params });
const notify = (method, params) => send({ method, params });

// Incoming messages, in arrival order. The framing is the same both ways: a
// header block, a blank line, then exactly that many bytes.
const received = [];
let buffered = Buffer.alloc(0);
server.stdout.on("data", (chunk) => {
  buffered = Buffer.concat([buffered, chunk]);
  for (;;) {
    const split = buffered.indexOf("\r\n\r\n");
    if (split === -1) return;
    const header = buffered.subarray(0, split).toString("ascii");
    const length = Number(/content-length:\s*(\d+)/i.exec(header)?.[1]);
    if (!Number.isFinite(length) || buffered.length < split + 4 + length) return;
    const body = buffered.subarray(split + 4, split + 4 + length).toString("utf8");
    buffered = buffered.subarray(split + 4 + length);
    received.push(JSON.parse(body));
  }
});

const waitFor = (test, what) =>
  new Promise((ok, fail) => {
    const started = Date.now();
    const poll = setInterval(() => {
      const hit = received.find(test);
      if (hit) {
        clearInterval(poll);
        ok(hit);
      } else if (Date.now() - started > 30000) {
        clearInterval(poll);
        fail(new Error(`the server never sent ${what}`));
      }
    }, 20);
  });


request("initialize", { processId: process.pid, rootUri: null, capabilities: {} });
await waitFor((m) => m.id === 1, "an answer to initialize");
notify("initialized", {});

let problems = 0;
for (const file of files) {
  const path = resolve(file);
  const uri = pathToFileURL(path).href;
  const text = readFileSync(path, "utf8");

  notify("textDocument/didOpen", {
    textDocument: { uri, languageId: "markdown", version: 1, text },
  });

  // The deck's own file always gets a message, even when it is clean, so this
  // is the one to wait for.
  await waitFor(
    (m) => m.method === "textDocument/publishDiagnostics" && m.params.uri === uri,
    `diagnostics for ${file}`
  );

  const published = received.filter((m) => m.method === "textDocument/publishDiagnostics");
  console.log(`${file}`);
  let count = 0;
  for (const { params } of published) {
    for (const d of params.diagnostics) {
      count += 1;
      const where = `${decodeURIComponent(new URL(params.uri).pathname)}:${d.range.start.line + 1}:${d.range.start.character + 1}`;
      console.log(`  ${where}  ${d.code}  ${d.message}`);
    }
  }
  if (count === 0) console.log(`  no diagnostics`);
  problems += count;

  if (outline) {
    request("textDocument/documentSymbol", { textDocument: { uri } });
    const answer = await waitFor((m) => m.id === id, "an outline");
    console.log(`  outline:`);
    for (const s of answer.result ?? []) {
      console.log(`    ${s.range.start.line + 1}: ${s.name}`);
    }
  }
  received.length = 0;
}

request("shutdown", null);
await waitFor((m) => m.id === id, "an answer to shutdown");
notify("exit", null);
server.stdin.end();

await new Promise((ok) => server.on("close", ok));
if (expectClean && problems > 0) {
  console.error(`\n✗ ${problems} diagnostic(s) where none were expected`);
  process.exit(1);
}
