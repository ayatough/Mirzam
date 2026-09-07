//! The cut-out a hand-written quote asks for, made by the build.
//!
//! The block a person writes to quote a paper beside a phrase is two lines:
//!
//! ```text
//! source: @devi2022
//! highlight #lim : quote="Outside the range over which the coefficients were fitted, …"
//! ```
//!
//! The core renders that as it stands - a chip on the phrase that opens a
//! card holding the words and the source - and it is what the editor's
//! preview shows, since the editor cannot open a PDF. A build can. Before a
//! slide is parsed, this module opens the paper the block names, finds the
//! words (on `page=` when it is written, on every page when it is not), cuts
//! the column out around them, converts the cut to an SVG, marks the lines
//! the words are on, and rewrites the block into the form `mirzam import pdf
//! --quote` prints: a `target:` naming the picture and one mark per line,
//! placed in percent. From there the renderer does what it does for any card.
//!
//! The picture goes under [`DIR`] beside the deck, named for the paper and
//! for what was asked - so the same quote makes the same file, a build finds
//! it there and opens no PDF, and a deck checked into version control keeps
//! its cut-outs out of the tree. The file carries its own marks in a comment
//! on its first line, which is what lets the next build skip the paper. A
//! paper newer than its cut-out is cut again.
//!
//! What cannot be cut - a source with no PDF on this machine, words the paper
//! does not print - is a warning, and the block stays as written: the chip
//! still opens on the words, only without the page behind them.

use crate::pdfimport;
use crate::pdfpage;
use crate::quotes::{self, Miss};
use lopdf::Document;
use mirzam_annot::{Kind, Place};
use mirzam_cite::Bibliography;
use mirzam_figure::quote::Mark;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// Where a deck's cut-outs are written, relative to the deck's directory.
pub const DIR: &str = ".mirzam/cutouts";

/// Lines of the page kept above and below the passage, so it is read where it
/// stands. The same number `import pdf --quote` keeps by default.
const CONTEXT: usize = 2;

/// The first line of a cut-out: what it is a picture of, for the build that
/// finds it already written.
const HEADER: &str = "<!--mirzam cut-out:";

/// The passage as cut: where the picture is, which page, and the marks.
struct Cut {
    /// The picture's path relative to the deck, as a `target:` writes it.
    target: String,
    page: u32,
    marks: Vec<Mark>,
}

/// One build's view of the papers a deck quotes. A paper is opened once per
/// build however many slides quote it, and not at all when every cut-out it
/// is asked for is already on disk.
pub struct Resolver<'a> {
    deck_dir: PathBuf,
    bib: &'a Bibliography,
    /// Where a bibliography entry's `file` field is relative to.
    bib_dir: PathBuf,
    papers: HashMap<PathBuf, Result<Document, String>>,
}

impl<'a> Resolver<'a> {
    pub fn new(deck_dir: &Path, bib: &'a Bibliography, bib_dir: PathBuf) -> Resolver<'a> {
        Resolver {
            deck_dir: deck_dir.to_path_buf(),
            bib,
            bib_dir,
            papers: HashMap::new(),
        }
    }

    /// The slide's text with every hand-written quote card completed: the
    /// picture cut out and named as the target, the lines marked. Any other
    /// block, and any block the build cannot complete, is left as written.
    /// The papers opened join `files`, so `serve` rebuilds when one changes.
    pub fn slide(
        &mut self,
        text: &str,
        files: &mut BTreeSet<PathBuf>,
        warnings: &mut Vec<String>,
    ) -> String {
        rewrite_blocks(text, |fence, body| self.block(fence, body, files, warnings))
    }

    /// One block. A block with several quoted phrases becomes one block per
    /// phrase, each with its own picture: a chip shows the marks of its own
    /// block, and two passages on two pages are two pictures anyway.
    fn block(
        &mut self,
        fence: &str,
        body: &str,
        files: &mut BTreeSet<PathBuf>,
        warnings: &mut Vec<String>,
    ) -> Option<String> {
        let doc = mirzam_annot::parse(body);
        if !doc.errors.is_empty() || doc.target.is_some() || !doc.is_card() {
            return None;
        }
        let source = doc.source.as_deref()?.to_string();
        let mut blocks: Vec<String> = Vec::new();
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with("target:")
                || trimmed.starts_with("source:")
            {
                continue;
            }
            // The parser has accepted the block, so every line here is a
            // chip with a quote.
            let one = mirzam_annot::parse(&format!("source: {source}\n{trimmed}\n"));
            let Some(item) = one.items.first() else {
                continue;
            };
            let (Place::Anchor(id), Some(quote)) = (&item.place, item.quote.as_deref()) else {
                continue;
            };
            let mut block = String::new();
            match self.cut(&source, quote, item.page, files) {
                Ok(cut) => {
                    block.push_str(&format!(
                        "target: {}\nsource: {source}\n{trimmed}",
                        cut.target
                    ));
                    if item.page.is_none() {
                        block.push_str(&format!(" page={}", cut.page));
                    }
                    block.push('\n');
                    let kind = match item.kind {
                        Kind::Underline => "underline",
                        _ => "highlight",
                    };
                    let color = item
                        .color
                        .as_deref()
                        .map(|c| format!(" : color={c}"))
                        .unwrap_or_default();
                    for m in &cut.marks {
                        block.push_str(&format!(
                            "{kind} {:.1},{:.1} {:.1}x{:.1}{color}\n",
                            m.x, m.y, m.w, m.h
                        ));
                    }
                }
                Err(why) => {
                    warnings.push(format!("annotate: no cut-out for #{id}: {why}"));
                    block.push_str(&format!("source: {source}\n{trimmed}\n"));
                }
            }
            blocks.push(block);
        }
        Some(blocks.join(&format!("{fence}\n\n{fence}annotate\n")))
    }

    /// The cut-out for one quote: found on disk from an earlier build, or made.
    fn cut(
        &mut self,
        source: &str,
        quote: &str,
        page: Option<u32>,
        files: &mut BTreeSet<PathBuf>,
    ) -> Result<Cut, String> {
        let pdf = quotes::resolve(source, self.bib, &self.bib_dir, &self.deck_dir)?;
        files.insert(pdf.clone());
        let name = match source.strip_prefix('@') {
            Some(key) => key.to_string(),
            None => pdf
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "paper".to_string()),
        };
        let name: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        // The source as the block writes it rather than the path it resolves
        // to, which changes with the working directory; the paper's size
        // stands for the paper, so another PDF under the same key is another
        // file. Same request, same file, on every machine.
        let asked = format!(
            "{source}\n{}\n{quote}\n{}\n{CONTEXT}",
            std::fs::metadata(&pdf).map(|m| m.len()).unwrap_or(0),
            page.map(|p| p.to_string()).unwrap_or_default()
        );
        let stem = format!("{name}-{:08x}", fnv1a(&asked) as u32);
        let target = format!("{DIR}/{stem}.svg");
        let out = self.deck_dir.join(DIR).join(format!("{stem}.svg"));
        let paper_time = mtime(&pdf);

        if let Some((page, marks)) = written(&out, paper_time) {
            return Ok(Cut {
                target,
                page,
                marks,
            });
        }
        if let Some(why) = missed(&out, paper_time) {
            return Err(why);
        }
        let made = self.make(&pdf, quote, page, &out);
        if let Err(why) = &made {
            remember_miss(&out, paper_time, why);
        }
        made.map(|(page, marks)| Cut {
            target,
            page,
            marks,
        })
    }

    /// Opens the paper, finds the words, cuts the column out around them and
    /// writes the picture, with its page and marks on the first line.
    fn make(
        &mut self,
        pdf: &Path,
        quote: &str,
        page: Option<u32>,
        out: &Path,
    ) -> Result<(u32, Vec<Mark>), String> {
        let name = pdf
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| pdf.display().to_string());
        let paper = self
            .papers
            .entry(pdf.to_path_buf())
            .or_insert_with(|| {
                Document::load(pdf).map_err(|e| format!("cannot read {}: {e}", pdf.display()))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let found = quotes::locate(paper, quote, page).map_err(|miss| match miss {
            Miss::NoPage(p) => format!("{name} has no page {p}"),
            Miss::Near {
                page,
                text,
                distance,
            } => format!(
                "the words are not printed quite so on p. {page} of {name}; the nearest are \
                 {distance} edit(s) away: \"{text}\". Quote the paper as it is printed - or, if \
                 the paper has the typo, quote the typo."
            ),
            Miss::Nothing => match page {
                Some(p) => format!("the words are not on p. {p} of {name}"),
                None => format!("the words are not in {name}"),
            },
            Miss::Unreadable(e) => e,
        })?;
        let number = found.page.number;
        let art = found
            .text
            .crop(std::slice::from_ref(&found.span), CONTEXT)
            .ok_or_else(|| "nothing to cut out".to_string())?;
        let marks = found.text.marks(&found.span, &art);

        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let crop = out.with_extension("pdf");
        pdfimport::crop_to(
            pdf,
            number,
            pdfpage::map_rect(found.page.to_pdf, art),
            &crop,
        )?;
        let lines = pdfimport::text_in(&found.page, &art);
        let svg = pdfimport::svg_from_crop(&crop, &lines, art);
        let _ = std::fs::remove_file(&crop);
        let svg = svg?;
        let header: Vec<String> = marks
            .iter()
            .map(|m| format!("{:.1},{:.1} {:.1}x{:.1}", m.x, m.y, m.w, m.h))
            .collect();
        std::fs::write(
            out,
            format!(
                "{HEADER} page={number} marks={}-->\n{svg}",
                header.join(";")
            ),
        )
        .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
        Ok((number, marks))
    }
}

/// The page and marks of a cut-out written by an earlier build, if it is
/// there and no older than the paper.
fn written(out: &Path, paper_time: Option<SystemTime>) -> Option<(u32, Vec<Mark>)> {
    let own_time = mtime(out)?;
    if paper_time.is_some_and(|t| t > own_time) {
        return None;
    }
    let file = std::fs::File::open(out).ok()?;
    let mut first = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(file), &mut first).ok()?;
    parse_header(&first)
}

/// `<!--mirzam cut-out: page=1 marks=54.6,43.8 84.2x9.6;48.3,56.3 93.3x9.6-->`
fn parse_header(line: &str) -> Option<(u32, Vec<Mark>)> {
    let rest = line.trim().strip_prefix(HEADER)?.strip_suffix("-->")?;
    let (page, marks) = rest.trim().split_once(" marks=")?;
    let page: u32 = page.strip_prefix("page=")?.parse().ok()?;
    let marks = marks
        .split(';')
        .filter(|m| !m.is_empty())
        .map(|m| {
            let (at, size) = m.split_once(' ')?;
            let (x, y) = at.split_once(',')?;
            let (w, h) = size.split_once('x')?;
            Some(Mark {
                x: x.parse().ok()?,
                y: y.parse().ok()?,
                w: w.parse().ok()?,
                h: h.parse().ok()?,
            })
        })
        .collect::<Option<Vec<Mark>>>()?;
    Some((page, marks))
}

/// What could not be cut in this process, by the picture it would have made.
///
/// A quote the paper does not print is searched for page by page, and the
/// language server builds the deck on every keystroke: remembering the miss
/// while the paper is unchanged keeps typing the quote in from reading the
/// paper each time.
type Misses = HashMap<PathBuf, (Option<SystemTime>, String)>;

fn misses() -> &'static Mutex<Misses> {
    static MISSES: OnceLock<Mutex<Misses>> = OnceLock::new();
    MISSES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn missed(out: &Path, paper_time: Option<SystemTime>) -> Option<String> {
    let misses = misses().lock().ok()?;
    let (when, why) = misses.get(out)?;
    (*when == paper_time).then(|| why.clone())
}

fn remember_miss(out: &Path, paper_time: Option<SystemTime>, why: &str) {
    if let Ok(mut misses) = misses().lock() {
        misses.insert(out.to_path_buf(), (paper_time, why.to_string()));
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// FNV-1a, written out rather than borrowed from the standard hasher: a file
/// name must be the same on every machine and under every Rust release, and
/// `DefaultHasher` promises neither.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The slide's text with the body of each `annotate` fence replaced by what
/// `f` returns for it, or kept when `f` returns `None`. `f` also gets the
/// fence itself, so a body may close it and open another. A fence quoted
/// inside a longer one is not a block on the slide, and is left alone with
/// everything else inside the outer fence.
fn rewrite_blocks(text: &str, mut f: impl FnMut(&str, &str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    // The open fence: its length, and the body so far when it is `annotate`.
    let mut open: Option<(usize, Option<String>)> = None;
    for raw in text.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let trimmed = line.trim();
        match &mut open {
            Some((len, body)) => {
                if mirzam_syntax::closes_fence(trimmed, *len) {
                    if let Some(body) = body.take() {
                        match f(&"`".repeat(*len), &body) {
                            Some(new) => out.push_str(&new),
                            None => out.push_str(&body),
                        }
                    }
                    out.push_str(raw);
                    open = None;
                } else if let Some(body) = body {
                    body.push_str(raw);
                } else {
                    out.push_str(raw);
                }
            }
            None => {
                out.push_str(raw);
                if let Some(len) = mirzam_syntax::fence_len(trimmed) {
                    let info = trimmed[len..].trim();
                    let is_annotate = info.split_whitespace().next() == Some("annotate");
                    open = Some((len, is_annotate.then(String::new)));
                }
            }
        }
    }
    // A fence the slide never closed: its body is still the slide's text.
    if let Some((_, Some(body))) = open {
        out.push_str(&body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_round_trips() {
        let line = "<!--mirzam cut-out: page=3 marks=54.6,43.8 84.2x9.6;48.3,56.3 93.3x9.6-->\n";
        let (page, marks) = parse_header(line).expect("a header");
        assert_eq!(page, 3);
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[1].x, 48.3);
        assert_eq!(marks[1].h, 9.6);
        assert_eq!(parse_header("<svg viewBox=\"0 0 1 1\">"), None);
        assert_eq!(parse_header("<!--mirzam cut-out: page=x marks=-->"), None);
    }

    /// The same request is the same file, on any machine; a different page
    /// or a different quote is a different one.
    #[test]
    fn a_file_name_follows_from_what_was_asked() {
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(fnv1a("a\nb"), fnv1a("a\nc"));
    }

    #[test]
    fn only_annotate_bodies_are_rewritten_and_the_fences_stay() {
        let slide = "# Head\n\n```annotate\nsource: @k\nhighlight #a : quote=\"w\"\n```\n\n\
                     ```chart\ntype: bar\n```\n\n````markdown\n```annotate\nquoted\n```\n````\n";
        let out = rewrite_blocks(slide, |fence, body| {
            assert_eq!(fence, "```");
            assert_eq!(body, "source: @k\nhighlight #a : quote=\"w\"\n");
            Some("REWRITTEN\n".to_string())
        });
        assert_eq!(
            out,
            "# Head\n\n```annotate\nREWRITTEN\n```\n\n\
             ```chart\ntype: bar\n```\n\n````markdown\n```annotate\nquoted\n```\n````\n"
        );
        let same = rewrite_blocks(slide, |_, _| None);
        assert_eq!(same, slide);
    }

    /// A block with no PDF behind it stays as written and says why; the
    /// resolver never touches a block that is not a hand-written card.
    #[test]
    fn a_block_the_build_cannot_complete_is_left_as_written() {
        let bib = Bibliography::new();
        let dir = std::env::temp_dir().join(format!("mirzam-cutouts-{}", std::process::id()));
        let mut resolver = Resolver::new(&dir, &bib, dir.clone());
        let mut files = BTreeSet::new();
        let mut warnings = Vec::new();
        let slide = "```annotate\nsource: @ghost\nhighlight #a : quote=\"w\"\n```\n";
        let out = resolver.slide(slide, &mut files, &mut warnings);
        assert_eq!(out, slide);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].starts_with("annotate: no cut-out for #a: source @ghost is not in"),
            "{warnings:?}"
        );

        let mut warnings = Vec::new();
        let plain = "```annotate\ntarget: #chart\nhighlight #a : quote=\"w\"\n```\n";
        assert_eq!(resolver.slide(plain, &mut files, &mut warnings), plain);
        let imported = "```annotate\ntarget: img/p.svg\nsource: @ghost\nhighlight #a\n\
                        highlight 50,50 90x8 : quote=\"w\" page=1\n```\n";
        assert_eq!(
            resolver.slide(imported, &mut files, &mut warnings),
            imported
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }
}
