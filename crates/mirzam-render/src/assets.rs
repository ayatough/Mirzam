//! Inlines local images and media as data URIs so a deck ships as a single
//! self-contained HTML file.

use base64::Engine as _;
use regex::Regex;
use std::path::{Path, PathBuf};

const MAX_EMBED_BYTES: u64 = 20 * 1024 * 1024;

/// How far a document that is itself inlined is followed.
///
/// An embedded HTML widget has references of its own — its script, its
/// stylesheet, the image it draws — and they are relative to *it*, not to the
/// deck. Following them is what keeps a deck with a widget in it one file. A
/// widget that embeds a widget is still reasonable; one that embeds itself is
/// not, and without a limit that document would be inlined until the machine
/// gave out.
const MAX_EMBED_DEPTH: usize = 3;

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
    embed_within(html, source, "", 0, warnings, referenced)
}

/// One document's references, resolved relative to the directory it lives in.
///
/// The deck itself is at the root, so `base` is empty and `depth` is zero. An
/// HTML widget inlined into a slide is one level in: its own `src` attributes
/// are written relative to the widget, which is what `base` carries.
fn embed_within(
    html: &str,
    source: &dyn AssetSource,
    base: &str,
    depth: usize,
    warnings: &mut Vec<String>,
    referenced: &mut Vec<PathBuf>,
) -> String {
    // `srcset` is here because a `<picture>` is how a deck offers one image for
    // a light background and another for a dark one. Miss it and the deck looks
    // fine until the reader's theme picks the source that was never inlined.
    let re = Regex::new(r#"(src|poster|srcset)="([^"]+)""#).expect("static regex");
    let out = re
        .replace_all(html, |c: &regex::Captures| {
            let attr = &c[1];
            let value = &c[2];
            let embedded = if attr == "srcset" {
                embed_srcset(value, source, base, depth, warnings, referenced)
            } else {
                embed_one(value, source, base, depth, warnings, referenced)
            };
            format!("{attr}=\"{embedded}\"")
        })
        .into_owned();
    if depth == 0 {
        return out;
    }
    // A stylesheet is the one thing a page loads through `href`, and only
    // inside an embedded document is that safe to rewrite: on a slide, `href`
    // is a link, and a link is meant to stay one. Scoped to `<link>` so it
    // cannot reach an `<a>` either way.
    let link = Regex::new(r#"(<link\b[^>]*?\bhref=")([^"]+)(")"#).expect("static regex");
    link.replace_all(&out, |c: &regex::Captures| {
        format!(
            "{}{}{}",
            &c[1],
            embed_one(&c[2], source, base, depth, warnings, referenced),
            &c[3]
        )
    })
    .into_owned()
}

/// The directory part of a relative reference: what its own references are
/// written against.
fn dir_of(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

/// Joins a reference to the directory of the document that wrote it, resolving
/// `.` and `..` here rather than handing a path full of them to an
/// [`AssetSource`] that may be a lookup table rather than a filesystem.
fn join_rel(base: &str, src: &str) -> String {
    if base.is_empty() || src.starts_with('/') {
        return src.to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in base.split('/').chain(src.split('/')) {
        match segment {
            "" | "." => {}
            ".." => {
                if matches!(parts.last(), Some(&last) if last != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
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
    base: &str,
    depth: usize,
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
    let rel = join_rel(base, src);
    let (result, path) = source.resolve(&rel);
    if let Some(p) = path {
        referenced.push(p);
    }
    match result {
        Ok(uri) => nested(uri, source, &rel, depth, warnings, referenced),
        Err(e) => {
            warnings.push(format!("{rel}: {e}"));
            placeholder_uri(&rel)
        }
    }
}

/// An inlined HTML document, inlined the rest of the way.
///
/// Anything else is already whole: an image carries no references. A widget
/// does, and they are relative to the widget, so this is where the walk goes
/// one level in — and where it stops, because a document deep enough to hit
/// [`MAX_EMBED_DEPTH`] is a loop rather than a widget.
fn nested(
    uri: String,
    source: &dyn AssetSource,
    rel: &str,
    depth: usize,
    warnings: &mut Vec<String>,
    referenced: &mut Vec<PathBuf>,
) -> String {
    // The head is spelled more than one way: Mirzam writes the charset, and a
    // host handing over a file the reader dropped on the page may not. Both are
    // the same document, so both are followed - and whichever one arrived is
    // the one written back.
    let Some((head, payload)) = uri.split_once(";base64,") else {
        return uri;
    };
    if !head.starts_with("data:text/html") {
        return uri;
    }
    if depth >= MAX_EMBED_DEPTH {
        warnings.push(format!(
            "{rel}: embedded documents nested more than {MAX_EMBED_DEPTH} deep, so its own references were left alone"
        ));
        return uri;
    }
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) else {
        return uri;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        // Not text after all: hand back what came in rather than guessing.
        return uri;
    };
    let walked = embed_within(&text, source, dir_of(rel), depth + 1, warnings, referenced);
    format!(
        "{head};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(walked.as_bytes())
    )
}

/// A `srcset` is candidates separated by commas, each one a URL and an optional
/// width or density descriptor. Every URL is inlined; the descriptors are passed
/// through untouched, because they describe the image rather than locate it.
fn embed_srcset(
    value: &str,
    source: &dyn AssetSource,
    base: &str,
    depth: usize,
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
        let embedded = embed_one(url, source, base, depth, warnings, referenced);
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
        // A widget: a document the reader operates, inlined like anything else.
        // The charset is not decoration - a `data:` URI defaults to US-ASCII,
        // which turns every non-Latin character in a widget into mojibake.
        "html" | "htm" => "text/html;charset=utf-8",
        // What a widget loads. A script served as `application/octet-stream`
        // from a `data:` URI is refused outright, so these matter as much as
        // the audio types below.
        "js" | "mjs" => "text/javascript;charset=utf-8",
        "css" => "text/css;charset=utf-8",
        "json" => "application/json;charset=utf-8",
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

    /// What [`embed_file`] writes for a widget, which is the spelling the tests
    /// below read back.
    const HTML_PREFIX: &str = "data:text/html;charset=utf-8;base64,";

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

    /// A table of files, so a test can watch the walk go one level into a
    /// document that has references of its own. Anything ending `.html` comes
    /// back as an inlined document; everything else as an image.
    struct Files(&'static [(&'static str, &'static str)]);

    impl AssetSource for Files {
        fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
            let result = match self.0.iter().find(|(name, _)| *name == rel) {
                Some((name, body)) if name.ends_with(".html") => Ok(format!(
                    "{HTML_PREFIX}{}",
                    base64::engine::general_purpose::STANDARD.encode(body.as_bytes())
                )),
                Some(_) => Ok(format!("data:image/svg+xml;base64,{rel}")),
                None => Err("file not found".to_string()),
            };
            (result, Some(PathBuf::from(rel)))
        }
    }

    /// The one inlined document, decoded, so an assertion can read it.
    fn inlined_document(html: &str) -> String {
        let at = html.find(HTML_PREFIX).expect("an inlined document");
        let tail = &html[at + HTML_PREFIX.len()..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='))
            .unwrap_or(tail.len());
        String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(&tail[..end])
                .expect("base64"),
        )
        .expect("utf-8")
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

    /// A widget is inlined the way an image is - and then the walk goes one
    /// level further, because unlike an image a document has references of its
    /// own. They are written relative to the widget, not to the deck, which is
    /// the whole reason this cannot be one flat pass.
    #[test]
    fn an_embedded_document_carries_its_own_references() {
        let files = Files(&[
            (
                "media/widget.html",
                "<link rel=\"stylesheet\" href=\"style.css\"><img src=\"chart.png\"><script src=\"../lib/plot.js\"></script>",
            ),
            ("media/style.css", ""),
            ("media/chart.png", ""),
            ("lib/plot.js", ""),
        ]);
        let mut warnings = Vec::new();
        let mut referenced = Vec::new();
        let out = embed_assets(
            r#"<div class="mz-embed mz-html"><iframe src="media/widget.html"></iframe></div>"#,
            &files,
            &mut warnings,
            &mut referenced,
        );
        let doc = inlined_document(&out);
        assert!(
            doc.contains(r#"href="data:image/svg+xml;base64,media/style.css""#),
            "{doc}"
        );
        assert!(
            doc.contains(r#"src="data:image/svg+xml;base64,media/chart.png""#),
            "{doc}"
        );
        // `..` is resolved here rather than handed to a source that may be a
        // lookup table rather than a filesystem.
        assert!(
            doc.contains(r#"src="data:image/svg+xml;base64,lib/plot.js""#),
            "{doc}"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        // Every one of them is watched, so editing the widget's stylesheet
        // rebuilds the deck that carries it.
        assert!(
            referenced.contains(&PathBuf::from("media/style.css")),
            "{referenced:?}"
        );
        assert!(
            referenced.contains(&PathBuf::from("lib/plot.js")),
            "{referenced:?}"
        );
    }

    /// `href` is a link everywhere else on a slide, and a link is meant to stay
    /// one: only a `<link>` inside an embedded document is rewritten.
    #[test]
    fn a_slides_own_links_are_never_rewritten() {
        let (out, warnings, _) = embed(r#"<a href="have/notes.html">notes</a>"#);
        assert_eq!(out, r#"<a href="have/notes.html">notes</a>"#);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    /// A widget that embeds itself would otherwise be inlined until the machine
    /// gave out. The deck still builds, and says what it stopped doing.
    #[test]
    fn a_document_that_embeds_itself_stops() {
        let files = Files(&[("loop.html", "<iframe src=\"loop.html\"></iframe>")]);
        let mut warnings = Vec::new();
        let mut referenced = Vec::new();
        let out = embed_assets(
            r#"<iframe src="loop.html"></iframe>"#,
            &files,
            &mut warnings,
            &mut referenced,
        );
        assert!(
            out.starts_with(&format!("<iframe src=\"{HTML_PREFIX}")),
            "{out}"
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("nested more than"), "{warnings:?}");
    }

    /// A host that hands the document over without naming a charset — a browser
    /// reading a file the reader dropped on the page does exactly that — is
    /// handing over the same document, so it is followed just the same, and it
    /// comes back spelled the way it arrived.
    #[test]
    fn a_document_is_followed_however_its_type_is_spelled() {
        struct Terse;
        impl AssetSource for Terse {
            fn resolve(&self, rel: &str) -> (Result<String, String>, Option<PathBuf>) {
                let uri = if rel == "w.html" {
                    format!(
                        "data:text/html;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(r#"<img src="a.png">"#)
                    )
                } else {
                    format!("data:image/png;base64,{rel}")
                };
                (Ok(uri), None)
            }
        }
        let mut warnings = Vec::new();
        let mut referenced = Vec::new();
        let out = embed_assets(
            r#"<iframe src="w.html"></iframe>"#,
            &Terse,
            &mut warnings,
            &mut referenced,
        );
        assert!(out.contains("data:text/html;base64,"), "{out}");
        let at =
            out.find("data:text/html;base64,").expect("document") + "data:text/html;base64,".len();
        let tail = &out[at..];
        let end = tail.find('"').unwrap_or(tail.len());
        let doc = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(&tail[..end])
                .expect("base64"),
        )
        .expect("utf-8");
        assert!(
            doc.contains(r#"src="data:image/png;base64,a.png""#),
            "{doc}"
        );
    }

    #[test]
    fn a_reference_is_joined_to_the_directory_that_wrote_it() {
        assert_eq!(join_rel("", "a.png"), "a.png");
        assert_eq!(join_rel("media", "chart.png"), "media/chart.png");
        assert_eq!(join_rel("media/deep", "../chart.png"), "media/chart.png");
        assert_eq!(join_rel("media", "./chart.png"), "media/chart.png");
        // Above the deck is somewhere an author can point; keeping the `..`
        // leaves that to the source to resolve or refuse.
        assert_eq!(join_rel("media", "../../out.png"), "../out.png");
        assert_eq!(join_rel("media", "/rooted.png"), "/rooted.png");
    }

    /// A widget served as `application/octet-stream` renders as nothing at all,
    /// and a script served that way from a `data:` URI is refused outright.
    #[test]
    fn a_document_and_what_it_loads_keep_their_types() {
        assert_eq!(mime_for(Path::new("w.html")), "text/html;charset=utf-8");
        assert_eq!(mime_for(Path::new("w.HTM")), "text/html;charset=utf-8");
        assert_eq!(
            mime_for(Path::new("plot.js")),
            "text/javascript;charset=utf-8"
        );
        assert_eq!(mime_for(Path::new("style.css")), "text/css;charset=utf-8");
    }
}
