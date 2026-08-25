//! `mirzam lsp` — the deck, understood by the editor rather than rendered.
//!
//! The analysis is not new: every diagnostic published here is a build warning
//! the CLI already produces, under the kind
//! [`warning_kind`](crate::pipeline::warning_kind) gives it — the same
//! vocabulary `check --format json` reports to an agent. What this module adds
//! is a channel (JSON-RPC over stdio, which is all the Language Server
//! Protocol is) and a *range*, since an editor underlines a span and a build
//! warning knows a slide.
//!
//! **No browser, ever.** Only the layout half of `check` drives Chromium, and
//! nothing here is allowed to: a server that opened a browser on a keystroke
//! would be unusable, so clipped and overlapping content stays a command the
//! author runs, not something this reports as they type.
//!
//! **No dependency.** The framing is a `Content-Length` header and a body;
//! `serde_json` was already here. A protocol that can be read off its
//! specification does not need a runtime brought in to speak it.

use crate::pipeline::{self, warning_kind, BuildOutput, RenderCache};
use mirzam_render::diagnose::{line_column, line_end, locate};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

/// Speaks the protocol on stdin and stdout until the client says `exit`.
pub fn serve_stdio() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut server = Server::default();

    while let Some(message) = read_message(&mut input)? {
        for reply in server.handle(&message) {
            write_message(&mut output, &reply)?;
        }
        if server.exited {
            break;
        }
    }
    Ok(())
}

/// One editor session: the buffers it has open, and what has been said about
/// each file so far.
#[derive(Default)]
pub struct Server {
    /// Open buffers by path. The text here, not the file on disk, is what gets
    /// analysed — an unsaved edit is exactly the state the author wants
    /// diagnosed.
    docs: BTreeMap<PathBuf, String>,
    /// Files this session has sent diagnostics for. A file whose last problem
    /// was just fixed has to be sent an empty list, or the editor keeps
    /// showing the problem forever.
    published: BTreeSet<PathBuf>,
    cache: RenderCache,
    pub exited: bool,
}

impl Server {
    /// Handles one message and returns what to send back: a response for a
    /// request, notifications for anything the client should be told, and
    /// nothing at all for a notification that asks for no answer.
    pub fn handle(&mut self, message: &Value) -> Vec<Value> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => vec![response(id, capabilities())],
            // Nothing to do, but it is a notification and must not be answered
            // with a method-not-found error.
            "initialized" | "$/setTrace" | "workspace/didChangeConfiguration" => Vec::new(),
            "shutdown" => vec![response(id, Value::Null)],
            "exit" => {
                self.exited = true;
                Vec::new()
            }
            "textDocument/didOpen" => {
                let doc = &params["textDocument"];
                match (path_of_uri(doc["uri"].as_str()), doc["text"].as_str()) {
                    (Some(path), Some(text)) => {
                        self.docs.insert(path.clone(), text.to_string());
                        self.diagnose(&path)
                    }
                    _ => Vec::new(),
                }
            }
            // Full sync: the client is told `change: 1` in `capabilities`, so
            // each notification carries the whole buffer. Incremental sync
            // would save copying a few kilobytes and cost a text-editing
            // implementation that can disagree with the editor's.
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str();
                let text = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str());
                match (path_of_uri(uri), text) {
                    (Some(path), Some(text)) => {
                        self.docs.insert(path.clone(), text.to_string());
                        self.diagnose(&path)
                    }
                    _ => Vec::new(),
                }
            }
            "textDocument/didSave" => match path_of_uri(params["textDocument"]["uri"].as_str()) {
                // A save changes what a *transcluded* file says, and those are
                // read from disk, so the deck is analysed again even though
                // its own buffer did not change.
                Some(path) => self.diagnose(&path),
                None => Vec::new(),
            },
            "textDocument/didClose" => match path_of_uri(params["textDocument"]["uri"].as_str()) {
                Some(path) => {
                    self.docs.remove(&path);
                    // Only this file's own marks go: a closed deck may still be
                    // the reason an open one is wrong.
                    self.published.remove(&path);
                    vec![publish(&path, Vec::new())]
                }
                None => Vec::new(),
            },
            "textDocument/documentSymbol" => {
                let symbols = path_of_uri(params["textDocument"]["uri"].as_str())
                    .map(|path| self.symbols(&path))
                    .unwrap_or_default();
                vec![response(id, Value::Array(symbols))]
            }
            "textDocument/completion" => {
                let items = at(&params)
                    .and_then(|(path, pos)| {
                        let offset = self.offset_in(&path, pos)?;
                        Some(self.completions(&path, offset))
                    })
                    .unwrap_or_default();
                vec![response(id, Value::Array(items))]
            }
            "textDocument/hover" => {
                let hover = at(&params).and_then(|(path, pos)| {
                    let offset = self.offset_in(&path, pos)?;
                    self.hover(&path, offset)
                });
                vec![response(id, hover.unwrap_or(Value::Null))]
            }
            "textDocument/definition" => {
                let target = at(&params).and_then(|(path, pos)| {
                    let offset = self.offset_in(&path, pos)?;
                    self.definition(&path, offset)
                });
                vec![response(id, target.unwrap_or(Value::Null))]
            }
            // A request the server never advertised. Answering with an error is
            // the specified reply; answering a *notification* at all is not.
            _ => match id {
                Some(_) => vec![method_not_found(id, method)],
                None => Vec::new(),
            },
        }
    }

    /// Analyses the deck this path belongs to and returns one
    /// `publishDiagnostics` per file that has anything to say — plus an empty
    /// one for every file that had something to say last time and no longer
    /// does.
    fn diagnose(&mut self, path: &Path) -> Vec<Value> {
        if !is_markdown(path) {
            return Vec::new();
        }
        let Some(source) = self.docs.get(path).cloned() else {
            return Vec::new();
        };

        let mut by_file: BTreeMap<PathBuf, Vec<Value>> = BTreeMap::new();
        for finding in self.findings(path, &source) {
            let text = self.text_of(&finding.file).unwrap_or_default();
            by_file
                .entry(finding.file.clone())
                .or_default()
                .push(finding.into_diagnostic(&text));
        }
        // The deck itself always gets a message, even an empty one: it is how
        // the last diagnostic on it disappears when the author fixes it.
        by_file.entry(path.to_path_buf()).or_default();

        let cleared: Vec<PathBuf> = self
            .published
            .iter()
            .filter(|f| !by_file.contains_key(*f))
            .cloned()
            .collect();
        self.published = by_file.keys().cloned().collect();

        cleared
            .into_iter()
            .map(|f| publish(&f, Vec::new()))
            .chain(by_file.into_iter().map(|(f, ds)| publish(&f, ds)))
            .collect()
    }

    /// Every problem in the deck rooted at `path`, placed in the file it
    /// belongs to. Public so the tests can read them without a client.
    pub fn findings(&mut self, path: &Path, source: &str) -> Vec<Finding> {
        let built = match pipeline::build_source(path, source, &mut self.cache, None, None) {
            Ok(built) => built,
            // A deck that will not build at all — unreadable frontmatter,
            // usually — has one problem, and it is at the top of the file.
            Err(e) => {
                return vec![Finding {
                    file: path.to_path_buf(),
                    start: 0,
                    end: 0,
                    kind: warning_kind(&e),
                    message: e,
                }]
            }
        };

        let mut out = Vec::new();
        for (message, site) in built.warnings.iter().zip(&built.warning_sites) {
            let file = site.file.clone().unwrap_or_else(|| path.to_path_buf());
            let Some(text) = self.text_of(&file) else {
                continue;
            };
            let from = site.offset.unwrap_or(0).min(text.len());
            let to = slide_end(&built, site.slide, &file).unwrap_or(text.len());
            let (start, end) = locate(&text, from, to.clamp(from, text.len()), message);
            out.push(Finding {
                file,
                start,
                end,
                kind: warning_kind(message),
                message: message.clone(),
            });
        }
        out
    }

    /// One symbol per authored slide, named by its first heading. A slide
    /// broken by `<!-- next -->` renders as several sections and is one entry
    /// here, because there is one slide in the source.
    fn symbols(&mut self, path: &Path) -> Vec<Value> {
        if !is_markdown(path) {
            return Vec::new();
        }
        let Some(source) = self.docs.get(path).cloned() else {
            return Vec::new();
        };
        let Ok(built) = pipeline::build_source(path, &source, &mut self.cache, None, None) else {
            return Vec::new();
        };

        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for section in 1..=built.sections.len() {
            let Some(authored) = built.section_slides.get(section - 1).copied() else {
                continue;
            };
            if !seen.insert(authored) {
                continue;
            }
            let Some((origin, start)) = built.slide_origin(section) else {
                continue;
            };
            if origin != path {
                continue;
            }
            let end = slide_end(&built, Some(section), path).unwrap_or(source.len());
            let range = span(&source, start, end.max(start));
            out.push(json!({
                "name": heading(&source, start, end).unwrap_or_else(|| format!("Slide {section}")),
                "kind": SYMBOL_STRING,
                "range": range,
                "selectionRange": span(&source, start, line_end(&source, start)),
            }));
        }
        out
    }

    /// The text of a file, preferring an open buffer to what is on disk.
    fn text_of(&self, file: &Path) -> Option<String> {
        self.docs
            .get(file)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
    }

    /// What the author could write where the cursor is. Read off the line's
    /// prefix rather than a grammar: every context here opens with a spelling
    /// nothing else uses (`::: pane `, `<!-- layout:`, `[@`, `{{`, a fence),
    /// which is a deliberate property of the markup, and this is where it
    /// pays.
    fn completions(&mut self, path: &Path, offset: usize) -> Vec<Value> {
        let Some((source, meta)) = self.deck(path) else {
            return Vec::new();
        };
        let line_start = source[..offset.min(source.len())]
            .rfind('\n')
            .map_or(0, |nl| nl + 1);
        let prefix = &source[line_start..offset.min(source.len())];
        let t = prefix.trim_start();

        // A theme name completes in three spellings of the same question.
        let themes = || -> Vec<Value> {
            let mut names: Vec<String> = mirzam_render::THEME_NAMES
                .iter()
                .map(|n| n.to_string())
                .collect();
            for sheet in meta.theme_sheets() {
                let stem = mirzam_render::FileTheme::new(sheet.path, "").name;
                if !names.contains(&stem) {
                    names.push(stem);
                }
            }
            names
                .into_iter()
                .map(|n| item(&n, KIND_VALUE, "theme"))
                .collect()
        };

        if let Some(rest) = t.strip_prefix("<!--") {
            let rest = rest.trim_start();
            return if rest.starts_with("layout:") {
                let mut out: Vec<Value> = self
                    .masters(path, &meta)
                    .keys()
                    .map(|n| item(n, KIND_VALUE, "master"))
                    .collect();
                out.push(item("none", KIND_VALUE, "no master at all"));
                out
            } else if rest.starts_with("theme:") {
                themes()
            } else if rest.starts_with("mode:") {
                ["light", "dark"]
                    .iter()
                    .map(|m| item(m, KIND_VALUE, "mode"))
                    .collect()
            } else if rest.starts_with("chrome:") {
                vec![item("none", KIND_VALUE, "no footer or slide number here")]
            } else {
                // The comment is still being opened: offer the settings a
                // slide understands, keys nothing lists anywhere on screen.
                [
                    ("note: ", "speaker note"),
                    ("layout: ", "draw this slide on a master"),
                    ("theme: ", "a theme for this slide only"),
                    ("mode: ", "light or dark for this slide"),
                    ("chrome: none", "drop the footer and slide number"),
                    ("autoplay: ", "hold this slide longer under autoplay"),
                    ("next", "carry this pane on to the next slide"),
                ]
                .iter()
                .map(|(k, d)| item(k, KIND_KEYWORD, d))
                .collect()
            };
        }

        if t.starts_with("::: pane") {
            return self
                .pane_names(path, &meta, &source, offset)
                .into_iter()
                .map(|n| item(&n, KIND_FIELD, "pane in this slide's grid"))
                .collect();
        }

        // Frontmatter's own `theme:` line, before the body begins.
        if in_frontmatter(&source, offset) && (t.starts_with("theme:") || t.starts_with("- ")) {
            return themes();
        }

        // `[@` opens a citation; complete the keys the bibliography holds.
        if let Some(open) = prefix.rfind("[@") {
            if !prefix[open..].contains(']') {
                return self
                    .bibliography(path, &meta)
                    .values()
                    .map(|e| {
                        let title = e.field(&["title"]).unwrap_or("");
                        item(&e.key, KIND_REFERENCE, title)
                    })
                    .collect();
            }
        }

        // `{{` opens a variable; complete what frontmatter `vars` defines.
        if let Some(open) = prefix.rfind("{{") {
            if !prefix[open..].contains("}}") {
                return meta
                    .var_table()
                    .iter()
                    .map(|(k, v)| item(k, KIND_VARIABLE, &v.to_display()))
                    .collect();
            }
        }

        // A fence being opened: the block forms Mirzam gives a meaning to.
        if t.starts_with("```") && !t[3..].contains(char::is_whitespace) {
            return mirzam_syntax::BLOCK_KINDS
                .iter()
                .map(|k| item(k, KIND_KEYWORD, "Mirzam block"))
                .collect();
        }

        Vec::new()
    }

    /// What the thing under the cursor *is*: a citation's entry, a variable's
    /// value, the shape of a data file. Nothing here opens a browser or does
    /// analysis a build does not already do.
    fn hover(&mut self, path: &Path, offset: usize) -> Option<Value> {
        let (source, meta) = self.deck(path)?;

        if let Some(key) = cite_key_at(&source, offset) {
            let bib = self.bibliography(path, &meta);
            let entry = bib.get(&key)?;
            let mut lines = Vec::new();
            if let Some(title) = entry.field(&["title"]) {
                lines.push(format!("**{title}**"));
            }
            let byline: Vec<&str> = [
                entry.field(&["author"]),
                entry.field(&["year", "date"]),
                entry.field(&["journal", "booktitle", "publisher", "howpublished"]),
            ]
            .into_iter()
            .flatten()
            .collect();
            if !byline.is_empty() {
                lines.push(byline.join(" · "));
            }
            lines.push(format!("`[@{}]` cites as [{}]", entry.key, entry.label()));
            return Some(hover_markdown(&lines.join("\n\n")));
        }

        if let Some(expr) = expression_at(&source, offset) {
            let evaluated = mirzam_core::substitute_vars(&expr, &meta.var_table());
            if evaluated != expr {
                return Some(hover_markdown(&format!("= {evaluated}")));
            }
        }

        // `data:` inside a `chart` or `each` fence: say what the file holds,
        // because the fields it names are the ones the template may use.
        if let Some(rel) = data_path_at(&source, offset) {
            let base = path.parent().unwrap_or(Path::new("."));
            let text = std::fs::read_to_string(base.join(&rel)).ok()?;
            let mut lines = text.lines().filter(|l| !l.trim().is_empty());
            let header = lines.next().unwrap_or("");
            let rows = lines.count();
            return Some(hover_markdown(&format!(
                "`{rel}` — fields `{header}`, {rows} row{}",
                if rows == 1 { "" } else { "s" }
            )));
        }

        None
    }

    /// Where the name under the cursor is defined: a transcluded file, a
    /// citation's BibTeX entry, a master's heading, a theme stylesheet.
    fn definition(&mut self, path: &Path, offset: usize) -> Option<Value> {
        let (source, meta) = self.deck(path)?;
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let (line_start, line) = line_around(&source, offset);
        let column = offset - line_start;

        // `![[section.md]]` — the file it pastes in.
        if let Some((at, rel)) = spanning(line, column, "![[", "]]") {
            let _ = at;
            let target = base.join(rel.trim());
            if target.is_file() {
                return Some(location(&target, 0, 0));
            }
        }

        // `[@key]` — the entry in the bibliography file.
        if let Some(key) = cite_key_at(&source, offset) {
            let rel = meta.bibliography_file()?;
            let file = base.join(rel);
            let text = std::fs::read_to_string(&file).ok()?;
            let at = text
                .find(&format!("{{{key},"))
                .or_else(|| text.find(&format!("{{{key}}}")))
                .or_else(|| text.find(&key))?;
            return Some(location(&file, at, at + key.len()));
        }

        // `<!-- layout: two-up -->` — the master's heading in the masters file.
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("<!--") {
            if let Some(name) = rest.trim_start().strip_prefix("layout:") {
                let name = name.trim().trim_end_matches("-->").trim();
                let rel = meta.masters_file()?;
                let file = base.join(rel);
                let text = std::fs::read_to_string(&file).ok()?;
                let mut at = 0usize;
                for l in text.split_inclusive('\n') {
                    let lt = l.trim();
                    let hashes = lt.chars().take_while(|c| *c == '#').count();
                    if (1..=6).contains(&hashes) && lt[hashes..].trim() == name {
                        return Some(location(&file, at, at + l.trim_end().len()));
                    }
                    at += l.len();
                }
                return Some(location(&file, 0, 0));
            }
        }

        // A `.css` the deck's `theme:` names, under the cursor.
        for sheet in meta.theme_sheets() {
            if let Some(at) = line.find(sheet.path) {
                if (at..at + sheet.path.len()).contains(&column) {
                    let target = base.join(sheet.path);
                    if target.is_file() {
                        return Some(location(&target, 0, 0));
                    }
                }
            }
        }

        None
    }

    /// A protocol position in an open buffer, as a byte offset into it.
    fn offset_in(&self, path: &Path, (line, character): (usize, usize)) -> Option<usize> {
        let text = self.docs.get(path)?;
        Some(offset_of(text, line, character))
    }

    /// The buffer and its parsed frontmatter, which every capability above
    /// starts from.
    fn deck(&mut self, path: &Path) -> Option<(String, mirzam_core::DeckMeta)> {
        if !is_markdown(path) {
            return None;
        }
        let source = self.docs.get(path).cloned()?;
        let (fm, _) = mirzam_syntax::split_frontmatter(&source);
        let meta = fm
            .and_then(|y| mirzam_core::parse_meta(y).ok())
            .unwrap_or_default();
        Some((source, meta))
    }

    /// The masters this deck can draw on, read from disk each time: the file
    /// is edited beside the deck, and a stale cache here would complete names
    /// that no longer exist.
    fn masters(&self, path: &Path, meta: &mirzam_core::DeckMeta) -> BTreeMap<String, String> {
        let Some(rel) = meta.masters_file() else {
            return BTreeMap::new();
        };
        let base = path.parent().unwrap_or(Path::new("."));
        mirzam_syntax::load_masters(rel, base, &mirzam_syntax::FsProvider)
            .map(|(m, _)| m)
            .unwrap_or_default()
    }

    /// Every reference the deck can cite.
    fn bibliography(
        &self,
        path: &Path,
        meta: &mirzam_core::DeckMeta,
    ) -> mirzam_render::Bibliography {
        let base = path.parent().unwrap_or(Path::new("."));
        let (bib, _) = mirzam_render::deck_bibliography(meta, |rel| {
            std::fs::read_to_string(base.join(rel)).map_err(|e| e.to_string())
        });
        bib
    }

    /// The panes the slide around `offset` may assign content to: its own
    /// grid's names, or the names of the master it would be drawn on.
    fn pane_names(
        &self,
        path: &Path,
        meta: &mirzam_core::DeckMeta,
        source: &str,
        offset: usize,
    ) -> Vec<String> {
        let (_, body) = mirzam_syntax::split_frontmatter(source);
        let body_off = source.len() - body.len();
        let slides = mirzam_syntax::split_slides_spanned(body, meta.split_level());
        let at = offset.saturating_sub(body_off);
        let slide = slides
            .iter()
            .rev()
            .find(|s| s.start <= at)
            .or_else(|| slides.first());
        let Some(slide) = slide else {
            return Vec::new();
        };
        let parsed = mirzam_syntax::parse_slide(&slide.text);
        let art = match &parsed.layout {
            Some(own) => Some(own.clone()),
            None => {
                let masters = self.masters(path, meta);
                let name = parsed
                    .layout_name
                    .clone()
                    .or_else(|| meta.layout.clone())
                    .unwrap_or_default();
                masters.get(name.trim()).cloned()
            }
        };
        let mut names: Vec<String> = Vec::new();
        for token in art
            .unwrap_or_default()
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        {
            if !token.is_empty()
                && token.chars().next().is_some_and(char::is_alphabetic)
                && !names.iter().any(|n| n == token)
            {
                names.push(token.to_string());
            }
        }
        names
    }
}

/// A request's document and position, as a path and a byte offset into the
/// open buffer.
fn at(params: &Value) -> Option<(PathBuf, (usize, usize))> {
    let path = path_of_uri(params["textDocument"]["uri"].as_str())?;
    let line = params["position"]["line"].as_u64()? as usize;
    let character = params["position"]["character"].as_u64()? as usize;
    Some((path, (line, character)))
}

const KIND_FIELD: u8 = 5;
const KIND_VARIABLE: u8 = 6;
const KIND_VALUE: u8 = 12;
const KIND_KEYWORD: u8 = 14;
const KIND_REFERENCE: u8 = 18;

fn item(label: &str, kind: u8, detail: &str) -> Value {
    let mut v = json!({ "label": label, "kind": kind });
    if !detail.is_empty() {
        v["detail"] = Value::String(detail.to_string());
    }
    v
}

fn hover_markdown(text: &str) -> Value {
    json!({ "contents": { "kind": "markdown", "value": text } })
}

fn location(file: &Path, start: usize, end: usize) -> Value {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    json!({ "uri": uri_of_path(file), "range": span(&text, start, end) })
}

/// Whether `offset` sits inside the deck's opening frontmatter block.
fn in_frontmatter(source: &str, offset: usize) -> bool {
    let (fm, body) = mirzam_syntax::split_frontmatter(source);
    fm.is_some() && offset < source.len() - body.len()
}

/// The byte offset of a protocol position: zero-based line, column in UTF-16
/// code units — the inverse of [`position`].
fn offset_of(text: &str, line: usize, character: usize) -> usize {
    let mut at = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i == line {
            let content = l.trim_end_matches(['\r', '\n']);
            let mut units = 0usize;
            for (ci, ch) in content.char_indices() {
                if units >= character {
                    return at + ci;
                }
                units += ch.len_utf16();
            }
            return at + content.len();
        }
        at += l.len();
    }
    text.len()
}

/// The line `offset` sits on, and where it starts.
fn line_around(text: &str, offset: usize) -> (usize, &str) {
    let offset = offset.min(text.len());
    let start = text[..offset].rfind('\n').map_or(0, |nl| nl + 1);
    let end = text[start..].find('\n').map_or(text.len(), |nl| start + nl);
    (start, &text[start..end])
}

/// The `key` of the `[@key]` the cursor is inside, if any.
fn cite_key_at(source: &str, offset: usize) -> Option<String> {
    let (start, line) = line_around(source, offset);
    let column = offset - start;
    let (open_at, key) = spanning(line, column, "[@", "]")?;
    let _ = open_at;
    let key = key.trim();
    (!key.is_empty() && !key.contains(char::is_whitespace)).then(|| key.to_string())
}

/// The `expr` of the `{{ expr }}` the cursor is inside, if any.
fn expression_at(source: &str, offset: usize) -> Option<String> {
    let (start, line) = line_around(source, offset);
    let column = offset - start;
    let (_, expr) = spanning(line, column, "{{", "}}")?;
    Some(format!("{{{{{expr}}}}}"))
}

/// The text between `open` and `close` on `line`, when `column` falls within
/// the whole span (delimiters included).
fn spanning<'a>(line: &'a str, column: usize, open: &str, close: &str) -> Option<(usize, &'a str)> {
    let mut from = 0;
    while let Some(rel) = line[from..].find(open) {
        let open_at = from + rel;
        let inner_at = open_at + open.len();
        let len = line[inner_at..].find(close)?;
        let end = inner_at + len + close.len();
        if (open_at..end).contains(&column) {
            return Some((open_at, &line[inner_at..inner_at + len]));
        }
        from = end;
    }
    None
}

/// The file a `data:` line names, when `offset` is on that line and the line
/// sits inside a `chart` or `each` fence.
fn data_path_at(source: &str, offset: usize) -> Option<String> {
    let (start, line) = line_around(source, offset);
    let rel = line.trim().strip_prefix("data:")?.trim();
    if rel.is_empty() {
        return None;
    }
    // Walk fences from the top: the line has to be inside a chart/each block,
    // not prose that happens to open with the word.
    let mut fence: Option<(usize, bool)> = None;
    let mut at = 0usize;
    for l in source.split_inclusive('\n') {
        if at >= start {
            break;
        }
        let t = l.trim();
        let ticks = t.len() - t.trim_start_matches('`').len();
        if ticks >= 3 {
            fence = match fence {
                Some((open, _)) if ticks >= open && t[ticks..].trim().is_empty() => None,
                None => {
                    let info = t[ticks..].trim();
                    Some((ticks, info == "chart" || info == "each"))
                }
                keep => keep,
            };
        }
        at += l.len();
    }
    matches!(fence, Some((_, true))).then(|| rel.to_string())
}

/// A problem, in the file it belongs to and at byte offsets within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: PathBuf,
    pub start: usize,
    pub end: usize,
    /// The stable kind an agent already reads out of `check --format json`;
    /// an editor shows it as the diagnostic's code.
    pub kind: &'static str,
    pub message: String,
}

impl Finding {
    fn into_diagnostic(self, text: &str) -> Value {
        json!({
            "range": span(text, self.start, self.end),
            // Everything a build reports is a warning: a deck with a problem
            // still renders, which is the whole shape of this tool's
            // degradations, and an editor that paints them red says otherwise.
            "severity": SEVERITY_WARNING,
            "code": self.kind,
            "source": "mirzam",
            "message": self.message,
        })
    }
}

const SEVERITY_WARNING: u8 = 2;
/// `SymbolKind.String`, which is what editors' Markdown outlines use for a
/// heading.
const SYMBOL_STRING: u8 = 15;

/// Where the slide after this one begins in `file`, which bounds the search
/// for whatever a warning is about. `None` when this is the last slide, or
/// when the next one lives in another file.
fn slide_end(built: &BuildOutput, slide: Option<usize>, file: &Path) -> Option<usize> {
    let next = built.slide_origin(slide? + 1)?;
    (next.0 == file).then_some(next.1)
}

/// The first heading in a slide, as its name in the outline.
fn heading(text: &str, from: usize, to: usize) -> Option<String> {
    let slide = text.get(from..to.min(text.len()))?;
    let line = slide
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with('#') && l.trim_start_matches('#').starts_with(' '))?;
    let name = line.trim_start_matches('#').trim();
    // Raw HTML reaches a slide as written, and a heading holding a `<br>` is
    // one line in the outline, not two words run together.
    let name = strip_tags(name);
    let name = name.trim();
    // A heading may carry an attribute list — `# Title {.title-slide}` — which
    // is markup, not part of what the slide is called.
    let name = match name.rfind('{') {
        Some(at) if name.ends_with('}') => name[..at].trim(),
        _ => name,
    };
    (!name.is_empty()).then(|| name.to_string())
}

/// Drops HTML tags and leaves one space where each was, so `a<br>b` reads as
/// two words rather than one.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for c in text.chars() {
        match c {
            '<' => {
                inside = true;
                out.push(' ');
            }
            '>' if inside => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A byte range as the protocol wants it: zero-based lines, and columns
/// counted in UTF-16 code units, which is what makes a deck written in
/// Japanese land its marks where the text is.
fn span(text: &str, start: usize, end: usize) -> Value {
    json!({ "start": position(text, start), "end": position(text, end.max(start)) })
}

fn position(text: &str, offset: usize) -> Value {
    let (line, character) = line_column(text, offset);
    json!({ "line": line, "character": character })
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

fn capabilities() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": { "openClose": true, "change": 1, "save": true },
            "documentSymbolProvider": true,
            // `@` opens a citation, `{` a variable, `:` follows a setting key,
            // and a space follows `::: pane` — the rest complete on request.
            "completionProvider": { "triggerCharacters": ["@", "{", ":", " "] },
            "hoverProvider": true,
            "definitionProvider": true,
        },
        "serverInfo": { "name": "mirzam", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn response(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn method_not_found(id: Option<Value>, method: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": format!("no such method: {method}") },
    })
}

fn publish(file: &Path, diagnostics: Vec<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri_of_path(file), "diagnostics": diagnostics },
    })
}

// ---- transport ----

/// How long a header line may be before the stream stops looking like a
/// client's. Real ones are `Content-Length: 1234`.
const MAX_HEADER_BYTES: u64 = 8 * 1024;
/// And how large a message body may claim to be. A deck is text; sixty-four
/// megabytes of it is already far past anything anyone edits.
const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;

/// Reads one message, or `None` once the client has closed its end.
///
/// The framing is the whole of it: headers, a blank line, then exactly
/// `Content-Length` bytes. Any other header is ignored, as the specification
/// says to.
fn read_message(input: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut length = None;
    loop {
        let mut line = String::new();
        // Bounded, because `read_line` on a stream that never sends a newline
        // buffers the whole stream: `mirzam lsp < /dev/zero` reached half a
        // gigabyte in three seconds at 99% of a core. No editor does that, but
        // a wrong pipe is not an editor, and the answer to one is an error
        // rather than a machine slowing down.
        match (&mut *input).take(MAX_HEADER_BYTES).read_line(&mut line) {
            Ok(0) => return Ok(None),
            Ok(_) => {}
            Err(e) => return Err(format!("cannot read from the client: {e}")),
        }
        if line.len() as u64 == MAX_HEADER_BYTES && !line.ends_with('\n') {
            return Err(format!(
                "a header line ran past {MAX_HEADER_BYTES} bytes with no end - \
                 this is not a language client's stream"
            ));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = length else {
        return Err("a message arrived with no Content-Length".into());
    };
    // A length is a promise about a buffer this process is about to allocate,
    // and a garbled one promises whatever the digits happen to say.
    if length as u64 > MAX_BODY_BYTES {
        return Err(format!(
            "a message claimed {length} bytes, past the {MAX_BODY_BYTES}-byte limit"
        ));
    }
    let mut body = vec![0u8; length];
    input
        .read_exact(&mut body)
        .map_err(|e| format!("a message stopped short of its Content-Length: {e}"))?;
    serde_json::from_slice(&body).map(Some).map_err(|e| {
        // Quoting the body would put an arbitrary buffer in the log; the
        // length is what tells you whether the framing or the JSON is wrong.
        format!("a {length}-byte message was not JSON: {e}")
    })
}

fn write_message(output: &mut impl Write, message: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(message).map_err(|e| format!("cannot encode a reply: {e}"))?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())
        .and_then(|()| output.write_all(&body))
        .and_then(|()| output.flush())
        .map_err(|e| format!("cannot write to the client: {e}"))
}

// ---- URIs ----

/// `file:///a/b.md` → `/a/b.md`, undoing the percent-encoding an editor
/// applies to spaces and to anything else outside the unreserved set.
///
/// Anything that is not a `file:` URI is not a file this server can read, and
/// answers `None` rather than a path that happens to parse.
fn path_of_uri(uri: Option<&str>) -> Option<PathBuf> {
    let rest = uri?.strip_prefix("file://")?;
    // `file://host/path` is somebody else's file; only an empty authority (the
    // usual `file:///path`) names one here.
    let path = rest.strip_prefix('/')?;
    let mut out = Vec::new();
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    let decoded = String::from_utf8(out).ok()?;
    // `file:///c:/x` is how Windows spells a drive, and the leading slash the
    // URI needs is not part of the path.
    let looks_like_a_drive = decoded
        .as_bytes()
        .get(1)
        .is_some_and(|c| *c == b':' || *c == b'|');
    Some(PathBuf::from(if looks_like_a_drive {
        decoded
    } else {
        format!("/{decoded}")
    }))
}

fn uri_of_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let mut out = String::from("file://");
    if !text.starts_with('/') {
        out.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "method": method, "params": params })
    }

    fn opened(server: &mut Server, path: &str, text: &str) -> Vec<Value> {
        server.handle(&message(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": uri_of_path(Path::new(path)), "text": text } }),
        ))
    }

    fn diagnostics(replies: &[Value]) -> Vec<Value> {
        replies
            .iter()
            .filter(|r| r["method"] == "textDocument/publishDiagnostics")
            .flat_map(|r| {
                r["params"]["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }

    const DECK: &str = "---\ntitle: T\n---\n\n# One\n\nA line.\n\n---\n\n# Two\n\nMore.\n";

    #[test]
    fn a_clean_deck_publishes_an_empty_list_rather_than_nothing() {
        // Not a formality: without it the editor keeps showing the last
        // problem after it has been fixed.
        let mut server = Server::default();
        let replies = opened(&mut server, "/tmp/mirzam-lsp-clean.md", DECK);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0]["method"], "textDocument/publishDiagnostics");
        assert_eq!(diagnostics(&replies), Vec::<Value>::new());
    }

    #[test]
    fn a_warning_underlines_the_word_the_message_quotes() {
        let deck = "---\ntitle: T\n---\n\n```pane\n+--------+\n|        |\n| body   |\n|        |\n+--------+\n```\n\n::: pane nowhere\nText.\n:::\n";
        let mut server = Server::default();
        let replies = opened(&mut server, "/tmp/mirzam-lsp-pane.md", deck);
        let found = diagnostics(&replies);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0]["code"], "build.layout");
        // `nowhere` is on line 12 (zero-based), after `::: pane `.
        assert_eq!(
            found[0]["range"]["start"],
            json!({ "line": 12, "character": 9 })
        );
        assert_eq!(
            found[0]["range"]["end"],
            json!({ "line": 12, "character": 16 })
        );
    }

    /// The mark has to go when the mistake does, and an editor only learns
    /// that from an empty list. A server that publishes nothing on a clean
    /// buffer leaves the old squiggle on screen for the rest of the session.
    #[test]
    fn fixing_the_deck_takes_the_mark_away() {
        let broken = "---\ntitle: T\ntheme: nosuchtheme\n---\n\n# One\n";
        let fixed = "---\ntitle: T\ntheme: nord\n---\n\n# One\n";
        let path = "/tmp/mirzam-lsp-fixed.md";
        let mut server = Server::default();
        assert_eq!(diagnostics(&opened(&mut server, path, broken)).len(), 1);

        let after = server.handle(&message(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri_of_path(Path::new(path)), "version": 2 },
                "contentChanges": [{ "text": fixed }],
            }),
        ));
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["params"]["diagnostics"], json!([]));
    }

    #[test]
    fn a_frontmatter_warning_lands_in_the_frontmatter() {
        let deck = "---\ntitle: T\ntheme: nosuchtheme\n---\n\n# One\n";
        let mut server = Server::default();
        let found = diagnostics(&opened(&mut server, "/tmp/mirzam-lsp-theme.md", deck));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0]["code"], "build.theme");
        assert_eq!(
            found[0]["range"]["start"],
            json!({ "line": 2, "character": 7 })
        );
    }

    /// A column is counted the way the protocol counts one, or every mark on a
    /// line of Japanese lands to the left of what it is about.
    #[test]
    fn a_column_is_utf16_code_units_not_bytes() {
        let text = "日本語のスライド `nosuchpane` です\n";
        assert_eq!(position(text, 0), json!({ "line": 0, "character": 0 }));
        // Nine characters, each one UTF-16 unit, before the backtick.
        assert_eq!(
            position(text, "日本語のスライド ".len()),
            json!({ "line": 0, "character": 9 })
        );
    }

    #[test]
    fn the_outline_names_a_slide_by_its_heading() {
        let mut server = Server::default();
        let path = "/tmp/mirzam-lsp-outline.md";
        opened(&mut server, path, DECK);
        let replies = server.handle(&json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "textDocument/documentSymbol",
            "params": { "textDocument": { "uri": uri_of_path(Path::new(path)) } },
        }));
        let symbols = replies[0]["result"].as_array().cloned().unwrap_or_default();
        let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
        assert_eq!(names, ["One", "Two"]);
        assert_eq!(symbols[1]["range"]["start"]["line"], 10);
    }

    #[test]
    fn a_request_the_server_never_advertised_is_an_error_and_a_notification_is_not() {
        let mut server = Server::default();
        let replies = server.handle(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "textDocument/codeAction", "params": {},
        }));
        assert_eq!(replies[0]["error"]["code"], -32601);
        assert!(server
            .handle(&message("$/cancelRequest", json!({ "id": 1 })))
            .is_empty());
    }

    #[test]
    fn exit_stops_the_loop_and_shutdown_answers_first() {
        let mut server = Server::default();
        let replies = server.handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }));
        assert_eq!(replies[0]["result"], Value::Null);
        assert!(!server.exited);
        server.handle(&message("exit", Value::Null));
        assert!(server.exited);
    }

    #[test]
    fn a_message_is_framed_by_its_length_and_read_back_whole() {
        let mut written = Vec::new();
        write_message(&mut written, &json!({ "hello": "デッキ" })).expect("written");
        let text = String::from_utf8(written.clone()).expect("utf-8");
        assert!(text.starts_with("Content-Length: "), "{text}");
        let mut reader = std::io::BufReader::new(&written[..]);
        let back = read_message(&mut reader).expect("read").expect("a message");
        assert_eq!(back["hello"], "デッキ");
        // And the end of the stream is an end, not an error.
        assert_eq!(read_message(&mut reader).expect("read"), None);
    }

    /// The failure this guard is for is not hypothetical: before it,
    /// `mirzam lsp < /dev/zero` grew half a gigabyte in three seconds,
    /// because a line with no end is a line `read_line` keeps buffering.
    #[test]
    fn a_stream_that_never_ends_a_line_is_an_error_not_a_swelling_buffer() {
        let endless = std::io::repeat(b'x');
        let mut reader = std::io::BufReader::new(endless);
        let answer = read_message(&mut reader);
        assert!(
            answer.is_err_and(|e| e.contains("not a language client's stream")),
            "an endless header line should be refused"
        );
    }

    #[test]
    fn a_body_larger_than_the_limit_is_refused_before_it_is_allocated() {
        let framed = format!("Content-Length: {}\r\n\r\n", MAX_BODY_BYTES + 1);
        let mut reader = std::io::BufReader::new(framed.as_bytes());
        assert!(read_message(&mut reader).is_err_and(|e| e.contains("past the")));
    }

    #[test]
    fn a_uri_survives_a_space_and_a_multibyte_name() {
        for path in ["/tmp/a deck.md", "/tmp/スライド.md", "/tmp/plain.md"] {
            let uri = uri_of_path(Path::new(path));
            assert!(!uri.contains(' '), "{uri}");
            assert_eq!(path_of_uri(Some(&uri)), Some(PathBuf::from(path)));
        }
        assert_eq!(path_of_uri(Some("untitled:Untitled-1")), None);
    }

    #[test]
    fn a_windows_drive_keeps_its_letter_and_loses_the_slash() {
        assert_eq!(
            path_of_uri(Some("file:///c%3A/decks/talk.md")),
            Some(PathBuf::from("c:/decks/talk.md"))
        );
    }

    #[test]
    fn a_file_that_is_not_markdown_is_not_a_deck() {
        let mut server = Server::default();
        assert!(opened(&mut server, "/tmp/notes.txt", DECK).is_empty());
    }

    fn request(server: &mut Server, method: &str, path: &str, line: u64, character: u64) -> Value {
        let replies = server.handle(&json!({
            "jsonrpc": "2.0", "id": 9, "method": method,
            "params": {
                "textDocument": { "uri": uri_of_path(Path::new(path)) },
                "position": { "line": line, "character": character },
            },
        }));
        replies[0]["result"].clone()
    }

    fn labels(result: &Value) -> Vec<String> {
        result
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| i["label"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `::: pane ` completes the names the slide's own grid defines — the
    /// answer to the most common typo the diagnostics catch after the fact.
    #[test]
    fn pane_names_complete_from_the_slides_own_grid() {
        let deck = "---\ntitle: T\n---\n\n```pane\n+------+-----+\n| main | fig |\n+------+-----+\n```\n\n::: pane \n";
        let path = "/tmp/mirzam-lsp-complete-pane.md";
        let mut server = Server::default();
        opened(&mut server, path, deck);
        // The cursor sits right after `::: pane ` on the last line.
        let result = request(&mut server, "textDocument/completion", path, 10, 9);
        assert_eq!(labels(&result), ["main", "fig"]);
    }

    #[test]
    fn a_slide_theme_comment_completes_the_theme_names() {
        let deck = "---\ntitle: T\ntheme: [mirzam, themes/acme.css]\n---\n\n<!-- theme: \n";
        let path = "/tmp/mirzam-lsp-complete-theme.md";
        let mut server = Server::default();
        opened(&mut server, path, deck);
        let result = request(&mut server, "textDocument/completion", path, 5, 12);
        let names = labels(&result);
        assert!(names.contains(&"nord".to_string()), "{names:?}");
        // The deck's own stylesheet registers under its stem, and completes.
        assert!(names.contains(&"acme".to_string()), "{names:?}");
    }

    #[test]
    fn a_citation_completes_the_bibliography_keys() {
        let deck = "---\ntitle: T\nbibliography:\n  vaswani2017:\n    title: Attention\n    author: Vaswani\n    year: \"2017\"\n---\n\nAs shown [@\n";
        let path = "/tmp/mirzam-lsp-complete-cite.md";
        let mut server = Server::default();
        opened(&mut server, path, deck);
        let result = request(&mut server, "textDocument/completion", path, 9, 11);
        assert_eq!(labels(&result), ["vaswani2017"]);
    }

    #[test]
    fn a_variable_completes_from_vars_and_an_open_comment_offers_the_settings() {
        let deck = "---\ntitle: T\nvars:\n  price: 1200\n---\n\nIt costs {{\n\n<!-- \n";
        let path = "/tmp/mirzam-lsp-complete-var.md";
        let mut server = Server::default();
        opened(&mut server, path, deck);
        let result = request(&mut server, "textDocument/completion", path, 6, 11);
        assert_eq!(labels(&result), ["price"]);
        let result = request(&mut server, "textDocument/completion", path, 8, 5);
        assert!(labels(&result).contains(&"note: ".to_string()));
    }

    #[test]
    fn an_opening_fence_completes_the_block_kinds() {
        let path = "/tmp/mirzam-lsp-complete-fence.md";
        let mut server = Server::default();
        opened(&mut server, path, "---\ntitle: T\n---\n\n```\n");
        let result = request(&mut server, "textDocument/completion", path, 4, 3);
        let names = labels(&result);
        assert!(names.contains(&"each".to_string()), "{names:?}");
        assert!(names.contains(&"chart".to_string()));
    }

    #[test]
    fn hovering_a_citation_shows_the_entry_and_a_variable_its_value() {
        let deck = "---\ntitle: T\nvars:\n  price: 1200\nbibliography:\n  vaswani2017:\n    title: Attention Is All You Need\n    author: Vaswani\n    year: \"2017\"\n---\n\nShown [@vaswani2017] at {{ price * 2 }} yen.\n";
        let path = "/tmp/mirzam-lsp-hover.md";
        let mut server = Server::default();
        opened(&mut server, path, deck);
        // Over the citation key.
        let hover = request(&mut server, "textDocument/hover", path, 11, 10);
        let text = hover["contents"]["value"].as_str().unwrap_or("");
        assert!(text.contains("Attention Is All You Need"), "{text}");
        assert!(text.contains("Vaswani"));
        // Over the expression: the evaluated value.
        let hover = request(&mut server, "textDocument/hover", path, 11, 28);
        assert_eq!(hover["contents"]["value"], "= 2400");
        // Over plain prose: nothing.
        let hover = request(&mut server, "textDocument/hover", path, 11, 0);
        assert_eq!(hover, Value::Null);
    }

    #[test]
    fn definition_of_an_include_is_the_file_it_pastes_in() {
        let dir = std::env::temp_dir().join(format!("mirzam-lsp-def-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("part.md"), "# Part\n").expect("part");
        let deck_path = dir.join("deck.md");
        let deck = "---\ntitle: T\n---\n\n![[part.md]]\n";
        std::fs::write(&deck_path, deck).expect("deck");
        let mut server = Server::default();
        let path = deck_path.to_string_lossy().to_string();
        opened(&mut server, &path, deck);
        let target = request(&mut server, "textDocument/definition", &path, 4, 5);
        let uri = target["uri"].as_str().unwrap_or("");
        assert!(uri.ends_with("part.md"), "{target}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn definition_of_a_citation_is_its_bibtex_entry() {
        let dir = std::env::temp_dir().join(format!("mirzam-lsp-bib-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join("refs.bib"),
            "@article{vaswani2017,\n  title={Attention},\n}\n",
        )
        .expect("bib");
        let deck_path = dir.join("deck.md");
        let deck = "---\ntitle: T\nbibliography: refs.bib\n---\n\nShown [@vaswani2017].\n";
        std::fs::write(&deck_path, deck).expect("deck");
        let mut server = Server::default();
        let path = deck_path.to_string_lossy().to_string();
        opened(&mut server, &path, deck);
        let target = request(&mut server, "textDocument/definition", &path, 5, 10);
        assert!(target["uri"].as_str().unwrap_or("").ends_with("refs.bib"));
        // The range lands on the key, which is on the first line.
        assert_eq!(target["range"]["start"]["line"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_position_round_trips_through_utf16_columns() {
        let text = "日本語 {{x}}\n";
        let at = offset_of(text, 0, 4);
        assert_eq!(&text[at..at + 2], "{{");
        assert_eq!(position(text, at), json!({ "line": 0, "character": 4 }));
        // Past the end of the line clamps to its end.
        assert_eq!(offset_of(text, 0, 99), text.len() - 1);
        assert_eq!(offset_of(text, 9, 0), text.len());
    }
}
