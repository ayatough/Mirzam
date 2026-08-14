//! `mirzam skill install`: the binary writes the agent's instructions, so the
//! card a model reads always matches the binary it drives.
//!
//! What is asserted here is the part somebody depends on: the files land where
//! Claude Code looks for them, the card is the repository's real `docs/llms.md`
//! rather than a copy, the stamp is readable by a rule written down in
//! `docs/agents.md` — this file re-implements it, which is how a silent change
//! to the format is caught — and an edited skill is never destroyed without
//! being asked twice.
//!
//! Every run gets its own `HOME` as well as its own working directory: the
//! installer and the drift check both look at `~/.claude/skills/`, and a
//! contributor who has the skill installed should not see this suite behave
//! differently because of it.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A directory that removes itself, so a failed assertion cannot leak one.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("mirzam-skill-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        // A repository boundary: `skill install` writes to the root of the
        // repository the working directory is in, and the drift check stops
        // walking up at one.
        std::fs::create_dir_all(dir.join(".git")).expect("a .git to stop at");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn skill(&self) -> PathBuf {
        self.0.join(".claude/skills/mirzam/SKILL.md")
    }

    fn card(&self) -> PathBuf {
        self.0.join(".claude/skills/mirzam/references/llms.md")
    }

    /// Runs `mirzam` in this directory, with this directory as `HOME` too.
    fn mirzam(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_mirzam"))
            .args(args)
            .current_dir(&self.0)
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .output()
            .expect("run mirzam")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn text(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// This binary's version, which is also what a fresh stamp must carry.
const VERSION: &str = env!("CARGO_PKG_VERSION");

// --- the stamp, re-implemented from its documented rule ---------------------

/// FNV-1a, 64-bit, hex. Written out again rather than called from the crate:
/// the stamp is a format an *older* installed card is verified against, so a
/// change to it has to fail a test rather than quietly re-hash everything.
fn fnv1a(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn stamp_line(text: &str) -> String {
    text.lines()
        .find(|l| l.starts_with("<!-- mirzam-skill "))
        .unwrap_or_else(|| panic!("no stamp in:\n{text}"))
        .to_string()
}

fn attr(line: &str, key: &str) -> String {
    let at = line.find(&format!("{key}=\"")).expect("the attribute") + key.len() + 2;
    let rest = &line[at..];
    rest[..rest.find('"').expect("a closing quote")].to_string()
}

/// Whether `text` hashes to the hash it carries — the rule an install applies
/// to decide whether somebody has edited the skill.
fn stamp_is_intact(text: &str) -> bool {
    let line = stamp_line(text);
    let hash = attr(&line, "hash");
    let blanked = text.replacen(
        &line,
        &line.replace(&format!("hash=\"{hash}\""), "hash=\"\""),
        1,
    );
    fnv1a(&blanked) == hash
}

/// What an older binary would have written: the same file, stamped with an
/// older version and re-hashed the same way. There is no other way to get one
/// without keeping an old binary around, and "an unmodified old card is
/// replaced in silence" is the behaviour that needs it.
fn restamp(text: &str, version: &str) -> String {
    let line = stamp_line(text);
    let old_version = attr(&line, "version");
    let hash = attr(&line, "hash");
    let blank_line = line
        .replace(
            &format!("version=\"{old_version}\""),
            &format!("version=\"{version}\""),
        )
        .replace(&format!("hash=\"{hash}\""), "hash=\"\"");
    let blanked = text.replacen(&line, &blank_line, 1);
    let fresh = fnv1a(&blanked);
    blanked.replacen(
        &blank_line,
        &blank_line.replace("hash=\"\"", &format!("hash=\"{fresh}\"")),
        1,
    )
}

// --- installing -------------------------------------------------------------

#[test]
fn install_writes_the_skill_and_the_card_it_reads() {
    let dir = TempDir::new("install");
    let out = dir.mirzam(&["skill", "install"]);
    assert!(out.status.success(), "{}", text(&out));

    let skill = std::fs::read_to_string(dir.skill()).expect("SKILL.md");
    let card = std::fs::read_to_string(dir.card()).expect("the card");

    // The card is the repository's own `docs/llms.md`, not a copy of it: a copy
    // is what would be a release behind.
    let real = std::fs::read_to_string(common::repo_root().join("docs/llms.md")).expect("llms.md");
    assert_eq!(card, real, "the installed card must be docs/llms.md itself");

    // Agent Skills conventions: a name and a description in frontmatter.
    assert!(skill.starts_with("---\nname: mirzam\n"), "{skill}");
    assert!(skill.contains("\ndescription: "), "{skill}");
    // And it points at the card rather than inlining it.
    assert!(skill.contains("references/llms.md"), "{skill}");
    assert!(
        skill.len() < card.len(),
        "SKILL.md is the instructions, not the whole card"
    );

    let line = stamp_line(&skill);
    assert_eq!(attr(&line, "version"), VERSION);
    assert_eq!(attr(&line, "card"), fnv1a(&card), "the card is hashed too");
    assert!(stamp_is_intact(&skill), "a fresh install verifies");
    // The version is also written where a person reads it, not only in a
    // comment a Markdown renderer hides.
    assert!(skill.contains(&format!("mirzam {VERSION}")), "{skill}");

    // What it wrote and what to do next, both said on stdout.
    let said = text(&out);
    assert!(said.contains(".claude/skills/mirzam/SKILL.md"), "{said}");
    assert!(said.contains("Claude Code"), "{said}");
}

#[test]
fn user_installs_into_the_home_directory_instead() {
    let dir = TempDir::new("user");
    let out = dir.mirzam(&["skill", "install", "--user"]);
    assert!(out.status.success(), "{}", text(&out));
    // `HOME` is this directory, so both paths live under it; what separates
    // them is that the project one was never written.
    assert!(dir.path().join(".claude/skills/mirzam/SKILL.md").exists());
    assert!(
        !dir.path().join("talks/.claude").exists(),
        "nothing outside the home skill folder"
    );
}

/// An unmodified card from an older binary is a stale copy, not somebody's
/// work, so upgrading over it says nothing and just writes.
#[test]
fn reinstalling_over_a_pristine_older_card_succeeds() {
    let dir = TempDir::new("older");
    assert!(dir.mirzam(&["skill", "install"]).status.success());
    let fresh = std::fs::read_to_string(dir.skill()).expect("SKILL.md");
    let older = restamp(&fresh, "0.1.0");
    assert!(stamp_is_intact(&older), "the fixture is a pristine 0.1.0");
    std::fs::write(dir.skill(), &older).expect("write the older card");

    let out = dir.mirzam(&["skill", "install"]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        std::fs::read_to_string(dir.skill()).expect("SKILL.md"),
        fresh,
        "overwritten with this version's"
    );
}

#[test]
fn an_edited_skill_is_not_overwritten_without_force() {
    let dir = TempDir::new("edited");
    assert!(dir.mirzam(&["skill", "install"]).status.success());
    let mine = format!(
        "{}\n\nOur house rule: every deck opens with the metric.\n",
        std::fs::read_to_string(dir.skill()).expect("SKILL.md")
    );
    std::fs::write(dir.skill(), &mine).expect("edit it");

    let out = dir.mirzam(&["skill", "install"]);
    assert!(!out.status.success(), "an edited skill must stop it");
    let said = text(&out);
    assert!(said.contains("has been edited"), "{said}");
    assert!(said.contains("--force"), "and says the way through: {said}");
    assert_eq!(
        std::fs::read_to_string(dir.skill()).expect("SKILL.md"),
        mine,
        "and the edit survives"
    );

    let out = dir.mirzam(&["skill", "install", "--force"]);
    assert!(out.status.success(), "{}", text(&out));
    let after = std::fs::read_to_string(dir.skill()).expect("SKILL.md");
    assert!(!after.contains("house rule"), "--force overwrites");
    assert!(stamp_is_intact(&after));
}

/// The card is half of the install, so editing *it* counts too - a model that
/// reads a card somebody trimmed is reading markup the binary does not have.
#[test]
fn an_edited_card_is_refused_the_same_way() {
    let dir = TempDir::new("edited-card");
    assert!(dir.mirzam(&["skill", "install"]).status.success());
    std::fs::write(dir.card(), "# My own card\n").expect("edit the card");

    let out = dir.mirzam(&["skill", "install"]);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("references/llms.md"), "{}", text(&out));
}

/// A `SKILL.md` this command did not write is somebody's own skill that
/// happens to share the folder name. Refusing is the only safe reading.
#[test]
fn an_unstamped_skill_in_the_way_is_refused() {
    let dir = TempDir::new("unstamped");
    std::fs::create_dir_all(dir.skill().parent().expect("parent")).expect("mkdir");
    std::fs::write(dir.skill(), "---\nname: mirzam\n---\n\nMine.\n").expect("write");

    let out = dir.mirzam(&["skill", "install"]);
    assert!(!out.status.success(), "{}", text(&out));
    assert_eq!(
        std::fs::read_to_string(dir.skill()).expect("SKILL.md"),
        "---\nname: mirzam\n---\n\nMine.\n"
    );
}

// --- the archive ------------------------------------------------------------

#[test]
fn zip_writes_an_uploadable_archive_with_skill_md_at_a_folder_root() {
    let dir = TempDir::new("zip");
    let out = dir.mirzam(&["skill", "install", "--zip"]);
    assert!(out.status.success(), "{}", text(&out));

    let path = dir.path().join("mirzam-writing-skill.zip");
    let bytes = std::fs::read(&path).expect("the archive");
    assert_eq!(&bytes[..4], b"PK\x03\x04", "a zip local file header");
    assert!(
        bytes.windows(4).any(|w| w == b"PK\x05\x06"),
        "an end-of-central-directory record"
    );
    // Stored, not deflated, so the entries and their contents are readable in
    // the bytes - which is the whole reason the writer is this small.
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("mirzam-writing/SKILL.md"),
        "SKILL.md at the root of one folder"
    );
    assert!(body.contains("mirzam-writing/references/llms.md"));
    assert!(
        body.contains("name: mirzam-writing"),
        "its frontmatter name"
    );
    assert!(
        body.contains("ayatough.github.io/Mirzam/try/"),
        "and it points at the browser editor, which is the renderer there"
    );
    assert!(
        body.contains("# Mirzam syntax card"),
        "the card travels with it"
    );

    // No skill folder: the archive is a different destination.
    assert!(!dir.skill().exists());
}

#[test]
fn zip_takes_a_path_and_creates_its_directory() {
    let dir = TempDir::new("zip-path");
    let out = dir.mirzam(&["skill", "install", "--zip", "dist/skill.zip"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(dir.path().join("dist/skill.zip").exists());
}

// --- drift ------------------------------------------------------------------

/// Whether a browser can be found, by the same route `mirzam check` takes.
fn chromium_available() -> bool {
    if std::env::var_os("MIRZAM_CHROMIUM").is_some() {
        return true;
    }
    ["chromium", "chromium-browser", "google-chrome", "chrome"]
        .iter()
        .any(|c| {
            Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
}

const DECK: &str = "---\ntitle: A deck\n---\n\n# One slide\n\nShort enough to fit.\n";

/// The loop this whole thing exists for: the agent runs `check` after an edit,
/// and the answer tells it the card it is holding is out of date.
#[test]
fn check_reports_a_stale_card_as_build_skill_and_names_the_binary() {
    if !chromium_available() {
        eprintln!("skipping: no Chromium; set MIRZAM_CHROMIUM to run this test");
        return;
    }
    let dir = TempDir::new("drift");
    assert!(dir.mirzam(&["skill", "install"]).status.success());
    let fresh = std::fs::read_to_string(dir.skill()).expect("SKILL.md");
    std::fs::write(dir.skill(), restamp(&fresh, "0.1.0")).expect("age the card");
    std::fs::write(dir.path().join("deck.md"), DECK).expect("write the deck");

    let out = dir.mirzam(&["check", "deck.md", "--format", "json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout was not JSON ({e}):\n{stdout}"));

    // Additive: the schema version is untouched, and the binary's own version
    // is what a caller repairs the card *to*.
    assert_eq!(report["version"], 1);
    assert_eq!(report["mirzam"], VERSION);

    let drift = report["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|d| d["kind"] == "build.skill")
        .unwrap_or_else(|| panic!("no build.skill diagnostic in {report}"));
    assert_eq!(drift["severity"], "warning", "drift does not fail a deck");
    let message = drift["message"].as_str().expect("a message");
    assert!(
        message.contains("0.1.0") && message.contains(VERSION),
        "{message}"
    );
    assert!(
        message.contains("mirzam skill install"),
        "it names the repair: {message}"
    );
    // A stale card is not a broken deck.
    assert_eq!(report["ok"], true);
    assert!(out.status.success(), "{}", text(&out));
}

/// The other direction: the teammate whose binary is the stale side cannot fix
/// it by rewriting the card, so they are told to upgrade instead.
#[test]
fn a_card_from_a_newer_binary_says_to_upgrade_the_binary() {
    let dir = TempDir::new("newer");
    assert!(dir.mirzam(&["skill", "install"]).status.success());
    let fresh = std::fs::read_to_string(dir.skill()).expect("SKILL.md");
    std::fs::write(dir.skill(), restamp(&fresh, "99.0.0")).expect("age the binary");
    // A subdirectory, to prove the search walks up towards the repository root.
    std::fs::create_dir_all(dir.path().join("talks")).expect("mkdir");
    std::fs::write(dir.path().join("talks/deck.md"), DECK).expect("write the deck");

    let out = dir.mirzam(&["build", "talks/deck.md", "-o", "out"]);
    assert!(out.status.success(), "{}", text(&out));
    let said = text(&out);
    assert!(said.contains("upgrade the binary"), "{said}");
    assert!(said.contains("99.0.0"), "{said}");
}

/// No skill installed is the common case, and it must cost nothing and say
/// nothing.
#[test]
fn no_installed_skill_means_no_warning() {
    let dir = TempDir::new("none");
    std::fs::write(dir.path().join("deck.md"), DECK).expect("write the deck");
    let out = dir.mirzam(&["build", "deck.md", "-o", "out"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(!text(&out).contains("skill card"), "{}", text(&out));
}

/// The stamp only drifts when the versions differ; a matching one is silent.
#[test]
fn a_current_card_is_silent() {
    let dir = TempDir::new("current");
    assert!(dir.mirzam(&["skill", "install"]).status.success());
    std::fs::write(dir.path().join("deck.md"), DECK).expect("write the deck");
    let out = dir.mirzam(&["build", "deck.md", "-o", "out"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(!text(&out).contains("skill card"), "{}", text(&out));
}
