//! `mirzam export video`: the deck as a silent WebM — the autoplay loop,
//! filmed. Each slide is photographed by headless Chromium exactly as
//! `export pptx` photographs it, held on screen for its autoplay dwell, and
//! the frames handed to an ffmpeg to encode as VP8 in WebM: the one
//! container YouTube takes that a Chromium build without proprietary codecs
//! can also play back, which is why the sample decks are `.webm` too.
//!
//! The ffmpeg is found rather than shipped, and the bar is set deliberately
//! low: the trimmed build Playwright keeps beside its browsers — present on
//! any machine that has run Playwright, and probed for here by path — can
//! decode MJPEG, encode VP8 and mux WebM, and nothing more. So the frames
//! cross the pipe as JPEG (re-packed from Chromium's PNG in pure Rust), the
//! output is WebM and only WebM, and a full ffmpeg simply also qualifies.
//! What no ffmpeg gets is a clear error saying which three capabilities the
//! command needs.
//!
//! Pacing is the viewer's own: the frontmatter's `autoplay:` interval, a
//! slide's `<!-- autoplay: 20s -->` dwell overriding it, and `--interval`
//! standing in for the deck-level pace from the command line. What the film
//! does not carry — yet — is what a screenshot cannot: click-step
//! animations play out (every step arrives revealed, as in the PDF), clips
//! inside slides become their poster stills, and there is no audio track.

use crate::{apply_deck_overrides, find_chromium, photograph_slides, DeckArgs};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

/// Frames per second of the encoded video. The slides are stills, so the
/// rate buys nothing but timing granularity: at 10, a dwell lands on its
/// nearest 100ms — the same floor `parse_autoplay` holds intervals to.
const FPS: u32 = 10;

/// How wide the video is, in pixels. Chromium photographs each slide at
/// whatever device scale turns the deck's CSS width into this: 1.5 for a
/// 16:9 deck (1920x1080), 1.875 for 4:3 (1920x1440).
const TARGET_WIDTH: u32 = 1920;

/// How long a slide holds when nothing says otherwise: no `autoplay:` in the
/// frontmatter, no `--interval`, no dwell of its own. Five seconds reads a
/// headline and a sentence.
const DEFAULT_DWELL_MS: u32 = 5000;

/// Everything `export video` was asked for: the deck-shaping flags every
/// export takes, the two browser/encoder locations, and the deck-level pace.
pub(crate) struct VideoArgs {
    pub(crate) deck: DeckArgs,
    pub(crate) chromium: Option<String>,
    pub(crate) ffmpeg: Option<String>,
    /// `--interval <dur>`: stands in for the frontmatter's `autoplay:`
    /// interval. A slide's own `<!-- autoplay: -->` dwell still wins — it is
    /// the slide saying it needs reading time, whatever the deck's pace.
    pub(crate) interval_ms: Option<u32>,
}

pub(crate) fn export_video(input: &Path, out_path: &Path, args: &VideoArgs) -> Result<(), String> {
    let t0 = Instant::now();
    if out_path.extension().and_then(|e| e.to_str()) != Some("webm") {
        return Err(format!(
            "export video writes WebM, so the output must end in .webm, not {} - \
             the encoder this command can count on (the trimmed ffmpeg Playwright \
             installs) carries VP8/WebM and no other codec, and YouTube takes \
             WebM directly",
            out_path.display()
        ));
    }
    let mut cache = HashMap::new();
    let mut out = mirzam_cli::pipeline::build_deck_with(input, &mut cache, args.deck.split, None)?;
    apply_deck_overrides(&mut out, &args.deck)?;
    for w in &out.warnings {
        println!("  ⚠ {w}");
    }
    if out.sections.is_empty() {
        return Err("the deck has no slides".into());
    }

    let deck_interval = out
        .meta
        .autoplay
        .as_deref()
        .and_then(|a| mirzam_anim::parse_autoplay(a).ok())
        .map(|a| a.interval_ms);
    let dwells: Vec<u32> = out
        .sections
        .iter()
        .map(|s| slide_dwell_ms(s, args.interval_ms, deck_interval))
        .collect();

    // Find both externals before an hour of photography, not after: the
    // missing-ffmpeg error should cost nothing but the build above.
    let chromium = find_chromium(args.chromium.as_deref())?;
    let ffmpeg = find_ffmpeg(args.ffmpeg.as_deref())?;

    let (w, _h) = out.meta.slide_size();
    let scale = f64::from(TARGET_WIDTH) / f64::from(w);
    let shots = photograph_slides(&chromium, &out, scale)?;

    let mut frames: Vec<Vec<u8>> = Vec::with_capacity(shots.len());
    for (i, (png, _notes)) in shots.iter().enumerate() {
        frames.push(
            frame_jpeg(png, out.meta.slide_size()).map_err(|e| format!("slide {}: {e}", i + 1))?,
        );
    }

    encode_webm(&ffmpeg, &frames, &dwells, out_path)?;

    let total_ms: u64 = dwells.iter().map(|&d| u64::from(d)).sum();
    let size = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "✓ wrote {} slides as {:.1}s of video to {} ({} ms, {} KB)",
        frames.len(),
        total_ms as f64 / 1000.0,
        out_path.display(),
        t0.elapsed().as_millis(),
        size / 1024,
    );
    Ok(())
}

/// How long one slide stays on screen. The slide's own `<!-- autoplay: -->`
/// dwell wins, then `--interval`, then the frontmatter's `autoplay:`
/// interval, then the default — the same precedence the viewer gives them,
/// with the flag standing where `?autoplay=` stands in a URL.
fn slide_dwell_ms(section: &str, flag: Option<u32>, deck: Option<u32>) -> u32 {
    section_dwell(section)
        .or(flag)
        .or(deck)
        .unwrap_or(DEFAULT_DWELL_MS)
}

/// Reads `data-dwell="…"` off a rendered section's opening tag — the same
/// attribute the viewer reads, written by the renderer from the slide's
/// `<!-- autoplay: 20s -->`. Only the opening tag is searched, so a code
/// sample showing the attribute never sets the pace.
fn section_dwell(section: &str) -> Option<u32> {
    let tag_end = section.find('>')?;
    let tag = &section[..tag_end];
    let at = tag.find(" data-dwell=\"")? + " data-dwell=\"".len();
    let rest = &tag[at..];
    rest[..rest.find('"')?].parse().ok()
}

/// One slide's screenshot, ready to cross the pipe: the PNG decoded, the
/// blank strip below the slide cropped away (`photograph_slides` says why it
/// is there), both dimensions floored to even for the encoder's 4:2:0
/// sampling, and the pixels re-packed as JPEG at a quality where flat slide
/// color and text survive — the trimmed ffmpeg reads nothing else.
fn frame_jpeg(png: &[u8], (slide_w, slide_h): (u32, u32)) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("unreadable screenshot: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("unreadable screenshot: {e}"))?;
    let (pw, ph) = (info.width, info.height);
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => return Err(format!("screenshot in unexpected color type {other:?}")),
    };

    // Where the slide ends and the blank strip begins, from the slide's own
    // aspect and the width the shot actually came out at.
    let want = (f64::from(pw) * f64::from(slide_h) / f64::from(slide_w)).round() as u32;
    let crop_h = want.min(ph) & !1;
    let crop_w = pw & !1;

    let mut rgb = Vec::with_capacity((crop_w * crop_h * 3) as usize);
    let stride = (pw * channels) as usize;
    for row in buf[..stride * crop_h as usize].chunks_exact(stride) {
        for px in row[..(crop_w * channels) as usize].chunks_exact(channels as usize) {
            rgb.extend_from_slice(&px[..3]);
        }
    }

    let mut jpg = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut jpg, 90);
    encoder
        .encode(
            &rgb,
            crop_w as u16,
            crop_h as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .map_err(|e| format!("cannot pack the frame as JPEG: {e}"))?;
    Ok(jpg)
}

/// How many video frames a dwell is: its nearest count at `FPS`, and never
/// zero — a slide asked for is a slide in the film.
fn frame_count(dwell_ms: u32) -> u32 {
    ((u64::from(dwell_ms) * u64::from(FPS) + 500) / 1000).max(1) as u32
}

/// Feeds the frames through ffmpeg into `out_path`. Each JPEG is written
/// once per frame of its dwell: the piped image stream is the only demuxer
/// the trimmed ffmpeg has that can carry them, and it has no per-frame
/// durations, so duration is spelled in copies. The copies are cheap twice
/// over — a slide's JPEG is small, and an encoder given an unchanged frame
/// emits almost nothing.
fn encode_webm(
    ffmpeg: &str,
    frames: &[Vec<u8>],
    dwells: &[u32],
    out_path: &Path,
) -> Result<(), String> {
    let out_abs = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(out_path);
    let mut child = std::process::Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "-framerate",
            &FPS.to_string(),
            "-i",
            // `pipe:0`, never `-`: ffmpeg 7 reads `-` as its `fd:` protocol,
            // which the trimmed Playwright build compiles out; the `pipe:`
            // protocol is in every build, that one included.
            "pipe:0",
            "-vcodec",
            "libvpx",
            "-pix_fmt",
            "yuv420p",
            // Constrained quality: `-crf` sets the quality slides need for
            // legible text, `-b:v` caps what a pathological deck could ask.
            "-crf",
            "8",
            "-b:v",
            "6M",
            "-qmin",
            "0",
            "-qmax",
            "42",
        ])
        .arg(&out_abs)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        // Inherited on purpose: `-loglevel error` keeps a clean run silent,
        // and a failing encode explains itself without this re-relaying it.
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("cannot run {ffmpeg}: {e}"))?;

    let mut stdin = child.stdin.take().expect("stdin was piped");
    let mut write_err = None;
    'feed: for (jpg, &dwell) in frames.iter().zip(dwells) {
        for _ in 0..frame_count(dwell) {
            if let Err(e) = stdin.write_all(jpg) {
                // A dead pipe means ffmpeg already failed; its own stderr —
                // inherited above — says why, and the wait below carries the
                // status. The write error itself is just the echo.
                write_err = Some(e);
                break 'feed;
            }
        }
    }
    drop(stdin);
    let status = child
        .wait()
        .map_err(|e| format!("cannot wait for {ffmpeg}: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg failed to encode the video ({status})"));
    }
    if let Some(e) = write_err {
        return Err(format!("ffmpeg stopped reading frames: {e}"));
    }
    Ok(())
}

/// Locates an ffmpeg that can do the job: `--ffmpeg`, then $MIRZAM_FFMPEG —
/// both taken at their word — then `ffmpeg` on PATH, then the trimmed build
/// Playwright keeps beside its browsers, under $PLAYWRIGHT_BROWSERS_PATH or
/// its default install. The found candidates are probed for the one encoder
/// this needs (`scripts/record-demo.mjs` probes the same way, for the same
/// reason: the failure of a bare existence check is a codec error after the
/// photography already succeeded).
fn find_ffmpeg(explicit: Option<&str>) -> Result<String, String> {
    if let Some(f) = explicit {
        return Ok(f.to_string());
    }
    if let Ok(f) = std::env::var("MIRZAM_FFMPEG") {
        return Ok(f);
    }
    let mut candidates: Vec<String> = vec!["ffmpeg".into()];
    let root =
        std::env::var("PLAYWRIGHT_BROWSERS_PATH").unwrap_or_else(|_| default_playwright_root());
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            if !e.file_name().to_string_lossy().starts_with("ffmpeg") {
                continue;
            }
            for name in ["ffmpeg-linux", "ffmpeg-mac", "ffmpeg.exe"] {
                let p = e.path().join(name);
                if p.exists() {
                    candidates.push(p.to_string_lossy().into_owned());
                }
            }
        }
    }
    for c in &candidates {
        if encodes_vp8(c) {
            return Ok(c.clone());
        }
    }
    Err(
        "no usable ffmpeg found: export video needs one that reads piped MJPEG and \
         encodes VP8 into WebM. Install ffmpeg, or point --ffmpeg or MIRZAM_FFMPEG \
         at one - the trimmed build Playwright installs beside its browsers is \
         enough, and is looked for automatically"
            .into(),
    )
}

/// Where Playwright puts browsers when $PLAYWRIGHT_BROWSERS_PATH is unset.
fn default_playwright_root() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    #[cfg(target_os = "macos")]
    return format!("{home}/Library/Caches/ms-playwright");
    #[cfg(not(target_os = "macos"))]
    format!("{home}/.cache/ms-playwright")
}

/// Whether this candidate has the VP8 encoder — the capability that actually
/// separates a build that can make this WebM from one that cannot.
fn encodes_vp8(bin: &str) -> bool {
    std::process::Command::new(bin)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("libvpx"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dwell_precedence_is_slide_then_flag_then_deck_then_default() {
        let plain = "<section class=\"slide\">\n<p>hi</p></section>";
        let held = "<section class=\"slide\" data-dwell=\"20000\">\n<p>hi</p></section>";
        assert_eq!(slide_dwell_ms(held, Some(3000), Some(8000)), 20000);
        assert_eq!(slide_dwell_ms(plain, Some(3000), Some(8000)), 3000);
        assert_eq!(slide_dwell_ms(plain, None, Some(8000)), 8000);
        assert_eq!(slide_dwell_ms(plain, None, None), DEFAULT_DWELL_MS);
    }

    #[test]
    fn dwell_in_a_code_sample_is_not_a_dwell() {
        // The attribute shown as text sits past the opening tag, so only the
        // real one counts.
        let s = "<section class=\"slide\">\n<code>data-dwell=\"9\"</code></section>";
        assert_eq!(section_dwell(s), None);
    }

    #[test]
    fn frame_counts_round_to_the_nearest_frame_and_never_zero() {
        assert_eq!(frame_count(5000), 50);
        assert_eq!(frame_count(750), 8); // 7.5 frames rounds up
        assert_eq!(frame_count(100), 1);
        assert_eq!(frame_count(1), 1); // a floor, not a rounding accident
    }

    #[test]
    fn frames_crop_to_the_slide_and_come_out_even() {
        // A 64x50 "screenshot" of a 1280x720 slide at 1/20 scale: the slide
        // occupies the top 36 rows, the rest is the blank viewport strip.
        let mut png_bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_bytes, 64, 50);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            writer.write_image_data(&vec![200u8; 64 * 50 * 4]).unwrap();
        }
        let jpg = frame_jpeg(&png_bytes, (1280, 720)).unwrap();
        // SOF0 carries the dimensions; find it and read them back.
        let sof = jpg.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        let h = u16::from_be_bytes([jpg[sof + 5], jpg[sof + 6]]);
        let w = u16::from_be_bytes([jpg[sof + 7], jpg[sof + 8]]);
        assert_eq!((w, h), (64, 36));
    }
}
