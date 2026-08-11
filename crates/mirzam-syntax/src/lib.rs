//! Structural decomposition of the source text.
//!
//! - Splitting off frontmatter
//! - `![[file.md]]` transclusion, with cycle detection
//! - Slide splitting on `---` (ignored inside code fences)
//! - Per-slide extraction: the `pane` layout block, `::: pane` divs,
//!   `<!-- note: -->` comments, and the shape/connect/anim fenced blocks
//! - A [`SourceMap`] from the expanded document back to the files it was
//!   assembled from, so an offset in the deck names a place someone can edit

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Abstraction over file reads; WASM hosts inject their own implementation.
pub trait FileProvider {
    fn read(&self, path: &Path) -> Result<String, String>;
}

/// Default implementation backed by `std::fs`.
pub struct FsProvider;

impl FileProvider for FsProvider {
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    }
}

/// Splits YAML frontmatter from the body.
pub fn split_frontmatter(src: &str) -> (Option<&str>, &str) {
    let src_norm = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lines = src_norm.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, src_norm);
    }
    // Find the closing `---`.
    let after_first = &src_norm[src_norm.find('\n').map(|i| i + 1).unwrap_or(src_norm.len())..];
    let mut offset = 0usize;
    for line in after_first.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let yaml = &after_first[..offset];
            let body = &after_first[offset + line.len()..];
            return (Some(yaml), body);
        }
        offset += line.len();
    }
    (None, src_norm)
}

/// Recursively expands `![[path]]`. Cycles are replaced with an error note.
pub fn expand_includes(body: &str, base_dir: &Path, provider: &dyn FileProvider) -> String {
    let mut files = BTreeSet::new();
    expand_includes_tracked(body, base_dir, provider, &mut files)
}

/// Same as `expand_includes`, but records every file that was read into
/// `files` so `serve` knows what to watch.
pub fn expand_includes_tracked(
    body: &str,
    base_dir: &Path,
    provider: &dyn FileProvider,
    files: &mut BTreeSet<PathBuf>,
) -> String {
    expand_includes_mapped(body, 0, Path::new(""), base_dir, provider, files).0
}

/// Expands includes and records where every byte of the result came from.
///
/// The tracker above knows *which* files a deck was built from; this knows
/// *where*, which is what turns an offset in the expanded document back into
/// a place a person can edit. `root` is the file `body` was read from and
/// `body_offset` is where `body` starts inside it — frontmatter has usually
/// been split off by the time this is called, and the caller is the only one
/// who knows how much.
pub fn expand_includes_mapped(
    body: &str,
    body_offset: usize,
    root: &Path,
    base_dir: &Path,
    provider: &dyn FileProvider,
    files: &mut BTreeSet<PathBuf>,
) -> (String, SourceMap) {
    let mut out = String::with_capacity(body.len());
    let mut map = SourceMap::default();
    let mut visited = BTreeSet::new();
    let root = map.intern(root);
    expand_includes_inner(
        body,
        body_offset,
        root,
        base_dir,
        provider,
        &mut visited,
        files,
        &mut out,
        &mut map,
    );
    (out, map)
}

#[allow(clippy::too_many_arguments)]
fn expand_includes_inner(
    body: &str,
    body_offset: usize,
    file: usize,
    base_dir: &Path,
    provider: &dyn FileProvider,
    visited: &mut BTreeSet<PathBuf>,
    files: &mut BTreeSet<PathBuf>,
    out: &mut String,
    map: &mut SourceMap,
) {
    let mut fence: Option<usize> = None;
    let mut pos = 0usize;
    for raw in body.split_inclusive('\n') {
        // The same line `lines()` would have yielded: no terminator, and no
        // stray `\r` from a CRLF file.
        let mut line = raw;
        if let Some(t) = line.strip_suffix('\n') {
            line = t;
        }
        if let Some(t) = line.strip_suffix('\r') {
            line = t;
        }
        let src = body_offset + pos..body_offset + pos + raw.len();
        pos += raw.len();

        let trimmed = line.trim();
        match fence {
            Some(open) if closes_fence(trimmed, open) => fence = None,
            None => {
                if let Some(n) = fence_len(trimmed) {
                    fence = Some(n);
                }
            }
            _ => {}
        }
        let emit = |out: &mut String, map: &mut SourceMap| {
            let start = out.len();
            out.push_str(line);
            out.push('\n');
            map.push(start..out.len(), file, src.clone());
        };
        if fence.is_some() || closes_fence(trimmed, 3) {
            emit(out, map);
            continue;
        }
        if let Some(target) = parse_include_line(trimmed) {
            let path = base_dir.join(target);
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if visited.contains(&canon) {
                // Generated text: it belongs to no file, so it gets no span.
                out.push_str(&format!(
                    "> ⚠ circular include, not expanded: `{}`\n",
                    target
                ));
                continue;
            }
            match provider.read(&path) {
                Ok(content) => {
                    visited.insert(canon.clone());
                    files.insert(path.clone());
                    // Frontmatter in included files is ignored — but its
                    // length still counts, or every offset in the child would
                    // be short by it.
                    let (_, child_body) = split_frontmatter(&content);
                    let child_offset = content.len() - child_body.len();
                    let child_dir = path.parent().unwrap_or(base_dir).to_path_buf();
                    let child = map.intern(&path);
                    expand_includes_inner(
                        child_body,
                        child_offset,
                        child,
                        &child_dir,
                        provider,
                        visited,
                        files,
                        out,
                        map,
                    );
                    visited.remove(&canon);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Err(e) => {
                    out.push_str(&format!("> ⚠ include failed: {e}\n"));
                }
            }
            continue;
        }
        emit(out, map);
    }
}

// ---- Source map ----

/// One run of the expanded document copied verbatim from a source file.
///
/// `out.len() == src.len()` for the ordinary case, and the mapping inside the
/// run is then byte-exact. A CRLF line is the exception — the expansion emits
/// `\n` where the file has `\r\n` — so such a line becomes a run of its own
/// rather than being merged into its neighbours and dragging the offsets of
/// everything after it out of place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub out: std::ops::Range<usize>,
    pub src: std::ops::Range<usize>,
    file: usize,
}

/// Where each byte of an expanded document came from.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<PathBuf>,
    spans: Vec<Span>,
}

impl SourceMap {
    fn intern(&mut self, path: &Path) -> usize {
        match self.files.iter().position(|p| p == path) {
            Some(i) => i,
            None => {
                self.files.push(path.to_path_buf());
                self.files.len() - 1
            }
        }
    }

    fn push(&mut self, out: std::ops::Range<usize>, file: usize, src: std::ops::Range<usize>) {
        // Merge with the previous run when both are byte-exact and the two are
        // adjacent on each side; a file included twice, or a CRLF line, breaks
        // the run instead of being folded into a wrong offset.
        if let Some(last) = self.spans.last_mut() {
            let exact = last.out.len() == last.src.len() && out.len() == src.len();
            if exact && last.file == file && last.out.end == out.start && last.src.end == src.start
            {
                last.out.end = out.end;
                last.src.end = src.end;
                return;
            }
        }
        self.spans.push(Span { out, src, file });
    }

    /// Every file that contributed to the document, in the order first seen.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Records, into the map of a *derived* document, that its bytes `out` are
    /// a verbatim copy of bytes `from` of this document.
    ///
    /// A later pass may rewrite the expanded text — variable substitution does
    /// — and a line it changed no longer corresponds to anything a person can
    /// edit. Such a line is simply not carried, so it resolves to nothing
    /// rather than to a plausible wrong place.
    pub fn carry(
        &self,
        out: std::ops::Range<usize>,
        from: std::ops::Range<usize>,
        into: &mut SourceMap,
    ) {
        if out.len() != from.len() {
            return;
        }
        let Some((file, src)) = self.resolve(from) else {
            return;
        };
        let file = file.to_path_buf();
        let f = into.intern(&file);
        into.push(out, f, src);
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The span covering `offset`, by binary search rather than a scan.
    fn span_at(&self, offset: usize) -> Option<usize> {
        let i = self
            .spans
            .partition_point(|s| s.out.start <= offset)
            .checked_sub(1)?;
        (offset < self.spans[i].out.end).then_some(i)
    }

    /// Which file, and where in it, a byte of the expanded document came from.
    /// `None` for text the expansion generated itself, such as the note left
    /// in place of a circular include.
    pub fn lookup(&self, offset: usize) -> Option<(&Path, usize)> {
        let s = &self.spans[self.span_at(offset)?];
        let within = (offset - s.out.start).min(s.src.len());
        Some((&self.files[s.file], s.src.start + within))
    }

    /// The byte range in the original file that produced `range`.
    ///
    /// `None` when the range covers generated text or crosses a file boundary:
    /// a caller about to rewrite those bytes must be refused rather than
    /// handed a range that means something else.
    pub fn resolve(
        &self,
        range: std::ops::Range<usize>,
    ) -> Option<(&Path, std::ops::Range<usize>)> {
        if range.start >= range.end {
            let (f, at) = self.lookup(range.start)?;
            return Some((f, at..at));
        }
        let lo = self.span_at(range.start)?;
        let hi = self.span_at(range.end - 1)?;
        // Every byte between them must be mapped, and mapped from the same
        // file in order, or the range covers something this map cannot speak
        // for — generated text, or a second file spliced into the middle.
        for pair in self.spans[lo..=hi].windows(2) {
            if pair[0].out.end != pair[1].out.start
                || pair[0].file != pair[1].file
                || pair[0].src.end != pair[1].src.start
            {
                return None;
            }
        }
        let (first, last) = (&self.spans[lo], &self.spans[hi]);
        let start = first.src.start + (range.start - first.out.start).min(first.src.len());
        let end = last.src.start + (range.end - last.out.start).min(last.src.len());
        (start <= end).then(|| (self.files[first.file].as_path(), start..end))
    }
}

/// Number of leading backticks when a line opens or closes a fence.
/// CommonMark lets a longer fence contain shorter ones, which is how a document
/// shows Mirzam syntax without the inner blocks being taken literally.
pub fn fence_len(trimmed: &str) -> Option<usize> {
    let n = trimmed.chars().take_while(|c| *c == '`').count();
    (n >= 3).then_some(n)
}

/// Whether `trimmed` closes a fence opened with `open` backticks.
fn closes_fence(trimmed: &str, open: usize) -> bool {
    match fence_len(trimmed) {
        Some(n) => n >= open && trimmed.chars().all(|c| c == '`'),
        None => false,
    }
}

/// Returns the target when the whole line is `![[...]]`.
fn parse_include_line(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("![[")?.strip_suffix("]]")?;
    if inner.is_empty() || inner.contains("[[") {
        return None;
    }
    Some(inner.trim())
}

/// A slide, and where its text starts in the document it was split from.
///
/// The expansion emits `\n` line endings throughout, so a slide's text is
/// exactly `body[start..start + text.len()]` — which is what lets a range
/// inside a slide be carried back through [`SourceMap`] to a file.
#[derive(Debug, Clone)]
pub struct SlideSpan {
    pub text: String,
    pub start: usize,
}

/// Splits the body into slides on `---` lines outside code fences.
pub fn split_slides(body: &str) -> Vec<String> {
    split_slides_at(body, None)
}

/// [`split_slides_at`], keeping each slide's offset in `body`.
pub fn split_slides_spanned(body: &str, level: Option<u8>) -> Vec<SlideSpan> {
    let mut slides: Vec<SlideSpan> = Vec::new();
    let mut current = String::new();
    let mut start = 0usize;
    let mut fence: Option<usize> = None;
    let mut pos = 0usize;
    let mut flush = |current: &mut String, start: &mut usize, next: usize| {
        slides.push(SlideSpan {
            text: std::mem::take(current),
            start: *start,
        });
        *start = next;
    };
    for raw in body.split_inclusive('\n') {
        let mut line = raw;
        if let Some(t) = line.strip_suffix('\n') {
            line = t;
        }
        if let Some(t) = line.strip_suffix('\r') {
            line = t;
        }
        let here = pos;
        pos += raw.len();

        let trimmed = line.trim();
        match fence {
            Some(open) if closes_fence(trimmed, open) => fence = None,
            None => {
                if let Some(n) = fence_len(trimmed) {
                    fence = Some(n);
                }
            }
            _ => {}
        }
        let in_code = fence.is_some();
        if !in_code && is_slide_break(trimmed) {
            flush(&mut current, &mut start, pos);
            continue;
        }
        if !in_code && level.is_some_and(|l| heading_level(trimmed) == Some(l)) {
            // The first heading opens the deck rather than an empty slide.
            if !current.trim().is_empty() {
                flush(&mut current, &mut start, here);
            } else {
                start = here;
            }
        }
        current.push_str(line);
        current.push('\n');
    }
    slides.push(SlideSpan {
        text: current,
        start,
    });
    slides
        .into_iter()
        .filter(|s| !s.text.trim().is_empty())
        .collect()
}

/// Splits into slides on `---`, and additionally before every heading of
/// `level` when given. Heading splitting is what lets an ordinary document -
/// a README, a set of notes - become a deck without editing it.
pub fn split_slides_at(body: &str, level: Option<u8>) -> Vec<String> {
    split_slides_spanned(body, level)
        .into_iter()
        .map(|s| s.text)
        .collect()
}

/// The ATX heading level of a line, if it is one.
fn heading_level(trimmed: &str) -> Option<u8> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    (1..=6)
        .contains(&hashes)
        .then(|| trimmed[hashes..].starts_with(' ').then_some(hashes as u8))
        .flatten()
}

fn is_slide_break(trimmed: &str) -> bool {
    trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-')
}

/// Every fenced info string Mirzam gives a meaning to.
///
/// The list is here rather than beside each consumer because its whole purpose
/// is to be complete: `commonmark_compat.rs` walks it and proves that each one
/// still degrades to an ordinary code block in a plain CommonMark parser, which
/// is the promise the markup makes. Two of these (`chart`, `toc`) are consumed
/// by `mirzam-render` rather than here — they reach it through `loose`, exactly
/// as a plain parser would see them — so a list built from this file's match
/// arms alone would quietly under-report.
///
/// Adding a block form means adding it here, in the same change.
pub const BLOCK_KINDS: &[&str] = &[
    "pane", "shape", "connect", "annotate", "effects", "anim", "chart", "toc",
];

/// Fenced blocks reserved for a later phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Anim,
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Anim => "anim",
        }
    }

    fn from_info(info: &str) -> Option<Self> {
        match info {
            "anim" => Some(BlockKind::Anim),
            _ => None,
        }
    }
}

/// A content block assigned to a pane via `::: pane`.
#[derive(Debug, Clone, Default)]
pub struct PaneBlock {
    pub name: String,
    /// Contents of `{align=center valign=middle .cls}`; empty when omitted.
    pub attrs: String,
    pub body: String,
}

/// The decomposed structure of a single slide.
#[derive(Debug, Clone, Default)]
pub struct SlideSource {
    /// Body of the ```pane block (the ASCII grid).
    pub layout: Option<String>,
    /// Content assigned through `::: pane X`.
    pub panes: Vec<PaneBlock>,
    /// Markdown not assigned to any pane.
    pub loose: String,
    /// Speaker notes.
    pub notes: Vec<String>,
    /// `<!-- theme: nord -->`: the palette this one slide is drawn in,
    /// whatever the deck's own is. `None` means it inherits the deck's.
    pub theme: Option<String>,
    /// `<!-- mode: dark -->`: light or dark for this one slide, independent of
    /// the deck and of the reader's `D` key.
    pub mode: Option<String>,
    /// ```shape blocks; multiple blocks are concatenated.
    pub shapes: Vec<String>,
    /// ```connect blocks.
    pub connects: Vec<String>,
    /// ```annotate blocks.
    pub annots: Vec<String>,
    /// ```effects blocks.
    pub effects: Vec<String>,
    /// Blocks reserved for a later phase.
    pub reserved: Vec<(BlockKind, String)>,
    /// Where each fenced block sits in the slide's own text. Carrying the
    /// ranges here is what lets an edit made in the preview be written back to
    /// the line of Markdown it came from, via [`SourceMap`].
    pub blocks: Vec<BlockSpan>,
}

/// A fenced block's place in the slide text it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSpan {
    /// The fence's info string: `pane`, `annotate`, `chart`, …
    pub info: String,
    /// The content between the fences.
    pub body: std::ops::Range<usize>,
    /// The whole block, fences included.
    pub whole: std::ops::Range<usize>,
}

/// Lines of `src` with the byte offset each one starts at, terminators
/// stripped the way `lines()` strips them.
fn line_offsets(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut pos = 0;
    for raw in src.split_inclusive('\n') {
        let mut line = raw;
        if let Some(t) = line.strip_suffix('\n') {
            line = t;
        }
        if let Some(t) = line.strip_suffix('\r') {
            line = t;
        }
        out.push((pos, line));
        pos += raw.len();
    }
    out
}

/// The author-chosen break inside a pane: `<!-- next -->`.
///
/// An HTML comment, so a plain Markdown parser shows nothing at all — the same
/// reason speaker notes are written that way.
fn is_continue_marker(trimmed: &str) -> bool {
    let Some(inner) = trimmed.strip_prefix("<!--") else {
        return false;
    };
    matches!(
        inner.strip_suffix("-->").unwrap_or(inner).trim(),
        "next" | "more"
    )
}

/// Splits a slide at `<!-- next -->` into the slides it stands for.
///
/// A slide with *n* markers in one pane becomes *n + 1* slides, identical
/// except for that pane — every other line is copied verbatim, so the panes
/// that did not break render to exactly the same HTML and the audience sees
/// them hold still. A slide with no marker yields itself, unchanged.
///
/// Doing this on the *text*, before the slide is parsed, is what keeps the rest
/// of the pipeline free of special cases: what comes out is ordinary slides,
/// and `anim`, `annotate`, `connect`, notes and the render cache never learn
/// that continuation exists.
///
/// `Err` when two panes both break: the result would be a cross product no
/// author can predict, so the caller reports it and renders the slide whole.
pub fn expand_continuations(slide: &str) -> Result<Vec<String>, String> {
    let lines = line_offsets(slide);
    // Which pane each line belongs to: `None` outside every `::: pane`.
    let mut owner: Vec<Option<String>> = vec![None; lines.len()];
    let mut current: Option<String> = None;
    let mut fence: Option<usize> = None;
    // A marker inside a fence is a marker being quoted, not one being used.
    let mut fenced = vec![false; lines.len()];
    for (i, (_, line)) in lines.iter().enumerate() {
        let t = line.trim();
        match fence {
            Some(open) if closes_fence(t, open) => fence = None,
            None => {
                if let Some(n) = fence_len(t) {
                    fence = Some(n);
                }
            }
            _ => {}
        }
        if fence.is_some() {
            owner[i] = current.clone();
            fenced[i] = true;
            continue;
        }
        if let Some(rest) = t.strip_prefix(":::") {
            let rest = rest.trim();
            if rest.is_empty() {
                current = None;
                continue;
            }
            if let Some((name, _)) = parse_pane_open(rest) {
                current = Some(name);
                continue;
            }
        }
        owner[i] = current.clone();
    }

    let marks: Vec<usize> = (0..lines.len())
        .filter(|i| !fenced[*i] && is_continue_marker(lines[*i].1.trim()))
        .collect();
    if marks.is_empty() {
        return Ok(vec![slide.to_string()]);
    }
    let mut panes: Vec<Option<String>> = marks.iter().map(|i| owner[*i].clone()).collect();
    panes.dedup();
    if panes.len() > 1 {
        let named = |p: &Option<String>| p.clone().unwrap_or_else(|| "the slide body".into());
        let names = panes.iter().map(named).collect::<Vec<_>>().join(", ");
        return Err(format!(
            "`<!-- next -->` appears in more than one pane ({names}); \
             only one pane may carry on to the next slide"
        ));
    }
    let region = panes.into_iter().next().unwrap_or(None);

    // Lines of the breaking region, cut into segments at each marker.
    let mut segments: Vec<Vec<usize>> = vec![Vec::new()];
    for (i, _) in lines.iter().enumerate() {
        if owner[i] != region {
            continue;
        }
        if marks.contains(&i) {
            segments.push(Vec::new());
        } else {
            segments
                .last_mut()
                .expect("one segment always exists")
                .push(i);
        }
    }

    let mut out = Vec::with_capacity(segments.len());
    for keep in &segments {
        let mut text = String::with_capacity(slide.len());
        for (i, (_, line)) in lines.iter().enumerate() {
            let mine = owner[i] == region;
            if marks.contains(&i) || (mine && !keep.contains(&i)) {
                continue;
            }
            text.push_str(line);
            text.push('\n');
        }
        out.push(text);
    }
    Ok(out)
}

/// Decomposes a slide's source into its parts.
pub fn parse_slide(src: &str) -> SlideSource {
    let mut slide = SlideSource::default();
    let lines = line_offsets(src);
    // The offset just past line `i`, which is where line `i + 1` begins.
    let after = |i: usize| lines.get(i + 1).map(|(o, _)| *o).unwrap_or(src.len());
    let mut i = 0;

    while i < lines.len() {
        let (line_at, line) = lines[i];
        i += 1;
        let trimmed = line.trim();

        // fenced code block
        if let Some(open) = fence_len(trimmed) {
            let info = trimmed[open..].trim();
            let body_at = after(i - 1);
            let mut body = String::new();
            let mut body_end = body_at;
            while i < lines.len() {
                let (at, inner) = lines[i];
                i += 1;
                if closes_fence(inner.trim(), open) {
                    body_end = at;
                    break;
                }
                body.push_str(inner);
                body.push('\n');
                body_end = after(i - 1);
            }
            slide.blocks.push(BlockSpan {
                info: info.to_string(),
                body: body_at..body_end,
                whole: line_at..after(i - 1),
            });
            // A longer fence quotes Mirzam syntax rather than using it.
            if open > 3 {
                slide.loose.push_str(&format!(
                    "{}{info}\n{body}{}\n",
                    "`".repeat(open),
                    "`".repeat(open)
                ));
                continue;
            }
            if info == "pane" || info.starts_with("pane ") {
                slide.layout = Some(body);
            } else if info == "shape" {
                slide.shapes.push(body);
            } else if info == "connect" {
                slide.connects.push(body);
            } else if info == "annotate" {
                slide.annots.push(body);
            } else if info == "effects" {
                slide.effects.push(body);
            } else if let Some(kind) = BlockKind::from_info(info) {
                slide.reserved.push((kind, body));
            } else {
                // Any other fence is an ordinary code block.
                slide.loose.push_str(&format!("```{info}\n{body}```\n"));
            }
            continue;
        }

        // fenced div: ::: pane NAME [{attrs}]
        if let Some(rest) = trimmed.strip_prefix(":::") {
            let rest = rest.trim();
            if let Some((pane_name, attrs)) = parse_pane_open(rest) {
                let mut body = String::new();
                let mut inner_fence: Option<usize> = None;
                while i < lines.len() {
                    let (_, inner) = lines[i];
                    i += 1;
                    let t = inner.trim();
                    match inner_fence {
                        Some(open) if closes_fence(t, open) => inner_fence = None,
                        None => {
                            if let Some(n) = fence_len(t) {
                                inner_fence = Some(n);
                            }
                        }
                        _ => {}
                    }
                    if inner_fence.is_none() && t == ":::" {
                        break;
                    }
                    body.push_str(inner);
                    body.push('\n');
                }
                slide.panes.push(PaneBlock {
                    name: pane_name,
                    attrs,
                    body,
                });
                continue;
            }
            // Other fenced divs (`::: note` etc.) stay in the body for now.
            slide.loose.push_str(line);
            slide.loose.push('\n');
            continue;
        }

        // HTML comments: speaker notes today, slide settings later.
        if trimmed.starts_with("<!--") {
            let mut comment = String::from(trimmed);
            while !comment.contains("-->") {
                if i >= lines.len() {
                    break;
                }
                comment.push('\n');
                comment.push_str(lines[i].1);
                i += 1;
            }
            if let Some(note) = parse_note_comment(&comment) {
                slide.notes.push(note);
                continue;
            }
            if let Some((key, value)) = parse_setting_comment(&comment) {
                match key {
                    "theme" => slide.theme = Some(value.to_string()),
                    "mode" => slide.mode = Some(value.to_string()),
                    _ => unreachable!("parse_setting_comment only returns known keys"),
                }
                continue;
            }
            // Non-note comments stay in the body, hidden as HTML comments.
            slide.loose.push_str(&comment);
            slide.loose.push('\n');
            continue;
        }

        slide.loose.push_str(line);
        slide.loose.push('\n');
    }

    slide
}

/// Extracts the pane name and attributes from a `pane NAME {attrs}` opener.
fn parse_pane_open(rest: &str) -> Option<(String, String)> {
    let rest = rest.strip_prefix("pane")?.trim();
    if rest.is_empty() {
        return None;
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if name.is_empty() {
        return None;
    }
    let after = rest[name.len()..].trim();
    let attrs = after
        .strip_prefix('{')
        .and_then(|a| a.strip_suffix('}'))
        .unwrap_or("")
        .to_string();
    Some((name, attrs))
}

/// A per-slide setting written as an HTML comment: `<!-- theme: nord -->`.
///
/// An HTML comment for the same reason a speaker note is one — a plain
/// CommonMark reader shows nothing at all, so a slide that dresses itself
/// differently still reads as ordinary Markdown on GitHub.
///
/// Only the keys listed here are settings; every other comment stays in the
/// body as a comment, so an author's `<!-- TODO: rewrite this -->` is not
/// quietly eaten by a parser looking for directives.
fn parse_setting_comment(comment: &str) -> Option<(&'static str, String)> {
    let inner = comment.strip_prefix("<!--")?;
    let inner = inner.strip_suffix("-->").unwrap_or(inner).trim();
    let (key, value) = inner.split_once(':')?;
    let key = key.trim().to_ascii_lowercase();
    let key = ["theme", "mode"].into_iter().find(|k| *k == key)?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some((key, value.to_string()))
}

fn parse_note_comment(comment: &str) -> Option<String> {
    let inner = comment.strip_prefix("<!--")?;
    let inner = inner.strip_suffix("-->").unwrap_or(inner);
    let inner = inner.trim();
    let note = inner.strip_prefix("note:")?;
    Some(note.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A filesystem in a map, so include tests need no temporary directory.
    struct Mem(BTreeMap<String, String>);

    impl Mem {
        fn new(files: &[(&str, &str)]) -> Self {
            Mem(files
                .iter()
                .map(|(p, c)| (p.to_string(), c.to_string()))
                .collect())
        }
    }

    impl FileProvider for Mem {
        fn read(&self, path: &Path) -> Result<String, String> {
            self.0
                .get(path.to_string_lossy().as_ref())
                .cloned()
                .ok_or_else(|| format!("cannot read {}", path.display()))
        }
    }

    fn mapped(body: &str, files: &[(&str, &str)]) -> (String, SourceMap) {
        let mut seen = BTreeSet::new();
        expand_includes_mapped(
            body,
            0,
            Path::new("deck.md"),
            Path::new(""),
            &Mem::new(files),
            &mut seen,
        )
    }

    /// What `lookup` says about the byte at `needle` inside the expanded text.
    fn source_of<'a>(out: &str, map: &'a SourceMap, needle: &str) -> (&'a Path, usize) {
        let at = out.find(needle).expect("needle is in the output");
        map.lookup(at).expect("that byte came from a file")
    }

    #[test]
    fn a_document_with_no_includes_maps_to_itself() {
        let body = "# Title\n\nbody text\n";
        let (out, map) = mapped(body, &[]);
        assert_eq!(out, body);
        // One run: nothing broke the 1:1 correspondence.
        assert_eq!(map.spans().len(), 1);
        for (i, _) in body.char_indices() {
            assert_eq!(map.lookup(i), Some((Path::new("deck.md"), i)), "byte {i}");
        }
    }

    #[test]
    fn an_included_file_maps_into_that_file() {
        let child = "## Section\n\nfrom the child\n";
        let (out, map) = mapped("intro\n\n![[part.md]]\n\nouter\n", &[("part.md", child)]);
        let (file, at) = source_of(&out, &map, "from the child");
        assert_eq!(file, Path::new("part.md"));
        assert_eq!(&child[at..at + 14], "from the child");
        // Text on either side still maps to the parent.
        assert_eq!(source_of(&out, &map, "intro").0, Path::new("deck.md"));
        assert_eq!(source_of(&out, &map, "outer").0, Path::new("deck.md"));
    }

    #[test]
    fn an_included_file_skips_its_own_frontmatter_but_not_its_offsets() {
        let child = "---\ntitle: ignored\n---\nreal content\n";
        let (out, map) = mapped("![[part.md]]\n", &[("part.md", child)]);
        assert!(!out.contains("ignored"));
        let (_, at) = source_of(&out, &map, "real content");
        assert_eq!(&child[at..at + 12], "real content");
    }

    #[test]
    fn nested_includes_map_to_the_innermost_file() {
        let (out, map) = mapped(
            "![[a.md]]\n",
            &[("a.md", "a says\n\n![[b.md]]\n"), ("b.md", "b says\n")],
        );
        assert_eq!(source_of(&out, &map, "a says").0, Path::new("a.md"));
        assert_eq!(source_of(&out, &map, "b says").0, Path::new("b.md"));
    }

    #[test]
    fn an_include_inside_a_fence_is_text_and_maps_to_the_parent() {
        let body = "```markdown\n![[part.md]]\n```\n";
        let (out, map) = mapped(body, &[("part.md", "expanded!\n")]);
        assert!(!out.contains("expanded!"), "{out}");
        assert_eq!(
            source_of(&out, &map, "![[part.md]]").0,
            Path::new("deck.md")
        );
    }

    #[test]
    fn a_file_included_twice_maps_each_copy_to_the_same_file() {
        let child = "shared\n";
        let (out, map) = mapped(
            "![[part.md]]\n\nmiddle\n\n![[part.md]]\n",
            &[("part.md", child)],
        );
        let first = out.find("shared").unwrap();
        let second = out[first + 1..].find("shared").unwrap() + first + 1;
        assert_ne!(first, second);
        // Both copies point at the one place the text actually lives.
        assert_eq!(map.lookup(first), Some((Path::new("part.md"), 0)));
        assert_eq!(map.lookup(second), Some((Path::new("part.md"), 0)));
    }

    #[test]
    fn crlf_line_endings_still_map_to_the_right_bytes() {
        let body = "alpha\r\nbeta\r\ngamma\r\n";
        let (out, map) = mapped(body, &[]);
        assert_eq!(out, "alpha\nbeta\ngamma\n");
        for word in ["alpha", "beta", "gamma"] {
            let (_, at) = source_of(&out, &map, word);
            assert_eq!(&body[at..at + word.len()], word, "{word}");
        }
        // A CRLF line cannot merge with its neighbours without dragging every
        // later offset out of place, so each one is its own run.
        assert_eq!(map.spans().len(), 3);
    }

    #[test]
    fn generated_text_belongs_to_no_file() {
        let (out, map) = mapped("![[missing.md]]\n", &[]);
        let at = out.find("include failed").unwrap();
        assert_eq!(map.lookup(at), None);
    }

    #[test]
    fn resolve_returns_the_range_a_block_occupies_in_its_own_file() {
        let child = "text\n\n```annotate\ntarget: fig\ncircle 40,30 20x20\n```\n";
        let (out, map) = mapped("![[part.md]]\n", &[("part.md", child)]);
        let start = out.find("target: fig").unwrap();
        let end = out.find("```\n").unwrap();
        let (file, range) = map.resolve(start..end).expect("one file, one range");
        assert_eq!(file, Path::new("part.md"));
        assert_eq!(&child[range], "target: fig\ncircle 40,30 20x20\n");
    }

    #[test]
    fn resolve_refuses_a_range_that_crosses_two_files() {
        let (out, map) = mapped(
            "![[a.md]]\n![[b.md]]\n",
            &[("a.md", "aaa\n"), ("b.md", "bbb\n")],
        );
        let start = out.find("aaa").unwrap();
        let end = out.find("bbb").unwrap() + 3;
        assert_eq!(map.resolve(start..end), None);
    }

    #[test]
    fn slide_spans_index_back_into_the_document() {
        let body = "one\n\n---\n\ntwo\n\n---\n\nthree\n";
        for slide in split_slides_spanned(body, None) {
            assert_eq!(
                &body[slide.start..slide.start + slide.text.len()],
                slide.text,
                "slide text is not the bytes it claims"
            );
        }
    }

    #[test]
    fn heading_split_slide_spans_start_at_the_heading() {
        let body = "# Title\n\nintro\n\n## A\n\nbody a\n";
        let slides = split_slides_spanned(body, Some(2));
        assert_eq!(slides.len(), 2);
        assert_eq!(&body[slides[1].start..slides[1].start + 4], "## A");
        for slide in &slides {
            assert_eq!(
                &body[slide.start..slide.start + slide.text.len()],
                slide.text
            );
        }
    }

    #[test]
    fn block_spans_cover_the_fence_and_its_contents() {
        let src = "intro\n\n```annotate\ntarget: fig\ncircle 1,2 3x4\n```\n\ntail\n";
        let slide = parse_slide(src);
        let block = slide
            .blocks
            .iter()
            .find(|b| b.info == "annotate")
            .expect("the block was recorded");
        assert_eq!(&src[block.body.clone()], "target: fig\ncircle 1,2 3x4\n");
        assert!(src[block.whole.clone()].starts_with("```annotate"));
        assert!(src[block.whole.clone()].ends_with("```\n"));
        // The recorded body is exactly what the parser handed the renderer.
        assert_eq!(slide.annots[0], src[block.body.clone()]);
    }

    #[test]
    fn a_block_range_survives_the_trip_back_to_its_file() {
        // The whole point of this stream: preview -> slide -> document -> file.
        let child = "## Figure\n\n```annotate\ntarget: fig\ncircle 40,30 20x20\n```\n";
        let (doc, map) = mapped("intro\n\n---\n\n![[part.md]]\n", &[("part.md", child)]);
        let slide = split_slides_spanned(&doc, None)
            .into_iter()
            .find(|s| s.text.contains("annotate"))
            .expect("the slide with the block");
        let parsed = parse_slide(&slide.text);
        let block = parsed.blocks.iter().find(|b| b.info == "annotate").unwrap();
        let in_doc = slide.start + block.body.start..slide.start + block.body.end;
        let (file, range) = map.resolve(in_doc).expect("resolves to one file");
        assert_eq!(file, Path::new("part.md"));
        assert_eq!(&child[range], "target: fig\ncircle 40,30 20x20\n");
    }

    #[test]
    fn frontmatter_split() {
        let (fm, body) = split_frontmatter("---\ntitle: x\n---\nbody\n");
        assert_eq!(fm, Some("title: x\n"));
        assert_eq!(body, "body\n");
    }

    #[test]
    fn no_frontmatter() {
        let (fm, body) = split_frontmatter("# hello\n");
        assert_eq!(fm, None);
        assert_eq!(body, "# hello\n");
    }

    #[test]
    fn heading_split_starts_a_slide_per_heading() {
        let body = "# Title\n\nintro\n\n## A\n\nbody a\n\n## B\n\nbody b\n";
        let slides = split_slides_at(body, Some(2));
        assert_eq!(slides.len(), 3);
        assert!(slides[0].contains("# Title") && slides[0].contains("intro"));
        assert!(slides[1].starts_with("## A"));
        assert!(slides[2].starts_with("## B"));
    }

    #[test]
    fn heading_split_ignores_other_levels_and_fences() {
        let body = "## A\n\n### sub\n\n```\n## not a heading\n```\n\n## B\n";
        let slides = split_slides_at(body, Some(2));
        assert_eq!(slides.len(), 2);
        assert!(slides[0].contains("### sub"));
        assert!(slides[0].contains("## not a heading"));
    }

    #[test]
    fn heading_level_recognises_atx_only() {
        assert_eq!(heading_level("## x"), Some(2));
        assert_eq!(heading_level("######## x"), None);
        assert_eq!(heading_level("##x"), None);
        assert_eq!(heading_level("text"), None);
    }

    #[test]
    fn slide_split_ignores_fences() {
        let body = "a\n---\nb\n```\n---\n```\nc\n";
        let slides = split_slides(body);
        assert_eq!(slides.len(), 2);
        assert!(slides[1].contains("```\n---\n```"));
    }

    #[test]
    fn parse_slide_structure() {
        let src = "\
## Title

```pane
+---+---+
| a | b |
+---+---+
```

::: pane a
hello **world**
:::

loose text

```connect
#x -> #y
```

<!-- note: remember this -->
";
        let s = parse_slide(src);
        assert!(s.layout.is_some());
        assert_eq!(s.panes.len(), 1);
        assert_eq!(s.panes[0].name, "a");
        assert!(s.panes[0].body.contains("**world**"));
        assert!(s.loose.contains("## Title"));
        assert!(s.loose.contains("loose text"));
        assert_eq!(s.connects.len(), 1);
        assert!(s.connects[0].contains("#x -> #y"));
        assert_eq!(s.notes, vec!["remember this"]);
    }

    #[test]
    fn a_slide_can_set_its_own_theme_and_mode() {
        let s = parse_slide("<!-- theme: wuwei -->\n<!-- mode: dark -->\n\n# Quiet\n");
        assert_eq!(s.theme.as_deref(), Some("wuwei"));
        assert_eq!(s.mode.as_deref(), Some("dark"));
        // The setting is a comment, so nothing of it survives into the body.
        assert!(!s.loose.contains("theme"));
        assert!(s.loose.contains("# Quiet"));
    }

    /// Every other comment is the author's, and must come out the far side
    /// unread — a directive parser that swallows `<!-- TODO -->` loses work.
    #[test]
    fn a_comment_that_is_not_a_setting_stays_in_the_body() {
        let s = parse_slide("<!-- TODO: rewrite this -->\n<!-- theme: -->\n\ntext\n");
        assert_eq!(s.theme, None);
        assert!(s.loose.contains("<!-- TODO: rewrite this -->"));
        assert!(s.loose.contains("<!-- theme: -->"));
    }

    #[test]
    fn annotate_block_is_collected_at_slide_level() {
        let s = parse_slide(
            "::: pane fig\n![x](x.png)\n:::\n\n```annotate\ntarget: fig\ncircle 40,30 20x20\n```\n",
        );
        assert_eq!(s.annots.len(), 1);
        assert!(s.annots[0].contains("target: fig"));
        assert!(!s.loose.contains("annotate"));
    }

    #[test]
    fn longer_fence_quotes_mirzam_syntax() {
        // A document that *shows* Mirzam syntax wraps it in a longer fence; the
        // inner blocks must stay text rather than being executed.
        let src =
            "````markdown\n```pane\n+---+\n| a |\n+---+\n```\n\n```connect\n#a -> #b\n```\n````\n";
        let s = parse_slide(src);
        assert!(s.layout.is_none(), "inner pane block must not be applied");
        assert!(
            s.connects.is_empty(),
            "inner connect block must not be applied"
        );
        assert!(s.loose.contains("```pane"));
        assert!(s.loose.contains("#a -> #b"));
    }

    #[test]
    fn longer_fence_hides_slide_breaks() {
        let body = "a\n\n````\n---\n````\n\nb\n";
        assert_eq!(split_slides(body).len(), 1);
    }

    #[test]
    fn code_fence_inside_pane_div() {
        let src = "::: pane main\n```rust\nlet x = 1;\n```\n:::\n";
        let s = parse_slide(src);
        assert_eq!(s.panes.len(), 1);
        assert!(s.panes[0].body.contains("let x = 1;"));
    }

    #[test]
    fn include_line_parse() {
        assert_eq!(parse_include_line("![[a/b.md]]"), Some("a/b.md"));
        assert_eq!(parse_include_line("![](x.png)"), None);
        assert_eq!(parse_include_line("text ![[a.md]]"), None);
    }

    /// A slide holding a layout, a still pane and a `main` pane broken into
    /// `parts` segments. `parts` are joined by the marker.
    fn continued(parts: &[&str]) -> String {
        format!(
            "```pane\n+---+---+\n| a | b |\n+---+---+\n```\n\n\
             ::: pane a\nstill\n:::\n\n\
             ::: pane b\n{}\n:::\n",
            parts.join("\n<!-- next -->\n")
        )
    }

    #[test]
    fn a_slide_with_no_marker_is_left_alone() {
        let src = continued(&["one"]);
        assert_eq!(expand_continuations(&src), Ok(vec![src.clone()]));
    }

    #[test]
    fn each_marker_adds_a_slide() {
        let out = expand_continuations(&continued(&["one", "two", "three"])).unwrap();
        assert_eq!(out.len(), 3);
        for (i, want) in ["one", "two", "three"].iter().enumerate() {
            let body = parse_slide(&out[i]);
            let b = body
                .panes
                .iter()
                .find(|p| p.name == "b")
                .expect("the broken pane survives");
            assert_eq!(b.body.trim(), *want, "part {i}");
        }
        // No marker reaches the rendered text.
        assert!(out.iter().all(|s| !s.contains("<!-- next -->")));
    }

    #[test]
    fn the_panes_that_did_not_break_are_byte_identical() {
        let out = expand_continuations(&continued(&["one", "two"])).unwrap();
        let others = |s: &str| {
            let p = parse_slide(s);
            (
                p.layout.clone(),
                p.panes
                    .iter()
                    .filter(|p| p.name != "b")
                    .map(|p| (p.name.clone(), p.body.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(others(&out[0]), others(&out[1]));
    }

    #[test]
    fn two_panes_breaking_at_once_is_an_error() {
        let src = "::: pane a\none\n<!-- next -->\ntwo\n:::\n\
                   ::: pane b\nthree\n<!-- next -->\nfour\n:::\n";
        let err = expand_continuations(src).expect_err("a cross product is refused");
        assert!(err.contains('a') && err.contains('b'), "{err}");
    }

    #[test]
    fn a_marker_outside_every_pane_breaks_the_slide_body() {
        let out = expand_continuations("one\n\n<!-- next -->\n\ntwo\n").unwrap();
        assert_eq!(out.len(), 2);
        assert!(out[0].contains("one") && !out[0].contains("two"));
        assert!(out[1].contains("two") && !out[1].contains("one"));
    }

    #[test]
    fn a_marker_inside_a_code_fence_is_just_text() {
        let src = "::: pane a\n```text\n<!-- next -->\n```\n:::\n";
        assert_eq!(expand_continuations(src).unwrap().len(), 1);
    }

    #[test]
    fn more_is_accepted_as_a_spelling_of_next() {
        assert!(is_continue_marker("<!-- more -->"));
        assert!(is_continue_marker("<!--next-->"));
        assert!(!is_continue_marker("<!-- next slide -->"));
        assert!(!is_continue_marker("next"));
    }
    /// [`BLOCK_KINDS`] is only worth walking if every name on it is live. A
    /// block form that no longer exists would make the compatibility test pass
    /// for a promise nobody is asking about any more.
    ///
    /// `chart` and `toc` are the two the renderer consumes: here they must stay
    /// in `loose`, which is how they reach it.
    #[test]
    fn every_listed_block_kind_is_one_something_consumes() {
        for kind in BLOCK_KINDS {
            let src = format!("```{kind}\nbody\n```\n");
            let s = parse_slide(&src);
            assert!(
                s.blocks.iter().any(|b| b.info == *kind),
                "`{kind}` was not recorded as a block"
            );
            let claimed = s.layout.is_some()
                || !s.shapes.is_empty()
                || !s.connects.is_empty()
                || !s.annots.is_empty()
                || !s.effects.is_empty()
                || !s.reserved.is_empty();
            if matches!(*kind, "chart" | "toc") {
                assert!(!claimed, "`{kind}` is the renderer's, not ours");
                assert!(
                    s.loose.contains(&format!("```{kind}")),
                    "`{kind}` must pass through to the renderer intact"
                );
            } else {
                assert!(claimed, "`{kind}` fell through to loose text");
            }
        }
    }
}
