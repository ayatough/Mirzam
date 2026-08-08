//! The build pipeline shared by `build` and `serve`.
//! Rendered slides are cached by source hash so only changed slides are
//! re-rendered.
//! Cache entries also record the mtimes of the assets a slide references, so
//! replacing an image re-renders exactly the slides that use it.

use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct BuildOutput {
    pub meta: mirzam_core::DeckMeta,
    /// Rendered `<section>` HTML, in slide order.
    pub sections: Vec<String>,
    /// Hash of each slide's rendered HTML, used for change detection.
    /// Hashing output rather than source means a changed image file is
    /// detected even though the Markdown is unchanged.
    pub hashes: Vec<u64>,
    /// Fingerprint of page-level settings (title, aspect, custom CSS),
    /// catching changes that need a page rebuild even when slides are identical.
    pub page_fingerprint: u64,
    /// Resolved contents of frontmatter `css:`.
    pub custom_css: Option<String>,
    pub warnings: Vec<String>,
    /// Source files and referenced assets making up this deck; the watch set.
    pub files: BTreeSet<PathBuf>,
    /// How many slides this build actually re-rendered (cache misses).
    pub rendered: usize,
}

pub struct CacheEntry {
    html: String,
    /// Assets referenced when rendering, with their mtimes.
    assets: Vec<(PathBuf, Option<SystemTime>)>,
}

impl CacheEntry {
    /// Whether every referenced asset still has the recorded mtime.
    fn is_fresh(&self) -> bool {
        self.assets.iter().all(|(p, t)| mtime(p) == *t)
    }
}

pub type RenderCache = HashMap<u64, CacheEntry>;

pub fn build_deck(input: &Path, cache: &mut RenderCache) -> Result<BuildOutput, String> {
    build_deck_with(input, cache, None)
}

/// `split_override` forces heading-based slide splitting regardless of
/// frontmatter, which is how `--split` turns an unmodified document into a deck.
pub fn build_deck_with(
    input: &Path,
    cache: &mut RenderCache,
    split_override: Option<u8>,
) -> Result<BuildOutput, String> {
    let src = std::fs::read_to_string(input)
        .map_err(|e| format!("cannot read {}: {e}", input.display()))?;
    let base_dir = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    files.insert(input.to_path_buf());

    // 1. frontmatter
    let (fm, body) = mirzam_syntax::split_frontmatter(&src);
    let meta = match fm {
        Some(yaml) => mirzam_core::parse_meta(yaml)?,
        None => mirzam_core::DeckMeta::default(),
    };
    let mut warnings = Vec::new();
    warnings.extend(mirzam_render::theme_warning(meta.theme.as_deref()));
    warnings.extend(mirzam_render::mode_warning(meta.mode.as_deref()));

    // A transition that does not parse leaves the deck with plain cuts, which
    // is the right outcome for a typo in a decoration - but say so, because
    // otherwise it looks like the feature is broken.
    if let Some(src) = &meta.transition {
        if let Err(e) = mirzam_anim::parse_transition(src) {
            warnings.push(format!("transition: {e}"));
        }
    }

    // Load the custom stylesheet; failures are warnings, not errors.
    let custom_css = match &meta.css {
        Some(rel) => {
            let path = base_dir.join(rel);
            files.insert(path.clone());
            match std::fs::read_to_string(&path) {
                Ok(css) => Some(css),
                Err(e) => {
                    warnings.push(format!("css: cannot read {rel}: {e}"));
                    None
                }
            }
        }
        None => None,
    };

    // 2. Expand includes, collecting the files that were read.
    let body = mirzam_syntax::expand_includes_tracked(
        body,
        &base_dir,
        &mirzam_syntax::FsProvider,
        &mut files,
    );

    // 3. Substitute variables outside code fences. A variable change shows up
    //    as changed post-substitution source, so slide hashes pick it up.
    let vars = meta.var_table();
    let body = substitute_outside_fences(&body, &vars);

    // 4. Split into slides and render each one through the cache.
    let level = split_override.or_else(|| meta.split_level());
    let slide_sources = mirzam_syntax::split_slides_at(&body, level);
    let mut sections = Vec::with_capacity(slide_sources.len());
    let mut hashes = Vec::with_capacity(slide_sources.len());
    let mut rendered = 0usize;

    for (i, slide_src) in slide_sources.iter().enumerate() {
        // The cache key includes the slide index, since data-index is baked into the HTML.
        let key = slide_hash(slide_src, i);
        match cache.get(&key).filter(|e| e.is_fresh()) {
            Some(entry) => {
                for (p, _) in &entry.assets {
                    files.insert(p.clone());
                }
                hashes.push(str_hash(&entry.html));
                sections.push(entry.html.clone());
            }
            None => {
                let slide = mirzam_syntax::parse_slide(slide_src);
                let out = mirzam_render::render_slide_html(&slide, i, &base_dir);
                warnings.extend(out.warnings);
                let assets: Vec<(PathBuf, Option<SystemTime>)> =
                    out.assets.iter().map(|p| (p.clone(), mtime(p))).collect();
                for (p, _) in &assets {
                    files.insert(p.clone());
                }
                cache.insert(
                    key,
                    CacheEntry {
                        html: out.html.clone(),
                        assets,
                    },
                );
                hashes.push(str_hash(&out.html));
                sections.push(out.html);
                rendered += 1;
            }
        }
    }

    // Keep the cache from growing without bound during a long editing session.
    if cache.len() > 4096 {
        let live_keys: std::collections::HashSet<u64> = slide_sources
            .iter()
            .enumerate()
            .map(|(i, s)| slide_hash(s, i))
            .collect();
        cache.retain(|k, _| live_keys.contains(k));
    }

    let page_fingerprint = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        meta.title.hash(&mut h);
        meta.author.hash(&mut h);
        meta.theme.hash(&mut h);
        meta.mode.hash(&mut h);
        meta.aspect.hash(&mut h);
        custom_css.hash(&mut h);
        level.hash(&mut h);
        // Whether math is present decides if the math font is bundled.
        mirzam_render::sections_have_math(&sections).hash(&mut h);
        h.finish()
    };

    Ok(BuildOutput {
        meta,
        sections,
        hashes,
        page_fingerprint,
        custom_css,
        warnings,
        files,
        rendered,
    })
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn slide_hash(src: &str, index: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    index.hash(&mut h);
    h.finish()
}

fn str_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Substitutes variables only on lines outside code fences.
fn substitute_outside_fences(
    body: &str,
    vars: &std::collections::BTreeMap<String, mirzam_core::Value>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_code = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line);
        } else {
            out.push_str(&mirzam_core::substitute_vars(line, vars));
        }
        out.push('\n');
    }
    out
}
