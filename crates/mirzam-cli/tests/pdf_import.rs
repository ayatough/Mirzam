//! `import pdf` against a PDF written here, byte by byte.
//!
//! The fixture is assembled in the test rather than committed as a file, for
//! the same reason the deck fixtures are written as strings: a binary blob in
//! the repository cannot be read in a diff, and nobody can tell what a failing
//! test is measuring. Everything this exercises is visible below — the box that
//! is Figure 1, the two-by-two picture that is Figure 2, and the paragraph
//! between them that stops each figure from swallowing the other.
//!
//! The vector path is tested here too, now that it needs nothing installed:
//! the same fixture goes through hayro, and its SVG is held to a snapshot.
//! What is still *not* tested is the external tool - `mutool` and
//! `pdftocairo` may not be on the machine running this - so its discovery and
//! the arguments it is handed stay unit-tested in `pdfimport.rs`.

use mirzam_cli::pdfimport::{self, Format, Options};
use std::path::PathBuf;

/// A directory that cleans up after itself.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("mirzam-pdf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        TempDir(dir)
    }

    /// The fixture, written where the test can point the importer at it.
    fn paper(&self) -> PathBuf {
        let path = self.0.join("paper.pdf");
        std::fs::write(&path, paper()).expect("write the fixture");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn options(dir: &TempDir, format: Format) -> Options {
    Options {
        input: dir.paper(),
        out_dir: dir.0.join("img"),
        format,
        cite: Some("someone2026".to_string()),
        ..Options::default()
    }
}

#[test]
fn both_figures_are_found_with_their_captions() {
    let dir = TempDir::new("list");
    let found = pdfimport::run(&Options {
        list: true,
        ..options(&dir, Format::Auto)
    })
    .expect("the fixture has two figures")
    .figures;

    assert_eq!(
        found.len(),
        2,
        "{:?}",
        found.iter().map(|f| &f.label).collect::<Vec<_>>()
    );
    assert_eq!(found[0].label, "Figure 1");
    assert_eq!(found[0].caption, "A drawn box.");
    assert_eq!(found[0].page, 1);
    assert_eq!(found[1].label, "Figure 2");
    assert_eq!(found[1].caption, "A stored picture.");

    // The box is the ink and a hairline of the page, and nothing of the
    // paragraph that follows it.
    let art = found[0].box_pt;
    assert!(art.width() > 200.0 && art.width() < 210.0, "{art:?}");
    assert!(art.height() > 80.0 && art.height() < 90.0, "{art:?}");
    assert!(art.y0 > 296.0, "the caption is not in the crop: {art:?}");
}

/// The figure that *is* a stored picture comes out as that picture: no
/// rendering, no tool, nothing resampled.
#[test]
fn a_stored_picture_is_lifted_out_whole() {
    let dir = TempDir::new("image");
    // Only Figure 2: Figure 1 would be converted by whatever tool this machine
    // has, and this test is about the path that needs none.
    let found = pdfimport::run(&Options {
        only: Some("2".to_string()),
        ..options(&dir, Format::Auto)
    })
    .expect("Figure 2")
    .figures;
    let file = found[0].file.as_ref().expect("Figure 2 was written");

    assert_eq!(file.extension().unwrap_or_default(), "png", "{file:?}");
    assert!(
        found[0].how.contains("stored in the page"),
        "{}",
        found[0].how
    );
    let png = std::fs::read(file).expect("the PNG is on disk");
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    assert_eq!(&png[16..20], &2u32.to_be_bytes(), "two pixels wide");
    assert_eq!(&png[20..24], &2u32.to_be_bytes(), "two pixels tall");
}

/// Without a tool installed there is still an answer: the crop itself, as a
/// one-page PDF narrowed to the figure.
#[test]
fn a_drawn_figure_is_cropped_to_one_page() {
    let dir = TempDir::new("crop");
    let found = pdfimport::run(&options(&dir, Format::Pdf))
        .expect("two figures")
        .figures;
    let file = found[0].file.as_ref().expect("Figure 1 was written");

    assert_eq!(file.extension().unwrap_or_default(), "pdf", "{file:?}");
    let crop = std::fs::read(file).expect("the crop is on disk");
    assert_eq!(&crop[..5], b"%PDF-");
    let text = String::from_utf8_lossy(&crop);
    assert!(text.contains("/MediaBox"), "the crop sets its own page box");
    assert!(
        !text.contains("A stored picture"),
        "the crop is one page, not the paper"
    );
}

#[test]
fn the_markdown_is_ready_to_paste() {
    let dir = TempDir::new("markdown");
    let options = Options {
        only: Some("2".to_string()),
        ..options(&dir, Format::Auto)
    };
    let import = pdfimport::run(&options).expect("Figure 2");
    let line = import.figures[0].markdown(&import.credit);

    assert!(line.starts_with("![Figure 2]("), "{line}");
    assert!(line.contains("someone2026-fig2.png"), "{line}");
    assert!(line.contains("caption=\"A stored picture.\""), "{line}");
    assert!(
        line.contains("credit=\"Figure 2 of [@someone2026]\""),
        "{line}"
    );
}

#[test]
fn a_pdf_with_no_captions_says_so_rather_than_guessing() {
    let dir = TempDir::new("empty");
    let path = dir.0.join("blank.pdf");
    std::fs::write(&path, assemble(blank())).expect("write");
    let error = pdfimport::run(&Options {
        input: path,
        out_dir: dir.0.join("img"),
        ..Options::default()
    })
    .expect_err("nothing to find");
    assert!(error.contains("no captioned figure"), "{error}");
}

/// The conversion nobody has to install: a drawn figure comes out as an SVG
/// with hayro, in this process, on a machine with no PDF tool at all.
#[test]
fn a_drawn_figure_becomes_an_svg_with_nothing_installed() {
    let dir = TempDir::new("svg");
    let found = pdfimport::run(&Options {
        only: Some("1".to_string()),
        ..options(&dir, Format::Auto)
    })
    .expect("Figure 1")
    .figures;
    let file = found[0].file.as_ref().expect("Figure 1 was written");

    assert_eq!(file.extension().unwrap_or_default(), "svg", "{file:?}");
    if std::env::var("MIRZAM_PDFTOOL").is_err() {
        assert_eq!(found[0].how, "svg", "converted here, not by a tool");
    }
    let svg = std::fs::read_to_string(file).expect("the SVG is on disk");
    assert!(svg.starts_with("<svg"), "{}", &svg[..svg.len().min(80)]);
    assert!(svg.contains("viewBox"), "{svg}");
}

/// The crop is a page with its box narrowed, so everything the paper drew is
/// still in it. What the view cannot show has to come back out, or a figure
/// carries two columns of body text it never displays — 1.49 MB of it, in a
/// deck with a 20 MB ceiling.
#[test]
fn the_svg_carries_the_figure_and_not_the_page() {
    let dir = TempDir::new("cull");
    let found = pdfimport::run(&Options {
        only: Some("1".to_string()),
        ..options(&dir, Format::Pdf)
    })
    .expect("Figure 1")
    .figures;
    let crop = std::fs::read(found[0].file.as_ref().unwrap()).expect("the crop");

    let svg = pdfimport::svg::convert(&crop).expect("hayro converts it");
    assert!(
        svg.len() < 20_000,
        "the page came along: {} bytes",
        svg.len()
    );
    // The paragraph under the figure is set in the same font as the label
    // inside it, so this counts placements rather than looking for glyphs. The
    // crop holds one label of a dozen characters; the page under it holds two
    // paragraphs of forty-two each, and a second caption besides.
    let uses = svg.matches("<use").count();
    assert!(uses < 30, "{uses} glyphs for a box with one label on it");
    assert!(
        svg.contains("<path"),
        "the box itself is still there: {svg}"
    );
    assert_eq!(svg.matches("<defs").count(), svg.matches("</defs>").count());
}

/// The picture is drawn as outlines, so the words in it would be lost. They
/// are laid over it again where nothing can see them but everything that reads
/// text can find them.
#[test]
fn the_words_inside_a_figure_come_back_as_text() {
    let dir = TempDir::new("textlayer");
    let found = pdfimport::run(&Options {
        only: Some("1".to_string()),
        ..options(&dir, Format::Svg)
    })
    .expect("Figure 1")
    .figures;
    let svg = std::fs::read_to_string(found[0].file.as_ref().unwrap()).expect("the svg");

    assert!(
        svg.contains(">rows &amp; columns</text>"),
        "the label, escaped for XML: {svg}"
    );
    assert!(
        svg.contains("fill-opacity=\"0.004\""),
        "laid on at an alpha that survives a PDF but shows nothing"
    );
    // The page around the figure is not in the crop and must not be in its
    // text either: a search of the deck would land on the wrong slide.
    assert!(
        !svg.contains("quick brown fox"),
        "the page's own prose came along"
    );
}

/// A snapshot, because an SVG that is subtly wrong still looks like an SVG.
/// Reviewed by eye when hayro moves:
///   MIRZAM_UPDATE_SNAPSHOTS=1 cargo test -p mirzam-cli --test pdf_import
#[test]
fn the_converted_figure_matches_its_snapshot() {
    let dir = TempDir::new("snapshot");
    let found = pdfimport::run(&Options {
        only: Some("1".to_string()),
        ..options(&dir, Format::Pdf)
    })
    .expect("Figure 1")
    .figures;
    let crop = std::fs::read(found[0].file.as_ref().unwrap()).expect("the crop");
    let svg = pdfimport::svg::convert(&crop).expect("hayro converts it");

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/figure.svg");
    if std::env::var("MIRZAM_UPDATE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).expect("snapshot directory");
        std::fs::write(&path, &svg).expect("write the snapshot");
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} - run with MIRZAM_UPDATE_SNAPSHOTS=1 to write it",
            path.display()
        )
    });
    assert_eq!(
        svg, want,
        "the conversion moved; review the diff at {path:?}"
    );
}

/// Asking for a figure the paper does not have is a different mistake from
/// pointing at a PDF that has none, and saying so saves the next guess.
#[test]
fn a_figure_that_is_not_there_says_what_is() {
    let dir = TempDir::new("missing");
    let error = pdfimport::run(&Options {
        only: Some("9".to_string()),
        ..options(&dir, Format::Auto)
    })
    .expect_err("there is no Figure 9");
    assert!(error.contains("2 captioned figure"), "{error}");
    assert!(error.contains("--figure 9"), "{error}");
}

/// The same paper on a landscape sheet marked `/Rotate 90` — content drawn
/// sideways, read upright, which is how a figure turned to fit a page arrives.
///
/// Measured in reading space it has to come out *identical* to the upright
/// fixture: same captions, same boxes. Anything else means the page was
/// measured in the file's coordinates rather than the reader's.
#[test]
fn a_rotated_page_is_read_the_way_it_is_seen() {
    let upright = TempDir::new("upright");
    let sideways = TempDir::new("sideways");
    let path = sideways.0.join("sideways.pdf");
    std::fs::write(&path, turned()).expect("write");

    let straight = pdfimport::run(&Options {
        list: true,
        ..options(&upright, Format::Auto)
    })
    .expect("two figures")
    .figures;
    let turned = pdfimport::run(&Options {
        input: path,
        list: true,
        ..Options::default()
    })
    .expect("two figures, sideways")
    .figures;

    assert_eq!(turned.len(), straight.len(), "{turned:?}");
    for (a, b) in turned.iter().zip(&straight) {
        assert_eq!(a.label, b.label);
        assert_eq!(a.caption, b.caption);
        assert!(
            (a.box_pt.width() - b.box_pt.width()).abs() < 1.0
                && (a.box_pt.height() - b.box_pt.height()).abs() < 1.0,
            "{:?} is not {:?}",
            a.box_pt,
            b.box_pt
        );
    }
}

/// The fixture: one page, 300 by 400, holding
///
/// * a filled rectangle with `Figure 1: A drawn box.` under it,
/// * a paragraph of body text,
/// * a two-by-two image with `Figure 2: A stored picture.` under it,
/// * another paragraph.
fn paper() -> Vec<u8> {
    assemble(objects("", "[0 0 300 400]", ""))
}

/// The same page, drawn sideways on a landscape sheet and marked to be turned.
fn turned() -> Vec<u8> {
    // The transform is the inverse of the one a reader's `/Rotate 90` applies,
    // so the drawing lands where an upright page would have put it.
    assemble(objects(
        "0 1 -1 0 400 0 cm\n",
        "[0 0 400 300]",
        "/Rotate 90 ",
    ))
}

/// The one thing written inside Figure 1, and the only text a crop of it
/// holds. The ampersand is there because a picture's words go into an SVG,
/// where three characters cannot stand for themselves.
const LABEL: &str = "rows & columns";

fn objects(prefix: &str, media: &str, rotate: &str) -> Vec<Vec<u8>> {
    let body = "the quick brown fox jumps over the lazy dog";
    let content = format!(
        "0 0 0 rg\n\
         40 300 200 80 re f\n\
         1 1 1 rg BT /F1 6 Tf 60 330 Td ({LABEL}) Tj ET 0 0 0 rg\n\
         BT /F1 8 Tf 40 290 Td (Figure 1: A drawn box.) Tj ET\n\
         BT /F1 10 Tf 40 270 Td ({body}) Tj ET\n\
         BT /F1 10 Tf 40 256 Td ({body}) Tj ET\n\
         q 200 0 0 60 40 170 cm /Im1 Do Q\n\
         BT /F1 8 Tf 40 160 Td (Figure 2: A stored picture.) Tj ET\n\
         BT /F1 10 Tf 40 140 Td ({body}) Tj ET\n\
         BT /F1 10 Tf 40 126 Td ({body}) Tj ET\n"
    );
    let content = format!("{prefix}{content}");

    // Four pixels: red, green, blue, white. Stored raw, which a PDF allows and
    // which keeps the fixture readable.
    let pixels: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];

    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R {rotate}/MediaBox {media} \
             /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> \
             /Contents 4 0 R >>"
        )
        .into_bytes(),
        stream(b"", content.as_bytes()),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    objects.push(stream(
        b"/Type /XObject /Subtype /Image /Width 2 /Height 2 \
          /ColorSpace /DeviceRGB /BitsPerComponent 8",
        &pixels,
    ));
    objects
}

/// A page with prose and nothing else.
fn blank() -> Vec<Vec<u8>> {
    vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] \
           /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            .to_vec(),
        stream(
            b"",
            b"BT /F1 10 Tf 40 300 Td (Nothing here is a figure.) Tj ET\n",
        ),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ]
}

fn stream(extra: &[u8], data: &[u8]) -> Vec<u8> {
    let mut object = b"<< ".to_vec();
    object.extend_from_slice(extra);
    object.extend_from_slice(format!(" /Length {} >>\nstream\n", data.len()).as_bytes());
    object.extend_from_slice(data);
    object.extend_from_slice(b"\nendstream");
    object
}

/// Objects in, a loadable file out: a header, the objects, a cross-reference
/// table of where each one starts, and a trailer pointing at it.
fn assemble(objects: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}
