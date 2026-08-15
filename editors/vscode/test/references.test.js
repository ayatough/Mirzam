// What the preview host has to read off disk, checked against the forms a deck
// can write and against the sample decks themselves.
//
//   node --test editors/vscode/test
//
// This exists because the list drifted: `bg-light=`, a chart's `data:` file and
// the `css:` stylesheet were all forms the core resolves and the host never
// collected, so `examples/pitch.md` previewed with two missing photographs, an
// empty chart and a connector pointing at a mark that was never drawn — while
// the same deck built cleanly from the CLI.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const { references } = require("../src/references");

const EXAMPLES = path.join(__dirname, "..", "..", "..", "examples");
const read = (deck) => fs.readFileSync(path.join(EXAMPLES, deck), "utf8");

test("a pane's background photograph is a file the host must read", () => {
  const found = references(
    "::: pane hero {.bleed bg-light=media/dawn.jpg bg-dark=media/night.jpg}\n# H\n:::\n"
  );
  assert.deepStrictEqual(found, ["media/dawn.jpg", "media/night.jpg"]);
  assert.deepStrictEqual(references("::: pane a {bg=media/city.jpg dim=0.4}\n"), [
    "media/city.jpg",
  ]);
});

test("a background named last in the attribute list keeps its path", () => {
  // The closing brace is not part of the value.
  assert.deepStrictEqual(references("::: pane a {dim=0.4 bg=media/city.jpg}\n"), [
    "media/city.jpg",
  ]);
});

test("a video's poster frame is read, from an attribute or from raw HTML", () => {
  assert.deepStrictEqual(
    references("![Demo](media/demo.webm){.loop poster=media/first.png}\n").sort(),
    ["media/demo.webm", "media/first.png"]
  );
  assert.deepStrictEqual(
    references('<video src="media/demo.webm" poster="media/first.png"></video>\n').sort(),
    ["media/demo.webm", "media/first.png"]
  );
});

test("markup a deck quotes rather than writes is not a reference", () => {
  // `examples/04-components.md` shows the syntax beside a pane that uses it,
  // and the illustration names files nobody ever added.
  const shown =
    "```markdown\n" +
    "::: pane hero {bg-light=art/dawn.webp bg-dark=art/night.webp}\n" +
    "![Logo](img/nope.svg)\n" +
    "```\n" +
    "In the PDF it becomes its `poster=` frame.\n" +
    "\n" +
    "::: pane hero {bg=media/bg/real.jpg}\n";
  assert.deepStrictEqual(references(shown), ["media/bg/real.jpg"]);
});

test("a <picture> picking artwork by colour scheme reads both sources", () => {
  const found = references(
    '<picture>\n' +
      '  <source media="(prefers-color-scheme: dark)" srcset="img/dark.svg">\n' +
      '  <img src="img/light.svg" alt="Mirzam" width="340">\n' +
      "</picture>\n"
  );
  assert.deepStrictEqual(found.sort(), ["img/dark.svg", "img/light.svg"]);
});

test("a srcset keeps its URLs and drops its descriptors", () => {
  assert.deepStrictEqual(references('<img srcset="img/a.png 1x, img/b.png 2x">\n'), [
    "img/a.png",
    "img/b.png",
  ]);
});

test("a chart's data file is read, and its inline data is not mistaken for one", () => {
  const named = "```chart\ntype: area\ndata: data/adoption.csv\n```\n";
  assert.deepStrictEqual(references(named), ["data/adoption.csv"]);

  const inline = "```chart\ntype: bar\ndata: |\n  quarter, 2024\n  Q1, 10\n```\n";
  assert.deepStrictEqual(references(inline), []);
});

test("the four frontmatter files are read; an inline mapping is not a path", () => {
  const front =
    "---\ntitle: T\ntheme: themes/deck.css\nmasters: masters.md\n" +
    "bibliography: refs.bib\n---\n\n![[sections/a.md]]\n";
  assert.deepStrictEqual(references(front).sort(), [
    "masters.md",
    "refs.bib",
    "sections/a.md",
    "themes/deck.css",
  ]);

  const mapping = "---\nvars:\n  seats: 8\ncss:\n---\n\n# H\n";
  assert.deepStrictEqual(references(mapping), []);
});

// `theme:` is the one frontmatter key holding a *list* of files, and only its
// `.css` entries are files at all. A theme the host fails to collect is a deck
// that previews unstyled, and a built-in name handed over as a path is a
// missing-file warning for a deck with nothing wrong with it.
test("theme: collects its .css entries, in every form, and only those", () => {
  const scalar = "---\ntheme: themes/acme.css\n---\n\n# H\n";
  assert.deepStrictEqual(references(scalar), ["themes/acme.css"]);

  const builtin = "---\ntheme: nord\n---\n\n# H\n";
  assert.deepStrictEqual(references(builtin), []);

  const inline = "---\ntheme: [mirzam, themes/acme.css, tweaks.css]\n---\n\n# H\n";
  assert.deepStrictEqual(references(inline).sort(), ["themes/acme.css", "tweaks.css"]);

  const block = "---\ntheme:\n  - mirzam\n  - themes/acme.css\nmode: dark\n---\n\n# H\n";
  assert.deepStrictEqual(references(block), ["themes/acme.css"]);

  // The retired spelling still names the same file, for one release.
  const alias = "---\ncss: themes/acme.css\n---\n\n# H\n";
  assert.deepStrictEqual(references(alias), ["themes/acme.css"]);

  // Quoted, and with a built-in named after it.
  const quoted = '---\ntheme: ["themes/acme.css", wuwei]\n---\n\n# H\n';
  assert.deepStrictEqual(references(quoted), ["themes/acme.css"]);
});

test("what the core resolves for itself is left alone", () => {
  const source =
    "![remote](https://example.com/a.png)\n" +
    "![inline](data:image/svg+xml;base64,QUJD)\n" +
    '<img src="#gradient">\n' +
    "::: pane a {bg=https://example.com/b.jpg}\n";
  assert.deepStrictEqual(references(source), []);
});

test("every file examples/pitch.md names is one the host would read", () => {
  // The deck in the bug report. Each of these was a warning in the preview and
  // silence from the CLI.
  const found = references(read("pitch.md"));
  for (const want of [
    "../docs/brand/mirzam-hero-light.webp",
    "../docs/brand/mirzam-hero-dark.webp",
    "data/adoption.csv",
  ]) {
    assert.ok(found.includes(want), `${want} missing from ${JSON.stringify(found)}`);
  }
});

test("the theme examples/06-theming.md loads is one the host would read", () => {
  // The deck that shows a theme of somebody's own. A theme file the host fails
  // to collect is a deck that previews unstyled - and this one would preview
  // with a pane in the wrong palette and nothing saying why.
  assert.ok(
    references(read("06-theming.md")).includes("themes/blueprint.css"),
    "themes/blueprint.css missing from what the host would collect"
  );
});

test("every path collected from a sample deck exists on disk", () => {
  // A path that comes out with a quote still on it, or with a `srcset`
  // descriptor attached, is a file the host then fails to read.
  for (const deck of fs.readdirSync(EXAMPLES).filter((f) => f.endsWith(".md"))) {
    for (const rel of references(read(deck))) {
      const abs = path.resolve(EXAMPLES, rel);
      assert.ok(fs.existsSync(abs), `${deck} references ${rel}, which does not exist`);
    }
  }
});
