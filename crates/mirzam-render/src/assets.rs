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
    let re = Regex::new(r#"(src|poster)="([^"]+)""#).expect("static regex");
    re.replace_all(html, |c: &regex::Captures| {
        let attr = &c[1];
        let src = &c[2];
        if src.starts_with("data:") || src.contains("://") || src.starts_with('#') {
            return c[0].to_string();
        }
        let (result, path) = source.resolve(src);
        if let Some(p) = path {
            referenced.push(p);
        }
        match result {
            Ok(uri) => format!("{attr}=\"{uri}\""),
            Err(e) => {
                warnings.push(format!("{src}: {e}"));
                format!("{attr}=\"{}\"", placeholder_uri(src))
            }
        }
    })
    .into_owned()
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
