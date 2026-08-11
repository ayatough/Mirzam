//! `mirzam new` - the first file.
//!
//! `build` and `serve` both start from a file that already exists, which left
//! no way in for someone with nothing yet: pointing either at a path that is
//! not there is an error, and the alternative - create an empty file by hand,
//! then guess the frontmatter - is the part a starter file should answer.

use std::io::Write;
use std::path::Path;

/// The deck `mirzam new` writes: frontmatter, a title slide, a slide break.
///
/// Deliberately short. This file is the first thing an author edits, so
/// everything in it has to be something they either keep or delete on sight -
/// a tour of the markup belongs in `examples/`, not in their deck.
pub const TEMPLATE: &str = "\
---
title: My deck
---

# My deck {.title-slide}

What it is about, in one line

---

## The first point

Write Markdown. A `---` on its own line starts the next slide.
";

/// Writes a new deck at `path`, refusing to touch a file that is already there.
///
/// `empty` writes nothing at all: a deck that starts from a blank page is a
/// legitimate way to start, and it is the one the old flow could not express.
pub fn create(path: &Path, empty: bool) -> Result<(), String> {
    // `mirzam new talks/2026/deck.md` should not fail on the directories.
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    // `create_new` rather than "does it exist?" then write: the check and the
    // write have to be one step, or a deck can be overwritten between them.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => format!(
                "{} already exists - `mirzam new` never overwrites a deck",
                path.display()
            ),
            _ => format!("cannot create {}: {e}", path.display()),
        })?;

    let body = if empty { "" } else { TEMPLATE };
    file.write_all(body.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(())
}
