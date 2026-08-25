// The wire format `mirzam lsp` speaks, and nothing else.
//
// A language server is JSON-RPC over a pipe, framed by a `Content-Length`
// header and a blank line. That is the whole protocol layer, which is why this
// extension speaks it itself rather than taking `vscode-languageclient` and
// everything under it: the package has no dependencies today, the `.vsix` is
// the WASM core plus a few files, and both stay true.
//
// Kept in its own module, with no `vscode` import, so `test/lsp-frame.test.js`
// can run it under plain node — the same reason `references.js` is separate.

/** One message, framed for the server's stdin. */
function encode(message) {
  const body = Buffer.from(JSON.stringify(message), "utf8");
  return Buffer.concat([
    Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "ascii"),
    body,
  ]);
}

/**
 * Pulls whole messages out of whatever has arrived so far.
 *
 * A pipe hands over bytes, not messages: one read can carry half a header, and
 * one read can carry three replies. Returns the messages that are complete and
 * the bytes that are not yet, for the caller to prepend to the next chunk.
 *
 * A body that is not JSON is skipped rather than thrown: the framing is still
 * intact, so the next message is still readable, and one bad reply should not
 * end the session.
 */
function decode(buffered) {
  const messages = [];
  let rest = buffered;
  for (;;) {
    const split = rest.indexOf("\r\n\r\n");
    if (split === -1) break;
    const header = rest.subarray(0, split).toString("ascii");
    const length = Number(/content-length:\s*(\d+)/i.exec(header)?.[1]);
    if (!Number.isFinite(length)) {
      // A header block with no length is not a message and never becomes one;
      // dropping it is what keeps the loop from spinning on the same bytes.
      rest = rest.subarray(split + 4);
      continue;
    }
    if (rest.length < split + 4 + length) break;
    const body = rest.subarray(split + 4, split + 4 + length).toString("utf8");
    rest = rest.subarray(split + 4 + length);
    try {
      messages.push(JSON.parse(body));
    } catch {
      // Skipped, deliberately: see above.
    }
  }
  return { messages, rest };
}

/**
 * Whether this document is a deck, and so whether the server should be told
 * about it at all.
 *
 * The server analyses any Markdown as a deck — `mirzam build README.md --split
 * h2` is a supported thing to do — but an editor that put slide diagnostics on
 * every README in a repository would be answering a question nobody asked. So
 * the extension asks for them only where the document says it is a deck:
 * frontmatter, or any of the block forms only a deck has.
 */
function looksLikeADeck(text) {
  if (/^---\r?\n/.test(text)) return true;
  return /^(```(pane|chart|shape|connect|anim|each|toc|effects|bibliography|mermaid)\b|::: pane\b|<!-- next -->|<!-- (layout|theme|chrome|autoplay):)/m.test(
    text
  );
}

module.exports = { encode, decode, looksLikeADeck };
