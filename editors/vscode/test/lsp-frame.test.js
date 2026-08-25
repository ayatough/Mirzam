// The wire format, and the question of whether a document is a deck at all.
//
// Both are pure, and both are where a mistake is invisible: framing that is
// one byte out looks like a server that went quiet, and a deck test that is
// too eager puts slide diagnostics on every README in a repository.

const test = require("node:test");
const assert = require("node:assert");
const { encode, decode, looksLikeADeck } = require("../src/lsp-frame");

test("a message is framed by the length of its body in bytes, not its characters", () => {
  const framed = encode({ hello: "スライド" });
  const header = framed.subarray(0, framed.indexOf("\r\n\r\n")).toString();
  const body = framed.subarray(framed.indexOf("\r\n\r\n") + 4);
  assert.match(header, /^Content-Length: \d+$/);
  assert.strictEqual(Number(/(\d+)/.exec(header)[1]), body.length);
  assert.deepStrictEqual(JSON.parse(body.toString("utf8")), { hello: "スライド" });
});

test("a pipe hands over bytes, so half a message waits for the rest", () => {
  const framed = encode({ id: 1 });
  const first = decode(framed.subarray(0, 8));
  assert.deepStrictEqual(first.messages, []);
  assert.strictEqual(first.rest.length, 8);

  const rest = decode(Buffer.concat([first.rest, framed.subarray(8)]));
  assert.deepStrictEqual(rest.messages, [{ id: 1 }]);
  assert.strictEqual(rest.rest.length, 0);
});

test("and one read can carry three replies", () => {
  const chunk = Buffer.concat([encode({ id: 1 }), encode({ id: 2 }), encode({ id: 3 })]);
  const { messages, rest } = decode(chunk);
  assert.deepStrictEqual(
    messages.map((m) => m.id),
    [1, 2, 3]
  );
  assert.strictEqual(rest.length, 0);
});

test("a body that is not JSON is skipped, and the message after it still arrives", () => {
  const bad = Buffer.from("Content-Length: 3\r\n\r\n{{{", "utf8");
  const { messages } = decode(Buffer.concat([bad, encode({ id: 9 })]));
  assert.deepStrictEqual(messages, [{ id: 9 }]);
});

test("a header block with no length is dropped rather than read forever", () => {
  const junk = Buffer.from("Proxy-Nonsense: 1\r\n\r\n", "utf8");
  const { messages, rest } = decode(Buffer.concat([junk, encode({ id: 4 })]));
  assert.deepStrictEqual(messages, [{ id: 4 }]);
  assert.strictEqual(rest.length, 0);
});

test("a deck is frontmatter, or a block only a deck has", () => {
  assert.ok(looksLikeADeck("---\ntitle: A talk\n---\n\n# One\n"));
  assert.ok(looksLikeADeck("# Notes\n\n```pane\n+---+\n| a |\n+---+\n```\n"));
  assert.ok(looksLikeADeck("# Notes\n\n::: pane main\nText.\n:::\n"));
  assert.ok(looksLikeADeck("# Notes\n\n<!-- next -->\n"));
  assert.ok(looksLikeADeck("# Notes\n\n<!-- layout: split -->\n"));
});

test("and an ordinary README is not, however many rules it draws", () => {
  assert.ok(!looksLikeADeck("# Project\n\nInstall it, then run it.\n\n---\n\n## Licence\n"));
  assert.ok(!looksLikeADeck("Some notes\n\n```js\nconst a = 1;\n```\n"));
  // A fence that only mentions a deck block is prose about Mirzam, not a deck.
  assert.ok(!looksLikeADeck("Write a ```pane block to lay out a slide.\n"));
});
