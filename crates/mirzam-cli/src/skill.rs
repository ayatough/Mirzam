//! `mirzam skill install`: the binary writes the agent skill, rather than the
//! repository shipping one for people to copy.
//!
//! The reason it works this way is drift. A skill is a file in somebody's deck
//! repository, and nothing in Claude Code versions it; a syntax card copied
//! once from a release will still be there two releases later, describing
//! markup the binary beside it no longer has. So the card is *embedded in the
//! binary* — `docs/llms.md` verbatim, by `include_str!`, so there is no second
//! copy in this crate that could drift from the real one — and the generated
//! `SKILL.md` is stamped with the version that wrote it. `build` and `check`
//! look for that stamp near the deck and report a mismatch as an ordinary
//! warning, which is how the agent repairs the drift in the loop it already
//! runs.
//!
//! Two skills, because two kinds of surface exist:
//!
//! - **`mirzam`** — the terminal. Check the binary, write the deck, run
//!   `mirzam check --format json`, fix what it names. The whole loop.
//! - **`mirzam-writing`** — claude.ai, the desktop app, a phone. No filesystem
//!   and no binary, so the checking half of the loop is gone: write correct
//!   markup from the card and hand the `.md` to the person, whose renderer is
//!   the browser editor. Shipped as the `.zip` those surfaces upload.
//!
//! Everything here is in the CLI, and nothing in the core crates: the core
//! must not touch the filesystem, because that is what keeps the WebAssembly
//! build possible.

use crate::pipeline::{BuildOutput, WarningSite};
use std::path::{Path, PathBuf};

/// The syntax card, embedded from the real `docs/llms.md` rather than copied
/// into this crate: a copy is a thing that can be a release behind, which is
/// the exact failure this whole command exists to prevent.
pub const CARD: &str = include_str!("../../../docs/llms.md");

/// The version this binary stamps into, and expects to find in, a skill.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const PROJECT_SKILL: &str = include_str!("skill/project-skill.md");
const WRITING_SKILL: &str = include_str!("skill/writing-skill.md");

/// Where the card is written inside the skill folder. A separate file rather
/// than 360 lines inlined into `SKILL.md`: the skill's own instructions are
/// what an agent reads every time, and the card is what it opens when it is
/// about to write markup.
const CARD_PATH: &str = "references/llms.md";

/// The skill folder installed into a deck repository, and the name the drift
/// check looks for.
pub const SKILL_DIR: &str = ".claude/skills/mirzam";

/// The sandbox skill's folder name, which is also its root inside the archive.
const WRITING_DIR: &str = "mirzam-writing";

/// Default name for `--zip` with no path.
pub const ZIP_NAME: &str = "mirzam-writing-skill.zip";

/// Where `skill install` writes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// `.claude/skills/mirzam/` in the repository the working directory is in,
    /// which is the skill Claude Code loads for that project alone.
    Project,
    /// `~/.claude/skills/mirzam/`, loaded in every directory.
    User,
}

/// What an install wrote, so the caller can say it.
pub struct Installed {
    pub skill: PathBuf,
    pub card: PathBuf,
}

/// Writes `SKILL.md` and the syntax card into the skill folder.
///
/// Refuses, without `force`, to overwrite a `SKILL.md` or a card that has been
/// edited since it was installed — an edited skill is somebody's work, and a
/// silent overwrite is the one thing an install command must never do. An
/// unmodified older version is overwritten without a word, because that is not
/// work, it is a stale copy.
pub fn install(dest: Destination, force: bool) -> Result<Installed, String> {
    let root = skill_root(dest)?;
    let skill_path = root.join("SKILL.md");
    let card_path = root.join(CARD_PATH);

    if !force {
        if let Some(edited) = edited_file(&skill_path, &card_path) {
            return Err(format!(
                "{} has been edited since it was installed.\n       \
                 `mirzam skill install --force` overwrites it (your edits are lost); \
                 deleting {} first does the same thing more visibly.",
                edited.display(),
                root.display()
            ));
        }
    }

    let contents = stamped(PROJECT_SKILL);
    write_file(&card_path, CARD)?;
    write_file(&skill_path, &contents)?;
    Ok(Installed {
        skill: skill_path,
        card: card_path,
    })
}

/// Writes the sandbox skill as a `.zip`, laid out the way an upload expects:
/// one folder at the archive's root, with `SKILL.md` directly inside it.
pub fn write_zip(path: &Path) -> Result<(), String> {
    let files = vec![
        (format!("{WRITING_DIR}/SKILL.md"), stamped(WRITING_SKILL)),
        (format!("{WRITING_DIR}/{CARD_PATH}"), CARD.to_string()),
    ];
    let bytes = mirzam_pptx::zip::archive(&files);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, &bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The skill folder for a destination. A project install goes to the root of
/// the repository the working directory is in, not to the working directory
/// itself: `mirzam skill install` run from `talks/2026/` should put the skill
/// where Claude Code will find it from anywhere in the repository.
fn skill_root(dest: Destination) -> Result<PathBuf, String> {
    match dest {
        Destination::Project => {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("cannot read the working directory: {e}"))?;
            let root = repo_root(&cwd).unwrap_or(cwd);
            Ok(root.join(SKILL_DIR))
        }
        Destination::User => Ok(home_dir()
            .ok_or("cannot find your home directory; set HOME, or install into the project")?
            .join(SKILL_DIR)),
    }
}

/// The nearest ancestor of `from` holding a `.git`, if any.
fn repo_root(from: &Path) -> Option<PathBuf> {
    from.ancestors()
        .find(|d| d.join(".git").exists())
        .map(Path::to_path_buf)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Which of the two installed files, if either, somebody has edited.
///
/// A file that is not there yet is not an edit: a first install writes both,
/// and a card deleted from an otherwise pristine skill is simply replaced.
fn edited_file(skill: &Path, card: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(skill).ok()?;
    let stamp = match parse_stamp(&text) {
        // Something is in the way that this command did not write. Refusing is
        // the only safe reading of it.
        None => return Some(skill.to_path_buf()),
        Some(s) => s,
    };
    if !stamp.matches(&text) {
        return Some(skill.to_path_buf());
    }
    match std::fs::read_to_string(card) {
        Ok(installed) if hex(fnv1a(&installed)) != stamp.card => Some(card.to_path_buf()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The stamp
// ---------------------------------------------------------------------------

/// The machine-readable half of the stamp. An HTML comment rather than a
/// frontmatter field: Claude Code ignores unknown frontmatter, so a stamp
/// there would be invisible to the person the warning is also for — this sits
/// under a sentence in the body that says the same thing in prose.
///
/// `hash` covers the whole `SKILL.md` with this attribute blanked, which is
/// what lets any version's pristine output be recognised as pristine without
/// this binary knowing what that version's template said. `card` covers
/// `references/llms.md`. **Neither the line's shape nor `fnv1a` may change**:
/// an older install's stamp has to stay verifiable by a newer binary.
fn stamp_line(version: &str, hash: &str, card: &str) -> String {
    format!("<!-- mirzam-skill version=\"{version}\" hash=\"{hash}\" card=\"{card}\" -->")
}

/// The skill body plus the block that says which binary wrote it: a sentence
/// for a person, the machine-readable line for `check`.
fn stamped(body: &str) -> String {
    stamped_by(VERSION, body, CARD)
}

/// Stamping with the version spelled out, so a test can produce what an older
/// binary would have written without one being installed.
fn stamped_by(version: &str, body: &str, card: &str) -> String {
    let card = hex(fnv1a(card));
    let head = format!(
        "{}\n\n## This skill's version\n\n\
         Written by **mirzam {version}**, which embedded the syntax card beside it. \
         It is generated: do not edit it. After upgrading the binary run \
         `mirzam skill install` again - and if you forget, `mirzam check` and \
         `mirzam build` say so, because they compare this stamp with their own \
         version.\n\n",
        body.trim_end()
    );
    // Hashed with the hash itself blank, then written with it filled in, so
    // the file describes its own contents without chasing its own tail.
    let blank = format!("{head}{}\n", stamp_line(version, "", &card));
    format!(
        "{head}{}\n",
        stamp_line(version, &hex(fnv1a(&blank)), &card)
    )
}

struct Stamp {
    version: String,
    hash: String,
    card: String,
    /// The stamp line exactly as it appeared, so it can be blanked again.
    line: String,
}

impl Stamp {
    /// Whether `text` is exactly what a pristine install of *some* version
    /// would have produced: blank the recorded hash back out and the file must
    /// hash to it again.
    fn matches(&self, text: &str) -> bool {
        let blanked_line = self
            .line
            .replace(&format!("hash=\"{}\"", self.hash), "hash=\"\"");
        let blanked = text.replacen(&self.line, &blanked_line, 1);
        hex(fnv1a(&blanked)) == self.hash
    }
}

/// Reads the stamp out of a `SKILL.md`, or `None` when there is not one -
/// which is what a hand-written skill, or a file from before this existed,
/// looks like.
fn parse_stamp(text: &str) -> Option<Stamp> {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("<!-- mirzam-skill "))?;
    Some(Stamp {
        version: attr(line, "version")?,
        hash: attr(line, "hash")?,
        card: attr(line, "card").unwrap_or_default(),
        line: line.to_string(),
    })
}

/// `key="value"` out of the stamp line.
fn attr(line: &str, key: &str) -> Option<String> {
    let at = line.find(&format!("{key}=\""))? + key.len() + 2;
    let rest = &line[at..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// FNV-1a, 64-bit. Not `DefaultHasher`: that one's output is explicitly
/// allowed to change between Rust releases, and a stamp written by last
/// year's binary has to still verify under this year's.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn hex(h: u64) -> String {
    format!("{h:016x}")
}

// ---------------------------------------------------------------------------
// Drift
// ---------------------------------------------------------------------------

/// Adds a warning when a skill card near this deck was written by a different
/// version of Mirzam. Goes through the ordinary warning channel, so it reaches
/// the text form as a `⚠` line and the JSON document as a `build.skill`
/// diagnostic — the agent reads it in the loop it already runs.
pub fn note_drift(deck: &Path, out: &mut BuildOutput) {
    if let Some(w) = drift_warning(deck) {
        out.warnings.push(w);
        // No slide, no file: the card is a property of the machine, not of a
        // line anybody wrote in this deck.
        out.warning_sites.push(WarningSite::default());
    }
}

/// The warning text, or `None` when no stamped card was found or it agrees
/// with this binary.
pub fn drift_warning(deck: &Path) -> Option<String> {
    let (path, version) = find_card(deck)?;
    if version == VERSION {
        return None;
    }
    // Which side is stale decides who repairs it, and only one of the two
    // repairs is available to an agent holding this binary.
    let stale_card = match (semver(&version), semver(VERSION)) {
        (Some(found), Some(ours)) => found < ours,
        // An unparsable version is treated as the stale one: rewriting the
        // card is the cheap, reversible half of the two repairs.
        _ => true,
    };
    Some(if stale_card {
        format!(
            "skill card {} was written by mirzam {version}, but this binary is {VERSION} - \
             run `mirzam skill install` to update it",
            path.display()
        )
    } else {
        format!(
            "skill card {} was written by mirzam {version}, and this binary is only {VERSION} - \
             this binary is older than the skill card; upgrade the binary",
            path.display()
        )
    })
}

/// The nearest installed skill card and the version that wrote it.
///
/// Walks up from the deck, because a deck two directories inside a repository
/// is still that repository's deck, and stops at the repository boundary: a
/// `.git` is where "this project" ends, and scanning past it towards the root
/// of the disk would be reading directories that have nothing to do with the
/// deck. The user-wide skill is checked last, the way Claude Code itself
/// resolves a project skill ahead of a personal one.
fn find_card(deck: &Path) -> Option<(PathBuf, String)> {
    let deck = std::fs::canonicalize(deck).unwrap_or_else(|_| deck.to_path_buf());
    let start = deck.parent().unwrap_or(Path::new(".")).to_path_buf();
    for dir in start.ancestors() {
        if let Some(found) = card_in(&dir.join(SKILL_DIR)) {
            return Some(found);
        }
        if dir.join(".git").exists() {
            break;
        }
    }
    card_in(&home_dir()?.join(SKILL_DIR))
}

fn card_in(dir: &Path) -> Option<(PathBuf, String)> {
    let path = dir.join("SKILL.md");
    let text = std::fs::read_to_string(&path).ok()?;
    let version = parse_stamp(&text)?.version;
    Some((path, version))
}

/// `major.minor.patch` for comparison. `None` for anything else, including a
/// pre-release suffix, which is a shape this project does not publish.
fn semver(v: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = v.split('.').collect();
    let [major, minor, patch] = parts[..] else {
        return None;
    };
    Some((
        major.parse().ok()?,
        minor.parse().ok()?,
        patch.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_skill_carries_a_stamp_naming_this_binary() {
        let text = stamped(PROJECT_SKILL);
        let stamp = parse_stamp(&text).expect("a stamp");
        assert_eq!(stamp.version, VERSION);
        assert_eq!(stamp.card, hex(fnv1a(CARD)));
        assert!(
            text.contains(&format!("mirzam {VERSION}")),
            "a person reads the version too, not only the comment"
        );
        assert!(stamp.matches(&text), "it hashes to what it says it does");
    }

    #[test]
    fn one_edited_character_stops_matching() {
        let text = stamped(PROJECT_SKILL).replace("Mirzam renders", "Mirzam renderz");
        let stamp = parse_stamp(&text).expect("a stamp");
        assert!(!stamp.matches(&text));
    }

    /// The point of hashing with the hash blanked: a card written by a version
    /// whose template and card this binary has never seen still verifies as
    /// pristine, so an unmodified older install is overwritten in silence
    /// while an edited one is not.
    #[test]
    fn a_pristine_card_from_another_version_still_verifies() {
        let older = stamped_by(
            "0.1.0",
            "# An older template\n\nOther words.",
            "an older card",
        );
        let stamp = parse_stamp(&older).expect("a stamp");
        assert_eq!(stamp.version, "0.1.0");
        assert!(stamp.matches(&older), "pristine, by a binary long gone");
        assert!(
            !stamp.matches(&older.replace("Other words", "My words")),
            "and an edit to it is still an edit"
        );
    }

    #[test]
    fn a_file_with_no_stamp_is_not_ours() {
        assert!(parse_stamp("# Somebody's own skill\n").is_none());
    }

    #[test]
    fn the_writing_skill_names_the_browser_editor_and_refuses_the_cli() {
        let text = stamped(WRITING_SKILL);
        assert!(text.contains("ayatough.github.io/Mirzam/try/"));
        assert!(text.contains("cannot run the `mirzam` CLI"));
        assert!(text.contains("name: mirzam-writing"));
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert_eq!(semver("0.5.0"), Some((0, 5, 0)));
        assert!(semver("0.10.0") > semver("0.9.9"));
        assert_eq!(semver("0.5"), None);
        assert_eq!(semver("0.5.0-rc1"), None);
    }

    #[test]
    fn the_hash_is_stable_across_builds() {
        // A literal, not a recomputation: if this number ever changes, every
        // installed card in the world stops verifying.
        assert_eq!(hex(fnv1a("mirzam")), "7b215ce1595c6939");
    }
}
