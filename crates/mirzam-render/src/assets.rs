//! Inlines local images and media as data URIs so a deck ships as a single
//! self-contained HTML file.

use base64::Engine as _;
use regex::Regex;
use std::path::{Path, PathBuf};

const MAX_EMBED_BYTES: u64 = 20 * 1024 * 1024;

/// Resolves asset references: the filesystem natively, or a host-provided
/// table in WASM (editor extension, browser).
pub trait AssetSource {
    /// Resolves a relative reference to a data URI or URL.
    /// The second element is the real path, when one exists, for watching and cache checks.
    fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>);
}

/// Default implementation backed by `std::fs`.
pub struct FsAssets<'a>(pub &'a Path);

impl AssetSource for FsAssets<'_> {
    fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
        let path = self.0.join(rel);
        let result = embed_file(&path);
        (result, Some(path))
    }
}

/// Replaces local asset references with data URIs.
/// Every referenced path is collected into `referenced` (including missing ones)
/// so callers can validate caches and watch files.
pub fn embed_assets(
    html: &str,
    source: &dyn AssetSource,
    warnings: &mut Vec<String>,
    referenced: &mut Vec<PathBuf>,
) -> String {
    // `srcset` is here because a `<picture>` is how a deck offers one image for
    // a light background and another for a dark one. Miss it and the deck looks
    // fine until the reader's theme picks the source that was never inlined.
    let re = Regex::new(r#"(src|poster|srcset)="([^"]+)""#).expect("static regex");
    re.replace_all(html, |c: &regex::Captures| {
        let attr = &c[1];
        let value = &c[2];
        let embedded = if attr == "srcset" {
            embed_srcset(value, source, warnings, referenced)
        } else {
            embed_one(value, source, warnings, referenced)
        };
        format!("{attr}=\"{embedded}\"")
    })
    .into_owned()
}

/// One reference: left alone if it is already a URI, inlined otherwise.
///
/// A reference the build cannot read off disk cannot be inlined, so it is
/// reported: the deck still builds and still shows the image wherever there is
/// a network, but it is no longer the one self-contained file the rest of this
/// module exists to keep — and nothing else in a build said so. A hosted video
/// is the exception the syntax documents, and it arrives here as a player URL
/// Mirzam wrote itself, so that one is silent.
fn embed_one(
    src: &str,
    source: &dyn AssetSource,
    warnings: &mut Vec<String>,
    referenced: &mut Vec<PathBuf>,
) -> String {
    if src.starts_with("data:") || src.starts_with('#') {
        return src.to_string();
    }
    if src.contains("://") {
        if !crate::inline::is_player_url(src) {
            warnings.push(format!(
                "{src}: fetched over the network when the slide is shown, so this deck is not self-contained"
            ));
        }
        return src.to_string();
    }
    let (result, path) = source.resolve(src);
    if let Some(p) = path {
        referenced.push(p);
    }
    match result {
        Ok(uri) => uri,
        Err(e) => {
            warnings.push(format!("{src}: {e}"));
            placeholder_uri(src)
        }
    }
}

/// A `srcset` is candidates separated by commas, each one a URL and an optional
/// width or density descriptor. Every URL is inlined; the descriptors are passed
/// through untouched, because they describe the image rather than locate it.
fn embed_srcset(
    value: &str,
    source: &dyn AssetSource,
    warnings: &mut Vec<String>,
    referenced: &mut Vec<PathBuf>,
) -> String {
    let mut out: Vec<String> = Vec::new();
    for candidate in split_srcset(value) {
        let candidate = candidate.trim();
        if candidate.is_empty() {
            continue;
        }
        let (url, descriptor) = match candidate.split_once(char::is_whitespace) {
            Some((u, d)) => (u, d.trim()),
            None => (candidate, ""),
        };
        let embedded = embed_one(url, source, warnings, referenced);
        out.push(if descriptor.is_empty() {
            embedded
        } else {
            format!("{embedded} {descriptor}")
        });
    }
    out.join(", ")
}

/// Splits a `srcset` on its candidate separators, ignoring the commas inside a
/// `data:` URI - there they are part of the URL, not a separator. Anywhere else
/// a comma always separates, whether or not a space follows it.
fn split_srcset(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, _) in value.match_indices(',') {
        let candidate = value[start..i].trim_start();
        // A data URI ends where its descriptor begins; until then every comma
        // it contains belongs to the payload.
        if candidate.starts_with("data:") && !candidate.contains(char::is_whitespace) {
            continue;
        }
        parts.push(&value[start..i]);
        start = i + 1;
    }
    parts.push(&value[start..]);
    parts
}

fn embed_file(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|_| "file not found".to_string())?;
    if meta.len() > MAX_EMBED_BYTES {
        return Err(format!(
            "larger than {}MB, not inlined",
            MAX_EMBED_BYTES / 1024 / 1024
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mime = mime_for(path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        // A browser will not play a recording served as octet-stream, so the
        // audio types matter as much as the video ones.
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "wav" => "audio/wav",
        "oga" | "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "application/octet-stream",
    }
}

/// Placeholder image (SVG data URI) for an asset that could not be found.
fn placeholder_uri(name: &str) -> String {
    let label = name.replace(['<', '>', '&'], "");
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400" viewBox="0 0 640 400"><rect width="640" height="400" rx="12" fill="#eceef4"/><rect x="8" y="8" width="624" height="384" rx="8" fill="none" stroke="#b7bdd1" stroke-width="2" stroke-dasharray="8 6"/><text x="320" y="185" text-anchor="middle" font-family="sans-serif" font-size="40" fill="#8b93ad">🖼</text><text x="320" y="235" text-anchor="middle" font-family="monospace" font-size="18" fill="#6d7590">{label}</text></svg>"##
    );
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    format!("data:image/svg+xml;base64,{b64}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolves anything under `have/` and fails everything else, so a test can
    /// see both the inlined and the missing path without touching the disk.
    struct Fake;

    impl AssetSource for Fake {
        fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
            let result = if let Some(name) = rel.strip_prefix("have/") {
                Ok(format!("data:image/svg+xml;base64,{name}"))
            } else {
                Err("file not found".to_string())
            };
            (result, Some(PathBuf::from(rel)))
        }
    }

    fn embed(html: &str) -> (String, Vec<String>, Vec<PathBuf>) {
        let mut warnings = Vec::new();
        let mut referenced = Vec::new();
        let out = embed_assets(html, &Fake, &mut warnings, &mut referenced);
        (out, warnings, referenced)
    }

    #[test]
    fn picture_sources_are_inlined_like_the_img_they_wrap() {
        // The dark source is the one a `prefers-color-scheme` reader sees, and
        // it lives in `srcset`, not `src`: leaving it relative ships a deck that
        // is broken for exactly half its readers.
        let (out, warnings, referenced) = embed(
            r#"<picture><source media="(prefers-color-scheme: dark)" srcset="have/dark.svg"><img src="have/light.svg"></picture>"#,
        );
        assert!(
            out.contains(r#"srcset="data:image/svg+xml;base64,dark.svg""#),
            "{out}"
        );
        assert!(
            out.contains(r#"src="data:image/svg+xml;base64,light.svg""#),
            "{out}"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(referenced.len(), 2);
    }

    #[test]
    fn srcset_keeps_its_descriptors_and_reports_what_is_missing() {
        let (out, warnings, _) = embed(r#"<img srcset="have/a.png 1x, gone/b.png 2x">"#);
        assert!(
            out.contains("data:image/svg+xml;base64,a.png 1x, "),
            "{out}"
        );
        assert!(out.contains(" 2x\""), "{out}");
        assert_eq!(warnings, vec!["gone/b.png: file not found"]);
    }

    #[test]
    fn a_data_uri_in_a_srcset_survives_its_own_commas() {
        // `data:` payloads contain commas; splitting on every one of them would
        // cut a valid image into two invalid ones.
        let html = r#"<img srcset="data:image/svg+xml,%3Csvg/%3E 1x, have/b.png 2x">"#;
        let (out, warnings, referenced) = embed(html);
        assert!(out.contains("data:image/svg+xml,%3Csvg/%3E 1x"), "{out}");
        assert!(out.contains("data:image/svg+xml;base64,b.png 2x"), "{out}");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(referenced, vec![PathBuf::from("have/b.png")]);
    }

    #[test]
    fn absolute_and_data_sources_are_left_alone() {
        let html = r#"<img src="https://example.com/a.png" srcset="https://example.com/b.png 2x">"#;
        let (out, warnings, referenced) = embed(html);
        assert_eq!(out, html);
        assert!(referenced.is_empty());
        // Left alone, but not left unsaid: neither URL can be inlined, so this
        // deck needs the network for both.
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings.iter().all(|w| w.contains("not self-contained")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_hosted_videos_player_is_the_documented_exception() {
        // The syntax says a hosted video is the one thing in a deck that is not
        // self-contained, and this URL is one Mirzam wrote for a reference that
        // asked for exactly that. Reporting it would make the documented case
        // look like a mistake — and make `--strict` unusable for any deck with
        // a talk in it.
        let html = r#"<div class="mz-embed"><iframe src="https://www.youtube-nocookie.com/embed/abc123"></iframe></div>"#;
        let (out, warnings, _) = embed(html);
        assert_eq!(out, html);
        assert!(warnings.is_empty(), "{warnings:?}");

        let vimeo = r#"<iframe src="https://player.vimeo.com/video/76979871"></iframe>"#;
        let (_, warnings, _) = embed(vimeo);
        assert!(warnings.is_empty(), "{warnings:?}");

        // A page on the same host that is *not* a player URL is an ordinary
        // remote reference, so it is reported like any other.
        let (_, warnings, _) = embed(r#"<img src="https://player.vimeo.com/logo.png">"#);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }
}
