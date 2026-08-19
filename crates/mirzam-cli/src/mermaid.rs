//! The host half of `mermaid` blocks: finding a renderer, and running it.
//!
//! `mirzam-render` defines [`mirzam_render::DiagramRenderer`] and calls it; it
//! may not implement it, because spawning a process is exactly what the
//! WebAssembly build cannot do. This is where the process is spawned.
//!
//! **`build` stays browser-free.** Chromium can draw Mermaid, and Mirzam
//! already drives one for `export pdf` and `check`, but making an ordinary
//! build need a browser is a regression in what this tool is. So the renderer
//! is `mmdc` — [mermaid-cli] — if the machine has one, and nothing otherwise.
//! It is discovered the way Chromium is for `export pdf`: an environment
//! variable first, then the well-known name on `PATH`. Absent, the build says
//! so and the fence stays a code block; see `check.rs`'s `build.mermaid`.
//!
//! [mermaid-cli]: https://github.com/mermaid-js/mermaid-cli

use mirzam_render::DiagramRenderer;
use std::path::PathBuf;
use std::process::Command;

/// What `mmdc` is told about the browser it launches.
///
/// Only the sandbox flag, for the reason given where it is written out: a
/// build running as root gets no browser at all without it.
const PUPPETEER_CONFIG: &str = "{\"args\":[\"--no-sandbox\"]}";

/// The environment variable naming an `mmdc` binary, for a machine where it is
/// installed somewhere `PATH` does not reach — the same escape hatch
/// `MIRZAM_CHROMIUM` is for the PDF exporter.
pub const MMDC_ENV: &str = "MIRZAM_MMDC";

/// `mmdc`, once it has been found.
pub struct Mmdc {
    program: PathBuf,
}

impl Mmdc {
    /// The renderer this machine has, or `None`.
    ///
    /// `None` is an ordinary answer, not a failure: it is what every machine
    /// without mermaid-cli installed says, and a deck with no `mermaid` fence
    /// in it never notices.
    pub fn discover() -> Option<Self> {
        Self::discover_with(|k| std::env::var(k).ok(), runs)
    }

    /// Discovery with its two questions about the world injected, so the
    /// not-found path can be tested on a machine that does have `mmdc` and the
    /// found path on one that does not.
    fn discover_with(
        env: impl Fn(&str) -> Option<String>,
        usable: impl Fn(&std::path::Path) -> bool,
    ) -> Option<Self> {
        if let Some(named) = env(MMDC_ENV).map(PathBuf::from).filter(|p| usable(p)) {
            return Some(Self { program: named });
        }
        let on_path = PathBuf::from("mmdc");
        usable(&on_path).then_some(Self { program: on_path })
    }

    /// What the build prints when it used one, for the record `check` writes.
    pub fn program(&self) -> &std::path::Path {
        &self.program
    }
}

/// Whether the binary at `program` is there and answers.
///
/// `--version` rather than `stat`: `mmdc` is usually a shell shim written by
/// npm, so its being executable says less than its running does.
fn runs(program: &std::path::Path) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

impl DiagramRenderer for Mmdc {
    fn render(&self, source: &str) -> Result<String, String> {
        // `mmdc` reads and writes files, not pipes, so the exchange goes
        // through a directory of its own that is removed however this ends.
        let dir = TempDir::new()?;
        let input = dir.path.join("diagram.mmd");
        let output = dir.path.join("diagram.svg");
        std::fs::write(&input, source).map_err(|e| format!("cannot write {input:?}: {e}"))?;

        // `mmdc` draws the diagram in a headless Chromium of its own, and
        // Chromium refuses to start as root without `--no-sandbox`. That is
        // every container that runs its build as root - CI images, devcontainers
        // - so without this a `mermaid` fence renders on a laptop and warns in
        // Docker, which is the worst way for a feature to differ. The exporter
        // and the checker already pass the same flag to the browser they drive;
        // this is the only channel `mmdc` offers for it.
        let puppeteer = dir.path.join("puppeteer.json");
        std::fs::write(&puppeteer, PUPPETEER_CONFIG)
            .map_err(|e| format!("cannot write {puppeteer:?}: {e}"))?;

        let out = Command::new(&self.program)
            .arg("--input")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .arg("--puppeteerConfigFile")
            .arg(&puppeteer)
            // Transparent, because the pane behind the diagram is the deck's
            // background and the diagram must not paint its own over it.
            .arg("--backgroundColor")
            .arg("transparent")
            .arg("--quiet")
            .output()
            .map_err(|e| format!("cannot run {}: {e}", self.program.display()))?;

        if !out.status.success() {
            return Err(format!(
                "{} failed ({}){}",
                self.program.display(),
                out.status,
                first_line(&String::from_utf8_lossy(&out.stderr))
            ));
        }
        std::fs::read_to_string(&output)
            .map_err(|e| format!("{} wrote no SVG: {e}", self.program.display()))
    }
}

/// The first line of a tool's diagnostics, prefixed for the warning it joins.
///
/// One line, because `mmdc` prints a stack trace under its message and a build
/// warning that scrolls the terminal is a warning nobody reads.
fn first_line(stderr: &str) -> String {
    match stderr.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(l) => format!(": {l}"),
        None => String::new(),
    }
}

/// A directory that removes itself. Small enough to own here rather than take
/// a dependency for, and the only place in the CLI that wants one.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, String> {
        // Nanoseconds plus the process id: two builds racing in the same
        // temporary directory must not write over each other's diagram.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("mirzam-mermaid-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|e| format!("cannot create {path:?}: {e}"))?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The path this environment can actually take: nothing is installed, so
    /// discovery answers `None` and every `mermaid` fence degrades.
    #[test]
    fn nothing_installed_is_no_renderer_rather_than_an_error() {
        let none = Mmdc::discover_with(|_| None, |_| false);
        assert!(none.is_none());
    }

    #[test]
    fn the_environment_variable_wins_over_the_name_on_path() {
        let found = Mmdc::discover_with(
            |k| (k == MMDC_ENV).then(|| "/opt/mermaid/mmdc".to_string()),
            |_| true,
        )
        .expect("a renderer");
        assert_eq!(found.program(), Path::new("/opt/mermaid/mmdc"));
    }

    /// A variable pointing at something that is not there falls through to
    /// `PATH` rather than failing the build — the same shape as a stale
    /// `MIRZAM_CHROMIUM`.
    #[test]
    fn a_variable_naming_nothing_falls_through_to_the_name_on_path() {
        let found = Mmdc::discover_with(
            |k| (k == MMDC_ENV).then(|| "/nowhere/mmdc".to_string()),
            |p| p == Path::new("mmdc"),
        )
        .expect("a renderer");
        assert_eq!(found.program(), Path::new("mmdc"));
    }

    #[test]
    fn a_tool_that_fails_is_quoted_by_its_first_line_only() {
        assert_eq!(first_line(""), "");
        assert_eq!(
            first_line("\nError: Parse error on line 2\n    at Object.parse\n"),
            ": Error: Parse error on line 2"
        );
    }

    /// The browser flag is not advice: a `mermaid` fence in a root container
    /// draws nothing without it, so the config file has to reach `mmdc` on
    /// every render. A stub renderer copies whatever config it was handed into
    /// the SVG it is asked for, which is how this reads the flag back out.
    #[cfg(unix)]
    #[test]
    fn the_renderer_is_told_not_to_sandbox_the_browser() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().expect("a directory");
        let stub = dir.path.join("mmdc-stub.sh");
        std::fs::write(
            &stub,
            "#!/bin/sh\n\
             while [ $# -gt 0 ]; do\n\
               case \"$1\" in\n\
                 --output) out=\"$2\"; shift 2;;\n\
                 --puppeteerConfigFile) cfg=\"$2\"; shift 2;;\n\
                 *) shift;;\n\
               esac\n\
             done\n\
             cat \"$cfg\" > \"$out\"\n",
        )
        .expect("the stub is written");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
            .expect("the stub is executable");

        let drawn = Mmdc { program: stub }
            .render("flowchart LR\n  a --> b\n")
            .expect("the stub renders");
        assert!(
            drawn.contains("--no-sandbox"),
            "the puppeteer config never reached mmdc: {drawn}"
        );
    }

    #[test]
    fn the_exchange_directory_removes_itself() {
        let path = {
            let dir = TempDir::new().expect("a directory");
            assert!(dir.path.is_dir());
            dir.path.clone()
        };
        assert!(!path.exists());
    }
}
