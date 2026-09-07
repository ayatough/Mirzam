//! The quote check: are the words a slide says a paper says actually on the
//! page it points at?
//!
//! `mirzam import pdf --quote` marks a passage's lines on a cut-out and writes
//! the words it matched into `quote=`, with the page in `page=` and the paper
//! in the block's `source:`. That turns a slide's claim about its source into
//! something a build can test, and this is the test: open the paper, read the
//! page, look for the words. `mirzam check` runs it beside the layout checks
//! and reports through the same list - a claim with nothing under it is
//! `source.quote`, an error; a claim a few letters off is the same kind, a
//! warning, with what the page prints beside what the slide says.
//!
//! A quote written on the phrase's own mark may leave `page=` out: the words
//! are then looked for on every page, and the build's cut-out (see `cutouts`)
//! finds the page the same way.
//!
//! Opening a PDF happens here, in `import` and in `cutouts`, and in the CLI on
//! purpose: the core never touches a file, and the WebAssembly build must not
//! learn how.

use crate::pdfpage::{self, Page};
use crate::pipeline::BuildOutput;
use lopdf::Document;
use mirzam_cite::Bibliography;
use mirzam_figure::quote::{Found, Span, Text};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where a quote was found: the page, its text in reading order, and the
/// lines the words cover.
pub struct Located {
    pub page: Page,
    pub text: Text,
    pub span: Span,
}

/// Why a quote was not found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Miss {
    /// The paper has no page of that number.
    NoPage(u32),
    /// No exact match, but a run of words on `page` this many edits away.
    Near {
        page: u32,
        text: String,
        distance: usize,
    },
    Nothing,
    /// A page whose text could not be read.
    Unreadable(String),
}

/// Looks for the words on one page, or on every page when none is named: the
/// first exact match wins, and failing that the nearest words anywhere.
pub fn locate(paper: &Document, quote: &str, page: Option<u32>) -> Result<Located, Miss> {
    let pages = paper.get_pages();
    if let Some(n) = page {
        if !pages.contains_key(&n) {
            return Err(Miss::NoPage(n));
        }
    }
    let mut nearest: Option<(u32, String, usize)> = None;
    let mut unreadable = None;
    for (number, id) in pages {
        if page.is_some_and(|only| only != number) {
            continue;
        }
        let read = match pdfpage::read(paper, number, id) {
            Ok(p) => p,
            Err(e) => {
                unreadable.get_or_insert(e);
                continue;
            }
        };
        let text = Text::new(read.rect, &read.lines);
        match text.find(quote) {
            Found::Exact(span) => {
                return Ok(Located {
                    page: read,
                    text,
                    span,
                })
            }
            Found::Near { text, distance }
                if nearest.as_ref().is_none_or(|(_, _, d)| distance < *d) =>
            {
                nearest = Some((number, text, distance));
            }
            _ => {}
        }
    }
    Err(match (nearest, unreadable) {
        (Some((page, text, distance)), _) => Miss::Near {
            page,
            text,
            distance,
        },
        (None, Some(e)) => Miss::Unreadable(e),
        (None, None) => Miss::Nothing,
    })
}

/// One thing the check has to say. Every finding is of kind `source.quote`;
/// the severity says whether it stops a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 1-based, as the deck counts its slides.
    pub slide: usize,
    pub error: bool,
    pub message: String,
}

/// Checks every `quote=` in the deck against the page of the source it names.
pub fn verify(input: &Path, out: &BuildOutput) -> Vec<Finding> {
    let deck_dir = input.parent().unwrap_or(Path::new("."));
    let (bib, _) = mirzam_render::deck_bibliography(&out.meta, |rel| {
        std::fs::read_to_string(deck_dir.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))
    });
    // A `file =` field is relative to the `.bib` that holds it, which is where
    // a reference manager keeps the two together.
    let bib_dir = out
        .meta
        .bibliography_file()
        .and_then(|rel| deck_dir.join(rel).parent().map(Path::to_path_buf))
        .unwrap_or_else(|| deck_dir.to_path_buf());

    let mut findings = Vec::new();
    let mut papers: HashMap<PathBuf, Result<Document, String>> = HashMap::new();
    for (i, slide) in out.slides.iter().enumerate() {
        let n = i + 1;
        for block in annotate_blocks(&slide.text) {
            let doc = mirzam_annot::parse(&block);
            let quoted: Vec<&mirzam_annot::Item> =
                doc.items.iter().filter(|it| it.quote.is_some()).collect();
            if quoted.is_empty() {
                continue;
            }
            let Some(source) = doc.source.as_deref() else {
                findings.push(warn(
                    n,
                    "a mark carries quote=, but the block has no `source:` line naming the \
                     paper to check it against - `source: @key` for a bibliography entry with a \
                     `file` field, or `source: path/to/paper.pdf`"
                        .to_string(),
                ));
                continue;
            };
            let path = match resolve(source, &bib, &bib_dir, deck_dir) {
                Ok(p) => p,
                Err(e) => {
                    findings.push(warn(n, e));
                    continue;
                }
            };
            let name = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| source.to_string());
            let paper = papers.entry(path.clone()).or_insert_with(|| {
                Document::load(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
            });
            let paper = match paper {
                Ok(p) => p,
                Err(e) => {
                    findings.push(warn(n, e.clone()));
                    continue;
                }
            };
            for item in quoted {
                let quote = item.quote.as_deref().unwrap_or_default();
                // Where the slide says the words are, or the whole paper
                // when it does not say.
                let place = match item.page {
                    Some(p) => format!("on p. {p} of {name}"),
                    None => format!("in {name}"),
                };
                match locate(paper, quote, item.page) {
                    Ok(_) => {}
                    Err(Miss::NoPage(p)) => {
                        findings.push(warn(n, format!("{name} has no page {p}")))
                    }
                    Err(Miss::Unreadable(e)) => findings.push(warn(n, format!("{place}: {e}"))),
                    Err(Miss::Near {
                        page,
                        text: printed,
                        distance,
                    }) => findings.push(warn(
                        n,
                        format!(
                            "quote differs from p. {page} of {name} by {distance} edit(s): the \
                             slide says \"{}\" and the page prints \"{}\"",
                            short(quote),
                            short(&printed)
                        ),
                    )),
                    Err(Miss::Nothing) => findings.push(Finding {
                        slide: n,
                        error: true,
                        message: format!("quote is not {place}: \"{}\"", short(quote)),
                    }),
                }
            }
        }
    }
    findings
}

fn warn(slide: usize, message: String) -> Finding {
    Finding {
        slide,
        error: false,
        message,
    }
}

/// A quote as a message can carry it: the first sixty characters.
fn short(quote: &str) -> String {
    let mut s: String = quote.chars().take(60).collect();
    if quote.chars().count() > 60 {
        s.push('…');
    }
    s
}

/// The bodies of a slide's `annotate` fences. A fence quoted inside a longer
/// one - the way a document shows Mirzam syntax - is not a block on the slide,
/// and is skipped with everything else inside the outer fence.
pub fn annotate_blocks(slide: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<(usize, Option<String>)> = None;
    for line in slide.lines() {
        let trimmed = line.trim();
        match &mut open {
            Some((len, body)) => {
                if mirzam_syntax::closes_fence(trimmed, *len) {
                    if let Some(body) = body.take() {
                        out.push(body);
                    }
                    open = None;
                } else if let Some(body) = body {
                    body.push_str(line);
                    body.push('\n');
                }
            }
            None => {
                if let Some(len) = mirzam_syntax::fence_len(trimmed) {
                    let info = trimmed[len..].trim();
                    let is_annotate = info.split_whitespace().next() == Some("annotate");
                    open = Some((len, is_annotate.then(String::new)));
                }
            }
        }
    }
    out
}

/// Where a block's `source:` points: a bibliography entry's `file`, or a path,
/// either way resolved to the PDF to open.
pub fn resolve(
    source: &str,
    bib: &Bibliography,
    bib_dir: &Path,
    deck_dir: &Path,
) -> Result<PathBuf, String> {
    let Some(key) = source.strip_prefix('@') else {
        let path = Path::new(source);
        return Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            deck_dir.join(path)
        });
    };
    let entry = bib
        .get(key)
        .ok_or_else(|| format!("source @{key} is not in the bibliography"))?;
    let field = entry.field(&["file"]).ok_or_else(|| {
        format!(
            "@{key} has no `file` field naming its PDF; a reference manager writes one, or \
             write `source: path/to/paper.pdf` in the block instead"
        )
    })?;
    let rel = pdf_in_file_field(field)
        .ok_or_else(|| format!("@{key}: no PDF in its `file` field (`{field}`)"))?;
    let rel = Path::new(&rel);
    if rel.is_absolute() {
        return Ok(rel.to_path_buf());
    }
    let beside_bib = bib_dir.join(rel);
    Ok(if beside_bib.exists() {
        beside_bib
    } else {
        deck_dir.join(rel)
    })
}

/// The PDF named by a BibTeX `file` field, in the forms reference managers
/// write it: a bare path; Zotero's `Title:path:application/pdf`; JabRef's
/// `:path:PDF`; several of those separated by `;`. The first that ends in
/// `.pdf` wins.
pub fn pdf_in_file_field(field: &str) -> Option<String> {
    for attachment in field.split(';') {
        let parts: Vec<&str> = attachment.split(':').collect();
        // `C:\papers\x.pdf` splits its drive letter off; put it back.
        let mut segments: Vec<String> = Vec::new();
        let mut i = 0;
        while i < parts.len() {
            let part = parts[i];
            if part.len() == 1
                && part.chars().all(|c| c.is_ascii_alphabetic())
                && parts
                    .get(i + 1)
                    .is_some_and(|next| next.starts_with('\\') || next.starts_with('/'))
            {
                segments.push(format!("{part}:{}", parts[i + 1]));
                i += 2;
            } else {
                segments.push(part.to_string());
                i += 1;
            }
        }
        if let Some(pdf) = segments
            .iter()
            .map(|s| s.trim())
            .find(|s| s.to_ascii_lowercase().ends_with(".pdf"))
        {
            return Some(pdf.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_field_is_read_in_every_shape_a_manager_writes() {
        assert_eq!(
            pdf_in_file_field("papers/a.pdf").as_deref(),
            Some("papers/a.pdf")
        );
        assert_eq!(
            pdf_in_file_field("Full Text PDF:files/123/duberg.pdf:application/pdf").as_deref(),
            Some("files/123/duberg.pdf")
        );
        assert_eq!(pdf_in_file_field(":a.pdf:PDF").as_deref(), Some("a.pdf"));
        assert_eq!(
            pdf_in_file_field("Snapshot:x.html:text/html;Full Text:b.PDF:application/pdf")
                .as_deref(),
            Some("b.PDF")
        );
        assert_eq!(
            pdf_in_file_field("C:\\papers\\a.pdf:PDF").as_deref(),
            Some("C:\\papers\\a.pdf")
        );
        assert_eq!(pdf_in_file_field("notes.txt"), None);
    }

    #[test]
    fn annotate_fences_are_found_and_quoted_ones_are_not() {
        let slide = "text\n\n```annotate\ntarget: p\nhighlight 1,1 1x1\n```\n\n\
                     ````markdown\n```annotate\nquoted example\n```\n````\n\n\
                     ```chart\ntype: bar\n```\n```annotate\nsource: @k\n```\n";
        let blocks = annotate_blocks(slide);
        assert_eq!(blocks, ["target: p\nhighlight 1,1 1x1\n", "source: @k\n"]);
    }

    #[test]
    fn a_source_resolves_to_a_pdf_or_says_why_not() {
        let mut bib = Bibliography::new();
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "file".to_string(),
            "Full Text:papers/d.pdf:application/pdf".to_string(),
        );
        bib.insert("d".into(), mirzam_cite::Entry::from_fields("d", fields));
        bib.insert(
            "nofile".into(),
            mirzam_cite::Entry::from_fields("nofile", std::collections::BTreeMap::new()),
        );
        let deck = Path::new("/deck");
        let bibs = Path::new("/deck/refs");

        assert_eq!(
            resolve("paper.pdf", &bib, bibs, deck).unwrap(),
            PathBuf::from("/deck/paper.pdf")
        );
        // Neither location exists here, so the deck's directory is the answer.
        assert_eq!(
            resolve("@d", &bib, bibs, deck).unwrap(),
            PathBuf::from("/deck/papers/d.pdf")
        );
        assert!(resolve("@ghost", &bib, bibs, deck)
            .unwrap_err()
            .contains("not in the bibliography"));
        assert!(resolve("@nofile", &bib, bibs, deck)
            .unwrap_err()
            .contains("`file` field"));
    }
}
