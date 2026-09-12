//! The command line itself: what `mirzam` does with the arguments before any
//! deck is read.
//!
//! Both things asserted here were reported from the outside, and both are the
//! kind of failure that costs a caller far more than the bug is worth. Asking
//! a command for help and being told the help is a missing file sends a reader
//! looking for a file; a checker that never returns cannot be told apart from
//! one still working, so the caller waits, and then waits longer.

use std::process::Command;

fn mirzam() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mirzam"))
}

/// Every command answers `--help` on stdout and succeeds.
///
/// A subcommand used to read the flag as its input path, so the first thing a
/// reader types when a command surprises them - and the only way to find out
/// what flags it takes - failed with a message about a file named `--help`.
#[test]
fn help_is_answered_at_every_level() {
    for args in [
        vec!["--help"],
        vec!["-h"],
        vec!["help"],
        vec!["new", "--help"],
        vec!["build", "--help"],
        vec!["build", "-h"],
        vec!["serve", "--help"],
        vec!["check", "--help"],
        vec!["lsp", "--help"],
        vec!["export", "--help"],
        vec!["export", "pdf", "--help"],
        vec!["export", "pptx", "--help"],
        vec!["export", "video", "--help"],
        vec!["import", "--help"],
        vec!["import", "pdf", "--help"],
        vec!["import", "pdf", "-h"],
        vec!["skill", "--help"],
        vec!["skill", "install", "--help"],
    ] {
        let out = mirzam().args(&args).output().expect("run mirzam");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "`mirzam {}` exited {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            stdout.contains("Usage:") && stdout.contains("mirzam import pdf"),
            "`mirzam {}` printed no help on stdout: {stdout}",
            args.join(" ")
        );
        assert!(
            out.stderr.is_empty(),
            "`mirzam {}` wrote to stderr: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// A flag that takes a value still takes `--help` as that value. Help is
/// answered where the question is asked, which is a flag position - not
/// wherever the four characters happen to appear.
#[test]
fn a_value_that_reads_like_the_help_flag_is_still_a_value() {
    let out = mirzam()
        .args(["check", "deck.md", "--chromium", "--help"])
        .output()
        .expect("run mirzam");
    assert!(!out.status.success(), "this should not have been help");
    assert!(
        String::from_utf8_lossy(&out.stdout).is_empty(),
        "help went to stdout for a browser path that reads like the flag"
    );
}

/// A browser that never answers is given up on, and the message says which
/// browser and where the path came from.
///
/// `check` is the command an agent runs after every edit and then waits on, so
/// a run that can block forever with nothing on either stream is worse than
/// one that fails: the caller cannot tell a large deck from a wedged browser.
#[cfg(unix)]
#[test]
fn a_browser_that_never_answers_does_not_hang_the_check() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("mirzam-cli-hang-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");

    // A "browser" that starts, holds the pipe and never writes to it - which
    // is what a Chromium missing a library, or one that cannot lock its
    // profile, looks like from here.
    let browser = dir.join("wedged-browser");
    let mut f = std::fs::File::create(&browser).expect("write the stub");
    f.write_all(b"#!/bin/sh\nsleep 120\n")
        .expect("write the stub");
    drop(f);
    std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let deck = dir.join("deck.md");
    std::fs::write(&deck, "# One\n\nA slide.\n").expect("write the deck");

    let started = std::time::Instant::now();
    let out = mirzam()
        .args(["check"])
        .arg(&deck)
        .args(["--chromium"])
        .arg(&browser)
        .args(["--timeout", "2"])
        .output()
        .expect("run mirzam");
    let took = started.elapsed();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(!out.status.success(), "a wedged browser is not a pass");
    assert!(
        took < std::time::Duration::from_secs(60),
        "gave up after {took:?}, which is not giving up"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("did not answer"),
        "the message does not say what it was waiting for: {err}"
    );
    assert!(
        err.contains("wedged-browser") && err.contains("--chromium"),
        "the message names neither the browser nor where the path came from: {err}"
    );
}
