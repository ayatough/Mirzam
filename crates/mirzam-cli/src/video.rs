//! `mirzam export video`: the deck as a silent WebM — the autoplay loop,
//! filmed. By default the film is a *recording*: the built deck, viewer and
//! all, playing itself in a headless Chromium while the DevTools screencast
//! hands back every painted frame — so click-step animations play out, the
//! deck's `transition:` draws between pages, and an embedded clip runs on
//! screen. `--stills` keeps the photographic mode instead: one screenshot
//! per slide, exactly as `export pptx` takes them, held for its dwell —
//! faster than real time and deterministic, at the price of everything that
//! moves.
//!
//! Either way the frames end as VP8 in WebM: the one container YouTube
//! takes that a Chromium build without proprietary codecs can also play
//! back, which is why the sample decks are `.webm` too.
//!
//! The ffmpeg is found rather than shipped, and the bar is set deliberately
//! low: the trimmed build Playwright keeps beside its browsers — present on
//! any machine that has run Playwright, and probed for here by path — can
//! decode MJPEG, encode VP8 and mux WebM, and nothing more. So the frames
//! cross the pipe as JPEG (the screencast's native wire format; the stills
//! re-packed from Chromium's PNG in pure Rust), the output is WebM and only
//! WebM, and a full ffmpeg simply also qualifies. What no ffmpeg gets is a
//! clear error saying which three capabilities the command needs.
//!
//! Pacing is the viewer's own — literally, in a recording: the deck is
//! walked once to warm the caches, started from the top through the
//! viewer's `mz-autoplay` hook, and filmed until the viewer says it has
//! played through (the `data-mz-autoplay-done` attribute its autoplay sets
//! when a non-looping run rests). The frontmatter's `autoplay:` interval
//! supplies that pace, `--interval` stands in for it, and a slide's
//! `<!-- autoplay: 20s -->` dwell, a clip holding its page, and every click
//! step are the viewer's business, exactly as in a browser. A `loop` deck
//! is filmed for one pass: a loop has no end to record. What neither mode
//! carries is audio — the film is pictures only.

use crate::{apply_deck_overrides, cdp, find_chromium, photograph_slides, DeckArgs};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// Frames per second of a stills export. The slides are stills, so the rate
/// buys nothing but timing granularity: at 10, a dwell lands on its nearest
/// 100ms — the same floor `parse_autoplay` holds intervals to.
const STILLS_FPS: u32 = 10;

/// Frames per second of a recording, where motion is the point.
const RECORD_FPS: u32 = 30;

/// JPEG quality asked of the screencast. 90 keeps slide text legible
/// through the VP8 encode behind it; the stills path packs its own JPEG at
/// the same number.
const JPEG_QUALITY: u32 = 90;

/// How wide the video is, in pixels. Chromium renders each frame at
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
    /// `--stills`: photograph each slide once instead of recording the live
    /// viewer — faster than real time, and still enough for a deck where
    /// nothing moves.
    pub(crate) stills: bool,
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

    // Find both externals before minutes of filming, not after: the
    // missing-ffmpeg error should cost nothing but the build above.
    let chromium = find_chromium(args.chromium.as_deref())?;
    let ffmpeg = find_ffmpeg(args.ffmpeg.as_deref())?;

    let (secs, verb) = if args.stills {
        (
            export_stills(&out, &dwells, &chromium, &ffmpeg, out_path)?,
            "wrote",
        )
    } else {
        let interval = args
            .interval_ms
            .or(deck_interval)
            .unwrap_or(DEFAULT_DWELL_MS);
        (
            export_recording(&out, &dwells, interval, &chromium, &ffmpeg, out_path)?,
            "recorded",
        )
    };

    let size = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "✓ {verb} {} slides as {secs:.1}s of video to {} ({} ms, {} KB)",
        out.sections.len(),
        out_path.display(),
        t0.elapsed().as_millis(),
        size / 1024,
    );
    Ok(())
}

/// The photographic mode: one screenshot per slide, held for its dwell.
/// Returns the seconds of video written.
fn export_stills(
    out: &mirzam_cli::pipeline::BuildOutput,
    dwells: &[u32],
    chromium: &str,
    ffmpeg: &str,
    out_path: &Path,
) -> Result<f64, String> {
    let (w, _h) = out.meta.slide_size();
    let scale = f64::from(TARGET_WIDTH) / f64::from(w);
    let shots = photograph_slides(chromium, out, scale)?;

    let mut frames: Vec<Vec<u8>> = Vec::with_capacity(shots.len());
    for (i, (png, _notes)) in shots.iter().enumerate() {
        frames.push(
            frame_jpeg(png, out.meta.slide_size()).map_err(|e| format!("slide {}: {e}", i + 1))?,
        );
    }

    let mut enc = Encoder::spawn(ffmpeg, STILLS_FPS, out_path)?;
    'feed: for (jpg, &dwell) in frames.iter().zip(dwells) {
        for _ in 0..frame_count(dwell, STILLS_FPS) {
            if enc.frame(jpg).is_err() {
                // A dead pipe means ffmpeg already failed; `finish` collects
                // its status and says so.
                break 'feed;
            }
        }
    }
    enc.finish()?;
    Ok(dwells.iter().map(|&d| f64::from(d)).sum::<f64>() / 1000.0)
}

/// The recording: the built deck — viewer and all — playing itself under
/// `?autoplay=`, filmed through the DevTools screencast until the viewer
/// marks the run played-through. Returns the seconds of video written.
fn export_recording(
    out: &mirzam_cli::pipeline::BuildOutput,
    dwells: &[u32],
    interval_ms: u32,
    chromium: &str,
    ffmpeg: &str,
    out_path: &Path,
) -> Result<f64, String> {
    // The page `build` writes, assets embedded, so it plays from anywhere.
    let opts = mirzam_render::PageOptions {
        live_version: None,
        file_themes: out.file_themes.clone(),
        debug_layout: false,
        all_themes: false,
        source: None,
    };
    let html = mirzam_render::assemble_page(&out.meta, &out.sections, &opts);
    let page_path = std::env::temp_dir().join(format!("mirzam-record-{}.html", std::process::id()));
    std::fs::write(&page_path, &html).map_err(|e| format!("cannot write a temporary page: {e}"))?;
    let result = record_page(
        &page_path,
        out,
        dwells,
        interval_ms,
        chromium,
        ffmpeg,
        out_path,
    );
    let _ = std::fs::remove_file(&page_path);
    result
}

fn record_page(
    page_path: &Path,
    out: &mirzam_cli::pipeline::BuildOutput,
    dwells: &[u32],
    interval_ms: u32,
    chromium: &str,
    ffmpeg: &str,
    out_path: &Path,
) -> Result<f64, String> {
    let (w, h) = out.meta.slide_size();
    let scale = f64::from(TARGET_WIDTH) / f64::from(w);

    let browser = cdp::Browser::launch(chromium, w, h + 300)?;
    let cdp = &browser.cdp;
    cdp.attach()?;
    cdp.call("Page.enable", json!({}))?;
    // The viewport is the slide, and the device scale turns its CSS pixels
    // into the video's: the viewer then fits the deck edge to edge, with
    // nothing to crop.
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width": w, "height": h, "deviceScaleFactor": scale, "mobile": false}),
    )?;

    // The deck is loaded stilled, walked once end to end, and only then
    // played for the camera — same document throughout. The walk is what
    // makes the film smooth: the first display of a slide decodes its
    // images, and on a machine rastering in software that can wedge the
    // renderer for seconds, which filmed live would be a frozen page turn.
    // Warmed, every arrival during the take re-uses the decoded images —
    // and staying in one document is what keeps those caches warm, so the
    // take starts through the viewer's own `mz-autoplay` hook rather than
    // a reload. The hook restarts from slide 1 by the loop's own wrap, so
    // its entrance and click steps play for the camera like everyone
    // else's, whatever state the walk left them in.
    cdp.call(
        "Page.navigate",
        json!({"url": format!("file://{}?autoplay=off&controls=none", page_path.display())}),
    )?;
    let load_deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let ready = cdp
            .eval("document.readyState === 'complete' && !!document.querySelector('#deck')")?
            .as_bool()
            .unwrap_or(false);
        if ready {
            break;
        }
        if Instant::now() > load_deadline {
            return Err("the deck never finished loading in the recording browser".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    for i in 2..=out.sections.len() {
        // Each eval queues behind whatever the last slide's arrival cost,
        // so the walk paces itself to the machine.
        cdp.eval(&format!("location.hash = '#{i}'"))?;
        std::thread::sleep(Duration::from_millis(300));
    }
    cdp.eval("location.hash = '#1'")?;
    // On a screen the deck sits inset — a margin, a corner radius, a shadow:
    // the page around the slide. A video has no page, so the take overrides
    // that presentation and pins the deck edge to edge; the viewport already
    // measures exactly one slide. `!important` outlives the viewer's own
    // `fit()`, which keeps writing its inline transform on resize.
    cdp.eval(
        "var st = document.createElement('style'); \
         st.textContent = '#deck { border-radius: 0 !important; box-shadow: none !important; \
         transform: translate(-50%, -50%) scale(1) !important; }'; \
         document.head.appendChild(st);",
    )?;
    // Long enough for the walk's last page turn to finish drawing: the take
    // must not open on the tail of a warm-up transition.
    std::thread::sleep(Duration::from_millis(2000));

    let mut enc = Encoder::spawn(ffmpeg, RECORD_FPS, out_path)?;
    cdp.call(
        "Page.startScreencast",
        json!({
            "format": "jpeg",
            "quality": JPEG_QUALITY,
            "maxWidth": (f64::from(w) * scale).round() as u32,
            "maxHeight": (f64::from(h) * scale).round() as u32,
            "everyNthFrame": 1,
        }),
    )?;

    // The page's own clock is the screencast's timestamp base; the take
    // begins where the play command lands.
    let t_start = clock(cdp)? - 0.05;
    cdp.eval(&format!(
        "document.dispatchEvent(new CustomEvent('mz-autoplay', {{detail: '{interval_ms}ms'}}))"
    ))?;

    // A ceiling on the recording, for the deck that never rests: generous,
    // because click steps multiply a slide's dwell and a clip can hold its
    // page for its own length — but not infinite, because a recorder that
    // never returns explains nothing.
    let cap =
        Duration::from_secs((dwells.iter().map(|&d| u64::from(d)).sum::<u64>() / 100).max(600));

    // Two frame sources, because headless has two behaviours. The
    // screencast delivers every frame the compositor produces — the cheap,
    // smooth source, and for main-thread animation it flows at full rate.
    // But under headless software compositing, a compositor-only animation
    // over freshly-rastered content — the page-turn fade onto a photograph
    // slide — stops producing frames a few in (verified against this repo's
    // own slideshow deck; fades onto photo-free slides played fine). So
    // while the page says something is moving and the screencast has gone
    // quiet anyway, `captureScreenshot` is used to *force* frames out: each
    // call makes the compositor draw, at whatever rate the machine can
    // manage. Stillness stays free — the forcer only runs while the page
    // reports live animations or playing media.
    let mut timeline = Timeline::new(RECORD_FPS);
    let started = Instant::now();
    let mut last_poll = Instant::now();
    let mut jpeg_checked = false;
    let mut moving = true; // the load itself animates; polls keep it current
    let mut last_frame = Instant::now();
    let mut check_frame = |jpg: &[u8]| -> Result<(), String> {
        if !jpeg_checked {
            jpeg_checked = true;
            if let Some((fw, fh)) = jpeg_dims(jpg) {
                if fw % 2 != 0 || fh % 2 != 0 {
                    return Err(format!(
                        "the recording came back {fw}x{fh}, which VP8's 4:2:0 \
                         sampling cannot take - please report this deck's \
                         aspect settings as a bug"
                    ));
                }
            }
        }
        Ok(())
    };
    let t_end = loop {
        if started.elapsed() > cap {
            return Err(format!(
                "the deck was still playing after {}s - it never marked its autoplay \
                 played-through. If the deck really is that long, export it in parts \
                 with --split, or photograph it with --stills",
                cap.as_secs()
            ));
        }
        match cdp.events.recv_timeout(Duration::from_millis(50)) {
            Ok(ev) => match ev["method"].as_str() {
                Some("Page.screencastFrame") => {
                    let p = &ev["params"];
                    if let Some(ack) = p["sessionId"].as_i64() {
                        cdp.call_no_wait("Page.screencastFrameAck", json!({"sessionId": ack}))?;
                    }
                    let ts = p["metadata"]["timestamp"].as_f64().unwrap_or(0.0);
                    if ts < t_start {
                        continue;
                    }
                    let jpg = cdp::base64_decode(p["data"].as_str().unwrap_or(""))?;
                    if std::env::var_os("MIRZAM_RECORD_DEBUG").is_some() {
                        eprintln!(
                            "frame ts={:.3} bytes={} (screencast)",
                            ts - t_start,
                            jpg.len()
                        );
                    }
                    check_frame(&jpg)?;
                    timeline.push(ts, jpg, &mut enc);
                    last_frame = Instant::now();
                }
                Some("__closed") => {
                    return Err("Chromium closed the connection mid-recording".into())
                }
                _ => {}
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Chromium closed the connection mid-recording".into())
            }
        }
        if moving && last_frame.elapsed() >= Duration::from_millis(100) {
            let shot = cdp.call(
                "Page.captureScreenshot",
                json!({"format": "jpeg", "quality": JPEG_QUALITY, "optimizeForSpeed": true}),
            )?;
            let ts = clock(cdp)?;
            let jpg = cdp::base64_decode(shot["data"].as_str().unwrap_or(""))?;
            if std::env::var_os("MIRZAM_RECORD_DEBUG").is_some() {
                eprintln!("frame ts={:.3} bytes={} (forced)", ts - t_start, jpg.len());
            }
            check_frame(&jpg)?;
            timeline.push(ts, jpg, &mut enc);
            last_frame = Instant::now();
        }
        if last_poll.elapsed() >= Duration::from_millis(250) {
            last_poll = Instant::now();
            let state = cdp
                .eval(
                    "document.querySelector('#deck[data-mz-autoplay-done]') ? 2 : \
                     (document.getAnimations().length > 0 || \
                      [...document.querySelectorAll('video')].some(v => !v.paused && !v.ended) \
                      ? 1 : 0)",
                )?
                .as_i64()
                .unwrap_or(0);
            if state == 2 {
                break clock(cdp)?;
            }
            moving = state == 1;
            if std::env::var_os("MIRZAM_RECORD_DEBUG").is_some() {
                eprintln!("poll t={:.3} moving={moving}", clock(cdp)? - t_start);
            }
        }
    };
    let _ = cdp.call("Page.stopScreencast", json!({}));
    let secs = timeline.finish(t_end, &mut enc)?;
    enc.finish()?;
    if secs == 0.0 {
        return Err("the recording delivered no frames".into());
    }
    Ok(secs)
}

/// The page's clock, in the screencast's terms: seconds since the epoch.
fn clock(cdp: &cdp::Cdp) -> Result<f64, String> {
    cdp.eval("Date.now() / 1000")?
        .as_f64()
        .ok_or_else(|| "the page's clock did not read as a number".into())
}

/// Turns the screencast's when-something-painted frames into the constant
/// rate the MJPEG pipe needs: each frame is repeated until the next one's
/// timestamp, so a still slide costs copies of one JPEG and an animation
/// plays at whatever rate it painted.
struct Timeline {
    fps: u32,
    t0: Option<f64>,
    prev: Option<Vec<u8>>,
    written: u64,
    write_err: bool,
}

impl Timeline {
    fn new(fps: u32) -> Timeline {
        Timeline {
            fps,
            t0: None,
            prev: None,
            written: 0,
            write_err: false,
        }
    }

    /// A frame painted at `ts`. The previous frame covered the span up to
    /// here; emit it that many times. A pipe error is remembered rather than
    /// returned — ffmpeg's own status, collected in `Encoder::finish`, says
    /// what actually went wrong.
    fn push(&mut self, ts: f64, jpg: Vec<u8>, enc: &mut Encoder) {
        if let (Some(t0), Some(prev)) = (self.t0, self.prev.as_ref()) {
            let target = ((ts - t0) * f64::from(self.fps)).round().max(0.0) as u64;
            while self.written < target && !self.write_err {
                self.write_err = enc.frame(prev).is_err();
                self.written += 1;
            }
        } else {
            self.t0 = Some(ts);
        }
        self.prev = Some(jpg);
    }

    /// The run is over at `t_end`: the last frame covers the tail. Returns
    /// the seconds of video written.
    fn finish(mut self, t_end: f64, enc: &mut Encoder) -> Result<f64, String> {
        if let (Some(t0), Some(prev)) = (self.t0, self.prev.as_ref()) {
            let target = ((t_end - t0) * f64::from(self.fps)).round().max(0.0) as u64;
            let target = target.max(self.written + 1);
            while self.written < target && !self.write_err {
                self.write_err = enc.frame(prev).is_err();
                self.written += 1;
            }
        }
        Ok(f64::from(u32::try_from(self.written).unwrap_or(u32::MAX)) / f64::from(self.fps))
    }
}

/// How many video frames a dwell is: its nearest count at `fps`, and never
/// zero — a slide asked for is a slide in the film.
fn frame_count(dwell_ms: u32, fps: u32) -> u32 {
    ((u64::from(dwell_ms) * u64::from(fps) + 500) / 1000).max(1) as u32
}

/// How long one slide stays on screen in the stills mode. The slide's own
/// `<!-- autoplay: -->` dwell wins, then `--interval`, then the
/// frontmatter's `autoplay:` interval, then the default — the same
/// precedence the viewer gives them, with the flag standing where
/// `?autoplay=` stands in a URL.
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
    let encoder = jpeg_encoder::Encoder::new(&mut jpg, JPEG_QUALITY as u8);
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

/// Width and height out of a JPEG's start-of-frame marker, for checking
/// what the screencast actually delivered.
fn jpeg_dims(jpg: &[u8]) -> Option<(u16, u16)> {
    let mut i = 2; // past FFD8
    while i + 4 <= jpg.len() {
        if jpg[i] != 0xFF {
            return None;
        }
        let marker = jpg[i + 1];
        // SOF0-15 carry dimensions; C4/C8/CC are tables and extensions.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            if i + 9 > jpg.len() {
                return None;
            }
            let h = u16::from_be_bytes([jpg[i + 5], jpg[i + 6]]);
            let w = u16::from_be_bytes([jpg[i + 7], jpg[i + 8]]);
            return Some((w, h));
        }
        let len = u16::from_be_bytes([jpg[i + 2], jpg[i + 3]]) as usize;
        i += 2 + len;
    }
    None
}

/// The ffmpeg encode, spelled once for both modes: piped MJPEG in, VP8 in
/// WebM out. The piped image stream is the only demuxer the trimmed ffmpeg
/// has that can carry the frames, and it has no per-frame durations, so
/// duration is spelled in copies — cheap twice over, because a frame's JPEG
/// is small and an encoder given an unchanged frame emits almost nothing.
struct Encoder {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    saw_write_error: bool,
}

impl Encoder {
    fn spawn(ffmpeg: &str, fps: u32, out_path: &Path) -> Result<Encoder, String> {
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
                &fps.to_string(),
                "-i",
                // `pipe:0`, never `-`: ffmpeg 7 reads `-` as its `fd:`
                // protocol, which the trimmed Playwright build compiles out;
                // the `pipe:` protocol is in every build, that one included.
                "pipe:0",
                "-vcodec",
                "libvpx",
                "-pix_fmt",
                "yuv420p",
                // Constrained quality: `-crf` sets the quality slides need
                // for legible text, `-b:v` caps what a pathological deck
                // could ask.
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
            // Inherited on purpose: `-loglevel error` keeps a clean run
            // silent, and a failing encode explains itself without this
            // re-relaying it.
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot run {ffmpeg}: {e}"))?;
        let stdin = child.stdin.take();
        Ok(Encoder {
            child,
            stdin,
            saw_write_error: false,
        })
    }

    /// One frame down the pipe. An error here usually echoes an encoder
    /// that already died — `finish` turns its status into the message.
    fn frame(&mut self, jpg: &[u8]) -> Result<(), ()> {
        let ok = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(jpg).is_ok(),
            None => false,
        };
        if ok {
            Ok(())
        } else {
            self.saw_write_error = true;
            Err(())
        }
    }

    /// Closes the pipe and waits the encode out.
    fn finish(mut self) -> Result<(), String> {
        drop(self.stdin.take());
        let status = self
            .child
            .wait()
            .map_err(|e| format!("cannot wait for ffmpeg: {e}"))?;
        if !status.success() {
            return Err(format!("ffmpeg failed to encode the video ({status})"));
        }
        if self.saw_write_error {
            return Err("ffmpeg stopped reading frames".into());
        }
        Ok(())
    }
}

/// Locates an ffmpeg that can do the job: `--ffmpeg`, then $MIRZAM_FFMPEG —
/// both taken at their word — then `ffmpeg` on PATH, then the trimmed build
/// Playwright keeps beside its browsers, under $PLAYWRIGHT_BROWSERS_PATH or
/// its default install. The found candidates are probed for the one encoder
/// this needs (`scripts/record-demo.mjs` probes the same way, for the same
/// reason: the failure of a bare existence check is a codec error after the
/// filming already succeeded).
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
        assert_eq!(frame_count(5000, 10), 50);
        assert_eq!(frame_count(750, 10), 8); // 7.5 frames rounds up
        assert_eq!(frame_count(100, 10), 1);
        assert_eq!(frame_count(1, 10), 1); // a floor, not a rounding accident
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
        assert_eq!(jpeg_dims(&jpg), Some((64, 36)));
    }

    /// An encoder stand-in for the timeline tests: `cat` consumes the pipe,
    /// and the written count carries the arithmetic under test.
    fn sink() -> Encoder {
        let mut child = std::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("cat runs anywhere the tests do");
        let stdin = child.stdin.take();
        Encoder {
            child,
            stdin,
            saw_write_error: false,
        }
    }

    #[test]
    fn timeline_repeats_each_frame_to_its_span() {
        let mut enc = sink();
        let mut tl = Timeline::new(10);
        tl.push(100.0, vec![1], &mut enc); // t0; nothing written yet
        tl.push(100.5, vec![2], &mut enc); // frame 1 covered 0.5s: 5 copies
        assert_eq!(tl.written, 5);
        // The run ends 1s in: frame 2 covers the tail, 10 frames total.
        let secs = tl.finish(101.0, &mut enc).unwrap();
        assert!((secs - 1.0).abs() < 1e-9);
        let _ = enc.child.kill();
    }

    #[test]
    fn timeline_writes_the_only_frame_at_least_once() {
        let mut enc = sink();
        let mut tl = Timeline::new(10);
        tl.push(100.0, vec![1], &mut enc);
        // The end lands on t0 exactly: the single frame still reaches the
        // video rather than rounding away.
        let secs = tl.finish(100.0, &mut enc).unwrap();
        assert!(secs > 0.0);
        let _ = enc.child.kill();
    }

    #[test]
    fn jpeg_dims_reads_a_real_sof() {
        let mut jpg = Vec::new();
        let e = jpeg_encoder::Encoder::new(&mut jpg, 90);
        e.encode(&[0u8; 12], 2, 2, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        assert_eq!(jpeg_dims(&jpg), Some((2, 2)));
    }
}
