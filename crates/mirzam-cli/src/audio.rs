//! The film's sound, mixed after the take rather than recorded during it:
//! DevTools hands back pictures and nothing else, and headless Chromium has
//! no tap for a tab's audio anyway. What the recorder does have is better
//! than a microphone — it knows exactly which clip started when, from the
//! `play`/`pause`/`ended` events its in-page log collected — and every
//! clip's own bytes, decoded straight out of the deck. So the sound is laid
//! under the silent film afterwards: each span delayed to where its clip
//! sat in the take, trimmed to how long it stayed audible, and mixed.
//!
//! That takes a *full* ffmpeg — audio codecs and the `adelay`/`atrim`/
//! `amix` filters — which the trimmed Playwright build has none of. The
//! encode alone deliberately never needed more than that build; sound is
//! the one feature that asks for a real one, and a machine without it gets
//! the silent film plus a warning saying exactly what was missing. Opus is
//! preferred (Vorbis as the fallback): both live in WebM royalty-free, and
//! YouTube takes either.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// One stretch of one clip audible in the film: the clip at `source_at`
/// seconds into its own audio, laid down `film_at` seconds into the video,
/// for `dur` seconds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Span {
    pub(crate) el: usize,
    pub(crate) film_at: f64,
    pub(crate) source_at: f64,
    pub(crate) dur: f64,
}

/// Shorter than this is a stray event, not a sound: a clip paused by the
/// page turn in the same tick it started.
const MIN_SPAN_SECS: f64 = 0.1;

/// Turns the in-page media log — `[element, kind, epoch seconds, position]`
/// rows, in event order — into the spans audible between `t0` and `t_end`.
/// A `play` opens a span at the clip position it carries; `pause` and
/// `ended` close it; a clip still playing when the take ends is closed at
/// `t_end`. Spans are clamped to the film: what played before frame one
/// (or after the last) was real, but nobody filmed it.
pub(crate) fn spans(log: &Value, t0: f64, t_end: f64) -> Vec<Span> {
    let mut open: std::collections::HashMap<usize, (f64, f64)> = Default::default();
    let mut out = Vec::new();
    let mut close = |el: usize, t: f64, open: &mut std::collections::HashMap<usize, (f64, f64)>| {
        let Some((started, source_at)) = open.remove(&el) else {
            return;
        };
        let from = started.max(t0);
        let to = t.min(t_end);
        let dur = to - from;
        if dur < MIN_SPAN_SECS {
            return;
        }
        out.push(Span {
            el,
            film_at: from - t0,
            source_at: source_at + (from - started),
            dur,
        });
    };
    for row in log.as_array().map(Vec::as_slice).unwrap_or_default() {
        let (Some(el), Some(kind), Some(t)) = (
            row[0].as_u64().map(|v| v as usize),
            row[1].as_str(),
            row[2].as_f64(),
        ) else {
            continue;
        };
        let at = row[3].as_f64().unwrap_or(0.0);
        match kind {
            "play" => {
                // Two plays without a pause: keep the first opening.
                open.entry(el).or_insert((t, at));
            }
            "pause" | "ended" => close(el, t, &mut open),
            _ => {}
        }
    }
    let still_open: Vec<usize> = open.keys().copied().collect();
    for el in still_open {
        close(el, t_end, &mut open);
    }
    out.sort_by(|a, b| a.film_at.total_cmp(&b.film_at));
    out
}

/// Materializes what an element's `currentSrc` points at as a file ffmpeg
/// can open: a `data:` URI — the built page inlines every asset as one — is
/// decoded next to the film, a `file://` URL is used where it lies.
pub(crate) fn source_file(src: &str, dir: &Path, idx: usize) -> Result<PathBuf, String> {
    if let Some(path) = src.strip_prefix("file://") {
        return Ok(PathBuf::from(path));
    }
    if src.starts_with("data:") {
        let b64 = src
            .split_once(";base64,")
            .map(|(_, b)| b)
            .ok_or("a clip's data: URI is not base64")?;
        let bytes = crate::cdp::base64_decode(b64)?;
        let path = dir.join(format!("mirzam-clip-{}-{idx}.bin", std::process::id()));
        std::fs::write(&path, bytes).map_err(|e| format!("cannot write a clip's audio: {e}"))?;
        return Ok(path);
    }
    Err(format!("a clip plays from somewhere unmixable: {src}"))
}

/// Whether this ffmpeg can lay sound under the film, and with which codec:
/// Opus first, Vorbis second — the two WebM carries — plus the three
/// filters the mix is made of. The trimmed Playwright build fails this on
/// every count, which is exactly the case the caller warns about.
pub(crate) fn mix_codec(ffmpeg: &str) -> Option<&'static str> {
    let run = |args: &[&str]| -> String {
        std::process::Command::new(ffmpeg)
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    let filters = run(&["-hide_banner", "-filters"]);
    if !["amix", "adelay", "atrim"]
        .iter()
        .all(|f| filters.contains(&format!(" {f} ")))
    {
        return None;
    }
    let encoders = run(&["-hide_banner", "-encoders"]);
    ["libopus", "libvorbis"]
        .into_iter()
        .find(|c| encoders.contains(c))
}

/// Whether the file has an audio stream at all. `ffmpeg -i` exits nonzero
/// with no output asked of it, but its stream listing on stderr is the
/// probe: a video-only clip — a screen recording, say — mixes nothing.
pub(crate) fn has_audio(ffmpeg: &str, file: &Path) -> bool {
    std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(file)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).contains("Audio:"))
        .unwrap_or(false)
}

/// The mix invocation: the silent film's video stream copied untouched,
/// each span's audio seeked, trimmed and delayed into place, everything
/// mixed without renormalizing — a lone clip should not get quieter when a
/// second one exists somewhere else in the deck.
fn mix_args(
    film: &Path,
    spans: &[(PathBuf, bool, Span)],
    codec: &str,
    out: &Path,
) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        film.into(),
    ];
    let mut filter = String::new();
    for (i, (file, loops, span)) in spans.iter().enumerate() {
        if *loops {
            // A looping clip has no end of its own; the trim below is what
            // bounds it.
            args.push("-stream_loop".into());
            args.push("-1".into());
        }
        args.push("-ss".into());
        args.push(format!("{:.3}", span.source_at).into());
        args.push("-i".into());
        args.push(file.into());
        filter.push_str(&format!(
            "[{n}:a]atrim=duration={dur:.3},adelay={delay}:all=1[a{i}];",
            n = i + 1,
            dur = span.dur,
            delay = (span.film_at * 1000.0).round() as u64,
        ));
    }
    for i in 0..spans.len() {
        filter.push_str(&format!("[a{i}]"));
    }
    filter.push_str(&format!(
        "amix=inputs={}:duration=longest:normalize=0[mix]",
        spans.len()
    ));
    args.extend([
        "-filter_complex".into(),
        filter.into(),
        "-map".into(),
        "0:v".into(),
        "-c:v".into(),
        "copy".into(),
        "-map".into(),
        "[mix]".into(),
        "-c:a".into(),
        codec.into(),
        "-b:a".into(),
        "128k".into(),
        "-f".into(),
        "webm".into(),
        out.into(),
    ]);
    args
}

/// Lays the spans under `film`, in place: the mix lands beside it and moves
/// over it only once ffmpeg succeeded, so a failed mix leaves the silent
/// film intact.
pub(crate) fn mix_into(
    ffmpeg: &str,
    film: &Path,
    spans: &[(PathBuf, bool, Span)],
    codec: &str,
) -> Result<(), String> {
    let tmp = film.with_extension("webm.mix");
    let status = std::process::Command::new(ffmpeg)
        .args(mix_args(film, spans, codec, &tmp))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("cannot run {ffmpeg}: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("ffmpeg failed to mix the audio ({status})"));
    }
    std::fs::rename(&tmp, film).map_err(|e| format!("cannot move the mixed film in: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_play_pause_pair_becomes_one_span() {
        let log = json!([[0, "play", 105.0, 0.0], [0, "pause", 109.5, 4.5]]);
        let s = spans(&log, 100.0, 120.0);
        assert_eq!(
            s,
            vec![Span {
                el: 0,
                film_at: 5.0,
                source_at: 0.0,
                dur: 4.5
            }]
        );
    }

    #[test]
    fn a_clip_still_playing_ends_with_the_take() {
        let log = json!([[2, "play", 110.0, 1.25]]);
        let s = spans(&log, 100.0, 118.0);
        assert_eq!(
            s,
            vec![Span {
                el: 2,
                film_at: 10.0,
                source_at: 1.25,
                dur: 8.0
            }]
        );
    }

    #[test]
    fn what_played_before_the_first_frame_is_clamped_into_it() {
        // Playing since 2s before the film's zero: the span starts at zero,
        // that far into the clip's own audio.
        let log = json!([[0, "play", 98.0, 0.5], [0, "ended", 103.0, 5.5]]);
        let s = spans(&log, 100.0, 120.0);
        assert_eq!(
            s,
            vec![Span {
                el: 0,
                film_at: 0.0,
                source_at: 2.5,
                dur: 3.0
            }]
        );
    }

    #[test]
    fn blips_and_pauses_without_a_play_are_dropped() {
        let log = json!([
            [0, "pause", 101.0, 0.0], // never opened
            [1, "play", 102.0, 0.0],
            [1, "pause", 102.05, 0.05], // a blip, not a sound
            [1, "play", 104.0, 0.0],
            [1, "ended", 106.0, 2.0]
        ]);
        let s = spans(&log, 100.0, 120.0);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].film_at, 4.0);
    }

    #[test]
    fn mix_args_seek_trim_delay_and_copy_video() {
        let spans = vec![
            (
                PathBuf::from("a.webm"),
                false,
                Span {
                    el: 0,
                    film_at: 5.0,
                    source_at: 0.0,
                    dur: 4.5,
                },
            ),
            (
                PathBuf::from("b.wav"),
                true,
                Span {
                    el: 1,
                    film_at: 12.25,
                    source_at: 1.0,
                    dur: 3.0,
                },
            ),
        ];
        let args = mix_args(Path::new("f.webm"), &spans, "libopus", Path::new("o.webm"));
        let s: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let joined = s.join(" ");
        assert!(joined.contains("-i f.webm"));
        // The looping input loops; the plain one does not.
        assert_eq!(joined.matches("-stream_loop -1").count(), 1);
        assert!(joined.contains("-stream_loop -1 -ss 1.000 -i b.wav"));
        assert!(joined.contains(
            "[1:a]atrim=duration=4.500,adelay=5000:all=1[a0];\
             [2:a]atrim=duration=3.000,adelay=12250:all=1[a1];\
             [a0][a1]amix=inputs=2:duration=longest:normalize=0[mix]"
        ));
        assert!(joined.contains("-map 0:v -c:v copy -map [mix] -c:a libopus"));
    }

    #[test]
    fn data_uris_and_file_urls_become_files() {
        let dir = std::env::temp_dir();
        let f = source_file("data:audio/wav;base64,Zm9vYmFy", &dir, 7).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"foobar");
        let _ = std::fs::remove_file(&f);
        assert_eq!(
            source_file("file:///tmp/x.wav", &dir, 0).unwrap(),
            PathBuf::from("/tmp/x.wav")
        );
        assert!(source_file("https://elsewhere/x.mp4", &dir, 0).is_err());
    }
}
