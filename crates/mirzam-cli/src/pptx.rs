//! `mirzam export pptx` — the deck as a PowerPoint file with the words in
//! it: text boxes where the slide put them, in the font, size and colour the
//! browser resolved; cards and code blocks as filled shapes; tables as
//! tables; the speaker notes in the notes pane. What has no PowerPoint
//! equivalent — a chart, a diagram, a formula, a picture in a format
//! PowerPoint may not open — is photographed, one picture per element, with
//! everything else on the slide hidden while the shot is taken, so nothing
//! is drawn twice and nothing is lost.
//!
//! The layout is the browser's. Each slide is opened in a headless Chromium
//! over DevTools (the client `export video` already carries), the extractor
//! `pptx.js` reads the laid-out DOM back as a scene, and `mirzam-pptx`
//! writes the scene as OOXML. The split is the same as `import pdf`'s: the
//! part that decides what a scene means is a pure crate, testable against
//! scenes written by hand; this module is the part that drives a browser.
//!
//! `--pictures` keeps the earlier form — one photograph per slide — for the
//! deck whose reader must see exactly what the browser drew and will never
//! edit it.

use crate::{apply_deck_overrides, cdp, find_chromium, DeckArgs};
use mirzam_pptx::{data_uri_media, Media, Slide};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

const EXTRACT_JS: &str = include_str!("pptx.js");

/// Device pixels per CSS pixel for the photographs: sharp on a projector,
/// without the file size of the full-slide pictures the first export took.
const RASTER_SCALE: f64 = 2.0;

/// JPEG quality for a photographed photograph — the one raster kind where a
/// lossy encode is the right trade.
const JPEG_QUALITY: u32 = 90;

pub(crate) struct PptxArgs {
    pub(crate) deck: DeckArgs,
    pub(crate) chromium: Option<String>,
    /// `--pictures`: one photograph per slide, nothing editable.
    pub(crate) pictures: bool,
}

pub(crate) fn export_pptx(input: &Path, out_path: &Path, args: &PptxArgs) -> Result<(), String> {
    let t0 = Instant::now();
    let mut cache = HashMap::new();
    let mut out = mirzam_cli::pipeline::build_deck_with(input, &mut cache, args.deck.split, None)?;
    apply_deck_overrides(&mut out, &args.deck)?;
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }
    if out.sections.is_empty() {
        return Err("the deck has no slides".into());
    }
    let bin = find_chromium(args.chromium.as_deref())?;
    let (w, h) = out.meta.slide_size();

    let browser = cdp::Browser::launch(&bin, w + 100, h + 400)?;
    let cdp = &browser.cdp;
    cdp.attach()?;
    cdp.call("Page.enable", json!({}))?;
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width": w, "height": h, "deviceScaleFactor": 1, "mobile": false}),
    )?;

    let tmp = std::env::temp_dir();
    let pid = std::process::id();
    let mut slides: Vec<Slide> = Vec::with_capacity(out.sections.len());
    let mut media: HashMap<u32, Media> = HashMap::new();
    let mut next_media: u32 = 1;
    let mut photographed = 0usize;

    for (i, section) in out.sections.iter().enumerate() {
        let (slide_html, notes) = mirzam_render::split_notes(section);
        let page = mirzam_render::assemble_shot_page(&out.meta, &slide_html, &out.file_themes);
        let html_path = tmp.join(format!("mirzam-pptx-{pid}-{i}.html"));
        std::fs::write(&html_path, &page)
            .map_err(|e| format!("cannot write a temporary page: {e}"))?;
        let result = extract_slide(cdp, &html_path, args.pictures, &mut next_media, &mut media);
        let _ = std::fs::remove_file(&html_path);
        let mut slide = result.map_err(|e| format!("slide {}: {e}", i + 1))?;
        photographed += slide.rasters.iter().filter(|r| r.kind != "data").count();
        slide.notes = notes
            .map(|n| mirzam_pptx::notes_text(&n))
            .filter(|t| !t.is_empty());
        slides.push(slide);
    }

    let bytes = mirzam_pptx::package(w, h, &slides, &media);
    std::fs::write(out_path, &bytes).map_err(|e| format!("cannot write the pptx: {e}"))?;
    let shapes: usize = slides
        .iter()
        .map(|s| {
            s.nodes
                .iter()
                .filter(|n| !matches!(n, mirzam_pptx::Node::Picture(_)))
                .count()
        })
        .sum();
    println!(
        "✓ wrote {} slides to {} ({} shapes, {} pictures of which {} photographed; {} ms, {} KB)",
        slides.len(),
        out_path.display(),
        shapes,
        media.len(),
        photographed,
        t0.elapsed().as_millis(),
        bytes.len() / 1024,
    );
    Ok(())
}

/// Opens one shot page in the attached tab, waits for it to settle, runs the
/// extractor, and photographs what it asked for. Media ids are renumbered
/// across the deck as they are handed out, so the map holds every slide's.
fn extract_slide(
    cdp: &cdp::Cdp,
    html_path: &Path,
    pictures: bool,
    next_media: &mut u32,
    media: &mut HashMap<u32, Media>,
) -> Result<Slide, String> {
    cdp.call(
        "Page.navigate",
        json!({"url": format!("file://{}", html_path.display())}),
    )?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let ready = cdp
            .eval(
                "document.readyState === 'complete' && !!document.querySelector('section.slide')",
            )?
            .as_bool()
            .unwrap_or(false);
        if ready {
            break;
        }
        if Instant::now() > deadline {
            return Err("the page never finished loading".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // Fonts, images, and the scripts the shot page runs on load
    // (shrink-to-fit, annotations) all move boxes; the scene is read only
    // once two frames have painted after the last of them.
    let settled = cdp.call(
        "Runtime.evaluate",
        json!({
            "expression": "(async () => {\
                await document.fonts.ready;\
                await Promise.all([...document.images].filter(i => !i.complete).map(i => new Promise(r => { i.addEventListener('load', r, {once: true}); i.addEventListener('error', r, {once: true}); })));\
                await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));\
                return true; })()",
            "awaitPromise": true,
            "returnByValue": true,
        }),
    )?;
    if let Some(text) = settled["exceptionDetails"]["exception"]["description"].as_str() {
        return Err(format!("the page threw while settling: {text}"));
    }
    cdp.eval(EXTRACT_JS)?;
    let scene = cdp.eval(&format!(
        "window.mzScene({{pictures: {}}})",
        if pictures { "true" } else { "false" }
    ))?;
    let scene = scene.as_str().ok_or("the extractor returned no scene")?;
    let origin = serde_json::from_str::<serde_json::Value>(scene)
        .ok()
        .map(|v| {
            (
                v["origin"]["x"].as_f64().unwrap_or(0.0),
                v["origin"]["y"].as_f64().unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0));
    // `MIRZAM_PPTX_SCENE_DIR=<dir>` keeps each slide's scene as JSON beside
    // the export: the one way to see what the extractor read when a box
    // lands somewhere surprising.
    if let Some(dir) = std::env::var_os("MIRZAM_PPTX_SCENE_DIR") {
        let name = html_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let _ = std::fs::write(Path::new(&dir).join(format!("{name}.json")), scene);
    }
    let mut slide = Slide::from_json(scene)?;

    // Renumber this slide's pictures into the deck-wide media map, taking
    // each photograph with the rest of the slide hidden.
    let mut renumber: HashMap<u32, u32> = HashMap::new();
    let mut in_pass = false;
    for raster in &slide.rasters {
        let id = *next_media;
        *next_media += 1;
        renumber.insert(raster.id, id);
        if raster.kind == "data" {
            if let Some(m) = raster.data.as_deref().and_then(data_uri_media) {
                media.insert(id, m);
            }
            continue;
        }
        let Some(rect) = raster.rect else {
            continue;
        };
        // A `page` shot is the slide as it stands; every other kind is one
        // element alone on a transparent page.
        if raster.mode != "page" {
            if !in_pass {
                cdp.call(
                    "Emulation.setDefaultBackgroundColorOverride",
                    json!({"color": {"r": 0, "g": 0, "b": 0, "a": 0}}),
                )?;
                in_pass = true;
            }
            cdp.eval(&format!("window.mzShow({}, '{}')", raster.id, raster.mode))?;
        }
        let jpeg = raster.kind == "jpeg";
        let mut params = json!({
            "format": if jpeg { "jpeg" } else { "png" },
            "clip": {
                "x": origin.0 + rect.x,
                "y": origin.1 + rect.y,
                "width": rect.w,
                "height": rect.h,
                "scale": RASTER_SCALE,
            },
            "captureBeyondViewport": true,
            "fromSurface": true,
        });
        if jpeg {
            params["quality"] = json!(JPEG_QUALITY);
        }
        let shot = cdp.call("Page.captureScreenshot", params)?;
        let data = shot["data"]
            .as_str()
            .ok_or("Page.captureScreenshot returned no data")?;
        let bytes = cdp::base64_decode(data)?;
        media.insert(
            id,
            Media {
                bytes,
                ext: if jpeg { "jpeg" } else { "png" },
            },
        );
    }
    if in_pass {
        cdp.eval("window.mzHide()")?;
        cdp.call("Emulation.setDefaultBackgroundColorOverride", json!({}))?;
    }
    for node in &mut slide.nodes {
        if let mirzam_pptx::Node::Picture(p) = node {
            if let Some(id) = renumber.get(&p.image) {
                p.image = *id;
            }
        }
    }
    Ok(slide)
}
