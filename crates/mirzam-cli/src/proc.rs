//! Running a child process under a clock.
//!
//! `std::process` offers `wait`, which blocks until the child is done, and
//! `try_wait`, which never blocks at all - and nothing in between. Every
//! browser this binary starts is a child that may never finish: a Chromium
//! that cannot open a display, one extracted from a package and missing a
//! library, one wedged on a profile directory it cannot lock. Waiting on
//! `wait` for that is the difference between a command that fails and a
//! command that hangs, and a caller cannot tell a hang from slow work.
//!
//! So the wait polls instead, at the same interval `cdp` polls for the
//! DevTools port, and the caller says how long it is prepared to wait.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often the child is asked whether it is done. Short enough that a fast
/// command is not held up by the poll, long enough to cost nothing.
const TICK: Duration = Duration::from_millis(50);

/// How long the output is still collected for after the child has exited. Its
/// own descendants hold the same pipe, so the write end closes a moment after
/// the process this waited on does.
const DRAIN: Duration = Duration::from_secs(5);

/// Runs `cmd` to completion with its stdout captured, giving up after `limit`.
///
/// `Ok(None)` is the clock running out: the child has been killed by the time
/// this returns. `None` for `limit` waits as long as it takes, which is what a
/// caller that has been told to wait forever means.
pub(crate) fn output_within(
    cmd: &mut Command,
    limit: Option<Duration>,
) -> std::io::Result<Option<Output>> {
    let mut child = cmd.stdout(Stdio::piped()).spawn()?;
    // Read on a thread rather than after the wait: a child that fills the pipe
    // buffer blocks until someone drains it, so a wait that drains nothing
    // deadlocks on exactly the large output this is here to collect - and a
    // dumped DOM is far larger than a pipe.
    let (tx, rx) = mpsc::channel();
    let mut pipe = child.stdout.take();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });

    let deadline = limit.map(|l| Instant::now() + l);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if deadline.is_some_and(|d| Instant::now() > d) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(TICK);
    };

    let Some(status) = status else {
        // Deliberately not waiting for the reader here. A browser spawns
        // helpers that inherit this pipe and outlive the process that was
        // killed, so the read may not return for as long as one of them holds
        // the write end - which is the hang this whole function exists to end.
        // The thread is left to the exit that follows the error being returned.
        return Ok(None);
    };
    Ok(Some(Output {
        status,
        stdout: rx.recv_timeout(DRAIN).unwrap_or_default(),
        stderr: Vec::new(),
    }))
}

/// Whether `cmd` runs at all, in the time given. A probe, so a child that
/// hangs answers `false` rather than never answering.
pub(crate) fn succeeds_within(cmd: &mut Command, limit: Duration) -> bool {
    cmd.stderr(Stdio::null());
    matches!(output_within(cmd, Some(limit)), Ok(Some(out)) if out.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_that_finishes_hands_back_what_it_wrote() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let out = output_within(&mut cmd, Some(Duration::from_secs(10)))
            .expect("spawn")
            .expect("not a timeout");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }

    #[test]
    fn a_child_that_never_finishes_is_given_up_on() {
        let mut cmd = Command::new("sleep");
        cmd.arg("60");
        let started = Instant::now();
        let out = output_within(&mut cmd, Some(Duration::from_millis(200))).expect("spawn");
        assert!(out.is_none(), "the sleep should have been killed");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "gave up after {:?}, which is not giving up",
            started.elapsed()
        );
    }

    /// The shape a browser actually has: the process that was launched is not
    /// the only one holding the pipe. Giving up has to mean giving up on the
    /// read as well, or the timeout is not one.
    #[test]
    fn a_child_whose_own_child_holds_the_pipe_is_still_given_up_on() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 60"]);
        let started = Instant::now();
        let out = output_within(&mut cmd, Some(Duration::from_millis(200))).expect("spawn");
        assert!(out.is_none(), "the sleep should have been given up on");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "gave up after {:?}, which is not giving up",
            started.elapsed()
        );
    }

    #[test]
    fn a_probe_of_a_hanging_binary_is_not_a_success() {
        let mut cmd = Command::new("sleep");
        cmd.arg("60");
        assert!(!succeeds_within(&mut cmd, Duration::from_millis(200)));
    }
}
