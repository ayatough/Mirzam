//! `mirzam import pdf`: a figure out of a paper and into a deck.
//!
//! The job this replaces is a screenshot. Somebody reading a paper wants
//! Figure 3 on a slide, so they crop it off the screen and paste the bitmap —
//! at screen resolution, in whatever colours the viewer happened to use, with
//! the caption retyped and the citation forgotten. The figure is *in* the file
//! already, usually as curves rather than pixels, with its caption next to it.
//!
//! So this command finds the captioned floats in a PDF, cuts each one out, and
//! prints the line of Markdown that puts it on a slide — caption filled in,
//! credit filled in, and the credit written as a citation so the paper lands in
//! the deck's reference list by being credited.
//!
//! # What comes out, and why it depends on the machine
//!
//! Cutting a rectangle out of a PDF is arithmetic: it is a one-page file with a
//! smaller `/CropBox`, and this crate does it. *Turning that page into an image
//! a browser can show* is rendering, and Rust has no rendering library this
//! project can take: the ones that draw a PDF page well are MuPDF and Poppler,
//! and both are copyleft. Linking either would relicense Mirzam.
//!
//! Running one is a different matter — a separate program, invoked the way the
//! exporter invokes Chromium and a build invokes `mmdc`, is not a derived work
//! of anything. So the vector path is there for whoever has such a tool
//! installed, discovered by [`TOOL_ENV`] or on `PATH`, and never shipped with
//! Mirzam. Without one the command still works: an image stored in the page
//! comes out whole — no re-rastering, no resampling — and everything else comes
//! out as a cropped PDF that any of those tools, or a colleague's, can convert
//! later.

use crate::pdfpage::{self, Page};
use lopdf::{Document, Object};
use mirzam_figure::{Figure, Rect};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The environment variable naming a PDF tool, for a machine where one is
/// installed somewhere `PATH` does not reach — the same escape hatch
/// `MIRZAM_CHROMIUM` and `MIRZAM_MMDC` are.
pub const TOOL_ENV: &str = "MIRZAM_PDFTOOL";

/// What to write for each figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// The best this machine can do: the stored image when the figure is one,
    /// a vector SVG when a tool is installed, a cropped PDF otherwise.
    #[default]
    Auto,
    Svg,
    Png,
    /// Only the image stored in the page, and only when the figure is one.
    Image,
    /// The crop itself, as a one-page PDF.
    Pdf,
}

impl Format {
    pub fn parse(src: &str) -> Result<Self, String> {
        match src {
            "auto" => Ok(Format::Auto),
            "svg" => Ok(Format::Svg),
            "png" => Ok(Format::Png),
            "image" => Ok(Format::Image),
            "pdf" => Ok(Format::Pdf),
            other => Err(format!(
                "--format: `{other}` is not a format; use auto, svg, png, image or pdf"
            )),
        }
    }
}

pub struct Options {
    pub input: PathBuf,
    pub out_dir: PathBuf,
    /// Only the float with this number, as the paper writes it (`3`, `1a`, `I`).
    pub only: Option<String>,
    pub page: Option<u32>,
    pub format: Format,
    pub dpi: u32,
    /// The bibliography key of the paper, which turns the credit into a
    /// citation.
    pub cite: Option<String>,
    /// Say what is in the file and write nothing.
    pub list: bool,
    pub tool: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input: PathBuf::new(),
            out_dir: PathBuf::from("img"),
            only: None,
            page: None,
            format: Format::default(),
            dpi: 300,
            cite: None,
            list: false,
            tool: None,
        }
    }
}

/// A float located on a page, before anything is written for it.
struct Found {
    page: u32,
    figure: Figure,
    /// The crop: the figure's own box with a little of the page around it.
    art: Rect,
    to_pdf: pdfpage::Matrix,
    images: Vec<(lopdf::ObjectId, Rect)>,
    /// The lines the crop contains, kept so the converted picture can carry
    /// them as text it does not show.
    text: Vec<mirzam_figure::Line>,
}

/// One float, found and (unless listing) written.
#[derive(Debug)]
pub struct Imported {
    pub page: u32,
    pub label: String,
    pub caption: String,
    pub box_pt: Rect,
    /// Where the picture was written, and how. Empty while listing.
    pub file: Option<PathBuf>,
    pub how: String,
}

impl Imported {
    /// The line to paste into a deck.
    pub fn markdown(&self, credit: &str) -> String {
        let path = self
            .file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "…".to_string());
        // The attribute list has no escape for a quotation mark, so a caption
        // carrying one gets typographic quotes rather than a broken reference.
        let caption = self.caption.replace('"', "”");
        format!(
            "![{}]({}){{fit=contain caption=\"{}\" credit=\"{} of {}\"}}",
            self.label, path, caption, self.label, credit
        )
    }
}

/// What one run of the command produced.
#[derive(Debug)]
pub struct Import {
    pub figures: Vec<Imported>,
    /// What every credit line ends with: the paper, as a citation or by name.
    pub credit: String,
}

/// Reads the PDF, writes the figures, and returns what it did.
pub fn run(options: &Options) -> Result<Import, String> {
    let doc = Document::load(&options.input)
        .map_err(|e| format!("cannot read {}: {e}", options.input.display()))?;

    let mut seen = 0;
    let mut found = Vec::new();
    for (number, id) in doc.get_pages() {
        if options.page.is_some_and(|only| only != number) {
            continue;
        }
        let page = match pdfpage::read(&doc, number, id) {
            Ok(page) => page,
            // One page that will not parse is not a reason to abandon the
            // paper; the figures on the others are still there.
            Err(_) => continue,
        };
        for figure in mirzam_figure::find(page.rect, &page.lines, &page.ink) {
            seen += 1;
            if options
                .only
                .as_ref()
                .is_some_and(|only| !only.eq_ignore_ascii_case(&figure.number))
            {
                continue;
            }
            let images = images_in(&page, &figure);
            // A hairline of the page around the picture: cut exactly on the
            // ink, a box's own stroke lands half outside the crop.
            let art = figure
                .art
                .grow(2.0)
                .intersect(&page.rect)
                .unwrap_or(figure.art);
            found.push(Found {
                page: page.number,
                art,
                figure,
                to_pdf: page.to_pdf,
                images,
                text: text_in(&page, &art),
            });
        }
    }
    if found.is_empty() {
        return Err(nothing_taken(options, seen));
    }

    let mut done = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    for one in &found {
        let stem = distinct(stem_for(options, &one.figure), &mut taken);
        let mut imported = Imported {
            page: one.page,
            label: one.figure.label.clone(),
            caption: one.figure.caption.clone(),
            box_pt: one.art,
            file: None,
            how: String::new(),
        };
        if !options.list {
            let (file, how) = write_one(options, &doc, one, &stem)?;
            imported.file = Some(file);
            imported.how = how;
        }
        done.push(imported);
    }
    Ok(Import {
        figures: done,
        credit: credit(options, title(&doc)),
    })
}

/// The credit's source: the paper as a citation when its key is known, and the
/// paper as a name when it is not.
fn credit(options: &Options, doc_title: Option<String>) -> String {
    if let Some(key) = &options.cite {
        return format!("[@{key}]");
    }
    doc_title.unwrap_or_else(|| {
        options
            .input
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    })
}

/// The `/Title` the paper gives itself, when it gives itself one worth using.
fn title(doc: &Document) -> Option<String> {
    let info = doc.trailer.get(b"Info").ok()?;
    let info = match info {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    let title = info.as_dict().ok()?.get(b"Title").ok()?.as_str().ok()?;
    let title = decode_text(title);
    (!title.trim().is_empty()).then(|| title.trim().to_string())
}

/// A PDF text string is UTF-16BE when it starts with a byte order mark and
/// PDFDocEncoding — near enough Latin-1 for a title — when it does not.
fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| ((*c.first().unwrap_or(&0) as u16) << 8) | *c.get(1).unwrap_or(&0) as u16)
            .collect();
        return String::from_utf16_lossy(&units);
    }
    bytes.iter().map(|&b| b as char).collect()
}

/// Why a run came back with nothing — which is a different sentence when the
/// paper had figures and the flags excluded them.
fn nothing_taken(options: &Options, seen: usize) -> String {
    if seen > 0 {
        let mut asked = Vec::new();
        if let Some(number) = &options.only {
            asked.push(format!("--figure {number}"));
        }
        if let Some(page) = options.page {
            asked.push(format!("--page {page}"));
        }
        return format!(
            "{}: {seen} captioned figure(s), none matching {}.\n\
             Run it without them - or with --list - to see what the paper calls them.",
            options.input.display(),
            asked.join(" ")
        );
    }
    format!(
        "{}: no captioned figure found.\n\
         A caption is what this looks for - a line starting `Figure 3`, `Fig. 3`, `Table 1`.\n\
         A scanned paper has no text to read, and a slide deck saved as a PDF has no captions;\n\
         for either, name the piece yourself: --page <n> and crop the page it writes.",
        options.input.display()
    )
}

/// The images stored in the page that this figure is made of.
fn images_in(page: &Page, figure: &Figure) -> Vec<(lopdf::ObjectId, Rect)> {
    page.images
        .iter()
        .filter(|image| image.rect.share_inside(&figure.art) > 0.9)
        .map(|image| (image.id, image.rect))
        .collect()
}

/// The lines a crop holds, for the text layer laid over the converted picture.
///
/// Judged by where most of a line is: one clipped by the crop's edge is a
/// fragment of the page around the figure, not part of it.
fn text_in(page: &Page, art: &Rect) -> Vec<mirzam_figure::Line> {
    page.lines
        .iter()
        .filter(|line| line.rect.share_inside(art) > 0.8)
        .cloned()
        .collect()
}

/// The name to write the file under: the citation key when there is one, since
/// that is what the deck calls this paper, and the file's own name otherwise.
/// A name no earlier figure in this run has taken.
///
/// Two floats can want the same one — a paper that numbers its appendix from
/// one again, a `Fig. 3` that is also written `FIG. 3`. Writing both to one
/// file loses a figure without saying so, and a figure that quietly turns into
/// a different figure is the worst outcome this command has.
fn distinct(stem: String, taken: &mut Vec<String>) -> String {
    let mut candidate = stem.clone();
    let mut n = 1;
    while taken.contains(&candidate) {
        n += 1;
        candidate = format!("{stem}-{n}");
    }
    taken.push(candidate.clone());
    candidate
}

fn stem_for(options: &Options, figure: &Figure) -> String {
    let base = options.cite.clone().unwrap_or_else(|| {
        options
            .input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "figure".to_string())
    });
    let base: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let number: String = figure
        .number
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    format!(
        "{}-{}{}",
        base.trim_matches('-').to_lowercase(),
        figure.kind.word(),
        number.to_lowercase()
    )
}

/// Writes one figure, in the best form this machine can produce.
fn write_one(
    options: &Options,
    doc: &Document,
    found: &Found,
    stem: &str,
) -> Result<(PathBuf, String), String> {
    let Found {
        page: number,
        figure,
        art,
        to_pdf,
        images,
        text,
    } = found;
    std::fs::create_dir_all(&options.out_dir)
        .map_err(|e| format!("cannot create {}: {e}", options.out_dir.display()))?;

    // A figure that *is* one stored image comes out of the file untouched:
    // no re-rastering, no second lossy pass, and usually smaller than a
    // rendering of it would be.
    let stored = images
        .first()
        .filter(|(_, rect)| images.len() == 1 && figure.art.share_inside(rect) > 0.85);
    if matches!(options.format, Format::Auto | Format::Image) {
        if let Some((id, _)) = stored {
            match image::extract(doc, *id) {
                Ok(image) => {
                    let file = options.out_dir.join(format!("{stem}.{}", image.extension));
                    std::fs::write(&file, &image.bytes)
                        .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
                    return Ok((file, format!("the {} stored in the page", image.what)));
                }
                Err(why) if options.format == Format::Image => return Err(why),
                Err(_) => {}
            }
        } else if options.format == Format::Image {
            return Err(format!(
                "{}: not a single stored image, so there is nothing to lift out. \
                 Leave --format off and it will be cropped instead.",
                figure.label
            ));
        }
    }

    let crop = options.out_dir.join(format!("{stem}.pdf"));
    crop_to(
        &options.input,
        *number,
        pdfpage::map_rect(*to_pdf, *art),
        &crop,
    )?;
    if options.format == Format::Pdf {
        return Ok((crop, "cropped".to_string()));
    }

    let wanted = match options.format {
        Format::Png => "png",
        _ => "svg",
    };

    // A vector figure is converted here, in this process, unless the author
    // named a tool - `--tool` and MIRZAM_PDFTOOL are a choice, where `mutool`
    // merely being on PATH is not.
    let named = Tool::named(options.tool.as_deref());
    let mut refused = None;
    if wanted == "svg" && named.is_none() {
        let bytes =
            std::fs::read(&crop).map_err(|e| format!("cannot read {}: {e}", crop.display()))?;
        match svg::convert(&bytes) {
            Ok(drawing) => {
                let file = options.out_dir.join(format!("{stem}.svg"));
                std::fs::write(&file, svg::with_text(&drawing, text, *art))
                    .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
                let _ = std::fs::remove_file(&crop);
                return Ok((file, "svg".to_string()));
            }
            // Not a failure to report and stop on: a figure hayro will not
            // take is one an installed tool may still manage, and the crop is
            // there either way.
            Err(why) => refused = Some(why),
        }
    }

    match named.or_else(|| Tool::discover(None)) {
        Some(tool) => {
            let file = options.out_dir.join(format!("{stem}.{wanted}"));
            tool.convert(&crop, &file, wanted, options.dpi)?;
            // A tool writes the page's measurements onto the root element too,
            // and a figure imported through one belongs on a slide just the
            // same. Rewritten only if it is read back whole.
            if wanted == "svg" {
                if let Ok(drawing) = std::fs::read_to_string(&file) {
                    let drawing = svg::with_text(&svg::scalable(&drawing), text, *art);
                    let _ = std::fs::write(&file, drawing);
                }
            }
            let _ = std::fs::remove_file(&crop);
            Ok((file, format!("{} by {}", wanted, tool.name())))
        }
        None if options.format == Format::Auto => Ok((
            crop,
            match &refused {
                Some(why) => format!("cropped - not converted here: {why}"),
                None => "cropped".to_string(),
            },
        )),
        None => Err(match (&refused, wanted) {
            (Some(why), _) => format!(
                "{}: {why}.\n\
                 The crop is at {}, and `mutool` or `pdftocairo` may still convert it.",
                figure.label,
                crop.display()
            ),
            (None, _) => format!(
                "a {wanted} is rendered by a tool, and this machine has none.\n\
                 Install mupdf-tools (`mutool`) or poppler-utils (`pdftocairo`), or point \
                 {TOOL_ENV} at one - neither is bundled, both being copyleft.\n\
                 An SVG needs none of that: leave `--format` off.\n\
                 The crop is at {}.",
                crop.display()
            ),
        }),
    }
}

/// The crop: the page on its own, with the box narrowed to the figure.
fn crop_to(input: &Path, number: u32, box_pt: Rect, out: &Path) -> Result<(), String> {
    let mut doc = Document::load(input).map_err(|e| format!("cannot read {input:?}: {e}"))?;
    let others: Vec<u32> = doc
        .get_pages()
        .keys()
        .copied()
        .filter(|&n| n != number)
        .collect();
    doc.delete_pages(&others);
    let id = *doc
        .get_pages()
        .values()
        .next()
        .ok_or_else(|| format!("page {number} is not in {input:?}"))?;

    let page = doc
        .get_object_mut(id)
        .and_then(|o| o.as_dict_mut())
        .map_err(|e| format!("page {number}: {e}"))?;
    let rect: Vec<Object> = [box_pt.x0, box_pt.y0, box_pt.x1, box_pt.y1]
        .iter()
        .map(|v| Object::Real(*v as f32))
        .collect();
    page.set("MediaBox", Object::Array(rect.clone()));
    page.set("CropBox", Object::Array(rect));
    // Links and highlights belong to the paper, not to the picture cut out of
    // it, and a stray annotation would be drawn over the figure.
    page.remove(b"Annots");

    doc.prune_objects();
    doc.compress();
    doc.save(out)
        .map(|_| ())
        .map_err(|e| format!("cannot write {}: {e}", out.display()))
}

/// A PDF tool this machine happens to have.
enum Tool {
    /// MuPDF's, which renders a page to SVG with the text as outlines.
    MuTool(PathBuf),
    /// Poppler's, which does the same through cairo.
    PdfToCairo(PathBuf),
}

impl Tool {
    /// Named first, then whichever is on `PATH`. `None` is an ordinary answer:
    /// it is what a machine with neither installed says, and the command has
    /// something to write either way.
    fn discover(named: Option<&str>) -> Option<Tool> {
        Self::discover_with(named, |k| std::env::var(k).ok(), runs)
    }

    /// Only the tool the author *asked* for, by `--tool` or by the
    /// environment. A tool that merely happens to be installed no longer wins:
    /// the conversion is in this process now, and PATH is not a preference.
    fn named(named: Option<&str>) -> Option<Tool> {
        Self::named_with(named, |k| std::env::var(k).ok(), runs)
    }

    fn named_with(
        named: Option<&str>,
        env: impl Fn(&str) -> Option<String>,
        usable: impl Fn(&Path) -> bool,
    ) -> Option<Tool> {
        named
            .map(PathBuf::from)
            .or_else(|| env(TOOL_ENV).map(PathBuf::from))
            .filter(|p| usable(p))
            .map(|p| Self::name_of(&p))
    }

    fn discover_with(
        named: Option<&str>,
        env: impl Fn(&str) -> Option<String>,
        usable: impl Fn(&Path) -> bool,
    ) -> Option<Tool> {
        let claimed = named
            .map(PathBuf::from)
            .or_else(|| env(TOOL_ENV).map(PathBuf::from));
        if let Some(path) = claimed.filter(|p| usable(p)) {
            return Some(Self::name_of(&path));
        }
        for name in ["mutool", "pdftocairo"] {
            let path = PathBuf::from(name);
            if usable(&path) {
                return Some(Self::name_of(&path));
            }
        }
        None
    }

    /// Which tool a path is, by what it is called. Both take entirely
    /// different arguments, so a wrong guess fails loudly rather than quietly.
    fn name_of(path: &Path) -> Tool {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.contains("pdftocairo") {
            Tool::PdfToCairo(path.to_path_buf())
        } else {
            Tool::MuTool(path.to_path_buf())
        }
    }

    fn program(&self) -> &Path {
        match self {
            Tool::MuTool(p) | Tool::PdfToCairo(p) => p,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Tool::MuTool(_) => "mutool",
            Tool::PdfToCairo(_) => "pdftocairo",
        }
    }

    fn convert(&self, crop: &Path, out: &Path, format: &str, dpi: u32) -> Result<(), String> {
        let stem = out
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut written = out.to_path_buf();
        let mut command = Command::new(self.program());
        match self {
            Tool::MuTool(_) => {
                command.args(["draw", "-q", "-F", format]);
                if format == "svg" {
                    // Resolution is a raster idea. Handed to the SVG device it
                    // multiplies every coordinate in the file and changes
                    // nothing about how it looks.
                    // Text as outlines: a deck embeds the picture as a data
                    // URI, where a font referenced by name is a font that is
                    // not there.
                    command.args(["-O", "text=path"]);
                } else {
                    command.args(["-r", &dpi.to_string()]);
                }
                // MuPDF writes one file per page and numbers it. Without `%d`
                // it appends the number to the name it was given, which is how
                // `fig1.svg` becomes `fig11.svg`; with it, the name is
                // predictable and gets moved back below.
                written = out.with_file_name(format!("{stem}-%d.{format}"));
                let landed = out.with_file_name(format!("{stem}-1.{format}"));
                command.arg("-o").arg(&written).arg(crop).arg("1");
                written = landed;
            }
            Tool::PdfToCairo(_) => {
                // `pdftocairo` adds the extension itself for a raster, and
                // does not for SVG.
                let stem = out.with_extension("");
                command.arg(format!("-{format}"));
                if format == "png" {
                    command.args(["-r", &dpi.to_string(), "-singlefile"]);
                    command.arg(crop).arg(stem);
                } else {
                    command.arg(crop).arg(out);
                }
            }
        }
        let out_status = command
            .output()
            .map_err(|e| format!("cannot run {}: {e}", self.program().display()))?;
        if out_status.status.success() && written != out {
            std::fs::rename(&written, out).map_err(|e| {
                format!(
                    "{} wrote {}, which cannot be moved: {e}",
                    self.name(),
                    written.display()
                )
            })?;
        }
        if !out_status.status.success() {
            let stderr = String::from_utf8_lossy(&out_status.stderr);
            let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            return Err(format!(
                "{} failed ({}): {first}",
                self.program().display(),
                out_status.status
            ));
        }
        // A tool that exits zero and writes nothing has still failed, and the
        // deck would carry a reference to a file that is not there.
        if !out.exists() {
            return Err(format!(
                "{} exited cleanly but wrote no {}",
                self.name(),
                out.display()
            ));
        }
        Ok(())
    }
}

/// Whether the binary at `program` is there and answers.
fn runs(program: &Path) -> bool {
    // `mutool -v` and `pdftocairo -v` both print a version; neither offers a
    // way to exit zero without doing something, so the status is read as
    // "it ran at all".
    Command::new(program)
        .arg("-v")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

mod image;
// Public so a test can drive the conversion without going through the
// command, and without a tool on the machine changing what it measures.
pub mod svg;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_figures_are_written_to_one_file() {
        let mut taken = Vec::new();
        let names: Vec<String> = ["p-fig3", "p-fig3", "p-fig4", "p-fig3"]
            .into_iter()
            .map(|s| distinct(s.to_string(), &mut taken))
            .collect();
        assert_eq!(names, ["p-fig3", "p-fig3-2", "p-fig4", "p-fig3-3"]);
    }

    #[test]
    fn a_named_tool_wins_and_a_missing_one_is_ignored() {
        let none = |_: &str| None;
        let all = |_: &Path| true;
        let tool = Tool::discover_with(Some("/opt/pdftocairo"), none, all).unwrap();
        assert_eq!(tool.name(), "pdftocairo");

        let env = |k: &str| (k == TOOL_ENV).then(|| "/opt/mutool".to_string());
        let tool = Tool::discover_with(None, env, all).unwrap();
        assert_eq!(tool.name(), "mutool");

        assert!(Tool::discover_with(Some("/opt/mutool"), none, |_: &Path| false).is_none());
    }

    #[test]
    fn the_markdown_carries_the_caption_and_the_citation() {
        let imported = Imported {
            page: 4,
            label: "Figure 3".to_string(),
            caption: "Readout fidelity, the \"good\" run".to_string(),
            box_pt: Rect::new(0.0, 0.0, 10.0, 10.0),
            file: Some(PathBuf::from("img/vaswani2017-fig3.svg")),
            how: String::new(),
        };
        let line = imported.markdown("[@vaswani2017]");
        assert!(
            line.contains("![Figure 3](img/vaswani2017-fig3.svg)"),
            "{line}"
        );
        assert!(
            line.contains("caption=\"Readout fidelity, the ”good” run\""),
            "{line}"
        );
        assert!(
            line.contains("credit=\"Figure 3 of [@vaswani2017]\""),
            "{line}"
        );
    }
}
