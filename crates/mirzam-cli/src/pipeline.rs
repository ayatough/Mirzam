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
    /// Where every byte of the expanded document came from. With the slide
    /// spans below, this is what turns "this block, on this slide" back into
    /// "these bytes, in this file".
    pub map: mirzam_syntax::SourceMap,
    /// Each *authored* slide's text and its offset in the expanded document.
    /// A slide broken by `<!-- next -->` renders as several sections but is one
    /// entry here: this list is the source view, and there is only one source.
    pub slides: Vec<mirzam_syntax::SlideSpan>,
}

/// One rendered section, and the authored slide it came from. The two differ
/// only where `<!-- next -->` broke a slide into parts.
struct Part {
    text: String,
    /// Index into [`BuildOutput::slides`].
    from: usize,
    /// Parts of the same broken slide share a group; the viewer reads it as
    /// "cut, do not animate".
    group: Option<usize>,
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
    build_deck_with(input, cache, None, None)
}

/// `split_override` forces heading-based slide splitting regardless of
/// frontmatter, which is how `--split` turns an unmodified document into a deck.
///
/// `base_url` is the URL the input file's directory maps to once published.
/// Set it when the deck is served from somewhere other than beside its source,
/// so its links to other documents still resolve.
pub fn build_deck_with(
    input: &Path,
    cache: &mut RenderCache,
    split_override: Option<u8>,
    base_url: Option<&str>,
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

    // 2. Expand includes, collecting the files that were read and where every
    //    byte of the result came from.
    let body_offset = src.len() - body.len();
    let (body, map) = mirzam_syntax::expand_includes_mapped(
        body,
        body_offset,
        input,
        &base_dir,
        &mirzam_syntax::FsProvider,
        &mut files,
    );

    // 3. Substitute variables outside code fences. A variable change shows up
    //    as changed post-substitution source, so slide hashes pick it up.
    let vars = meta.var_table();
    let (body, map) = substitute_outside_fences(&body, &vars, &map);

    // 4. Split into slides and render each one through the cache.
    let level = split_override.or_else(|| meta.split_level());
    let slides = mirzam_syntax::split_slides_spanned(&body, level);

    // "slide 7" is not much help when slide 7 lives in a file the author has
    // not opened. Now that the map knows, say so.
    //
    // Measured from the slide's first real character, not its first byte: a
    // slide that begins with the blank line the parent left before `![[…]]`
    // would otherwise be attributed to the parent.
    let origin = |si: usize| -> String {
        let s: &mirzam_syntax::SlideSpan = &slides[si];
        let head = s.text.find(|c: char| !c.is_whitespace());
        match head.and_then(|o| map.lookup(s.start + o)) {
            Some((f, _)) if f != input => format!(" (in {})", f.display()),
            _ => String::new(),
        }
    };

    // 4a. Expand `<!-- next -->`. A slide that breaks one pane becomes several
    //     slides, identical but for that pane. Doing it on the text, before
    //     anything is parsed, keeps the rest of the pipeline - anim, annotate,
    //     connectors, notes, the render cache - free of the idea.
    let mut parts: Vec<Part> = Vec::with_capacity(slides.len());
    for (si, slide) in slides.iter().enumerate() {
        match mirzam_syntax::expand_continuations(&slide.text) {
            Ok(texts) if texts.len() > 1 => {
                // The group only has to be unique within the deck.
                let group = Some(si);
                for text in texts {
                    parts.push(Part {
                        text,
                        from: si,
                        group,
                    });
                }
            }
            Ok(_) => parts.push(Part {
                text: slide.text.clone(),
                from: si,
                group: None,
            }),
            // Two panes breaking at once is a cross product no author can
            // predict. Say so and render the slide whole: the content is all
            // still there, just not split.
            Err(e) => {
                warnings.push(format!("{e}{}", origin(si)));
                parts.push(Part {
                    text: slide.text.clone(),
                    from: si,
                    group: None,
                });
            }
        }
    }

    let mut sections = Vec::with_capacity(parts.len());
    let mut hashes: Vec<u64> = Vec::with_capacity(parts.len());
    let mut rendered = 0usize;

    for (i, part) in parts.iter().enumerate() {
        let slide_src = &part.text;
        // The cache key includes the slide index, since data-index is baked into the HTML.
        let key = slide_hash(slide_src, i);
        let mut html = match cache.get(&key).filter(|e| e.is_fresh()) {
            Some(entry) => {
                for (p, _) in &entry.assets {
                    files.insert(p.clone());
                }
                entry.html.clone()
            }
            None => {
                let slide = mirzam_syntax::parse_slide(slide_src);
                let out = mirzam_render::render_slide_html(&slide, i, &base_dir);
                let from = origin(part.from);
                warnings.extend(out.warnings.into_iter().map(|w| format!("{w}{from}")));
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
                rendered += 1;
                out.html
            }
        };
        if let Some(g) = part.group {
            html = mirzam_render::mark_continuation(&html, g);
        }
        hashes.push(str_hash(&html));
        sections.push(html);
    }

    // Applied outside the cache: the base URL is a property of this build, not
    // of a slide, so a cached slide must not carry one build's URLs into the next.
    if let Some(base) = base_url {
        for section in &mut sections {
            *section = mirzam_render::rewrite_relative_links(section, base);
        }
        hashes = sections.iter().map(|s| str_hash(s)).collect();
    }

    // Keep the cache from growing without bound during a long editing session.
    if cache.len() > 4096 {
        let live_keys: std::collections::HashSet<u64> = parts
            .iter()
            .enumerate()
            .map(|(i, p)| slide_hash(&p.text, i))
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
        map,
        slides,
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

/// Substitutes variables only on lines outside code fences, carrying the
/// source map through: a line the substitution left alone still points at the
/// file it came from, and a line it rewrote points at nothing, because the
/// value on screen is not text anyone typed there.
fn substitute_outside_fences(
    body: &str,
    vars: &std::collections::BTreeMap<String, mirzam_core::Value>,
    map: &mirzam_syntax::SourceMap,
) -> (String, mirzam_syntax::SourceMap) {
    let mut out = String::with_capacity(body.len());
    let mut derived = mirzam_syntax::SourceMap::default();
    let mut in_code = false;
    let mut pos = 0usize;
    for raw in body.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let from = pos..pos + raw.len();
        pos += raw.len();
        let start = out.len();
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line);
        } else {
            out.push_str(&mirzam_core::substitute_vars(line, vars));
        }
        out.push('\n');
        map.carry(start..out.len(), from, &mut derived);
    }
    (out, derived)
}
