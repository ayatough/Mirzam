//! `mirzam export pptx` — the deck as a PowerPoint file: one picture per
//! slide, at the deck's exact aspect, with the speaker notes in the notes
//! pane where a presenter expects them.
//!
//! **Stage one, and honestly labelled.** The slides are images, which is
//! where Marp, Slidev and Touying all stop too; what none of them ship — and
//! what stays on the roadmap as the differentiator — is native text boxes.
//! What this stage refuses to lose is the *notes*: an image-only export that
//! drops them turns a rehearsed deck into a stack of pictures, and the notes
//! are exactly the part PowerPoint's presenter view exists for.
//!
//! **No dependency.** The package is a ZIP (the writer `skill install --zip`
//! already carries) holding a fixed set of hand-written OOXML parts. The
//! shapes below follow ECMA-376's minimal presentation package; everything
//! variable is the slide size, the images and the notes text.

/// EMUs per CSS pixel: 914400 EMU to the inch, 96 px to the inch.
const EMU_PER_PX: u64 = 9525;

const NS: &str = concat!(
    "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\""
);

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
const REL_NOTES_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const REL_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const REL_THEME: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";

/// The empty shape-tree header every `cSld` opens with.
const EMPTY_TREE_HEAD: &str = concat!(
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>",
    "<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>",
    "<a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>"
);

/// The colour map every master declares, naming the theme's twelve slots.
const CLR_MAP: &str = concat!(
    "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" ",
    "accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" ",
    "accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>"
);

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A complete, minimal DrawingML theme — the part PowerPoint refuses to open
/// a package without. Neutral values: the slides are pictures, so nothing on
/// them ever reads a theme colour.
fn theme_xml() -> String {
    let scheme = concat!(
        "<a:clrScheme name=\"Mirzam\">",
        "<a:dk1><a:srgbClr val=\"0B0E1A\"/></a:dk1>",
        "<a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1>",
        "<a:dk2><a:srgbClr val=\"44546A\"/></a:dk2>",
        "<a:lt2><a:srgbClr val=\"E7E6E6\"/></a:lt2>",
        "<a:accent1><a:srgbClr val=\"6557D9\"/></a:accent1>",
        "<a:accent2><a:srgbClr val=\"38B2AC\"/></a:accent2>",
        "<a:accent3><a:srgbClr val=\"A5A5A5\"/></a:accent3>",
        "<a:accent4><a:srgbClr val=\"FFC000\"/></a:accent4>",
        "<a:accent5><a:srgbClr val=\"5B9BD5\"/></a:accent5>",
        "<a:accent6><a:srgbClr val=\"70AD47\"/></a:accent6>",
        "<a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink>",
        "<a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink>",
        "</a:clrScheme>"
    );
    let fonts = concat!(
        "<a:fontScheme name=\"Mirzam\">",
        "<a:majorFont><a:latin typeface=\"Calibri Light\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>",
        "<a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>",
        "</a:fontScheme>"
    );
    // The format scheme wants three fills, three lines, three effects and two
    // background fills; the plainest legal ones will do.
    let line = "<a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>";
    let fmt = format!(
        concat!(
            "<a:fmtScheme name=\"Mirzam\">",
            "<a:fillStyleLst>{f}{f}{f}</a:fillStyleLst>",
            "<a:lnStyleLst>{l}{l}{l}</a:lnStyleLst>",
            "<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle>",
            "<a:effectStyle><a:effectLst/></a:effectStyle>",
            "<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>",
            "<a:bgFillStyleLst>{f}{f}{f}</a:bgFillStyleLst>",
            "</a:fmtScheme>"
        ),
        f = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
        l = line,
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         name=\"Mirzam\"><a:themeElements>{scheme}{fonts}{fmt}</a:themeElements>\
         <a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"
    )
}

fn rels(entries: &[(String, &str, String)]) -> String {
    let body: String = entries
        .iter()
        .map(|(id, ty, target)| {
            format!("<Relationship Id=\"{id}\" Type=\"{ty}\" Target=\"{target}\"/>")
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{REL_NS}\">{body}</Relationships>"
    )
}

/// A PNG's pixel size, read straight off its IHDR header. `None` for
/// anything that is not a PNG — the caller then embeds it uncropped.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return None;
    }
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    Some((be(&bytes[16..20]), be(&bytes[20..24])))
}

/// How much of the picture's bottom to crop away, in OOXML's thousandths of a
/// percent. Headless Chromium's screenshot canvas is the *window*, and in
/// current builds the window is taller than the viewport by the height of
/// browser chrome that headless never draws — so the shot is the slide at the
/// top and a blank strip below it. The slide's own aspect says exactly where
/// the strip begins, whatever the chrome height or device scale was.
fn bottom_crop(png: &[u8], w: u32, h: u32) -> u64 {
    let Some((pw, ph)) = png_size(png) else {
        return 0;
    };
    let want = f64::from(pw) * f64::from(h) / f64::from(w);
    let extra = (f64::from(ph) - want).max(0.0);
    (extra / f64::from(ph) * 100_000.0).round() as u64
}

/// One slide: a single picture covering the whole surface.
fn slide_xml(cx: u64, cy: u64, crop_b: u64) -> String {
    let src_rect = if crop_b > 0 {
        format!("<a:srcRect b=\"{crop_b}\"/>")
    } else {
        String::new()
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:sld {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}\
         <p:pic><p:nvPicPr><p:cNvPr id=\"2\" name=\"Slide\"/>\
         <p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
         <p:blipFill><a:blip r:embed=\"rId2\"/>{src_rect}<a:stretch><a:fillRect/></a:stretch></p:blipFill>\
         <p:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>\
         </p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    )
}

/// A notes page: the note text in the body placeholder, one paragraph per
/// line the author wrote.
fn notes_xml(text: &str) -> String {
    let paragraphs: String = text
        .lines()
        .map(|l| {
            let l = l.trim_end();
            if l.is_empty() {
                "<a:p/>".to_string()
            } else {
                format!("<a:p><a:r><a:t>{}</a:t></a:r></a:p>", xml_escape(l))
            }
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:notes {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}\
         <p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr>\
         <p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr><p:spPr/>\
         <p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>\
         </p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"
    )
}

/// Builds the whole `.pptx` from one PNG per slide and the notes beside each.
/// `w`/`h` are the deck's slide size in CSS pixels.
pub fn package(w: u32, h: u32, slides: &[(Vec<u8>, Option<String>)]) -> Vec<u8> {
    let cx = u64::from(w) * EMU_PER_PX;
    let cy = u64::from(h) * EMU_PER_PX;
    let n = slides.len();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let text = |s: String| s.into_bytes();

    // [Content_Types].xml
    let mut overrides = String::new();
    let over = |part: &str, ty: &str| {
        format!("<Override PartName=\"{part}\" ContentType=\"application/vnd.openxmlformats-officedocument.{ty}+xml\"/>")
    };
    overrides.push_str(&over(
        "/ppt/presentation.xml",
        "presentationml.presentation.main",
    ));
    overrides.push_str(&over(
        "/ppt/slideMasters/slideMaster1.xml",
        "presentationml.slideMaster",
    ));
    overrides.push_str(&over(
        "/ppt/slideLayouts/slideLayout1.xml",
        "presentationml.slideLayout",
    ));
    overrides.push_str(&over(
        "/ppt/notesMasters/notesMaster1.xml",
        "presentationml.notesMaster",
    ));
    overrides.push_str(&over("/ppt/theme/theme1.xml", "theme"));
    overrides.push_str(&over("/ppt/theme/theme2.xml", "theme"));
    for i in 1..=n {
        overrides.push_str(&over(
            &format!("/ppt/slides/slide{i}.xml"),
            "presentationml.slide",
        ));
        if slides[i - 1].1.is_some() {
            overrides.push_str(&over(
                &format!("/ppt/notesSlides/notesSlide{i}.xml"),
                "presentationml.notesSlide",
            ));
        }
    }
    files.push((
        "[Content_Types].xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Default Extension=\"png\" ContentType=\"image/png\"/>\
             {overrides}</Types>"
        )),
    ));

    files.push((
        "_rels/.rels".into(),
        text(rels(&[(
            "rId1".into(),
            REL_OFFICE,
            "ppt/presentation.xml".into(),
        )])),
    ));

    // presentation.xml and its relationships.
    let sld_ids: String = (1..=n)
        .map(|i| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + i, i + 2))
        .collect();
    files.push((
        "ppt/presentation.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:presentation {NS}>\
             <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
             <p:notesMasterIdLst><p:notesMasterId r:id=\"rId2\"/></p:notesMasterIdLst>\
             <p:sldIdLst>{sld_ids}</p:sldIdLst>\
             <p:sldSz cx=\"{cx}\" cy=\"{cy}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/>\
             </p:presentation>"
        )),
    ));
    let mut pres_rels: Vec<(String, &str, String)> = vec![
        (
            "rId1".into(),
            REL_MASTER,
            "slideMasters/slideMaster1.xml".into(),
        ),
        (
            "rId2".into(),
            REL_NOTES_MASTER,
            "notesMasters/notesMaster1.xml".into(),
        ),
    ];
    for i in 1..=n {
        pres_rels.push((
            format!("rId{}", i + 2),
            REL_SLIDE,
            format!("slides/slide{i}.xml"),
        ));
    }
    files.push((
        "ppt/_rels/presentation.xml.rels".into(),
        text(rels(&pres_rels)),
    ));

    // The master, its one blank layout, the notes master, and their themes.
    files.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:sldMaster {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}</p:spTree></p:cSld>{CLR_MAP}\
             <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
             </p:sldMaster>"
        )),
    ));
    files.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        text(rels(&[
            (
                "rId1".into(),
                REL_LAYOUT,
                "../slideLayouts/slideLayout1.xml".into(),
            ),
            ("rId2".into(), REL_THEME, "../theme/theme1.xml".into()),
        ])),
    ));
    files.push((
        "ppt/slideLayouts/slideLayout1.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:sldLayout {NS} type=\"blank\" preserve=\"1\">\
             <p:cSld name=\"Blank\"><p:spTree>{EMPTY_TREE_HEAD}</p:spTree></p:cSld>\
             <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
        )),
    ));
    files.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        text(rels(&[(
            "rId1".into(),
            REL_MASTER,
            "../slideMasters/slideMaster1.xml".into(),
        )])),
    ));
    files.push((
        "ppt/notesMasters/notesMaster1.xml".into(),
        text(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:notesMaster {NS}><p:cSld><p:spTree>{EMPTY_TREE_HEAD}</p:spTree></p:cSld>{CLR_MAP}\
             </p:notesMaster>"
        )),
    ));
    files.push((
        "ppt/notesMasters/_rels/notesMaster1.xml.rels".into(),
        text(rels(&[(
            "rId1".into(),
            REL_THEME,
            "../theme/theme2.xml".into(),
        )])),
    ));
    files.push(("ppt/theme/theme1.xml".into(), text(theme_xml())));
    files.push(("ppt/theme/theme2.xml".into(), text(theme_xml())));

    // The slides, their pictures, and the notes beside them.
    for (i, (png, notes)) in slides.iter().enumerate() {
        let i = i + 1;
        files.push((
            format!("ppt/slides/slide{i}.xml"),
            text(slide_xml(cx, cy, bottom_crop(png, w, h))),
        ));
        let mut slide_rels: Vec<(String, &str, String)> = vec![
            (
                "rId1".into(),
                REL_LAYOUT,
                "../slideLayouts/slideLayout1.xml".into(),
            ),
            ("rId2".into(), REL_IMAGE, format!("../media/image{i}.png")),
        ];
        if notes.is_some() {
            slide_rels.push((
                "rId3".into(),
                REL_NOTES_SLIDE,
                format!("../notesSlides/notesSlide{i}.xml"),
            ));
        }
        files.push((
            format!("ppt/slides/_rels/slide{i}.xml.rels"),
            text(rels(&slide_rels)),
        ));
        files.push((format!("ppt/media/image{i}.png"), png.clone()));
        if let Some(note) = notes {
            files.push((
                format!("ppt/notesSlides/notesSlide{i}.xml"),
                text(notes_xml(note)),
            ));
            files.push((
                format!("ppt/notesSlides/_rels/notesSlide{i}.xml.rels"),
                text(rels(&[
                    (
                        "rId1".into(),
                        REL_NOTES_MASTER,
                        "../notesMasters/notesMaster1.xml".into(),
                    ),
                    ("rId2".into(), REL_SLIDE, format!("../slides/slide{i}.xml")),
                ])),
            ));
        }
    }

    crate::skill::zip::archive_bytes(&files)
}

/// Speaker notes arrive as rendered HTML; PowerPoint's notes pane wants text.
/// Paragraphs and `<br>` become line breaks, tags go, entities come back.
pub fn notes_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find('>') else {
            break;
        };
        let tag = &rest[at + 1..at + end];
        let name = tag
            .trim_start_matches('/')
            .split([' ', '/'])
            .next()
            .unwrap_or("");
        if matches!(name, "p" | "br" | "li" | "div") && !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        rest = &rest[at + end + 1..];
    }
    out.push_str(rest);
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-pixel PNG, enough for the package tests.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R', 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0, b'I', b'E', b'N',
        b'D', 0xAE, 0x42, 0x60, 0x82,
    ];

    fn names(zip: &[u8]) -> String {
        String::from_utf8_lossy(zip).to_string()
    }

    #[test]
    fn the_package_holds_every_required_part() {
        let zip = package(
            1280,
            720,
            &[
                (PNG.to_vec(), Some("say hello".to_string())),
                (PNG.to_vec(), None),
            ],
        );
        let text = names(&zip);
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/notesMasters/notesMaster1.xml",
            "ppt/theme/theme1.xml",
            "ppt/theme/theme2.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
            "ppt/media/image1.png",
            "ppt/media/image2.png",
            "ppt/notesSlides/notesSlide1.xml",
        ] {
            assert!(text.contains(part), "missing {part}");
        }
        // The second slide has no notes, so no second notes part.
        assert!(!text.contains("notesSlide2.xml"));
        // 16:9 at 1280x720 is the standard PowerPoint size, in EMUs.
        assert!(text.contains("<p:sldSz cx=\"12192000\" cy=\"6858000\"/>"));
        assert!(text.contains("say hello"));
    }

    /// The screenshot canvas is the window, and headless windows are taller
    /// than their viewport by chrome nothing draws — the crop is what puts
    /// the slide's own aspect back. A picture already at the right aspect is
    /// embedded whole.
    #[test]
    fn the_blank_strip_below_the_slide_is_cropped_by_aspect() {
        // A fake 2560x1740 PNG header: 1440 rows of slide, 300 of strip.
        let mut png = vec![
            0x89, b'P', b'N', b'G', 0, 0, 0, 0, 0, 0, 0, 0, b'I', b'H', b'D', b'R',
        ];
        png.extend_from_slice(&2560u32.to_be_bytes());
        png.extend_from_slice(&1740u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((2560, 1740)));
        let crop = bottom_crop(&png, 1280, 720);
        // (1740 - 1440) / 1740 of the height, in thousandths of a percent.
        assert_eq!(crop, 17241);
        assert!(slide_xml(1, 1, crop).contains("<a:srcRect b=\"17241\"/>"));
        assert!(!slide_xml(1, 1, 0).contains("srcRect"));

        // Already the right shape: nothing to crop.
        let mut exact = png.clone();
        exact[20..24].copy_from_slice(&1440u32.to_be_bytes());
        assert_eq!(bottom_crop(&exact, 1280, 720), 0);
        // Not a PNG: embedded whole rather than guessed at.
        assert_eq!(bottom_crop(b"GIF89a", 1280, 720), 0);
    }

    #[test]
    fn note_text_is_escaped_into_the_xml() {
        let xml = notes_xml("a < b & \"c\"\nsecond");
        assert!(xml.contains("a &lt; b &amp; &quot;c&quot;"));
        assert_eq!(xml.matches("<a:p>").count(), 2);
    }

    #[test]
    fn notes_html_becomes_lines_of_text() {
        assert_eq!(
            notes_text("<p>one</p><p>two &amp; three</p>"),
            "one\ntwo & three"
        );
        assert_eq!(notes_text("a<br>b"), "a\nb");
        assert_eq!(notes_text("plain"), "plain");
    }
}
