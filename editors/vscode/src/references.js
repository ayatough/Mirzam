// Which files a deck's source names.
//
// The webview has no filesystem, so every path a deck writes has to be read by
// the extension host and handed over in one of the two tables the WASM core
// reads: transcluded Markdown, a masters file, a bibliography and the
// stylesheets in `theme:` come through `FileProvider`; images, video and a
// chart's CSV come through `AssetSource` as data URIs.
//
// A form missing from this list does not look like a missing form. It looks
// like a broken deck: the preview prints `was not provided by the host` for a
// file that is sitting right there on disk, and the CLI renders the same deck
// without a word. So this list is the counterpart of what the core resolves —
// change one and change the other.
//
// Kept in its own module, with no `vscode` import, so `test/references.test.js`
// can run it under plain node.

/**
 * Frontmatter settings whose value is one path to a file the core reads as
 * text. `theme:` is not among them: it holds a *list*, and only the entries
 * ending in `.css` are paths — see `themeFiles`.
 *
 * `css:` is the retired spelling of a one-entry `theme:`. It is accepted for
 * one release, exactly as the core accepts it, and goes at the same time.
 */
const FRONTMATTER_FILES = ["masters", "bibliography", "css"];

/**
 * Every local path `source` references, without duplicates; the caller resolves
 * them against the file's directory.
 *
 * URLs, `data:` payloads and `#id` links are left out: the core passes those
 * through untouched rather than asking the host for them. So is anything
 * inside code, which a deck writes to *show* the markup rather than use it —
 * `examples/04-components.md` demonstrates `bg-light=art/dawn.webp` beside a
 * pane that has no such file.
 */
function references(source) {
  const out = new Set();
  const push = (raw) => {
    const p = String(raw).trim().replace(/^["']|["']$/g, "");
    if (!p || p.startsWith("data:") || p.includes("://") || p.startsWith("#")) {
      return;
    }
    out.add(p);
  };

  // Charts are read from the source: a ```chart fence is a chart, not code.
  for (const path of chartData(source)) push(path);

  const prose = withoutCode(source);
  // `![[a.md]]` transclusion and `![alt](path)` media.
  const include = /^!\[\[([^\]]+)\]\]\s*$/gm;
  const media = /!\[[^\]]*\]\(([^()\s"]+)\)/g;
  // Raw HTML reaches the page as written, and the renderer inlines the sources
  // it finds there like any other asset — a `<picture>` is the documented way
  // to pick artwork by colour scheme.
  const rawHtml = /\b(?:src|poster)="([^"]*)"/g;
  for (const re of [include, media, rawHtml]) {
    let m;
    while ((m = re.exec(prose))) push(m[1]);
  }

  // Attribute values that name a file: a pane's background photograph, one per
  // colour mode, and a video's poster frame. Read from inside an attribute
  // block, because `poster=` is also a thing prose says; the block is one line
  // and the core splits it on whitespace, so a value never contains a space.
  const block = /\{[^{}\n]*\}/g;
  const attribute = /\b(?:bg|bg-light|bg-dark|poster)=([^\s}]+)/g;
  let brace;
  while ((brace = block.exec(prose))) {
    let m;
    while ((m = attribute.exec(brace[0]))) push(m[1]);
  }

  // A `srcset` is candidates separated by commas, each a URL and an optional
  // width or density descriptor. Only the URL is a file.
  const srcset = /\bsrcset="([^"]*)"/g;
  let m;
  while ((m = srcset.exec(prose))) {
    for (const candidate of m[1].split(",")) {
      push(candidate.trim().split(/\s+/)[0] || "");
    }
  }

  for (const key of FRONTMATTER_FILES) {
    const named = frontmatterPath(source, key);
    if (named) push(named);
  }
  for (const path of themeFiles(source)) push(path);
  return [...out];
}

/**
 * The stylesheets `theme:` names. The key takes a built-in theme's name, a
 * path ending in `.css`, or a list of both in cascade order, written either
 * inline (`theme: [mirzam, themes/acme.css]`) or as a block list. Only the
 * `.css` entries are files; a bare name is a theme the renderer already has,
 * and asking the host to read `mirzam` off disk would report a missing file
 * for a deck that is perfectly fine.
 */
function themeFiles(source) {
  const front = /^---\r?\n([\s\S]*?)\r?\n---\s*$/m.exec(source);
  if (!front || front.index !== 0) return [];
  const scalar = /^theme:[ \t]*(\S.*)$/m.exec(front[1]);
  const entries = [];
  if (scalar) {
    const value = scalar[1].trim();
    // An inline list, or a single entry.
    const list = /^\[(.*)\]$/.exec(value);
    for (const part of list ? list[1].split(",") : [value]) entries.push(part);
  } else if (/^theme:[ \t]*$/m.test(front[1])) {
    // A block list: `- entry` lines under the key, until the indentation ends.
    const after = front[1].slice(front[1].search(/^theme:[ \t]*$/m));
    for (const line of after.split(/\r?\n/).slice(1)) {
      const item = /^[ \t]*-[ \t]*(\S.*)$/.exec(line);
      if (!item) break;
      entries.push(item[1]);
    }
  }
  return entries
    .map((e) => e.trim().replace(/^["']|["']$/g, ""))
    .filter((e) => e.toLowerCase().endsWith(".css"));
}

/**
 * `source` with fenced blocks and inline code spans blanked out, so a deck that
 * quotes markup is not read as writing it. Line structure is kept, because
 * `![[…]]` is only a transclusion on a line of its own.
 */
function withoutCode(source) {
  const out = [];
  let fence = null;
  for (const line of source.split(/\r?\n/)) {
    const m = /^(`{3,}|~{3,})/.exec(line.trim());
    if (fence) {
      if (m && m[1][0] === fence[0] && m[1].length >= fence.length) fence = null;
      out.push("");
    } else if (m) {
      fence = m[1];
      out.push("");
    } else {
      out.push(line.replace(/`[^`]*`/g, ""));
    }
  }
  return out.join("\n");
}

/**
 * The CSV files named by `data:` inside a ```chart fence.
 *
 * Inline data is written as a `data: |` block, so a value only names a file
 * when it is a single line ending in `.csv` — the same rule `mirzam-chart`
 * applies when it decides whether to ask the host for anything.
 */
function chartData(source) {
  const out = [];
  let inChart = false;
  for (const line of source.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!inChart) {
      if (trimmed === "```chart") inChart = true;
      continue;
    }
    if (trimmed === "```") {
      inChart = false;
      continue;
    }
    const m = /^data:[ \t]*(\S.*)$/.exec(trimmed);
    if (m && m[1].trim().endsWith(".csv")) out.push(m[1].trim());
  }
  return out;
}

/**
 * The path in a frontmatter setting that names one, or null when the deck
 * writes the thing inline (a mapping, so the value is empty and the entries
 * are indented under it) or names none.
 */
function frontmatterPath(source, key) {
  const front = /^---\r?\n([\s\S]*?)\r?\n---\s*$/m.exec(source);
  if (!front || front.index !== 0) return null;
  const m = new RegExp(`^${key}:[ \\t]*(\\S.*)$`, "m").exec(front[1]);
  if (!m) return null;
  const value = m[1].trim().replace(/^["']|["']$/g, "");
  return value && !value.startsWith("{") ? value : null;
}

/**
 * Whether a reference is text the core reads through `FileProvider` rather
 * than an asset it inlines as a data URI.
 */
function isTextFile(rel) {
  return [".md", ".bib", ".css"].some((ext) => rel.endsWith(ext));
}

module.exports = { references, chartData, frontmatterPath, themeFiles, isTextFile, withoutCode };
