//! Deck metadata (frontmatter) and the evaluator for `{{ variable/expression }}`.

mod expr;

pub use expr::{eval_expr, Value};

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Deck settings declared in frontmatter.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeckMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    /// The deck's look, in cascade order: built-in theme names and paths to
    /// stylesheets of your own. See [`ThemeSpec`].
    pub theme: ThemeSpec,
    /// Forces light or dark mode. Unset defers to the viewer's
    /// `prefers-color-scheme`, overridable there with `?mode=` or `D`.
    pub mode: Option<String>,
    /// Aspect ratio, e.g. "16:9" or "4:3".
    pub aspect: Option<String>,
    /// Start a new slide at every heading of this level: "h1", "h2", "h3".
    /// Slides always break on `---` as well.
    pub split: Option<String>,
    /// How pages turn, e.g. "fade" or "slide-left 400ms". A slide that
    /// declares its own whole-slide `[enter]`/`[exit]` track overrides the
    /// matching half. Parsed by `mirzam_anim::parse_transition`.
    pub transition: Option<String>,
    /// `fit: shrink` asks every pane to shrink its text rather than clip it.
    /// Panes opt in individually with `{fit=shrink}`; this is the same thing
    /// said once for the whole deck.
    pub fit: Option<String>,
    /// Which syntax `$...$` holds: `latex` (the default) or `typst`.
    /// Per deck, not per formula — a deck reads as one language.
    pub math: Option<String>,
    /// Named layouts a slide can be drawn on instead of carrying a `pane`
    /// block of its own. A slide picks one with `<!-- layout: name -->`.
    pub masters: Masters,
    /// The master every slide takes when it neither draws a grid nor names
    /// one. A slide opts out of it with `<!-- layout: none -->`.
    pub layout: Option<String>,
    /// Text drawn along the bottom of every slide. `{n}` and `{total}` are
    /// substituted; a slide drops it with `<!-- chrome: none -->`.
    pub footer: Option<String>,
    /// The slide's own number, drawn opposite the footer. Same substitutions,
    /// so `"{n} / {total}"` is the usual value.
    #[serde(rename = "slide-number", alias = "slide_number")]
    pub slide_number: Option<String>,
    /// References the deck may cite with `[@key]`, and list with a
    /// `bibliography` block. Unset means `[@key]` is ordinary text.
    pub bibliography: BibSource,
    /// What a `[@key]` reads as: `numeric` for `[1]`, `author` for
    /// `[Vaswani+17]`. Per deck, because a deck cites one way.
    #[serde(rename = "citation-style", alias = "citation_style")]
    pub citation_style: Option<String>,
    /// The grid's horizontal margin, e.g. `64px` (or bare `64`). One number
    /// behind the `--mz-grid-pad-x` custom property; see [`GridMetrics`] for
    /// why the core reads it rather than leaving it to a stylesheet.
    #[serde(rename = "grid-pad-x", alias = "grid_pad_x")]
    pub grid_pad_x: Option<PxLength>,
    /// The grid's vertical margin — `--mz-grid-pad-y`.
    #[serde(rename = "grid-pad-y", alias = "grid_pad_y")]
    pub grid_pad_y: Option<PxLength>,
    /// The gutter between panes — `--mz-grid-gap`.
    #[serde(rename = "grid-gap", alias = "grid_gap")]
    pub grid_gap: Option<PxLength>,
    pub vars: BTreeMap<String, serde_yaml::Value>,
}

/// What `theme:` holds: built-in names and paths to stylesheets, in cascade
/// order.
///
/// ```yaml
/// theme: mirzam                        # a built-in
/// theme: themes/acme.css               # a file, relative to the deck
/// theme: [mirzam, themes/tweaks.css]   # a built-in, then a file over it
/// ```
///
/// An entry ending in `.css` is a path and anything else is a built-in name —
/// see [`is_theme_path`]. No built-in is named that way and no stylesheet path
/// is not, so the rule needs no escape syntax; it costs a constraint on future
/// theme names, which is cheap.
///
/// A scalar is a list of one, so every deck that already wrote `theme: nord`
/// parses unchanged.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
pub enum ThemeSpec {
    /// `theme: nord`, or `theme: themes/acme.css`.
    One(String),
    /// `theme: [nord, themes/acme.css]`, in cascade order.
    Many(Vec<String>),
    /// The key absent, or written with nothing after it.
    #[default]
    Unset,
}

impl ThemeSpec {
    /// The entries as written, in order.
    pub fn entries(&self) -> &[String] {
        match self {
            ThemeSpec::One(one) => std::slice::from_ref(one),
            ThemeSpec::Many(many) => many,
            ThemeSpec::Unset => &[],
        }
    }
}

/// Whether an entry of `theme:` names a stylesheet rather than a built-in
/// theme. The whole of the grammar's ambiguity, in one place: an entry ending
/// in `.css` is a path, and anything else is a name.
pub fn is_theme_path(entry: &str) -> bool {
    entry.trim().to_ascii_lowercase().ends_with(".css")
}

/// A stylesheet a deck loads with `theme:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSheet<'a> {
    /// The path as written, relative to the deck.
    pub path: &'a str,
}

/// A pixel length in frontmatter: `64px`, `"64px"` or bare `64`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PxLength {
    Num(f64),
    Str(String),
}

impl PxLength {
    /// The value in pixels, or an explanation of why it is not one.
    pub fn px(&self) -> Result<f64, String> {
        match self {
            PxLength::Num(n) => Ok(*n),
            PxLength::Str(s) => s
                .trim()
                .trim_end_matches("px")
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("`{s}` is not a pixel length (write `64px` or `64`)")),
        }
    }
}

/// The grid's margin and gutter in slide pixels — the numbers behind the
/// `--mz-grid-pad-x/y` and `--mz-grid-gap` custom properties, with the
/// stylesheet's defaults.
///
/// The core reads these because pane rectangles are computed at build time —
/// a `shape` block inside a `::: pane` is drawn in that pane's coordinate
/// space, and the pane's rectangle is margin and gutter arithmetic. A value
/// declared in frontmatter is also emitted as CSS so the browser lays the
/// grid out with the same numbers; a stylesheet that overrides the custom
/// properties instead moves the panes without telling the core, and anchored
/// shapes drift by the difference — which is why frontmatter is the
/// supported place to change them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMetrics {
    pub pad_x: f64,
    pub pad_y: f64,
    pub gap: f64,
}

impl Default for GridMetrics {
    /// The values `base.css` falls back to when no custom property is set.
    fn default() -> Self {
        Self {
            pad_x: 60.0,
            pad_y: 44.0,
            gap: 20.0,
        }
    }
}

/// Where a deck's references come from.
///
/// The same two forms as [`Masters`], for the same reason and with a stronger
/// case: a `.bib` is what a reference manager exports and what a paper already
/// has beside it, so naming the file is the path that costs an author nothing.
/// A deck citing three papers can write them in its own frontmatter instead
/// and stay one file.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BibSource {
    /// `bibliography: refs.bib`, resolved relative to the deck like `theme:` paths are.
    File(String),
    /// `bibliography: {key: {author: …, title: …, year: …}}`, in frontmatter.
    /// The field names are BibTeX's, so an entry can be lifted either way.
    Inline(BTreeMap<String, BTreeMap<String, String>>),
}

impl Default for BibSource {
    fn default() -> Self {
        BibSource::Inline(BTreeMap::new())
    }
}

impl BibSource {
    /// Whether the deck declared any references at all. This is what decides
    /// whether `[@key]` is a citation or the text somebody typed: a deck with
    /// no bibliography must leave the brackets alone.
    pub fn is_empty(&self) -> bool {
        matches!(self, BibSource::Inline(m) if m.is_empty())
    }
}

/// Where a deck's named slide shapes come from.
///
/// Two forms because the drawings are big. A set worth sharing between decks —
/// or one long enough that it pushes the first slide off the screen — belongs
/// in a file of its own, where the ASCII sits in `pane` fences at column zero
/// rather than indented inside a YAML block scalar. A deck with one shape it
/// reuses can keep it in frontmatter and skip the second file.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Masters {
    /// `masters: masters.md`, resolved relative to the deck like `theme:` paths are.
    File(String),
    /// `masters: {two-up: |…}`, written in the deck's own frontmatter.
    Inline(BTreeMap<String, String>),
}

impl Default for Masters {
    fn default() -> Self {
        Masters::Inline(BTreeMap::new())
    }
}

impl Masters {
    /// Whether a deck said nothing about masters at all.
    pub fn is_empty(&self) -> bool {
        matches!(self, Masters::Inline(m) if m.is_empty())
    }

    /// Whether these are the same shapes as `other`, given the directory each
    /// was declared in — a path is relative to its own file, so the root's
    /// `masters/deck.md` and a section's `../masters/deck.md` are one file.
    ///
    /// Used to tell a transcluded file that carries frontmatter so its author
    /// can build it alone (the same shapes, said twice) from one that names
    /// shapes the deck will never draw it on.
    pub fn same_as(&self, dir: &Path, other: &Masters, other_dir: &Path) -> bool {
        match (self, other) {
            (Masters::File(a), Masters::File(b)) => resolve(dir, a) == resolve(other_dir, b),
            (Masters::Inline(a), Masters::Inline(b)) => a == b,
            _ => false,
        }
    }
}

/// `dir/rel` with `.` and `..` folded away, so two spellings of one path
/// compare equal without touching the filesystem — which the core cannot do,
/// and which would answer differently for a file that is not there yet.
fn resolve(dir: &Path, rel: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for part in dir.join(rel).components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The syntax math is written in. Every dialect renders through the same
/// LaTeX -> MathML path; this only chooses the front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MathDialect {
    #[default]
    Latex,
    Typst,
}

impl DeckMeta {
    /// Logical slide size (width, height) for the aspect ratio. Defaults to 16:9.
    pub fn slide_size(&self) -> (u32, u32) {
        match self.aspect.as_deref() {
            Some("4:3") => (1024, 768),
            _ => (1280, 720),
        }
    }

    /// The math front end `math:` asks for. `Err` carries a warning for an
    /// unrecognised value; the deck still renders, with the default.
    pub fn math_dialect(&self) -> Result<MathDialect, String> {
        match self.math.as_deref().map(str::trim) {
            None | Some("latex") => Ok(MathDialect::Latex),
            Some("typst") => Ok(MathDialect::Typst),
            Some(other) => Err(format!(
                "math: unknown dialect `{other}`; `latex` and `typst` are supported, \
                 rendering as latex"
            )),
        }
    }

    /// Every entry of `theme:`, in cascade order.
    ///
    /// Empty entries are dropped, so `theme:` written with nothing after it,
    /// or a list with a stray blank in it, is the same as not writing the key.
    pub fn theme_entries(&self) -> Vec<&str> {
        self.theme
            .entries()
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .collect()
    }

    /// The built-in names among them, in the order written.
    pub fn theme_names(&self) -> Vec<&str> {
        self.theme_entries()
            .into_iter()
            .filter(|e| !is_theme_path(e))
            .collect()
    }

    /// The built-in palette the deck renders in: the **last** built-in named,
    /// because the list is a cascade and a later entry overrides an earlier
    /// one. `None` when the deck names only stylesheets of its own, or
    /// nothing at all — the renderer's fallback theme, whose tokens the shared
    /// stylesheet reads, then applies.
    pub fn theme_name(&self) -> Option<&str> {
        self.theme_names().pop()
    }

    /// The stylesheets this deck loads, in cascade order. Reading them is the
    /// caller's job, like `masters:` and `bibliography:`: the core has no
    /// filesystem, and both hosts already carry a `FileProvider`.
    pub fn theme_sheets(&self) -> Vec<ThemeSheet<'_>> {
        self.theme
            .entries()
            .iter()
            .map(|e| e.trim())
            .filter(|e| !e.is_empty() && is_theme_path(e))
            .map(|path| ThemeSheet { path })
            .collect()
    }

    /// The masters file this deck names, if it names one rather than writing
    /// its shapes inline. Reading it is the caller's job: the core has no
    /// filesystem, and both hosts already carry a `FileProvider`.
    pub fn masters_file(&self) -> Option<&str> {
        match &self.masters {
            Masters::File(path) => Some(path.as_str()),
            Masters::Inline(_) => None,
        }
    }

    /// Shapes written in the deck's own frontmatter; `None` when it names a
    /// file instead.
    pub fn inline_masters(&self) -> Option<&BTreeMap<String, String>> {
        match &self.masters {
            Masters::Inline(m) => Some(m),
            Masters::File(_) => None,
        }
    }

    /// The `.bib` this deck names, if it names a file rather than writing its
    /// references inline. Read by the caller, like `masters:` and for the same
    /// reason: the core has no filesystem.
    pub fn bibliography_file(&self) -> Option<&str> {
        match &self.bibliography {
            BibSource::File(path) => Some(path.as_str()),
            BibSource::Inline(_) => None,
        }
    }

    /// References written in the deck's own frontmatter; `None` when it names
    /// a file instead.
    pub fn inline_bibliography(&self) -> Option<&BTreeMap<String, BTreeMap<String, String>>> {
        match &self.bibliography {
            BibSource::Inline(m) => Some(m),
            BibSource::File(_) => None,
        }
    }

    /// Heading level that starts a new slide, if `split:` asks for one.
    pub fn split_level(&self) -> Option<u8> {
        match self.split.as_deref()?.trim().to_ascii_lowercase().as_str() {
            "h1" | "1" => Some(1),
            "h2" | "2" => Some(2),
            "h3" | "3" => Some(3),
            _ => None,
        }
    }

    /// The grid metrics this deck declares, with the stylesheet defaults for
    /// anything unsaid. A value that does not parse keeps its default and is
    /// returned as a warning; the deck still renders.
    pub fn grid_metrics(&self) -> (GridMetrics, Vec<String>) {
        let mut m = GridMetrics::default();
        let mut warnings = Vec::new();
        let mut take = |field: &Option<PxLength>, name: &str, slot: &mut f64| {
            if let Some(v) = field {
                match v.px() {
                    Ok(px) => *slot = px,
                    Err(e) => warnings.push(format!("{name}: {e}")),
                }
            }
        };
        take(&self.grid_pad_x, "grid-pad-x", &mut m.pad_x);
        take(&self.grid_pad_y, "grid-pad-y", &mut m.pad_y);
        take(&self.grid_gap, "grid-gap", &mut m.gap);
        (m, warnings)
    }

    /// The CSS that carries declared grid metrics to the browser, so the grid
    /// is laid out with the same numbers the core computed pane rectangles
    /// from. Empty when the deck declares none — the stylesheet defaults (or a
    /// theme's overrides) then apply, exactly as before the keys existed.
    /// Emitted after the theme and any custom stylesheet, so a frontmatter
    /// declaration wins over both.
    ///
    /// `:root` is not enough on its own. A theme scope undefines every dial it
    /// does not set, so a slide or a pane carrying `theme=` would stop
    /// inheriting these three from the document and fall back to the
    /// stylesheet's numbers — while the core went on computing that slide's
    /// pane rectangles from the declared ones. Naming the scopes as well keeps
    /// one set of numbers for the whole deck, which is what the pane geometry
    /// already assumes; the selector also outranks a theme's own
    /// zero-specificity block, which is the promise above.
    pub fn grid_metrics_css(&self) -> String {
        let mut props = String::new();
        for (field, prop) in [
            (&self.grid_pad_x, "--mz-grid-pad-x"),
            (&self.grid_pad_y, "--mz-grid-pad-y"),
            (&self.grid_gap, "--mz-grid-gap"),
        ] {
            if let Some(px) = field.as_ref().and_then(|v| v.px().ok()) {
                props.push_str(&format!("{prop}:{px}px;"));
            }
        }
        if props.is_empty() {
            String::new()
        } else {
            format!(":root,[data-theme],[data-mode]{{{props}}}")
        }
    }

    /// Variable table used by the expression evaluator.
    pub fn var_table(&self) -> BTreeMap<String, Value> {
        self.vars
            .iter()
            .map(|(k, v)| {
                let val = match v {
                    serde_yaml::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
                    serde_yaml::Value::Bool(b) => Value::Str(b.to_string()),
                    serde_yaml::Value::String(s) => {
                        // Treat numeric-looking strings as numbers so they can be used in arithmetic.
                        match s.parse::<f64>() {
                            Ok(n) => Value::Num(n),
                            Err(_) => Value::Str(s.clone()),
                        }
                    }
                    other => Value::Str(
                        serde_yaml::to_string(other)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                    ),
                };
                (k.clone(), val)
            })
            .collect()
    }
}

/// Parses frontmatter as YAML.
pub fn parse_meta(yaml: &str) -> Result<DeckMeta, String> {
    serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse frontmatter: {e}"))
}

/// Warns about a transcluded file whose `masters:` names shapes the deck will
/// never draw it on.
///
/// A transcluded file's frontmatter is ignored, which is deliberate and
/// usually invisible — a section repeating the deck's own `masters:` so its
/// author can build it alone is the pattern that setting is for, and it costs
/// the deck nothing. Naming a *different* set is the case that is not a
/// convenience: the section is drawn on shapes its author never previewed, and
/// nothing else in a build would say so. Same pane names in both files and the
/// only trace is proportions nobody compared.
///
/// `root_dir` and each file's own directory are needed because a `masters:`
/// path is relative to the file that wrote it.
pub fn transclusion_warnings(
    root: &DeckMeta,
    root_dir: &Path,
    children: &[(PathBuf, String)],
) -> Vec<String> {
    let mut out = Vec::new();
    for (path, yaml) in children {
        let Ok(child) = parse_meta(yaml) else {
            continue;
        };
        if child.masters.is_empty() {
            continue;
        }
        let child_dir = path.parent().unwrap_or(Path::new(""));
        if child.masters.same_as(child_dir, &root.masters, root_dir) {
            continue;
        }
        out.push(format!(
            "{}: its `masters:` names different shapes from the deck's; a \
             transcluded file's frontmatter is not read, so these slides are \
             drawn on the deck's masters, not the ones this file was previewed on",
            path.display()
        ));
    }
    out
}

/// Evaluates and substitutes `{{ ... }}` occurrences in `text`.
/// Anything that fails to evaluate is left verbatim rather than dropped.
pub fn substitute_vars(text: &str, vars: &BTreeMap<String, Value>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let inner = &after[..end];
                match eval_expr(inner, vars) {
                    Ok(v) => out.push_str(&v.to_display()),
                    Err(_) => {
                        out.push_str("{{");
                        out.push_str(inner);
                        out.push_str("}}");
                    }
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> BTreeMap<String, Value> {
        let mut m = BTreeMap::new();
        m.insert("price".to_string(), Value::Num(1200.0));
        m.insert("product".to_string(), Value::Str("Mirzam".to_string()));
        m
    }

    #[test]
    fn substitute_simple_and_expr() {
        let v = vars();
        assert_eq!(
            substitute_vars("{{product}} costs {{price * 12}} per year", &v),
            "Mirzam costs 14400 per year"
        );
    }

    #[test]
    fn theme_and_mode_are_parsed_from_frontmatter() {
        let meta = parse_meta("theme: nord\nmode: dark\n").unwrap();
        assert_eq!(meta.theme_names(), ["nord"]);
        assert_eq!(meta.theme_name(), Some("nord"));
        assert!(meta.theme_sheets().is_empty());
        assert_eq!(meta.mode.as_deref(), Some("dark"));
    }

    /// A scalar is a list of one, so every deck written before `theme:` took a
    /// list parses unchanged; a list is cascade order; and an entry ending in
    /// `.css` is a path rather than a name.
    #[test]
    fn theme_takes_a_name_a_path_or_a_list() {
        let one = parse_meta("theme: themes/acme.css\n").unwrap();
        assert!(one.theme_names().is_empty());
        assert_eq!(one.theme_name(), None);
        assert_eq!(
            one.theme_sheets(),
            [ThemeSheet {
                path: "themes/acme.css",
            }]
        );

        let list = parse_meta("theme: [nord, themes/acme.css]\n").unwrap();
        assert_eq!(list.theme_entries(), ["nord", "themes/acme.css"]);
        assert_eq!(list.theme_name(), Some("nord"));
        assert_eq!(list.theme_sheets()[0].path, "themes/acme.css");

        // A block list is the same list.
        let block = parse_meta("theme:\n  - nord\n  - themes/acme.css\n").unwrap();
        assert_eq!(block.theme_entries(), list.theme_entries());

        // Two built-ins is a cascade, and the last one is the deck's palette.
        assert_eq!(
            parse_meta("theme: [nord, wuwei]\n").unwrap().theme_name(),
            Some("wuwei")
        );

        // The key with nothing after it is the key not written.
        let empty = parse_meta("theme:\n").unwrap();
        assert!(empty.theme_entries().is_empty());
        assert_eq!(empty.theme_name(), None);
    }

    /// The `css:` key was retired in `v0.6.0` (a warning carried the exact
    /// `theme:` replacement line for one release) and removed after it. It is
    /// an unknown key now, ignored like any other.
    #[test]
    fn the_removed_css_key_is_an_unknown_key() {
        let old = parse_meta("theme: mirzam\ncss: themes/acme.css\n").unwrap();
        assert_eq!(old.theme_entries(), ["mirzam"]);
        assert!(old.theme_sheets().is_empty());
    }

    #[test]
    fn split_level_parses_forms() {
        let meta = |v: &str| parse_meta(&format!("split: {v}\n")).unwrap();
        assert_eq!(meta("h2").split_level(), Some(2));
        assert_eq!(meta("3").split_level(), Some(3));
        assert_eq!(meta("none").split_level(), None);
        assert_eq!(DeckMeta::default().split_level(), None);
    }

    /// One key, two forms: a string names a file, a mapping is the shapes
    /// themselves. A deck that says neither has no masters and no complaint.
    #[test]
    fn masters_is_either_a_path_or_the_shapes_themselves() {
        let file = parse_meta("masters: masters/cookbook.md\n").unwrap();
        assert_eq!(file.masters_file(), Some("masters/cookbook.md"));
        assert_eq!(file.inline_masters(), None);

        let inline =
            parse_meta("masters:\n  two-up: |\n    +---+\n    | a |\n    +---+\n").unwrap();
        assert_eq!(inline.masters_file(), None);
        assert!(inline.inline_masters().unwrap()["two-up"].contains("| a |"));

        let none = DeckMeta::default();
        assert_eq!(none.masters_file(), None);
        assert!(none.inline_masters().unwrap().is_empty());
    }

    /// A `masters:` path is relative to the file that wrote it, so the root's
    /// and a section's spellings of one file differ and still have to compare
    /// equal — without asking the filesystem, which the core cannot do and
    /// which would answer differently for a file nobody has written yet.
    #[test]
    fn two_spellings_of_one_masters_file_are_the_same_shapes() {
        let root = parse_meta("masters: shapes/deck.md\n").unwrap();
        let child = parse_meta("masters: ../shapes/deck.md\n").unwrap();
        assert!(child
            .masters
            .same_as(Path::new("sections"), &root.masters, Path::new("")));

        let other = parse_meta("masters: shapes/other.md\n").unwrap();
        assert!(!other
            .masters
            .same_as(Path::new(""), &root.masters, Path::new("")));
    }

    #[test]
    fn a_transcluded_file_is_only_reported_when_its_masters_differ() {
        let root = parse_meta("masters: shapes/deck.md\n").unwrap();
        let same = (
            PathBuf::from("sections/a.md"),
            "masters: ../shapes/deck.md\n".to_string(),
        );
        let differs = (
            PathBuf::from("sections/b.md"),
            "masters: ../shapes/own.md\n".to_string(),
        );
        let silent = (PathBuf::from("sections/c.md"), "title: C\n".to_string());
        let out = transclusion_warnings(&root, Path::new(""), &[same, differs, silent]);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].contains("sections/b.md"), "{out:?}");
    }

    /// The three grid keys accept `64px`, `"64px"` and bare numbers; anything
    /// unsaid keeps the stylesheet default, and a bad value warns instead of
    /// failing the deck.
    #[test]
    fn grid_metrics_parse_declared_values_and_keep_defaults() {
        let (m, w) = DeckMeta::default().grid_metrics();
        assert_eq!(m, GridMetrics::default());
        assert!(w.is_empty());
        assert_eq!(DeckMeta::default().grid_metrics_css(), "");

        let meta = parse_meta("grid-pad-x: 64px\ngrid-pad-y: \"48px\"\ngrid-gap: 24\n").unwrap();
        let (m, w) = meta.grid_metrics();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!((m.pad_x, m.pad_y, m.gap), (64.0, 48.0, 24.0));
        assert_eq!(
            meta.grid_metrics_css(),
            ":root,[data-theme],[data-mode]\
             {--mz-grid-pad-x:64px;--mz-grid-pad-y:48px;--mz-grid-gap:24px;}"
        );

        let meta = parse_meta("grid-gap: wide\n").unwrap();
        let (m, w) = meta.grid_metrics();
        assert_eq!(m.gap, GridMetrics::default().gap);
        assert!(w.iter().any(|e| e.contains("grid-gap")), "{w:?}");
    }

    /// A partial declaration emits only what was declared — the other custom
    /// properties stay the stylesheet's business.
    #[test]
    fn grid_metrics_css_is_partial() {
        let meta = parse_meta("grid-gap: 24px\n").unwrap();
        assert_eq!(
            meta.grid_metrics_css(),
            ":root,[data-theme],[data-mode]{--mz-grid-gap:24px;}"
        );
    }

    /// A slide or a pane wearing a theme of its own is a scope that undefines
    /// every dial the theme leaves unset, this trio included — so declared
    /// metrics have to be written for those elements too, or the browser lays
    /// that slide's grid out with numbers the core never used.
    #[test]
    fn declared_grid_metrics_reach_a_re_themed_slide() {
        let css = parse_meta("grid-gap: 24px\n").unwrap().grid_metrics_css();
        assert!(css.contains("[data-theme]"), "{css}");
        assert!(css.contains("[data-mode]"), "{css}");
    }

    #[test]
    fn unknown_var_left_as_is() {
        let v = vars();
        assert_eq!(substitute_vars("{{unknown}}", &v), "{{unknown}}");
    }

    #[test]
    fn unterminated_braces_kept() {
        let v = vars();
        assert_eq!(substitute_vars("a {{price", &v), "a {{price");
    }
}
