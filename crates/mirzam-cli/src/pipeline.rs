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
    /// The deck's frontmatter as written, without the `---` fences. `None`
    /// when the deck declared none — a README built with `--split`, say.
    pub frontmatter: Option<String>,
    /// Rendered `<section>` HTML, in slide order.
    pub sections: Vec<String>,
    /// Hash of each slide's rendered HTML, used for change detection.
    /// Hashing output rather than source means a changed image file is
    /// detected even though the Markdown is unchanged.
    pub hashes: Vec<u64>,
    /// Fingerprint of page-level settings (title, aspect, custom CSS),
    /// catching changes that need a page rebuild even when slides are identical.
    pub page_fingerprint: u64,
    /// The stylesheets frontmatter's `theme:` named, in cascade order, read
    /// from disk. Each registers under its filename stem, so a slide or a pane
    /// can name it in a `theme=`.
    pub file_themes: Vec<mirzam_render::FileTheme>,
    pub warnings: Vec<String>,
    /// Where each warning came from, in the same order and of the same length
    /// as `warnings`. Split out rather than folded into the message because
    /// the message is prose a person reads and this is what a tool needs to
    /// open the file — `mirzam check --format json` is the caller.
    pub warning_sites: Vec<WarningSite>,
    /// Source files and referenced assets making up this deck; the watch set.
    pub files: BTreeSet<PathBuf>,
    /// How many slides this build actually re-rendered (cache misses).
    pub rendered: usize,
    /// Where every byte of the expanded document came from. With the slide
    /// spans below, this is what turns "this block, on this slide" back into
    /// "these bytes, in this file".
    pub map: mirzam_syntax::SourceMap,
    /// The document the slides were split from: transclusions expanded and
    /// variables substituted, without the frontmatter. With the spans below
    /// it is what lets a caller hand the deck's own text to something else —
    /// `mirzam build --embed-source` puts it in the page.
    pub body: String,
    /// Each *authored* slide's text and its offset in the expanded document.
    /// A slide broken by `<!-- next -->` renders as several sections but is one
    /// entry here: this list is the source view, and there is only one source.
    pub slides: Vec<mirzam_syntax::SlideSpan>,
    /// The authored slide each rendered section came from, as an index into
    /// `slides`. The two lists differ only where `<!-- next -->` broke a slide
    /// into parts, and this is what turns a rendered slide number — the number
    /// the viewer and the layout checker both count in — back into source.
    pub section_slides: Vec<usize>,
}

/// Where a build warning came from, as far as the source map can say.
///
/// Every field is optional, and absent means *not known* rather than *none*:
/// a warning about the frontmatter belongs to no slide, and a line variable
/// substitution rewrote belongs to no file, because the text on screen is not
/// text anyone typed there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarningSite {
    /// 1-based rendered slide number, counted the way the deck numbers itself.
    pub slide: Option<usize>,
    pub file: Option<PathBuf>,
    /// Byte offset of the slide's first character within `file`.
    pub offset: Option<usize>,
}

/// A stable name for what a build warning is about.
///
/// Here rather than beside `check`, because two readers now need the same
/// answer: the JSON report an agent parses, and the language server, which
/// hands it to an editor as a diagnostic code. A table in two places is a
/// table that disagrees with itself the first time a warning is added.
///
/// The messages themselves are prose written for a person and are free to be
/// reworded; this is the part a program may branch on, so it is matched on the
/// one distinctive token each family of warnings carries. Order matters - the
/// first match wins - and anything unrecognised is `build.other` rather than a
/// guess, which is also what a warning added after this table gets until it is
/// added here.
pub fn warning_kind(message: &str) -> &'static str {
    const TABLE: &[(&str, &str)] = &[
        // First, because this is the one message that quotes another program:
        // `mmdc` says "flowchart", which contains `chart`, and it is free to
        // say anything else on this list too. Classifying it before the table
        // can misread it is cheaper than teaching every other needle about a
        // tool Mirzam does not control.
        ("mermaid:", "build.mermaid"),
        // Then, and matched on two words: the message carries a filesystem
        // path, and a deck living under `charts/` must not be classified by
        // somebody's directory name.
        ("skill card", "build.skill"),
        ("shape line ", "build.shape"),
        ("shape:", "build.shape"),
        ("grid-pad", "build.layout"),
        ("grid-gap", "build.layout"),
        ("anim ", "build.anim"),
        ("cannot split", "build.anim"),
        ("a target is split", "build.anim"),
        ("annotate ", "build.annotate"),
        ("effects line ", "build.effects"),
        ("connect ", "build.connect"),
        ("chart", "build.chart"),
        ("footnote reference", "build.footnote"),
        ("toc:", "build.toc"),
        ("bibliography", "build.bibliography"),
        ("citations:", "build.bibliography"),
        ("masters:", "build.master"),
        ("master ", "build.master"),
        ("is not in the layout", "build.layout"),
        ("pane block", "build.layout"),
        ("merged region", "build.layout"),
        ("bg-light", "build.layout"),
        ("bg-dark", "build.layout"),
        ("is still on the slide as text", "build.span"),
        ("the brace over", "build.math"),
        ("math:", "build.math"),
        ("unknown theme", "build.theme"),
        // `theme: default` is an unknown name that gets its own wording, so it
        // needs its own needle or it would classify as `build.other`.
        ("no longer a theme name", "build.theme"),
        ("unknown mode", "build.theme"),
        // The stem rule, reported against the slide or pane that named a
        // theme file which cannot answer to a name.
        ("file theme is usable", "build.theme"),
        // A stylesheet the deck named and the host could not read.
        ("theme: cannot read", "build.css"),
        // The `css:` key, removed in v0.7.0 and kept only to say so.
        ("`css:` was removed", "build.css"),
        // Everything else a theme file has to say about itself: a stem that
        // collides with a built-in, one palette where two are needed, text
        // that cannot be read on its own background.
        ("theme: `", "build.theme"),
        ("transition:", "build.transition"),
        ("autoplay:", "build.autoplay"),
        ("no slides:", "build.deck"),
        ("<!-- next -->", "build.continuation"),
        ("file not found", "build.asset"),
        ("not inlined", "build.asset"),
    ];
    TABLE
        .iter()
        .find(|(needle, _)| message.contains(needle))
        .map_or("build.other", |(_, kind)| *kind)
}

impl BuildOutput {
    /// Where a rendered slide begins: its file, and the byte offset of its
    /// first real character in that file. `slide` is 1-based, as everything
    /// the viewer and the layout checker report is.
    pub fn slide_origin(&self, slide: usize) -> Option<(&Path, usize)> {
        let span = self
            .slides
            .get(*self.section_slides.get(slide.checked_sub(1)?)?)?;
        let head = span.text.find(|c: char| !c.is_whitespace())?;
        self.map.lookup(span.start + head)
    }

    /// Where a named pane's `::: pane` block is on that slide, falling back to
    /// the slide itself when the pane has no block of its own — content that
    /// names no pane flows into `main`, so `main` is often nowhere in the
    /// source, and the slide is then the closest true answer.
    pub fn pane_origin(&self, slide: usize, pane: &str) -> Option<(&Path, usize)> {
        let span = self
            .slides
            .get(*self.section_slides.get(slide.checked_sub(1)?)?)?;
        match pane_block_offset(&span.text, pane) {
            Some(at) => self.map.lookup(span.start + at),
            None => self.slide_origin(slide),
        }
    }
}

/// The offset within a slide's text of the line opening `pane`, or `None` when
/// the slide assigns nothing to it. Matched on the whole name so `fig` does not
/// find `figure`.
fn pane_block_offset(slide: &str, pane: &str) -> Option<usize> {
    let mut at = 0usize;
    for raw in slide.split_inclusive('\n') {
        let line = raw.trim_end();
        if let Some(rest) = line.trim_start().strip_prefix("::: pane ") {
            let name = rest
                .trim_start()
                .split(|c: char| c.is_whitespace() || c == '{')
                .next()
                .unwrap_or("");
            if name == pane {
                return Some(at);
            }
        }
        at += raw.len();
    }
    None
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
    // A path that is not there is usually a deck nobody has started yet, not a
    // typo, so the error says how to start one rather than only what failed.
    let src = std::fs::read_to_string(input).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!(
            "cannot read {}: no such file - `mirzam new {}` writes a deck there to start from",
            input.display(),
            input.display()
        ),
        _ => format!("cannot read {}: {e}", input.display()),
    })?;
    build_source(input, &src, cache, split_override, base_url)
}

/// The same build, over source the caller already has.
///
/// `input` still names where that source lives, because everything a deck
/// refers to - a transcluded file, an image, a bibliography - resolves against
/// its directory, and the source map answers in that file's offsets. What the
/// caller supplies is the *text*: the language server analyses a buffer that
/// has been typed into and not saved, and a build that read the file back off
/// disk would diagnose the version before the edit.
pub fn build_source(
    input: &Path,
    src: &str,
    cache: &mut RenderCache,
    split_override: Option<u8>,
    base_url: Option<&str>,
) -> Result<BuildOutput, String> {
    let base_dir = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    files.insert(input.to_path_buf());

    // 1. frontmatter
    let (fm, body) = mirzam_syntax::split_frontmatter(src);
    let meta = match fm {
        Some(yaml) => mirzam_core::parse_meta(yaml)?,
        None => mirzam_core::DeckMeta::default(),
    };
    // Kept as text, not only as `meta`: `--embed-source` hands a slide to the
    // browser editor with the deck's settings attached, and what the editor
    // needs is the YAML somebody wrote, not this crate's reading of it.
    let frontmatter = fm.map(str::to_string);
    let mut warnings = Vec::new();
    // Sites are recorded by the index of the warning they belong to, rather
    // than pushed alongside it, so the places that know nothing about location
    // — most of this function — stay untouched.
    let mut sites: HashMap<usize, WarningSite> = HashMap::new();
    warnings.extend(mirzam_render::theme_warnings(&meta));
    warnings.extend(mirzam_render::mode_warning(meta.mode.as_deref()));

    // A typo in `math:` renders the deck as LaTeX rather than failing, but
    // must say so — otherwise every formula just turns into an error span.
    // The dialect itself reaches the slides through the deck context below.
    if let Err(w) = meta.math_dialect() {
        warnings.push(w);
    }

    // Same rule for the grid keys: a value that is not a pixel length keeps
    // the stylesheet default, and the warning says which key to fix.
    warnings.extend(meta.grid_metrics().1);

    // A transition that does not parse leaves the deck with plain cuts, which
    // is the right outcome for a typo in a decoration - but say so, because
    // otherwise it looks like the feature is broken.
    if let Some(src) = &meta.transition {
        if let Err(e) = mirzam_anim::parse_transition(src) {
            warnings.push(format!("transition: {e}"));
        }
    }

    // Same shape for `autoplay:`: a value that does not parse is a deck driven
    // by hand, which on a kiosk looks exactly like the feature not existing.
    if let Some(src) = &meta.autoplay {
        if let Err(e) = mirzam_anim::parse_autoplay(src) {
            warnings.push(format!("autoplay: {e}"));
        }
    }

    // Load the deck's own stylesheets, in the order `theme:` names them.
    // Failures are warnings, not errors: a deck without its theme is still a
    // deck.
    let mut file_themes = Vec::new();
    for sheet in meta.theme_sheets() {
        let path = base_dir.join(sheet.path);
        // Watched even when the read fails, so creating the file it named
        // brings the deck's theme back without restarting `serve`.
        files.insert(path.clone());
        match std::fs::read_to_string(&path) {
            Ok(css) => file_themes.push(mirzam_render::FileTheme::new(sheet.path, css)),
            Err(e) => warnings.push(format!("theme: cannot read {}: {e}", sheet.path)),
        }
    }
    // What a theme somebody wrote has to say about itself: a stem that collides
    // with a built-in, a palette with no second mode, text that cannot be read
    // on its own background. The built-in themes have always been held to these;
    // a custom theme is the one that can actually fail them.
    warnings.extend(mirzam_render::file_theme_warnings(&file_themes));

    // 2. Expand includes, collecting the files that were read and where every
    //    byte of the result came from.
    let body_offset = src.len() - body.len();
    let expanded = mirzam_syntax::expand_includes_mapped(
        body,
        body_offset,
        input,
        &base_dir,
        &mirzam_syntax::FsProvider,
        &mut files,
    );
    // A transcluded file's frontmatter is ignored by design. Say so when one of
    // them named masters the deck will not draw it on, which is the one case
    // where ignoring it changes the slides rather than costing nothing.
    warnings.extend(mirzam_core::transclusion_warnings(
        &meta,
        &base_dir,
        &expanded.frontmatter,
    ));
    let (body, map) = (expanded.text, expanded.map);

    // 3. Substitute variables outside code fences. A variable change shows up
    //    as changed post-substitution source, so slide hashes pick it up.
    let vars = meta.var_table();
    let (body, map) = substitute_outside_fences(&body, &vars, &map);

    // 4. Split into slides and render each one through the cache.
    let level = split_override.or_else(|| meta.split_level());
    let slides = mirzam_syntax::split_slides_spanned(&body, level);

    // A deck with no slides builds fine and opens as a blank page that says
    // nothing about why - the same picture as a broken build. It stays a
    // success, because an empty file is where a new deck starts and `serve`
    // has to survive one until the first slide is typed into it; but it says so.
    if slides.is_empty() {
        warnings.push(if src.trim().is_empty() {
            format!("no slides: {} is empty", input.display())
        } else {
            format!(
                "no slides: {} has nothing outside its frontmatter",
                input.display()
            )
        });
    }

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

    // The same lookup `origin` makes, kept as coordinates instead of prose: a
    // tool wants the file and the offset, not a sentence naming the file.
    let site = |si: usize, slide_no: usize| -> WarningSite {
        let s: &mirzam_syntax::SlideSpan = &slides[si];
        let head = s.text.find(|c: char| !c.is_whitespace());
        let at = head.and_then(|o| map.lookup(s.start + o));
        WarningSite {
            slide: Some(slide_no),
            file: at.map(|(f, _)| f.to_path_buf()),
            offset: at.map(|(_, o)| o),
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
                sites.insert(warnings.len(), site(si, parts.len() + 1));
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
    let mut rendered = 0usize;

    // What a slide needs to know about the deck around it: the math dialect,
    // the masters it can be drawn on, and the footer every slide carries.
    // `parts.len()` rather than `slides.len()`, because a slide broken by
    // `<!-- next -->` is several pages to the audience and its number counts
    // them. Variables are substituted here for the same reason they are in the
    // body: a footer is text the author wrote, and `{{ }}` works in it.
    let mut ctx = mirzam_render::DeckContext::new(&meta, parts.len());
    // A slide asks the deck's themes one question - whether a `theme=` names
    // one of them - so they travel with the rest of the deck's settings.
    ctx.file_themes = file_themes.clone();
    // A `masters:` naming a file is read here rather than in the core, which
    // has no filesystem. It joins the watch set, so editing the shared shapes
    // re-renders the decks that use them.
    if let Some(rel) = meta.masters_file() {
        // Watched even when the read fails, so creating the file it named
        // brings the deck's layouts back without restarting `serve`.
        files.insert(base_dir.join(rel));
        match mirzam_syntax::load_masters(rel, &base_dir, &mirzam_syntax::FsProvider) {
            Ok((masters, master_warnings)) => {
                ctx.masters = masters;
                warnings.extend(master_warnings);
            }
            Err(w) => {
                ctx.masters_unavailable = true;
                warnings.push(w);
            }
        }
    }
    // The references a `[@key]` can name. Read here rather than in the core
    // for the same reason `masters:` is, and watched even when the read fails
    // so writing the file brings the citations to life without restarting
    // `serve`. Only the flag reaches a slide: the entries are substituted into
    // the deck once every slide has rendered, which is what lets an edit to
    // the `.bib` rewrite the reference list without re-rendering one slide.
    if let Some(rel) = meta.bibliography_file() {
        files.insert(base_dir.join(rel));
    }
    let (bib, bib_warnings) = mirzam_render::deck_bibliography(&meta, |rel| {
        std::fs::read_to_string(base_dir.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))
    });
    warnings.extend(bib_warnings);
    let (cite_style, style_warning) = mirzam_render::citation_style(&meta);
    warnings.extend(style_warning);

    for text in [&mut ctx.footer, &mut ctx.slide_number]
        .into_iter()
        .flatten()
    {
        *text = mirzam_core::substitute_vars(text, &vars);
    }
    warnings.extend(ctx.warnings());
    // The diagram renderer is a property of the machine, not of the deck, and
    // it is discovered once per build rather than once per fence: `mmdc`
    // answers `--version` by starting Node, which is not a cost to pay on
    // every slide. It joins the cache key because installing mermaid-cli
    // changes what a slide renders to without changing a byte of its source.
    let diagrams = crate::mermaid::Mmdc::discover();
    let ctx_key = ctx.fingerprint() ^ diagram_key(diagrams.as_ref());

    for (i, part) in parts.iter().enumerate() {
        let slide_src = &part.text;
        // The cache key includes the slide index, since data-index is baked
        // into the HTML — and the deck context, since the same source renders
        // differently under a different frontmatter `math:`, `masters:` or
        // `footer:`.
        let key = slide_hash(slide_src, i, ctx_key);
        let mut html = match cache.get(&key).filter(|e| e.is_fresh()) {
            Some(entry) => {
                for (p, _) in &entry.assets {
                    files.insert(p.clone());
                }
                entry.html.clone()
            }
            None => {
                let slide = mirzam_syntax::parse_slide(slide_src);
                let out = mirzam_render::render_slide_html(
                    &slide,
                    i,
                    &base_dir,
                    diagrams
                        .as_ref()
                        .map(|d| d as &dyn mirzam_render::DiagramRenderer),
                    &ctx,
                );
                let from = origin(part.from);
                let at = site(part.from, i + 1);
                for w in out.warnings {
                    sites.insert(warnings.len(), at.clone());
                    warnings.push(format!("{w}{from}"));
                }
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
        sections.push(html);
    }

    // `toc` needs to know about slides other than its own, so it resolves here,
    // once every slide has rendered. Each slide left a self-describing marker,
    // which is what lets a cached slide take part without being re-rendered.
    mirzam_render::resolve_deck(&mut sections);
    // Citations need the whole deck too, and one thing more: which slide the
    // reference list is on, which is not known until every `bibliography`
    // block has been placed. So it runs last, over the same assembled deck.
    warnings.extend(mirzam_render::resolve_citations(
        &mut sections,
        &bib,
        cite_style,
    ));
    let mut hashes: Vec<u64> = sections.iter().map(|s| str_hash(s)).collect();

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
            .map(|(i, p)| slide_hash(&p.text, i, ctx_key))
            .collect();
        cache.retain(|k, _| live_keys.contains(k));
    }

    // What `serve` reloads for: everything the page carries around the slides.
    // Asked of the renderer rather than listed here, so a setting added to the
    // page cannot be forgotten in this file — the options are the ones `serve`
    // assembles with, since those are what the answer describes.
    let page_fingerprint = {
        let opts = mirzam_render::PageOptions {
            file_themes: file_themes.clone(),
            all_themes: true,
            ..Default::default()
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        mirzam_render::page_fingerprint(&meta, &sections, &opts).hash(&mut h);
        // Not on the page, but it decides what a slide *is*: `--split` handed
        // in on the command line reaches no frontmatter this could read.
        level.hash(&mut h);
        h.finish()
    };

    let warning_sites = (0..warnings.len())
        .map(|i| sites.remove(&i).unwrap_or_default())
        .collect();

    Ok(BuildOutput {
        meta,
        frontmatter,
        body,
        sections,
        hashes,
        page_fingerprint,
        file_themes,
        warnings,
        warning_sites,
        files,
        rendered,
        map,
        slides,
        section_slides: parts.iter().map(|p| p.from).collect(),
    })
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Which diagram renderer this build found, as part of the slide cache key.
///
/// A slide holding a `mermaid` fence renders to a diagram or to a code block
/// depending on nothing in the deck at all, so a cache warmed before
/// mermaid-cli was installed would keep serving the code block afterwards.
/// The path is hashed rather than a bare flag, because pointing `MIRZAM_MMDC`
/// at a different `mmdc` is a different renderer.
fn diagram_key(renderer: Option<&crate::mermaid::Mmdc>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    renderer.map(|r| r.program().to_path_buf()).hash(&mut h);
    h.finish()
}

fn slide_hash(src: &str, index: usize, ctx: u64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    index.hash(&mut h);
    ctx.hash(&mut h);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_block_is_found_by_its_whole_name() {
        let slide = "# Head\n\n::: pane figure\nx\n:::\n\n::: pane fig {align=center}\ny\n:::\n";
        let at = pane_block_offset(slide, "fig").expect("fig has a block");
        assert!(
            slide[at..].starts_with("::: pane fig {"),
            "{:?}",
            &slide[at..]
        );
        assert_eq!(pane_block_offset(slide, "figure"), Some(8));
        assert_eq!(pane_block_offset(slide, "main"), None);
    }
}
